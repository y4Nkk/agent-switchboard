//! Gateway listener startup, binding, and retry lifecycle.

use super::*;

impl GatewayController {
    /// Resolves state, recovers a port transaction, binds, and rehydrates.
    /// Every failure becomes an observable controller state so the window can
    /// still open with an actionable repair surface.
    #[cfg(test)]
    pub(crate) fn start(local: &LocalState) -> Self {
        Self::start_with_write_lock(local, Arc::new(Mutex::new(())))
    }

    pub(crate) fn start_with_write_lock(
        local: &LocalState,
        endpoint_write_lock: Arc<Mutex<()>>,
    ) -> Self {
        let state_path = local.gateway_state_path();
        let (mut state, unusable, repair_reason) = match read_or_create_state(&state_path) {
            StateLoad::Ready(state) | StateLoad::Created(state) => (state, None, None),
            StateLoad::Unusable(detail) => {
                log::warn!("本机协议网关状态不可用：{detail}");
                (fresh_state(), Some(detail.clone()), Some(detail))
            }
        };
        let blocked = if unusable.is_none() {
            match port_change::recover_pending(local, &state_path, &mut state) {
                Ok(blocked) => blocked,
                Err(reason) => {
                    log::warn!("本机协议网关端口修改恢复失败：{reason}");
                    Some(port_change::BlockedPortChange {
                        from_port: state.port,
                        to_port: state.port,
                        apps: Vec::new(),
                        reason,
                    })
                }
            }
        } else {
            None
        };
        let state_available = unusable.is_none();
        let health_store = Arc::new(health_state::ClaudeHealthStore::load_preserving_damage(
            local.root(),
        ));
        if let Some(warning) = health_store.warning() {
            log::warn!("{warning}");
        }
        let claude_request_ledger = Arc::new(request_ledger::ClaudeRequestLedger::new(
            local.claude_request_ledger_path(),
        ));
        let controller = Self {
            inner: Arc::new(GatewayInner {
                state_path,
                state_root: local.root().to_path_buf(),
                endpoint_write_lock,
                profile_save_gate: RwLock::new(()),
                state: Mutex::new(state),
                routes: RwLock::new(BTreeMap::new()),
                candidate_routes: RwLock::new(BTreeMap::new()),
                provider_health: RwLock::new(BTreeMap::new()),
                health_store,
                codex_health: Arc::new(codex::health::CodexHealthStore::new(local.root())),
                codex_history: Arc::new(codex::history::CodexToolHistory::default()),
                claude_request_ledger,
                claude_auth: crate::claude_auth::ClaudeAuth::shared(local.root()),
                activation_lock: Mutex::new(()),
                codex_activation_generation: std::sync::atomic::AtomicU64::new(0),
                listener: RwLock::new(ListenerState::Failed(Box::new(GatewayFailureReport {
                    port: 0,
                    kind: GatewayFailureKind::StateUnusable,
                    os_code: None,
                    message: unusable.unwrap_or_else(|| "本机协议网关正在启动".to_string()),
                    process: None,
                }))),
                maintenance: AtomicBool::new(false),
                inflight: AtomicUsize::new(0),
                websocket_connections: AtomicUsize::new(0),
                stopping: AtomicBool::new(false),
                state_available: AtomicBool::new(state_available),
                repair_reason: Mutex::new(repair_reason),
                blocked_recovery: Mutex::new(blocked),
                metrics: Arc::new(GatewayMetrics::new()),
            }),
        };
        if controller.inner.state_available.load(Ordering::Acquire) {
            controller.bind_configured_port();
            controller.rehydrate(local);
        }
        controller
    }

    /// Binds the configured port. Tests may initially use zero so the OS
    /// assigns a port; production state always carries an explicit port.
    fn bind_configured_port(&self) {
        let state_path = self.inner.state_path.clone();
        let mut port = self.configured_port();
        let listener = match BoundListener::bind(port) {
            Ok(server) => server,
            Err(error) => {
                let report = bind_failure_report(port, Box::new(error));
                log::warn!("{}", report.message);
                self.set_listener(ListenerState::Failed(Box::new(report)));
                return;
            }
        };
        let bound_port = listener.port();
        if bound_port != port {
            if port != 0 {
                self.set_listener(ListenerState::Failed(Box::new(GatewayFailureReport {
                    port,
                    kind: GatewayFailureKind::SystemRejected,
                    os_code: None,
                    message: format!(
                        "无法监听 127.0.0.1:{port}：监听层返回了其他端口 {bound_port}"
                    ),
                    process: None,
                })));
                return;
            }
            port = bound_port;
            let mut state = match self.inner.state.lock() {
                Ok(state) => state,
                Err(_) => {
                    self.set_listener(ListenerState::Failed(Box::new(test_failure_report(
                        port,
                        "本机协议网关状态锁不可用",
                    ))));
                    return;
                }
            };
            state.port = port;
            if let Err(error) = write_state(&state_path, &state) {
                log::warn!("无法持久化本机协议网关端口：{error}");
            }
        }
        if let Err(error) = self.spawn_serve(listener.clone()) {
            log::warn!("{error}");
            self.set_listener(ListenerState::Failed(Box::new(GatewayFailureReport {
                port,
                kind: GatewayFailureKind::SystemRejected,
                os_code: None,
                message: error,
                process: None,
            })));
            return;
        }
        self.set_listener(ListenerState::Listening { listener });
    }

