//! Codex degradation probe commands: the question catalog, the single-batch
//! start/cancel/retry trio, and the persisted history reads and deletes.
//! Every run spends real Codex quota and is recorded in the local probe
//! history before, during, and after the calls.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager};

use crate::codex_probe::{
    self, ProbeBatchStatus, ProbeBatchView, ProbeHistoryPage, ProbeHistoryProfile,
    ProbeGrading, ProbeProfileFilter, ProbeRegistry, MAX_RUN_COUNT,
};
use crate::commands::error::{blocking, state, CommandError};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProbeQuestionDto<'a> {
    id: &'a str,
    label: &'a str,
    text: &'a str,
    grading_version: ProbeGrading,
    reference_visual: Option<&'a str>,
    max_runs: u32,
    timeout_seconds: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProbeCatalogDto {
    questions: Vec<CodexProbeQuestionDto<'static>>,
    custom_timeout_seconds: BTreeMap<&'static str, u64>,
    custom_max_runs: u32,
}

#[tauri::command]
pub async fn get_codex_probe_catalog() -> Result<CodexProbeCatalogDto, CommandError> {
    let questions = codex_probe::QUESTIONS
        .iter()
        .map(|question| CodexProbeQuestionDto {
            id: question.id,
            label: question.label,
            text: question.prompt,
            grading_version: question.grading,
            reference_visual: question.reference_visual,
            max_runs: question.max_runs,
            timeout_seconds: question.timeout_seconds,
        })
        .collect();
    let custom_timeout_seconds = [ProbeGrading::Integer, ProbeGrading::SvgVisual, ProbeGrading::HtmlVisual]
        .into_iter().map(|grading| (grading.as_str(), grading.custom_timeout_seconds())).collect();
    Ok(CodexProbeCatalogDto { questions, custom_timeout_seconds, custom_max_runs: MAX_RUN_COUNT })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartCodexProbeRequest {
    run_count: u32,
    /// A built-in question id, or `custom` to use the user-authored question.
    question_id: String,
    custom_question: Option<String>,
    custom_answer: Option<String>,
    custom_grading_version: Option<ProbeGrading>,
}

#[tauri::command]
pub async fn start_codex_probe(
    app: AppHandle,
    request: StartCodexProbeRequest,
) -> Result<String, CommandError> {
    if request.run_count == 0 || request.run_count > MAX_RUN_COUNT {
        return Err(CommandError::localized(
            "codex-probe-invalid-count",
            "errors.misc.probeInvalidCount",
            format!("检测次数必须在 1 到 {MAX_RUN_COUNT} 之间"),
            serde_json::json!({ "max": MAX_RUN_COUNT }),
        ));
    }
    let question = codex_probe::resolve_question(
        &request.question_id,
        request.custom_question,
        request.custom_answer,
        request.custom_grading_version,
    )
    .map_err(|message| CommandError::new("codex-probe-invalid-question", message))?;
    if request.run_count > question.max_runs {
        return Err(CommandError::localized(
            "codex-probe-question-run-limit",
            "errors.misc.probeRunLimit",
            format!("该题目最多检测 {} 次", question.max_runs),
            serde_json::json!({ "max": question.max_runs }),
        ));
    }
    let local = state(&app)?;
    let registry = app.state::<ProbeRegistry>().inner().clone();
    let gateway = app
        .try_state::<crate::gateway::GatewayController>()
        .map(|controller| controller.inner().clone());
    let worker_app = app.clone();
    let batch_id = blocking(move || {
        codex_probe::spawn_probe(&registry, &local, question, request.run_count, gateway.as_ref(), worker_app)
            .map_err(|message| CommandError::new("codex-probe-start-failed", message))
    })
    .await?;
    codex_probe::notify_changed(&app, &batch_id);
    Ok(batch_id)
}

/// The live-or-latest batch; the panel reconnects here after a refresh or
/// restart without remembering any id.
#[tauri::command]
pub async fn get_current_codex_probe(app: AppHandle) -> Result<Option<ProbeBatchView>, CommandError> {
    let local = state(&app)?;
    let registry = app.state::<ProbeRegistry>().inner().clone();
    blocking(move || {
        codex_probe::current_view(&local, &registry)
            .map_err(|message| CommandError::new("codex-probe-read-failed", message))
    })
    .await
}

#[tauri::command]
pub async fn cancel_codex_probe(app: AppHandle) -> Result<bool, CommandError> {
    Ok(app.state::<ProbeRegistry>().cancel())
}

/// Flushes results whose persistence failed mid-batch.
#[tauri::command]
pub async fn retry_codex_probe_save(app: AppHandle) -> Result<(), CommandError> {
    let local = state(&app)?;
    let registry = app.state::<ProbeRegistry>().inner().clone();
    blocking(move || {
        let batch = codex_probe::current_view(&local, &registry)
            .map_err(|message| CommandError::new("codex-probe-read-failed", message))?;
        let result = codex_probe::retry_save(&local, &registry)
            .map_err(|message| CommandError::new("codex-probe-save-failed", message));
        if let Some(view) = batch.filter(|view| view.persist_pending) {
            codex_probe::notify_changed(&app, &view.batch.id);
        }
        result
    })
    .await
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum CodexProbeProfileFilterDto {
    All {},
    Unlinked {},
    Profile { id: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListCodexProbeHistoryRequest {
    offset: u32,
    limit: u32,
    /// Omitted (or zero) means all time.
    days: Option<u32>,
    profile: Option<CodexProbeProfileFilterDto>,
    /// Omitted means every status.
    status: Option<String>,
}

#[tauri::command]
pub async fn list_codex_probe_history(
    app: AppHandle,
    request: ListCodexProbeHistoryRequest,
) -> Result<ProbeHistoryPage, CommandError> {
    let status = match request.status.as_deref() {
        None | Some("all") => None,
        Some(value) => Some(ProbeBatchStatus::parse(value).ok_or_else(|| {
            CommandError::localized(
                "codex-probe-history-invalid",
                "errors.misc.probeUnknownStatus",
                format!("未知的检测状态：{value}"),
                serde_json::json!({ "status": value }),
            )
        })?),
    };
    let profile = match request.profile {
        None | Some(CodexProbeProfileFilterDto::All {}) => ProbeProfileFilter::All,
        Some(CodexProbeProfileFilterDto::Unlinked {}) => ProbeProfileFilter::Unlinked,
        Some(CodexProbeProfileFilterDto::Profile { id }) => ProbeProfileFilter::Id(id),
    };
    let local = state(&app)?;
    blocking(move || {
        let ledger = codex_probe::ProbeLedger::new(local.root());
        ledger
            .list_history(&codex_probe::ProbeHistoryQuery {
                offset: request.offset,
                limit: request.limit,
                days: request.days.filter(|days| *days > 0),
                profile,
                status,
            })
            .map_err(|message| CommandError::new("codex-probe-history-failed", message))
    })
    .await
}

#[tauri::command]
pub async fn list_codex_probe_history_profiles(
    app: AppHandle,
) -> Result<Vec<ProbeHistoryProfile>, CommandError> {
    let local = state(&app)?;
    blocking(move || {
        codex_probe::ProbeLedger::new(local.root())
            .list_history_profiles()
            .map_err(|message| CommandError::new("codex-probe-history-failed", message))
    })
    .await
}

#[tauri::command]
pub async fn get_codex_probe_batch(
    app: AppHandle,
    batch_id: String,
) -> Result<Option<ProbeBatchView>, CommandError> {
    let local = state(&app)?;
    let registry = app.state::<ProbeRegistry>().inner().clone();
    blocking(move || {
        codex_probe::batch_view(&local, &registry, &batch_id)
            .map_err(|message| CommandError::new("codex-probe-history-failed", message))
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteCodexProbeBatchesRequest {
    batch_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedCodexProbeBatchesDto {
    deleted: u64,
    warning: Option<String>,
}

/// Deletes finished batches and their runs — only the radar's own records;
/// Codex sessions and the real usage ledger stay untouched.
#[tauri::command]
pub async fn delete_codex_probe_batches(
    app: AppHandle,
    request: DeleteCodexProbeBatchesRequest,
) -> Result<DeletedCodexProbeBatchesDto, CommandError> {
    let local = state(&app)?;
    blocking(move || {
        codex_probe::ProbeLedger::new(local.root())
            .delete_batches(&request.batch_ids)
            .map(|result| DeletedCodexProbeBatchesDto { deleted: result.removed, warning: result.warning })
            .map_err(|message| CommandError::new("codex-probe-delete-failed", message))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_status_filter_rejects_unknown_values() {
        assert!(ProbeBatchStatus::parse("completed").is_some());
        assert!(ProbeBatchStatus::parse("config-changed").is_some());
        assert!(ProbeBatchStatus::parse("expired").is_none());
    }
}
