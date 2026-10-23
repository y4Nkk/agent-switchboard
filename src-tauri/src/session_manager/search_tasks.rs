use super::{search, SessionSearchPage, SessionSearchRequest};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Arc, LazyLock, Mutex, Once};
use std::time::{Duration, Instant};

const LEASE: Duration = Duration::from_secs(60);
const MAX_TASKS: usize = 16;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static TASKS: LazyLock<Mutex<HashMap<String, Task>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static REAPER: Once = Once::new();

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchStage { Discovering, Indexing, Searching }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProgress {
    pub stage: SearchStage,
    pub completed: usize,
    pub total: Option<usize>,
    pub messages_completed: usize,
    pub messages_total: Option<usize>,
}

impl Default for SearchProgress {
    fn default() -> Self {
        Self { stage: SearchStage::Discovering, completed: 0, total: None, messages_completed: 0, messages_total: None }
    }
}

pub(crate) struct SearchControl {
    cancelled: AtomicBool,
    touched: Mutex<Option<Instant>>,
    progress: Mutex<SearchProgress>,
}

impl Default for SearchControl {
    fn default() -> Self {
        Self { cancelled: AtomicBool::new(false), touched: Mutex::new(None), progress: Mutex::new(SearchProgress::default()) }
    }
}

impl SearchControl {
    pub(crate) fn cancel(&self) { self.cancelled.store(true, Ordering::Relaxed); }

    pub(crate) fn checkpoint(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) { return Err("会话搜索已取消".into()); }
        if self.expired()? { return Err("会话搜索读取已停止，任务已取消".into()); }
        Ok(())
    }

    fn expired(&self) -> Result<bool, String> {
        Ok(self.touched.lock().map_err(|_| "搜索租约锁不可用")?.as_ref().is_some_and(|touched| touched.elapsed() > LEASE))
    }

    pub(crate) fn sessions(&self, completed: usize, total: usize) -> Result<(), String> {
        self.checkpoint()?;
        *self.progress.lock().map_err(|_| "搜索进度锁不可用")? = SearchProgress {
            stage: SearchStage::Searching, completed, total: Some(total), messages_completed: 0, messages_total: None,
        };
        Ok(())
    }

    pub(crate) fn messages(&self, completed: usize, total: usize) -> Result<(), String> {
        self.checkpoint()?;
        let mut progress = self.progress.lock().map_err(|_| "搜索进度锁不可用")?;
        progress.stage = SearchStage::Searching;
        progress.messages_completed = completed;
        progress.messages_total = Some(total);
        Ok(())
    }

    pub(crate) fn indexing(&self) -> Result<(), String> {
        self.checkpoint()?;
        self.progress.lock().map_err(|_| "搜索进度锁不可用")?.stage = SearchStage::Indexing;
        Ok(())
    }
}

struct Task {
    root: PathBuf,
    control: Arc<SearchControl>,
    result: Arc<Mutex<Option<Result<SessionSearchPage, String>>>>,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SearchTaskState {
    Running { progress: SearchProgress },
    Completed { page: SessionSearchPage },
    Failed { message: String },
}

fn reap(tasks: &mut HashMap<String, Task>) {
    tasks.retain(|_, task| {
        match task.control.expired() {
            Ok(true) => task.control.cancel(),
            Ok(false) => {},
            Err(error) => { log::error!("搜索任务租约不可用：{error}"); task.control.cancel(); },
        }
        // A cancelled worker still owns resources until it actually exits.
        if !task.control.cancelled.load(Ordering::Relaxed) { return true; }
        match task.result.lock() {
            Ok(result) => result.is_none(),
            Err(error) => { log::error!("搜索任务结果不可用：{error}"); false },
        }
    });
}

fn start_reaper() {
    REAPER.call_once(|| {
        tauri::async_runtime::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                match TASKS.lock() {
                    Ok(mut tasks) => reap(&mut tasks),
                    Err(error) => { log::error!("搜索任务回收锁不可用：{error}"); break; },
                }
            }
        });
    });
}

pub(crate) fn start(root: PathBuf, request: SessionSearchRequest) -> Result<String, String> {
    start_reaper();
    let mut tasks = TASKS.lock().map_err(|_| "搜索任务锁不可用")?;
    reap(&mut tasks);
    if tasks.len() >= MAX_TASKS { return Err("同时读取的会话搜索过多，请先停止已有搜索".into()); }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string();
    let control = Arc::new(SearchControl::default());
    *control.touched.lock().map_err(|_| "搜索租约锁不可用")? = Some(Instant::now());
    let result = Arc::new(Mutex::new(None));
    tasks.insert(id.clone(), Task { root: root.clone(), control: control.clone(), result: result.clone() });
    let task_id = id.clone();
    tauri::async_runtime::spawn(async move {
        let worker_control = control.clone();
        let outcome = tauri::async_runtime::spawn_blocking(move || search::search_observed(&root, request, &worker_control)).await
            .unwrap_or_else(|error| Err(format!("会话搜索执行失败：{error}")));
        let mut tasks = match TASKS.lock() {
            Ok(tasks) => tasks,
            Err(error) => { log::error!("搜索任务完成锁不可用：{error}"); return; },
        };
        let expired = match control.expired() {
            Ok(expired) => expired,
            Err(error) => { log::error!("搜索任务完成时租约不可用：{error}"); control.cancel(); false },
        };
        if control.cancelled.load(Ordering::Relaxed) || expired {
            tasks.remove(&task_id);
            return;
        }
        match result.lock() {
            Ok(mut slot) => *slot = Some(outcome),
            Err(error) => log::error!("会话搜索结果锁不可用：{error}"),
        };
    });
    Ok(id)
}

pub(crate) fn read(root: &Path, id: &str) -> Result<SearchTaskState, String> {
    let mut tasks = TASKS.lock().map_err(|_| "搜索任务锁不可用")?;
    reap(&mut tasks);
    let task = tasks.get(id).filter(|task| task.root == root && !task.control.cancelled.load(Ordering::Relaxed))
        .ok_or("会话搜索已结束或停止，请重新搜索")?;
    *task.control.touched.lock().map_err(|_| "搜索租约锁不可用")? = Some(Instant::now());
    let result = task.result.lock().map_err(|_| "搜索结果锁不可用")?.take();
    if let Some(result) = result {
        tasks.remove(id);
        return Ok(match result {
            Ok(page) => SearchTaskState::Completed { page },
            Err(message) => SearchTaskState::Failed { message: asb_core::adapter::scrub_message(message) },
        });
    }
    let progress = task.control.progress.lock().map_err(|_| "搜索进度锁不可用")?.clone();
    Ok(SearchTaskState::Running { progress })
}

pub(crate) fn cancel(root: &Path, id: &str) -> Result<(), String> {
    let mut tasks = TASKS.lock().map_err(|_| "搜索任务锁不可用")?;
    if tasks.get(id).is_some_and(|task| task.root == root) {
        if let Some(task) = tasks.get(id) { task.control.cancel(); }
    }
    reap(&mut tasks);
    Ok(())
}

#[cfg(test)]
mod tests;