    /// Starts the serving loop and waits until its client and workers are
    /// initialized; a bound but inert socket is never published as healthy.
    pub(crate) fn spawn_serve(&self, listener: BoundListener) -> Result<(), String> {
        let inner = Arc::clone(&self.inner);
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let worker_listener = listener.clone();
        if thread::Builder::new()
            .name("asb-local-gateway".to_string())
            .spawn(move || server::serve(worker_listener, inner, ready_sender))
            .is_err()
        {
            listener.stop();
            return Err("无法启动本机协议网关工作线程".to_string());
        }
        match ready_receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                listener.stop();
                Err(error)
            }
            Err(_) => {
                listener.stop();
                Err("无法确认本机协议网关工作线程已就绪".to_string())
            }
        }
    }

    /// Brings the listener up on the configured port. An existing listener is
    /// left untouched; a failure is reported in the returned observation.
    pub(crate) fn retry_bind(&self, local: &LocalState) -> GatewayObservation {
        if !self.inner.state_available.load(Ordering::Acquire) && !self.reload_state(local) {
            return self.observe(local);
        }
        if self.listening().is_none() {
            self.bind_configured_port();
            self.rehydrate(local);
        }
        self.observe(local)
    }

    /// A discarded recovery keeps whatever client files survived. If a
    /// pre-commit failure left the saved endpoint ahead of the live listener,
    /// make the application-owned port fact follow the listener before
    /// normal operations resume; unmatched clients then remain visibly in
    /// repair instead of being reported as healthy against a dead address.
    pub(crate) fn synchronize_port_to_listener(&self) -> Result<(), String> {
        let Some(listener) = self.listening() else {
            return Ok(());
        };
        if listener.port() != self.configured_port() {
            self.persist_port(listener.port())?;
        }
        Ok(())
    }

    fn reload_state(&self, local: &LocalState) -> bool {
        let state_path = local.gateway_state_path();
        let mut state = match state_repair::read_for_retry(&state_path) {
            StateLoad::Ready(state) | StateLoad::Created(state) => state,
            StateLoad::Unusable(detail) => {
                if let Ok(mut reason) = self.inner.repair_reason.lock() {
                    *reason = Some(detail.clone());
                }
                self.inner.state_available.store(false, Ordering::Release);
                self.set_listener(ListenerState::Failed(Box::new(GatewayFailureReport {
                    port: self.configured_port(),
                    kind: GatewayFailureKind::StateUnusable,
                    os_code: None,
                    message: detail,
                    process: None,
                })));
                return false;
            }
        };
        if let Ok(mut reason) = self.inner.repair_reason.lock() {
            *reason = None;
        }
        let blocked =
            port_change::recover_pending(local, &state_path, &mut state).unwrap_or_else(|reason| {
                Some(port_change::BlockedPortChange {
                    from_port: state.port,
                    to_port: state.port,
                    apps: Vec::new(),
                    reason,
                })
            });
        let Ok(mut stored) = self.inner.state.lock() else {
            return false;
        };
        *stored = state;
        drop(stored);
        if let Ok(mut routes) = self.inner.routes.write() {
            routes.clear();
        } else {
            return false;
        }
        if let Ok(mut candidates) = self.inner.candidate_routes.write() {
            candidates.clear();
        } else {
            return false;
        }
        if let Ok(mut blocked_slot) = self.inner.blocked_recovery.lock() {
            *blocked_slot = blocked;
        } else {
            return false;
        }
        self.inner.state_available.store(true, Ordering::Release);
        true
    }
}

