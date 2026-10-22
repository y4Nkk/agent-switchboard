use super::*;

// ----------------------------------------------------------------- executor

/// What the streaming session-id callback learned during one run.
#[derive(Default)]
struct SessionProgress {
    id: Option<String>,
    save_error: Option<String>,
}

/// Claims the single slot first (a second start must never leave a batch
/// row no worker will run), creates the batch record — any failure after
/// the claim releases the slot, because no call may run unrecorded — then
/// spawns the worker that executes and persists run by run.
pub(crate) fn spawn_probe(
    registry: &ProbeRegistry,
    local: &crate::local_state::LocalState,
    question: ResolvedQuestion,
    run_count: u32,
    gateway: Option<&crate::gateway::GatewayController>,
) -> Result<String, String> {
    let ledger = ProbeLedger::new(local.root());
    let batch_id = uuid::Uuid::new_v4().to_string();
    let cancel = registry.begin(&batch_id)?;
    let config_watch = match super::config_watch::ConfigWatch::start(local) {
        Ok(watch) => watch,
        Err(error) => { registry.abandon(&batch_id); return Err(error); }
    };
    let prepared = ledger::NewProbeBatch {
        id: batch_id.clone(),
        started_at: ledger::now_rfc3339(),
        planned_runs: run_count,
        question: ledger::ProbeQuestionSnapshot {
            id: question.id.clone(),
            label: question.label.clone(),
            text: question.prompt.clone(),
            expected_answer: question.expected_answer.clone(),
        },
        grading_version: question.grading,
        cli_version: cli_version(),
        config: match snapshot::capture(local, gateway) {
            Ok(config) => config,
            Err(error) => {
                registry.abandon(&batch_id);
                return Err(error);
            }
        },
    };
    if let Err(error) = ledger.recover_interrupted().and_then(|_| ledger.insert_batch(&prepared)) {
        registry.abandon(&batch_id);
        return Err(error);
    }
    let fingerprint = prepared.config.fingerprint.clone();
    let worker_registry = registry.clone();
    let ledger_for_worker = ledger.clone();
    let local = local.clone();
    let gateway = gateway.cloned();
    let worker_batch_id = batch_id.clone();
    let spawned = std::thread::Builder::new().name("codex-probe".into()).spawn(move || {
        BatchWorker { registry: worker_registry, ledger: ledger_for_worker, local, gateway, batch_id: worker_batch_id,
            question, run_count, fingerprint, cancel, config_watch }.execute();
    });
    if let Err(error) = spawned {
        let message = format!("无法启动检测工作线程：{error}");
        stop(registry, &ledger, &batch_id, ProbeBatchStatus::Failed, Some(message.clone()), Vec::new());
        return Err(message);
    }
    Ok(batch_id)
}

struct BatchWorker {
    registry: ProbeRegistry,
    ledger: ProbeLedger,
    local: crate::local_state::LocalState,
    gateway: Option<crate::gateway::GatewayController>,
    batch_id: String,
    question: ResolvedQuestion,
    run_count: u32,
    fingerprint: String,
    cancel: Arc<AtomicBool>,
    config_watch: super::config_watch::ConfigWatch,
}

impl BatchWorker {
    fn execute(&self) {
        let Self { registry, ledger, local: _, gateway: _, batch_id, question: _,
            run_count, fingerprint: _, cancel, config_watch: _ } = self;
        for seq in 1..=*run_count {
            if cancel.load(Ordering::SeqCst) {
                stop(registry, ledger, batch_id, ProbeBatchStatus::Cancelled, None, Vec::new());
                return;
            }
            if let Err(reason) = self.check_config() {
                let (status, message) = abort_status(&reason);
                stop(registry, ledger, batch_id, status, Some(message), Vec::new());
                return;
            }
            let clock = match registry.start_run(ledger, batch_id, seq, self.question.timeout_seconds) {
                Ok(clock) => clock,
                Err(error) => {
                    let message = format!("运行记录创建失败，已停止后续调用：{error}");
                    if let Err(error) = registry.fail_start(batch_id, message) { log::error!("检测启动状态无法保存：{error}"); }
                    return;
                }
            };
            let (result, progress) = self.run(seq, &clock);
            clock.collecting();
            match result {
                Err(reason) => {
                    let (status, message) = abort_status(&reason);
                    let mut record = undetermined(seq, &message);
                    record.session_id = progress.id;
                    backfill_usage(&mut record);
                    record.duration_ms = Some(clock.started().elapsed().as_millis() as u64);
                    clock.saving();
                    stop(registry, ledger, batch_id, status, Some(message), vec![record]);
                    return;
                }
                Ok(result) => {
                    let record = run_record(seq, result);
                    clock.saving();
                    if seq == *run_count {
                        stop(registry, ledger, batch_id, ProbeBatchStatus::Completed, None, vec![record]);
                        return;
                    }
                    if let Err(error) = registry.save_run(ledger, batch_id, record) {
                        log::warn!("检测运行结果保存失败，已停止后续调用：{error}");
                        return;
                    }
                }
            }
        }
    }

