//! Local-only native performance ingestion. Model requests never pass this receiver.
mod contracts;
mod decode;
mod otlp;
mod server;
pub(crate) mod configuration;
pub(crate) mod store;
pub(crate) use contracts::{MeasurementScope, NativeTimeSource, ReceiverStatus, RequestOutcome, RequestUsageRow};
use std::{path::PathBuf, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}};

pub(crate) const ENDPOINT: &str = "http://127.0.0.1:4318";

struct Worker {
    stop: Arc<AtomicBool>,
    server: Arc<tiny_http::Server>,
    thread: std::thread::JoinHandle<()>,
}

impl Drop for Worker {
    fn drop(&mut self) { self.stop.store(true, Ordering::Release); self.server.unblock(); }
}

pub(crate) struct NativeTelemetry {
    root: PathBuf,
    status: Arc<Mutex<ReceiverStatus>>,
    worker: Mutex<Option<Worker>>,
}

impl NativeTelemetry {
    pub(crate) fn start(root: PathBuf) -> Self {
        let service = Self { root, status: Arc::new(Mutex::new(ReceiverStatus::default())),
            worker: Mutex::new(None) };
        if let Err(error) = service.retry() {
            log::error!("原生性能接收器未启动：{error}");
        }
        service
    }

    pub(crate) fn status(&self) -> Result<ReceiverStatus, String> {
        self.status.lock().map(|status| status.clone()).map_err(|_| "原生性能状态锁不可用".into())
    }

    pub(crate) fn retry(&self) -> Result<(), String> {
        let result = self.bind();
        if let Err(error) = &result {
            if let Ok(mut status) = self.status.lock() { status.error = Some(error.clone()); }
        }
        result
    }

    fn bind(&self) -> Result<(), String> {
        let mut worker = self.worker.lock().map_err(|_| "原生性能监听锁不可用")?;
        if worker.as_ref().is_some_and(|worker| !worker.thread.is_finished()) {
            return if self.status()?.listening { Ok(()) } else { Err("原生性能接收器仍在关闭，请稍后重试".into()) };
        }
        worker.take();
        store::initialize(&self.root)?;
        let server = Arc::new(tiny_http::Server::http(("127.0.0.1", 4318))
            .map_err(|error| format!("无法监听 {ENDPOINT}，请检查端口占用后重试：{error}"))?);
        let stop = Arc::new(AtomicBool::new(false));
        let status = self.status.clone();
        let thread_server = server.clone();
        let thread_stop = stop.clone();
        let root = self.root.clone();
        let thread = std::thread::Builder::new().name("native-performance".into()).spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match thread_server.recv_timeout(std::time::Duration::from_millis(250)) {
                    Ok(Some(request)) => server::receive(request, &root, &status),
                    Ok(None) => {}
                    Err(error) => {
                        if let Ok(mut status) = status.lock() {
                            status.listening = false;
                            status.error = Some(format!("原生性能监听中断：{error}"));
                        }
                        break;
                    }
                }
            }
        }).map_err(|error| error.to_string())?;
        *worker = Some(Worker { server, stop, thread });
        let mut status = self.status.lock().map_err(|_| "原生性能状态锁不可用")?;
        status.listening = true;
        status.error = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