/// Classifies a bind error into the structured failure report, identifying
/// the holder process only when the platform reports it reliably.
pub(super) fn bind_failure_report(
    port: u16,
    error: Box<dyn std::error::Error + Send + Sync>,
) -> GatewayFailureReport {
    const WSAEADDRINUSE: i32 = 10048;
    let io_error = error.downcast_ref::<std::io::Error>();
    let os_code = io_error.and_then(|error| error.raw_os_error());
    let in_use = matches!(
        io_error.map(|error| error.kind()),
        Some(std::io::ErrorKind::AddrInUse)
    ) || os_code == Some(WSAEADDRINUSE);
    let holder = if in_use {
        port_probe::find_listener(port)
    } else {
        None
    };
    let (kind, message) = if in_use {
        let holder_text = match &holder {
            Some(info) => match &info.name {
                Some(name) => format!("（占用进程 {}，PID {}）", name, info.pid),
                None => format!("（占用进程 PID {}）", info.pid),
            },
            None => "（占用进程未知）".to_string(),
        };
        (
            GatewayFailureKind::PortInUse,
            format!("无法监听 127.0.0.1:{port}：端口已被占用{holder_text}。"),
        )
    } else {
        let code_text = os_code
            .map(|code| format!("（系统错误码 {code}）"))
            .unwrap_or_default();
        (
            GatewayFailureKind::SystemRejected,
            format!("无法监听 127.0.0.1:{port}：绑定请求被系统拒绝{code_text}。"),
        )
    };
    GatewayFailureReport {
        port,
        kind,
        os_code,
        message,
        process: holder.map(|info| GatewayPortProcess {
            pid: info.pid,
            name: info.name,
        }),
    }
}

impl GatewayController {
    /// The saved port every client-facing address is derived from.
    pub(crate) fn configured_port(&self) -> u16 {
        self.inner
            .state
            .lock()
            .map(|state| state.port)
            .unwrap_or(DEFAULT_GATEWAY_PORT)
    }

    /// The loopback base URL that client configurations are written against.
    /// It follows the configured port, the single address contract, whether
    /// or not the listener is currently up.
    pub(crate) fn configured_base_url(&self) -> String {
        self.inner.configured_base_url()
    }

    pub(crate) fn listening(&self) -> Option<BoundListener> {
        let listener = self.inner.listener.read().ok()?;
        match &*listener {
            ListenerState::Listening { listener }
                if !listener.stop_signal().load(Ordering::Acquire) =>
            {
                Some(listener.clone())
            }
            ListenerState::Listening { .. } => None,
            ListenerState::Failed(_) => None,
        }
    }

    /// The served base URL; `None` while the listener is down.
    pub(crate) fn listening_base_url(&self) -> Option<String> {
        self.listening()
            .map(|listener| format!("http://127.0.0.1:{}", listener.port()))
    }

    #[cfg(test)]
    #[allow(dead_code)] // verification harness
    pub(crate) fn is_listening(&self) -> bool {
        self.listening().is_some()
    }

    pub(crate) fn blocked_recovery(&self) -> Option<port_change::BlockedPortChange> {
        self.inner
            .blocked_recovery
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub(crate) fn state_available(&self) -> bool {
        self.inner.state_available.load(Ordering::Acquire)
    }

    /// Persists the one listener-address contract before publishing it to
    /// callers. The port-change journal decides whether a later failure rolls
    /// this fact back or completes it.
    pub(crate) fn persist_port(&self, port: u16) -> Result<(), String> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| "本机协议网关状态锁不可用".to_string())?;
        let mut next = state.clone();
        next.port = port;
        write_state(&self.inner.state_path, &next)?;
        *state = next;
        Ok(())
    }

    /// Publishes a replacement listener and returns the previous listener so
    /// the caller can stop that exact socket after the handoff.
    pub(crate) fn replace_listener(&self, listener: BoundListener) -> Option<BoundListener> {
        // The listener lock protects only this process's publication slot. If
        // a handler panicked while holding it, keeping the poisoned old slot
        // would strand a committed endpoint transaction, so retain its value
        // and complete the deterministic handoff.
        let mut state = self
            .inner
            .listener
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::mem::replace(&mut *state, ListenerState::Listening { listener });
        match previous {
            ListenerState::Listening { listener } => Some(listener),
            ListenerState::Failed(_) => None,
        }
    }

    pub(crate) fn block_port_change(&self, blocked: port_change::BlockedPortChange) {
        if let Ok(mut slot) = self.inner.blocked_recovery.lock() {
            *slot = Some(blocked);
        }
    }

    pub(crate) fn clear_blocked_port_change(&self) -> Result<(), String> {
        let mut slot = self
            .inner
            .blocked_recovery
            .lock()
            .map_err(|_| "端口修改恢复状态锁不可用".to_string())?;
        *slot = None;
        Ok(())
    }

    /// Replaces the listener fact atomically.
    pub(crate) fn set_listener(&self, state: ListenerState) {
        if let Ok(mut listener) = self.inner.listener.write() {
            *listener = state;
        }
    }

    pub(crate) fn shutdown(&self) {
        self.inner.stopping.store(true, Ordering::Release);
        if let Some(listener) = self.listening() {
            listener.stop();
        }
    }
}
