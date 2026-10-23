use super::*;

fn register(root: &Path, result: Option<Result<SessionSearchPage, String>>) -> (String, Arc<SearchControl>) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string();
    let control = Arc::new(SearchControl::default());
    *control.touched.lock().unwrap() = Some(Instant::now());
    TASKS.lock().unwrap().insert(id.clone(), Task {
        root: root.into(), control: control.clone(), result: Arc::new(Mutex::new(result)),
    });
    (id, control)
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
fn terminal_result_is_consumed_once_and_failures_are_terminal_too() {
    let root = tempfile::tempdir().unwrap();
    let page = SessionSearchPage { results: vec![], total: 0, issues: vec![], projects: vec![], tags: vec![] };
    let (id, _) = register(root.path(), Some(Ok(page)));
    assert!(matches!(read(root.path(), &id).unwrap(), SearchTaskState::Completed { .. }));
    assert!(read(root.path(), &id).is_err());
    let (id, _) = register(root.path(), Some(Err("unreadable".into())));
    assert!(matches!(read(root.path(), &id).unwrap(), SearchTaskState::Failed { .. }));
    assert!(read(root.path(), &id).is_err());
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
    assert!(read(root.path(), &old).is_err());
    assert!(matches!(read(root.path(), &current).unwrap(), SearchTaskState::Running { .. }));
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
