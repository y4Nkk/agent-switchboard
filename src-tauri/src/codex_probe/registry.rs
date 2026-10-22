use super::*;

#[derive(Clone, Default)]
pub(crate) struct ProbeRegistry {
    slot: Arc<Mutex<Option<ProbeSlot>>>,
    shutting_down: Arc<AtomicBool>,
}

enum ProbeSlot {
    Running(LiveProbe),
    Unsaved(UnsavedProbe),
}

impl ProbeSlot {
    fn batch_id(&self) -> &str {
        match self {
            Self::Running(live) => &live.batch_id,
            Self::Unsaved(pending) => &pending.batch_id,
        }
    }
}

struct LiveProbe {
    batch_id: String,
    cancel: Arc<AtomicBool>,
    run: LiveRun,
}

enum LiveRun {
    Preparing(u32),
    Executing(Arc<progress::RunProgress>),
}

impl LiveProbe {
    fn view(&self, runs: &[ledger::ProbeRunRecord], planned: u32) -> Result<progress::ProbeProgressView, String> {
        let cancelling = self.cancel.load(Ordering::SeqCst);
        let completed = runs.iter().filter(|run| run.status != ProbeRunStatus::Running).count() as u32;
        if runs.iter().enumerate().any(|(index, run)| run.seq != index as u32 + 1) {
            return Err("检测运行记录序号不连续，无法读取进度".into());
        }
        match &self.run {
            LiveRun::Preparing(seq) if *seq == completed + 1 && *seq <= planned && completed as usize == runs.len() =>
                Ok(progress::ProbeProgressView::Preparing { seq: *seq, cancelling }),
            LiveRun::Executing(clock) if clock.seq() == completed + 1 && clock.seq() <= planned
                && runs.iter().filter(|run| run.status == ProbeRunStatus::Running).count() == 1
                && runs.last().is_some_and(|run| run.seq == clock.seq() && run.status == ProbeRunStatus::Running) =>
                Ok(progress::ProbeProgressView::Run { clock: clock.view(cancelling) }),
            _ => Err("检测运行记录与活动时钟不一致，无法读取进度".into()),
        }
    }
}

fn running_mut<'a>(slot: &'a mut Option<ProbeSlot>, batch_id: &str) -> Result<&'a mut LiveProbe, String> {
    match slot {
        Some(ProbeSlot::Running(live)) if live.batch_id == batch_id => Ok(live),
        _ => Err("检测任务已失去运行槽".into()),
    }
}

impl ProbeRegistry {
    pub(crate) fn new() -> Self { Self::default() }

