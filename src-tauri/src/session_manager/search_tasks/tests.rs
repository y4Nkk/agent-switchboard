use super::*;

fn register(root: &Path, result: Option<Result<SearchOutput, String>>) -> (String, Arc<SearchControl>) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string();
    let control = Arc::new(SearchControl::default());
    *control.touched.lock().unwrap() = Some(Instant::now());
    TASKS.lock().unwrap().insert(id.clone(), Task {
        root: root.into(), library: true, control: control.clone(), result: Arc::new(Mutex::new(result)),
    });
    (id, control)
}

fn read_library(root: &Path, id: &str, page: usize) -> Result<SearchTaskState, String> {
    read(root, SearchReadRequest::Library { id: id.into(), page })
}

#[test]
fn progress_reports_real_counts_without_a_total_during_discovery() {
    let control = SearchControl::default();
    let value = serde_json::to_value(&*control.progress.lock().unwrap()).unwrap();
    assert_eq!(value["stage"], "discovering");
    assert!(value["total"].is_null());
    control.sessions(2, 8).unwrap();
    control.indexing().unwrap();
    assert_eq!(serde_json::to_value(&*control.progress.lock().unwrap()).unwrap()["stage"], "indexing");
    control.messages(17, 23).unwrap();
    let value = serde_json::to_value(&*control.progress.lock().unwrap()).unwrap();
    assert_eq!(value, serde_json::json!({ "stage":"searching", "completed":2, "total":8,
        "messagesCompleted":17, "messagesTotal":23 }));
    control.sessions(3, 8).unwrap();
    assert!(control.progress.lock().unwrap().messages_total.is_none());
}

#[test]
fn cancellation_and_expired_read_lease_abort_processing() {
    let control = SearchControl::default();
    control.cancel();
    assert!(control.sessions(0, 8).is_err());
    assert!(control.messages(1, 4).is_err());
    let expired = SearchControl::default();
    *expired.touched.lock().unwrap() = Some(Instant::now() - LEASE - Duration::from_secs(1));
    assert!(expired.checkpoint().is_err());
}

#[test]
fn completed_library_snapshot_supports_multiple_pages_and_expires_after_idle_lease() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = SearchSnapshot::build(vec![], vec![], |_| Ok(vec![])).unwrap();
    let (id, control) = register(root.path(), Some(Ok(SearchOutput::Library(Arc::new(Mutex::new(snapshot))))));
    assert!(read_library(root.path(), &id, 0).is_err());
    let completed = serde_json::to_value(read_library(root.path(), &id, 1).unwrap()).unwrap();
    assert_eq!(completed["state"], "completed");
    assert_eq!(completed["result"]["scope"], "library");
    assert_eq!(completed["result"]["value"]["total"], 0);
    assert!(read(root.path(), SearchReadRequest::Transcript { id: id.clone() }).is_err());
    *control.touched.lock().unwrap() = Some(Instant::now() - LEASE - Duration::from_secs(1));
    assert!(matches!(read_library(root.path(), &id, 2).unwrap(), SearchTaskState::Completed { .. }));
    *control.touched.lock().unwrap() = Some(Instant::now() - SNAPSHOT_LEASE - Duration::from_secs(1));
    assert!(matches!(read_library(root.path(), &id, 1).unwrap(), SearchTaskState::Expired));
}

#[test]
fn terminal_failure_is_consumed_once() {
    let root = tempfile::tempdir().unwrap();
    let (id, _) = register(root.path(), Some(Err("unreadable".into())));
    assert!(matches!(read_library(root.path(), &id, 1).unwrap(), SearchTaskState::Failed { .. }));
    assert!(matches!(read_library(root.path(), &id, 1).unwrap(), SearchTaskState::Expired));
}

#[test]
fn only_accepts_explicit_search_scopes() {
    let library = serde_json::json!({ "query":"test", "pinnedOnly":false, "archived":false });
    assert!(serde_json::from_value::<SearchRequest>(library.clone()).is_err());
    assert!(serde_json::from_value::<SearchRequest>(serde_json::json!({
        "scope": "library", "value": { "query": "test", "offset": 0, "pinnedOnly": false, "archived": false }
    })).is_err());
    assert!(matches!(serde_json::from_value::<SearchRequest>(serde_json::json!({
        "scope": "library", "value": library
    })).unwrap(), SearchRequest::Library(_)));
    assert!(matches!(serde_json::from_value::<SearchRequest>(serde_json::json!({
        "scope": "transcript", "value": { "app":"codex", "sessionId":"one", "query":"test", "offset":0, "revision":null }
    })).unwrap(), SearchRequest::Transcript(_)));
    assert!(serde_json::from_value::<SearchReadRequest>(serde_json::json!({ "id": "1" })).is_err());
    assert!(serde_json::from_value::<SearchReadRequest>(serde_json::json!({
        "scope": "library", "id": "1", "offset": 50
    })).is_err());
    assert!(matches!(serde_json::from_value::<SearchReadRequest>(serde_json::json!({
        "scope": "library", "id": "1", "page": 2
    })).unwrap(), SearchReadRequest::Library { page: 2, .. }));
}

#[test]
fn cancellation_is_scoped_to_exact_task_and_root() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let (old, control) = register(root.path(), None);
    let (current, _) = register(root.path(), None);
    cancel(other.path(), &old).unwrap();
    assert!(!control.cancelled.load(Ordering::Relaxed));
    cancel(root.path(), &old).unwrap();
    assert!(control.checkpoint().is_err());
    assert!(matches!(read_library(root.path(), &old, 1).unwrap(), SearchTaskState::Expired));
    assert!(matches!(read_library(root.path(), &current, 1).unwrap(), SearchTaskState::Running { .. }));
    assert!(TASKS.lock().unwrap().contains_key(&old));
    cancel(root.path(), &current).unwrap();
    TASKS.lock().unwrap().remove(&old);
    TASKS.lock().unwrap().remove(&current);
}

#[test]
fn expired_running_work_remains_bounded_until_exit_but_terminal_work_is_reclaimed() {
    let root = tempfile::tempdir().unwrap();
    let (id, control) = register(root.path(), None);
    *control.touched.lock().unwrap() = Some(Instant::now() - LEASE - Duration::from_secs(1));
    let mut tasks = TASKS.lock().unwrap();
    reap(&mut tasks);
    assert!(control.checkpoint().is_err());
    assert!(tasks.contains_key(&id));
    *tasks.get(&id).unwrap().result.lock().unwrap() = Some(Err("cancelled".into()));
    reap(&mut tasks);
    assert!(!tasks.contains_key(&id));
}
