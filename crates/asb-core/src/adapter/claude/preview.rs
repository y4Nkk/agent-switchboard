use crate::adapter::AdapterError;
use crate::contracts::{AppKind, SwitchPlan, SwitchPreview};
use super::document::{get, parse};
use super::overlay::ENV_MODEL_KEY;

pub(crate) fn preview(current: &str, plan: &SwitchPlan, backup_dir: &str) -> Result<SwitchPreview, AdapterError> {
    let mut warnings = Vec::new();
    let root = parse(current)?;
    if plan.profile.model.is_some() && get(&root, ENV_MODEL_KEY).is_some() {
        warnings.push(crate::contracts::LocalizedMessage::new(
            "warnings.claude.modelOverridden", serde_json::json!({}),
            "settings.json 的 env.ANTHROPIC_MODEL 会覆盖 model；切换将移除该键",
        ));
    }
    let rendered = super::render::render(current, plan)?;
    Ok(SwitchPreview {
        app: AppKind::Claude,
        target: AppKind::Claude.config_label().into(),
        changes: super::state::owned_diff(&rendered, current)?,
        warnings,
        backup_dir: backup_dir.into(),
    })
}
