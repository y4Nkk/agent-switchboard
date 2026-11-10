mod snapshot;
mod execution;

use asb_core::AppKind;
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, sync::{Mutex, MutexGuard}};
use super::SessionMeta;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Action { Archive, Unarchive, Delete }

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub app: Option<AppKind>,
    pub archived: bool,
    pub active_after: String,
    pub active_before: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item { pub session: SessionMeta, pub related: bool }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub id: String,
    pub action: Action,
    pub request: Request,
    pub matched: usize,
    pub items: Vec<Item>,
    pub issues: Vec<super::SessionIssue>,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub id: Option<String>,
    pub running: bool,
    pub completed: usize,
    pub total: usize,
    pub outcomes: Vec<Outcome>,
    pub warning: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome { pub app: AppKind, pub session_id: String, pub completed: bool, pub error: Option<String> }

struct Plan {
    root: PathBuf,
    preview: Preview,
    fingerprints: std::collections::BTreeMap<(AppKind, String), String>,
    roots: Vec<super::SessionDeleteRequest>,
}

#[derive(Default)]
struct State { plan: Option<Plan>, progress: Progress, preparing: bool }

#[derive(Default)]
pub struct Operations { state: Mutex<State> }

impl Operations {
    fn lock(&self) -> Result<MutexGuard<'_, State>, String> { self.state.lock().map_err(|_| "会话区间操作状态不可用".into()) }

    pub fn progress(&self) -> Result<Progress, String> { Ok(self.lock()?.progress.clone()) }

    pub fn preview(&self, root: &Path, request: Request, action: Action) -> Result<Preview, String> {
        {
            let mut state = self.lock()?;
            if state.progress.running || state.preparing { return Err("已有会话区间操作正在处理".into()); }
            state.plan = None; state.preparing = true;
        }
        let prepared = snapshot::prepare(root, request, action);
        let mut state = self.lock()?;
        state.preparing = false;
        let plan = prepared?;
        let preview = plan.preview.clone();
        state.plan = Some(plan);
        Ok(preview)
    }

    pub fn discard(&self, id: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.plan.as_ref().is_some_and(|plan| plan.preview.id == id) { state.plan = None; }
        Ok(())
    }

    pub fn execute(&self, root: &Path, id: &str, confirmed: bool) -> Result<Progress, String> {
        if !confirmed { return Err("请先确认日期范围及全部关联会话".into()); }
        let plan = {
            let mut state = self.lock()?;
            if state.progress.running { return Err("已有会话区间操作正在执行".into()); }
            let plan = state.plan.as_ref().ok_or("预览已失效，请重新预览")?;
            if plan.preview.id != id || plan.root != root { return Err("预览已失效，请重新预览".into()); }
            if !plan.preview.issues.is_empty() { return Err("仍有会话来源读取问题，请处理后重新预览".into()); }
            let plan = state.plan.take().ok_or("预览已失效，请重新预览")?;
            state.progress = Progress { id: Some(plan.preview.id.clone()), running: true, total: plan.preview.items.len(), ..Progress::default() };
            plan
        };
        let result = execution::run(self, root, plan);
        self.lock()?.progress.running = false;
        result?;
        self.progress()
    }
}

#[cfg(test)]
mod tests;
