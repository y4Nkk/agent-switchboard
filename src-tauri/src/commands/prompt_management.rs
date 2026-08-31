//! User-global prompt document commands.
//!
//! The renderer chooses only a supported client, Markdown text, and the hash
//! it read. This module resolves the absolute document path and delegates all
//! mutation to `asb-switch`.

use super::error::{blocking, observe, require_write_confirmation, state, CommandError};
use crate::runtime_log::RuntimeLogAction;
use asb_core::{AppKind, GlobalPromptDocument};
use asb_switch::{
    read_global_prompt_document, write_global_prompt_document, FsIo, GlobalPromptDocumentRequest,
    RecoveryOutcome, SwitchError,
};
use tauri::AppHandle;

fn prompt_error(error: SwitchError) -> CommandError {
    match error {
        SwitchError::ReadCurrent { .. } => {
            CommandError::new("prompt-document-unreadable", "无法读取全局提示词文档")
        }
        SwitchError::BlockedByLock { .. } => CommandError::new(
            "prompt-document-locked",
            "全局提示词文档正被其他写入操作占用",
        ),
        SwitchError::ExternalChange { .. } => CommandError::new(
            "prompt-document-changed",
            "全局提示词文档已在读取后被外部修改，请重新读取后再保存",
        ),
        SwitchError::CommitFailed { recovery, .. } => {
            let message = match recovery {
                RecoveryOutcome::NotNeeded => "保存全局提示词文档失败，原文未被替换",
                RecoveryOutcome::Restored { .. } => "保存全局提示词文档失败，已恢复保存前的内容",
                RecoveryOutcome::RestoreFailed { .. } => {
                    "保存全局提示词文档失败，且无法自动恢复；请从应用备份恢复"
                }
            };
            CommandError::new("prompt-document-save-failed", message)
        }
        other => CommandError::from(other),
    }
}

/// Reads the selected client's one global instruction document. The backend
/// never returns its absolute path to the renderer.
#[tauri::command]
pub async fn get_global_prompt_document(
    app: AppHandle,
    target: AppKind,
) -> Result<GlobalPromptDocument, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let target_path = state
            .global_prompt_target(target)
            .map_err(|error| CommandError::new("prompt-document-path-unavailable", error))?;
        read_global_prompt_document(&FsIo, &target_path, target).map_err(prompt_error)
    })
    .await
}

/// Applies one explicit global prompt-document save through the full executor
/// transaction. The document's read hash blocks a stale draft from replacing
/// a newer external edit.
#[tauri::command]
pub async fn save_global_prompt_document(
    app: AppHandle,
    target: AppKind,
    content: String,
    expected_hash: String,
    confirm_write: bool,
) -> Result<GlobalPromptDocument, CommandError> {
    observe(RuntimeLogAction::GlobalPromptDocumentSaved, async move {
        require_write_confirmation(confirm_write, "保存全局提示词文档")?;
        let state = state(&app)?;
        blocking(move || {
            let target_path = state
                .global_prompt_target(target)
                .map_err(|error| CommandError::new("prompt-document-path-unavailable", error))?;
            let backup_dir = state.prompt_backup_dir();
            let outcome = write_global_prompt_document(
                &FsIo,
                &GlobalPromptDocumentRequest {
                    target: &target_path,
                    app: target,
                    content: &content,
                    backup_dir: &backup_dir,
                    expected_hash: &expected_hash,
                },
            )
            .map_err(prompt_error)?;
            Ok(outcome.document)
        })
        .await
    })
    .await
}
