use super::{ledger, progress, ProbeGrading, UnsavedProbe};
use ledger::{ProbeBatchStatus, ProbeRunRecord, ProbeRunStatus};
use serde::Serialize;

pub(super) struct RunObservation {
    pub seq: u32,
    pub status: ProbeRunStatus,
    pub execution_failed: bool,
}

impl From<&ProbeRunRecord> for RunObservation {
    fn from(run: &ProbeRunRecord) -> Self {
        Self { seq: run.seq, status: run.status, execution_failed: run.execution_error.is_some() }
    }
}

pub(super) struct ObservationSnapshot {
    pub batch_id: String,
    pub status: ProbeBatchStatus,
    pub planned_runs: u32,
    pub question_label: String,
    pub grading_version: ProbeGrading,
    pub status_error: Option<String>,
    pub runs: Vec<RunObservation>,
}

/// Background reads never load answer, prompt, or configuration payloads.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeObservation {
    pub batch_id: String,
    pub status: ProbeBatchStatus,
    pub planned_runs: u32,
    pub completed_runs: u32,
    pub passed_runs: u32,
    pub judged_runs: u32,
    pub execution_failed: bool,
    pub question_label: String,
    pub grading_version: ProbeGrading,
    pub status_error: Option<String>,
    pub persist_pending: bool,
    pub persist_error: Option<String>,
    pub progress: Option<progress::ProbeProgressView>,
}

pub(super) fn assemble(
    mut snapshot: ObservationSnapshot,
    pending: Option<&UnsavedProbe>,
    progress: Option<progress::ProbeProgressView>,
) -> ProbeObservation {
    if let Some(pending) = pending {
        for record in &pending.pending_runs {
            let summary = RunObservation::from(record);
            if let Some(stored) = snapshot.runs.iter_mut().find(|run| run.seq == record.seq) { *stored = summary; }
            else { snapshot.runs.push(summary); }
        }
        snapshot.status = pending.terminal.0;
        snapshot.status_error = pending.terminal.1.clone();
    }
    ProbeObservation {
        batch_id: snapshot.batch_id, status: snapshot.status, planned_runs: snapshot.planned_runs,
        completed_runs: snapshot.runs.iter().filter(|run| run.status != ProbeRunStatus::Running).count() as u32,
        passed_runs: snapshot.runs.iter().filter(|run| run.status == ProbeRunStatus::Passed).count() as u32,
        judged_runs: snapshot.runs.iter().filter(|run| matches!(run.status, ProbeRunStatus::Passed | ProbeRunStatus::Failed)).count() as u32,
        execution_failed: snapshot.runs.iter().any(|run| run.execution_failed),
        question_label: snapshot.question_label, grading_version: snapshot.grading_version,
        status_error: snapshot.status_error, persist_pending: pending.is_some(),
        persist_error: pending.map(|pending| format!("检测结果尚未保存：{}", pending.save_error)), progress,
    }
}
