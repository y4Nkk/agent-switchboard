use super::Extra;

pub const EFFORT_LEVELS: &[&str] = &["low", "medium", "high", "xhigh"];
pub const COMPACT_WINDOW_MIN: u64 = 100_000;
pub const COMPACT_WINDOW_MAX: u64 = 1_000_000;

pub(super) fn validate(extra: &Extra) -> Result<(), String> {
    if let Some(effort) = extra.get("effortLevel") {
        validate_effort(effort, false, "effortLevel")?;
    }
    let Some(settings) = extra.get("modelSettings") else { return Ok(()); };
    let settings = settings.as_object().ok_or("modelSettings 必须是以模型标识为键的对象")?;
    for (model, preferences) in settings {
        if model.trim().is_empty() || model != model.trim() || model.chars().any(char::is_control) {
            return Err("modelSettings 的模型标识必须是非空文本且不能带首尾空白".into());
        }
        let preferences = preferences.as_object()
            .ok_or_else(|| format!("modelSettings.{model} 必须是对象"))?;
        if let Some(effort) = preferences.get("effortLevel") {
            validate_effort(effort, false, &format!("modelSettings.{model}.effortLevel"))?;
        }
        if let Some(cap) = preferences.get("maxEffortLevel") {
            validate_effort(cap, true, &format!("modelSettings.{model}.maxEffortLevel"))?;
        }
        if let Some(window) = preferences.get("autoCompactWindow") {
            if window.as_str() != Some("auto")
                && !window.as_f64().is_some_and(|value| value.fract() == 0.0
                    && value >= COMPACT_WINDOW_MIN as f64 && value <= COMPACT_WINDOW_MAX as f64) {
                return Err(format!("modelSettings.{model}.autoCompactWindow 必须是 auto 或 100000–1000000 的整数"));
            }
        }
    }
    Ok(())
}

fn validate_effort(value: &serde_json::Value, cap: bool, path: &str) -> Result<(), String> {
    let allowed = |level: &str| if cap {
        crate::ownership::choice_spec(crate::AppKind::Claude, "maxEffortLevel")
            .is_some_and(|spec| spec.options.iter().any(|option| option.value == level))
    } else { EFFORT_LEVELS.contains(&level) };
    if value.as_str().is_some_and(allowed) {
        Ok(())
    } else {
        Err(format!("{path} 仅支持 low、medium、high、xhigh{}", if cap { "、max" } else { "；max 不能保存为默认值" }))
    }
}
