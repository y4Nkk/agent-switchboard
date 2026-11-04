use crate::adapter::{AdapterError, OverlayEntry};
use crate::contracts::{ChangeKind, KeyChange, SwitchPlan, SwitchPreview};
use crate::ownership::CODEX_PROVIDER_ID;
use crate::redact::redact;
use crate::AppKind;

use crate::adapter::codex::document::{item_at, item_repr, parse, validate_path};
use crate::adapter::codex::overlay::overlay;

pub(crate) fn preview(
    current: &str,
    plan: &SwitchPlan,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    super::validate_projection(plan)?;
    super::render::render(current, plan)?;
    let mut preview = preview_entries(current, overlay(plan), backup_dir)?;
    let doc = parse(current)?;
    let previous = item_at(&doc, "model_provider")
        .and_then(item_repr)
        .unwrap_or_else(|| super::OFFICIAL_PROVIDER.to_string());
    let selected = if plan.profile.route_mode == crate::contracts::RouteMode::Custom {
        CODEX_PROVIDER_ID
    } else {
        super::OFFICIAL_PROVIDER
    };
    if previous != selected {
        preview.warnings.push(crate::contracts::LocalizedMessage::new(
            "warnings.codex.providerChanged",
            serde_json::json!({ "previous": previous, "selected": selected }),
            format!(
                "Codex 的 Provider 标识将从 {previous} 变为 {selected}。Codex 按 Provider 归属区分会话；此操作不会改写已有会话，需要在对应 Provider 下继续原会话。"
            ),
        ));
    }
    Ok(preview)
}

pub(crate) fn preview_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    let doc = parse(current)?;
    for (key, entry) in &entries {
        if !matches!(
            entry,
            OverlayEntry::Leave | OverlayEntry::RemoveTableIfEmpty
        ) {
            validate_path(&doc, key)?;
        }
    }
    let mut changes = Vec::new();
    for (key, entry) in entries {
        let existing = item_at(&doc, &key).and_then(|item| {
            if key == "approval_policy" {
                super::common::item_value(item, &key).ok().map(|value| value.display())
            } else { item_repr(item) }
        });
        match entry {
            OverlayEntry::Set(value) => {
                let after = value.display();
                if existing.as_deref() != Some(after.as_str()) {
                    changes.push(KeyChange {
                        key: key.clone(),
                        kind: ChangeKind::Set,
                        before: existing.map(|b| redact(&key, &b)),
                        after: Some(redact(&key, &after)),
                    });
                }
            }
            OverlayEntry::Leave => {}
            OverlayEntry::RemoveIfPresent => {
                if let Some(before) = existing {
                    changes.push(KeyChange {
                        key: key.clone(),
                        kind: ChangeKind::Remove,
                        before: Some(redact(&key, &before)),
                        after: None,
                    });
                }
            }
            OverlayEntry::RemoveTableIfEmpty => {}
        }
    }

    Ok(SwitchPreview {
        app: AppKind::Codex,
        target: AppKind::Codex.config_label().to_string(),
        changes,
        warnings: vec![],
        backup_dir: backup_dir.to_string(),
    })
}