    fn check_config(&self) -> Result<(), ProbeAbort> {
        self.config_watch.check()?;
        match snapshot::matches(&self.local, self.gateway.as_ref(), &self.fingerprint) {
            Ok(true) => Ok(()),
            Ok(false) => Err(ProbeAbort::ConfigChanged),
            Err(error) => Err(ProbeAbort::Spawn(format!("无法校验检测配置：{error}"))),
        }
    }

    fn run(&self, seq: u32, clock: &Arc<progress::RunProgress>) -> (Result<ProbeRunResult, ProbeAbort>, SessionProgress) {
        let session_clock = Arc::clone(clock);
        let progress = Arc::new(Mutex::new(SessionProgress::default()));
        let sink = Arc::clone(&progress);
        let ledger = self.ledger.clone();
        let batch_id = self.batch_id.clone();
        let cancel = self.cancel.clone();
        let on_session_id: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |id| {
            session_clock.running();
            let mut progress = sink.lock().expect("probe session progress");
            if progress.id.is_none() { progress.id = Some(id.to_string()); }
            if let Err(error) = ledger.save_run_session(&batch_id, seq, id) {
                progress.save_error = Some(error);
                cancel.store(true, Ordering::SeqCst);
            }
        });
        let result = run_once(&self.question, &self.cancel, &on_session_id, &|| self.check_config(), clock);
        let mut progress = progress.lock().expect("probe session progress");
        let result = match progress.save_error.as_ref() {
            Some(error) => Err(ProbeAbort::Spawn(format!("会话记录保存失败：{error}"))),
            None => result,
        };
        (result, std::mem::take(&mut *progress))
    }
}

fn run_record(seq: u32, result: ProbeRunResult) -> ledger::ProbeRunRecord {
    ledger::ProbeRunRecord {
        seq,
        status: match result.outcome {
            ProbeRunOutcome::Passed => ProbeRunStatus::Passed,
            ProbeRunOutcome::Failed => ProbeRunStatus::Failed,
            ProbeRunOutcome::Undetermined => ProbeRunStatus::Undetermined,
        },
        session_id: result.session_id,
        final_answer: result.final_answer,
        reported_model: result.model,
        duration_ms: Some(result.duration_ms),
        reasoning_tokens: result.reasoning_tokens,
        total_tokens: result.total_tokens,
        execution_error: result.error,
        usage_error: result.usage_error,
    }
}

fn abort_status(reason: &ProbeAbort) -> (ProbeBatchStatus, String) {
    match reason {
        ProbeAbort::Cancelled => (ProbeBatchStatus::Cancelled, "检测被取消，未判定".into()),
        ProbeAbort::ConfigChanged => (ProbeBatchStatus::ConfigChanged,
            "检测期间配置发生变化，当前调用未判定，已停止后续调用".into()),
        ProbeAbort::Spawn(error) => (ProbeBatchStatus::Failed, error.clone()),
    }
}

fn backfill_usage(record: &mut ledger::ProbeRunRecord) {
    let Some(id) = record.session_id.as_deref() else { return; };
    match super::output::read_usage(id) {
        Ok(usage) => {
            record.total_tokens = usage.total;
            record.reasoning_tokens = usage.reasoning_output;
            record.reported_model = usage.model;
            if usage.total.is_none() { record.usage_error = Some("检测会话未提供总消耗".into()); }
        }
        Err(error) => record.usage_error = Some(error),
    }
}

/// Terminal write for the whole batch, storing any last pending run rows
/// first. A write that cannot be stored keeps its data in the slot for the
/// retry entry instead of reporting unearned success.
fn stop(
    registry: &ProbeRegistry,
    ledger: &ProbeLedger,
    batch_id: &str,
    status: ProbeBatchStatus,
    status_error: Option<String>,
    pending_runs: Vec<ledger::ProbeRunRecord>,
) {
    if let Err(error) = registry.finish_batch(ledger, batch_id, status, status_error, pending_runs) {
        log::warn!("检测结束结果保存失败：{error}");
    }
}

fn undetermined(seq: u32, error: &str) -> ledger::ProbeRunRecord {
    ledger::ProbeRunRecord {
        seq,
        status: ProbeRunStatus::Undetermined,
        session_id: None,
        final_answer: None,
        reported_model: None,
        duration_ms: None,
        reasoning_tokens: None,
        total_tokens: None,
        execution_error: Some(error.to_string()),
        usage_error: None,
    }
}
