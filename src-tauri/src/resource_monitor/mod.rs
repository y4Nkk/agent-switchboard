mod discovery;
mod executables;
mod observation;
mod ownership;
mod platform;
mod sampling;
pub(crate) mod types;
mod webview;

use sampling::{Sampler, Snapshot};
use std::{sync::{Arc, Mutex}, time::Duration};
use tauri::AppHandle;
use tokio::sync::watch;
use types::{ReadRequest, ResourceRead, SAMPLE_INTERVAL_MS};

type SampleResult = Result<Arc<Snapshot>, String>;
type Flight = watch::Receiver<Option<SampleResult>>;

#[derive(Default)]
struct State {
    cached: Option<Arc<Snapshot>>,
    sampler: Sampler,
    flight: Option<Flight>,
    sequence: u64,
}

#[derive(Clone, Default)]
pub struct ResourceMonitor { state: Arc<Mutex<State>> }

enum ReadAction {
    Cached(Arc<Snapshot>),
    Waiting(Flight),
    Start { receiver: Flight, sender: watch::Sender<Option<SampleResult>>, sampler: Sampler, id: u64 },
}

impl ResourceMonitor {
    pub async fn read(&self, app: AppHandle, request: ReadRequest) -> Result<ResourceRead, String> {
        if request.details.as_ref().is_some_and(|detail| detail.page == 0) {
            return Err("进程明细页码必须从 1 开始".into());
        }
        let receiver = match self.begin(request.reset_interval)? {
            ReadAction::Cached(snapshot) => return Ok(snapshot.read(&request)),
            ReadAction::Waiting(receiver) => receiver,
            ReadAction::Start { receiver, sender, sampler, id } => {
                let owner = self.clone();
                let reset = request.reset_interval;
                tauri::async_runtime::spawn(async move { owner.collect(app, sampler, sender, reset, id).await; });
                receiver
            }
        };
        let snapshot = tokio::time::timeout(Duration::from_secs(6), wait(receiver)).await
            .map_err(|_| "资源采样尚未完成，当前采样仍在继续，请稍后重试".to_string())??;
        Ok(snapshot.read(&request))
    }

    fn begin(&self, reset: bool) -> Result<ReadAction, String> {
        let mut state = self.state.lock().map_err(|_| "资源采样状态锁不可用")?;
        if let Some(receiver) = &state.flight { return Ok(ReadAction::Waiting(receiver.clone())); }
        if !reset {
            if let Some(snapshot) = &state.cached {
                if snapshot.at.elapsed() < Duration::from_millis(SAMPLE_INTERVAL_MS) {
                    return Ok(ReadAction::Cached(snapshot.clone()));
                }
            }
        }
        let (sender, receiver) = watch::channel(None);
        state.sequence = state.sequence.checked_add(1).ok_or("资源样本序号已耗尽")?;
        state.flight = Some(receiver.clone());
        Ok(ReadAction::Start { receiver, sender, sampler: std::mem::take(&mut state.sampler), id: state.sequence })
    }

    async fn collect(&self, app: AppHandle, mut sampler: Sampler,
        sender: watch::Sender<Option<SampleResult>>, reset: bool, id: u64) {
        let interface = webview::collect(&app).await;
        let task = tauri::async_runtime::spawn_blocking(move || {
            let result = discovery::collect(interface, &sampler.previous)
                .map(|collection| Arc::new(sampler.sample(collection, reset, id)));
            (sampler, result)
        }).await;
        let (sampler, result) = match task {
            Ok(result) => result,
            Err(error) => (Sampler::default(), Err(format!("资源采样任务未完成：{error}"))),
        };
        let result = match self.state.lock() {
            Ok(mut state) => {
                state.sampler = sampler;
                if let Ok(snapshot) = &result { state.cached = Some(snapshot.clone()); }
                state.flight = None;
                result
            }
            Err(_) => Err("资源采样状态锁不可用".into()),
        };
        let _ = sender.send(Some(result));
    }
}

async fn wait(mut receiver: Flight) -> SampleResult {
    loop {
        let result = receiver.borrow_and_update().clone();
        if let Some(result) = result { return result; }
        receiver.changed().await.map_err(|_| "资源采样结果通道已关闭".to_string())?;
    }
}

#[cfg(test)]
mod tests;