    pub(super) fn begin(&self, batch_id: &str) -> Result<Arc<AtomicBool>, String> {
        let mut slot = self.slot.lock().expect("probe registry");
        if self.shutting_down.load(Ordering::SeqCst) { return Err("应用正在退出，不能开始检测".into()); }
        match slot.as_ref() {
            Some(ProbeSlot::Running(_)) => return Err("已有一次降智检测在运行，请先取消或等待完成".into()),
            Some(ProbeSlot::Unsaved(_)) => return Err("上次检测结果尚未保存，请先重试保存".into()),
            None => {}
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(ProbeSlot::Running(LiveProbe {
            batch_id: batch_id.to_string(), cancel: Arc::clone(&cancel), run: LiveRun::Preparing(1),
        }));
        Ok(cancel)
    }

    /// The row and its clock become visible together. Only the next run may start.
    pub(super) fn start_run(
        &self, ledger: &ProbeLedger, batch_id: &str, seq: u32, timeout_seconds: u64,
    ) -> Result<Arc<progress::RunProgress>, String> {
        let mut slot = self.slot.lock().expect("probe registry");
        let live = running_mut(&mut slot, batch_id)?;
        if !matches!(live.run, LiveRun::Preparing(next) if next == seq) {
            return Err("检测轮次尚未就绪或序号不匹配，未发起调用".into());
        }
        ledger.start_run(batch_id, seq)?;
        let clock = Arc::new(progress::RunProgress::new(seq, timeout_seconds));
        live.run = LiveRun::Executing(Arc::clone(&clock));
        Ok(clock)
    }

    /// A successful commit is the only transition to preparing the next call.
    pub(super) fn save_run(
        &self, ledger: &ProbeLedger, batch_id: &str, record: ledger::ProbeRunRecord,
    ) -> Result<(), String> {
        if record.status == ProbeRunStatus::Running { return Err("检测结果必须具有终态，未保存".into()); }
        let mut slot = self.slot.lock().expect("probe registry");
        let live = running_mut(&mut slot, batch_id)?;
        let LiveRun::Executing(clock) = &live.run else { return Err("当前没有可保存的检测运行".into()); };
        if clock.seq() != record.seq { return Err("结果不属于当前检测轮次，未保存".into()); }
        match ledger.save_run_result(batch_id, &record) {
            Ok(()) => { live.run = LiveRun::Preparing(record.seq + 1); Ok(()) }
            Err(error) => {
                *slot = Some(ProbeSlot::Unsaved(UnsavedProbe {
                    batch_id: batch_id.to_string(), pending_runs: vec![record], save_error: error.clone(),
                    terminal: (ProbeBatchStatus::Failed, Some("检测结果保存失败，已停止后续调用".into())),
                }));
                Err(error)
            }
        }
    }

    pub(super) fn finish_batch(
        &self, ledger: &ProbeLedger, batch_id: &str, status: ProbeBatchStatus,
        status_error: Option<String>, pending_runs: Vec<ledger::ProbeRunRecord>,
    ) -> Result<(), String> {
        if !status.is_terminal() || pending_runs.iter().any(|run| run.status == ProbeRunStatus::Running) {
            return Err("结束检测需要批次与运行结果的终态".into());
        }
        let mut slot = self.slot.lock().expect("probe registry");
        let live = running_mut(&mut slot, batch_id)?;
        match (&live.run, pending_runs.as_slice()) {
            (LiveRun::Executing(clock), [record]) if clock.seq() == record.seq => {}
            (LiveRun::Preparing(_), []) => {}
            _ => return Err("结束结果不属于当前检测轮次，未保存".into()),
        }
        let result = ledger.finish_batch(batch_id, status, status_error.clone(), &pending_runs);
        *slot = match &result {
            Ok(()) => None,
            Err(error) => Some(ProbeSlot::Unsaved(UnsavedProbe {
                batch_id: batch_id.to_string(), pending_runs, save_error: error.clone(),
                terminal: (status, status_error),
            })),
        };
        result
    }

    pub(super) fn fail_start(&self, batch_id: &str, error: String) -> Result<(), String> {
        let mut slot = self.slot.lock().expect("probe registry");
        let live = running_mut(&mut slot, batch_id)?;
        if !matches!(live.run, LiveRun::Preparing(_)) { return Err("当前检测轮次已经开始，不能记为启动失败".into()); }
        *slot = Some(ProbeSlot::Unsaved(UnsavedProbe {
            batch_id: batch_id.to_string(), pending_runs: Vec::new(), save_error: error.clone(),
            terminal: (ProbeBatchStatus::Failed, Some(error)),
        }));
        Ok(())
    }

    /// Lifecycle writes use this same lock; SQLite supplies one batch/run snapshot.
    pub(super) fn read_view(&self, ledger: &ProbeLedger, batch_id: Option<&str>) -> Result<Option<ProbeBatchView>, String> {
        let slot = self.slot.lock().expect("probe registry");
        let selected = batch_id.or_else(|| slot.as_ref().map(ProbeSlot::batch_id));
        let Some((batch, runs)) = ledger.load_snapshot(selected)? else { return Ok(None); };
        let (pending, progress) = match slot.as_ref() {
            Some(ProbeSlot::Running(live)) if live.batch_id == batch.id => {
                if batch.status != ProbeBatchStatus::Running { return Err("活动检测批次已被外部结束，状态不一致".into()); }
                (None, Some(live.view(&runs, batch.planned_runs)?))
            }
            Some(ProbeSlot::Unsaved(pending)) if pending.batch_id == batch.id => (Some(pending.clone()), None),
            _ if batch.status == ProbeBatchStatus::Running => return Err("检测批次缺少活动运行槽，请重启应用恢复中断记录".into()),
            _ => (None, None),
        };
        Ok(Some(assemble_view(batch, runs, pending, progress)))
    }

    pub(crate) fn cancel(&self) -> bool {
        let slot = self.slot.lock().expect("probe registry");
        match slot.as_ref() {
            Some(ProbeSlot::Running(live)) => !live.cancel.swap(true, Ordering::SeqCst),
            _ => false,
        }
    }

    /// Only a claim made before batch creation can be abandoned.
    pub(super) fn abandon(&self, batch_id: &str) {
        let mut slot = self.slot.lock().expect("probe registry");
        if matches!(slot.as_ref(), Some(ProbeSlot::Running(live))
            if live.batch_id == batch_id && matches!(live.run, LiveRun::Preparing(1))) {
            *slot = None;
        }
    }

    /// The pending results retain the slot until their transaction commits.
    pub(super) fn retry_save(&self, ledger: &ProbeLedger) -> Result<(), String> {
        let mut slot = self.slot.lock().expect("probe registry");
        match slot.as_mut() {
            Some(ProbeSlot::Unsaved(pending)) => {
                if let Err(error) = ledger.finish_batch(&pending.batch_id, pending.terminal.0,
                    pending.terminal.1.clone(), &pending.pending_runs) {
                    pending.save_error = error.clone();
                    return Err(error);
                }
                *slot = None;
                Ok(())
            }
            Some(ProbeSlot::Running(_)) => Err("检测仍在运行，不能重试保存".into()),
            None => Ok(()),
        }
    }

    pub(crate) fn resume_after_blocked_exit(&self) {
        self.shutting_down.store(false, Ordering::SeqCst);
    }

    pub(crate) fn request_shutdown(&self, timeout: Duration) -> Result<(), String> {
        let batch_id = {
            let slot = self.slot.lock().expect("probe registry");
            self.shutting_down.store(true, Ordering::SeqCst);
            let Some(ProbeSlot::Running(live)) = slot.as_ref() else { return Ok(()); };
            live.cancel.store(true, Ordering::SeqCst);
            live.batch_id.clone()
        };
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            let settled = {
                let slot = self.slot.lock().expect("probe registry");
                !matches!(slot.as_ref(), Some(ProbeSlot::Running(live)) if live.batch_id == batch_id)
            };
            if settled { return Ok(()); }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err("检测进程尚未结束，已取消退出，请稍后重试".into())
    }
}

#[cfg(test)]
mod tests;
