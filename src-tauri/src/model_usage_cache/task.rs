use asb_core::contracts::{ModelUsageProgress, ModelUsageRead, ModelUsageRefresh};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

struct Run { root: PathBuf, scope: String, id: String, started_at: String, view: ModelUsageRefresh }

#[derive(Clone, Default)]
pub(crate) struct ModelUsageManager { state: Arc<Mutex<Option<Run>>> }

impl ModelUsageManager {
    pub(crate) fn start(&self, root: PathBuf) -> Result<ModelUsageRefresh, String> {
        let scope = super::cache_key()?;
        self.start_with(root, scope, super::rebuild)
    }

    fn start_with(&self, root: PathBuf, scope: String,
        build: impl FnOnce(&Path, &str, &dyn Fn(ModelUsageProgress)) -> Result<ModelUsageRead,String> + Send + 'static,
    ) -> Result<ModelUsageRefresh, String> {
        let mut state = self.state.lock().map_err(|_| "用量任务状态不可用")?;
        if let Some(run) = state.as_ref().filter(|run| matches!(&run.view, ModelUsageRefresh::Running { .. })) {
            if run.root != root || run.scope != scope { return Err("来源已经变化，请等待当前统计结束后刷新".into()); }
            return Ok(run.view.clone());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let started_at = Utc::now().to_rfc3339();
        let view = ModelUsageRefresh::Running { id: id.clone(), started_at: started_at.clone(),
            progress: crate::model_usage::progress::initial() };
        *state = Some(Run { root: root.clone(), scope: scope.clone(), id: id.clone(), started_at, view: view.clone() });
        drop(state);
        let manager = self.clone();
        let task_id = id.clone();
        let spawn = std::thread::Builder::new().name("model-usage-refresh".into()).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                build(&root, &scope, &|progress| manager.progress(&task_id, progress))))
                .unwrap_or_else(|_| Err("统计任务异常终止；已保留上次完整汇总".into()));
            manager.finish(&task_id, result);
        });
        if let Err(error) = spawn {
            let message = format!("无法启动统计：{error}");
            self.finish(&id, Err(message.clone()));
            return Err(message);
        }
        Ok(view)
    }

    pub(crate) fn read(&self, root: &Path, id: &str) -> Result<ModelUsageRefresh, String> {
        let state = self.state.lock().map_err(|_| "用量任务状态不可用")?;
        state.as_ref().filter(|run| run.root == root && run.id == id).map(|run| run.view.clone())
            .ok_or_else(|| "统计任务已经结束或被替换，请重新刷新".into())
    }

    fn progress(&self, id: &str, progress: ModelUsageProgress) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(run) = state.as_mut().filter(|run| run.id == id) {
                run.view = ModelUsageRefresh::Running { id: run.id.clone(), started_at: run.started_at.clone(), progress };
            }
        }
    }

    fn finish(&self, id: &str, result: Result<ModelUsageRead,String>) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(run) = state.as_mut().filter(|run| run.id == id) {
                let id = run.id.clone();
                let started_at = run.started_at.clone();
                run.view = match result {
                    Ok(read) => ModelUsageRefresh::Completed { id, started_at, read },
                    Err(message) => {
                        log::warn!("本地统计失败：{message}");
                        ModelUsageRefresh::Failed { id, started_at, message: asb_core::adapter::scrub_message(message) }
                    }
                };
            }
        }
    }
}

#[cfg(test)]
mod tests;
