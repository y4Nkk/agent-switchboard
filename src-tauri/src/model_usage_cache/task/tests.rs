use super::*;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn id(task: &ModelUsageRefresh) -> &str {
    match task {
        ModelUsageRefresh::Running { id, .. } | ModelUsageRefresh::Completed { id, .. }
        | ModelUsageRefresh::Failed { id, .. } => id,
    }
}

fn terminal(manager: &ModelUsageManager, root: &Path, id: &str) -> ModelUsageRefresh {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let view = manager.read(root,id).unwrap();
        if !matches!(view, ModelUsageRefresh::Running { .. }) { return view; }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn overlapping_starts_share_one_task_and_failures_have_a_terminal_state() {
    let temp = tempfile::tempdir().unwrap();
    let manager = ModelUsageManager::default();
    let (release, waiting) = mpsc::channel();
    let task = manager.start_with(temp.path().into(), "scope".into(), move |_,_,publish| {
        let mut progress = crate::model_usage::progress::initial();
        progress.total_sources = Some(5);
        progress.completed_sources = 2;
        publish(progress);
        waiting.recv().unwrap();
        Err("source unavailable".into())
    }).unwrap();
    let duplicate = manager.start_with(temp.path().into(), "scope".into(), |_,_,_| panic!("must join")).unwrap();
    assert_eq!(id(&task),id(&duplicate));
    assert!(manager.read(&temp.path().join("other"),id(&task)).is_err());
    release.send(()).unwrap();
    assert!(matches!(terminal(&manager,temp.path(),id(&task)),ModelUsageRefresh::Failed { message, .. } if message == "source unavailable"));
    let next = manager.start_with(temp.path().into(),"scope".into(),|_,_,_| Err("new attempt".into())).unwrap();
    assert_ne!(id(&next),id(&task));
    terminal(&manager,temp.path(),id(&next));
}

#[test]
fn a_panicking_worker_does_not_leave_a_permanent_loading_state() {
    let temp = tempfile::tempdir().unwrap();
    let manager = ModelUsageManager::default();
    let task = manager.start_with(temp.path().into(),"scope".into(),|_,_,_| panic!("fixture failure")).unwrap();
    assert!(matches!(terminal(&manager,temp.path(),id(&task)),ModelUsageRefresh::Failed { .. }));
}
