use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static PREVIOUS: Mutex<Option<HashMap<(PathBuf, &'static str), BTreeSet<String>>>> = Mutex::new(None);

pub(crate) fn report(root: &Path, area: &'static str, messages: impl Iterator<Item = String>) {
    let current: BTreeSet<_> = messages.collect();
    let mut state = PREVIOUS.lock().unwrap_or_else(|error| {
        log::warn!("正在恢复会话来源诊断状态锁");
        error.into_inner()
    });
    let state = state.get_or_insert_with(HashMap::new);
    let key = (root.to_path_buf(), area);
    let previous = state.remove(&key).unwrap_or_default();
    for message in current.difference(&previous) { log::warn!("{area}：{message}"); }
    if current.is_empty() {
        if !previous.is_empty() { log::info!("{area}已自动恢复"); }
    } else { state.insert(key, current); }
}
