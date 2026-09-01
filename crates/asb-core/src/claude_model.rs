//! Claude Code's wire-only 1M model suffix.
//!
//! Provider profiles store a model identifier and an explicit 1M flag. Only
//! this codec turns that semantic state into the Claude Code configuration
//! spelling or parses it from an external configuration during import.

pub(crate) const ONE_M_CONTEXT_SUFFIX: &str = "[1m]";

pub(crate) fn render_model(model: &str, one_m: bool) -> String {
    if one_m {
        format!("{model}{ONE_M_CONTEXT_SUFFIX}")
    } else {
        model.to_string()
    }
}

pub(crate) fn parse_model(
    raw: &str,
    field: &str,
    supports_one_m: bool,
) -> Result<(String, bool), String> {
    if let Some(model) = raw.strip_suffix(ONE_M_CONTEXT_SUFFIX) {
        if model.is_empty() || model != model.trim() || contains_one_m_marker(model) {
            return Err(invalid_marker(field));
        }
        if !supports_one_m {
            return Err(format!("{field} 不支持 1M 上下文"));
        }
        return Ok((model.to_string(), true));
    }
    if contains_one_m_marker(raw) {
        return Err(invalid_marker(field));
    }
    Ok((raw.to_string(), false))
}

/// Decodes one optional model field from an external Claude configuration.
/// Absence means the mapping is absent; it never becomes an empty model id.
pub(crate) fn parse_optional_model(
    raw: Option<&str>,
    field: &str,
    supports_one_m: bool,
) -> Result<(Option<String>, bool), String> {
    match raw {
        Some(raw) => {
            parse_model(raw, field, supports_one_m).map(|(model, one_m)| (Some(model), one_m))
        }
        None => Ok((None, false)),
    }
}

/// Decodes a model field stored by CC Switch.
///
/// Older CC Switch rows use an uppercase `[1M]` suffix for its 1M checkbox.
/// That spelling never becomes part of an Agent Switchboard profile or a
/// Claude Code write: it is normalized here to the same semantic flag as the
/// canonical Claude Code `[1m]` suffix.
pub(crate) fn parse_ccswitch_model(
    raw: Option<&str>,
    field: &str,
    supports_one_m: bool,
) -> Result<(Option<String>, bool), String> {
    let Some(raw) = raw else {
        return Ok((None, false));
    };
    if let Some(model) = raw.strip_suffix("[1M]") {
        if model.is_empty() || model != model.trim() || contains_one_m_marker(model) {
            return Err(invalid_marker(field));
        }
        if !supports_one_m {
            return Err(format!("{field} 不支持 1M 上下文"));
        }
        return Ok((Some(model.to_string()), true));
    }
    parse_optional_model(Some(raw), field, supports_one_m)
}

pub(crate) fn contains_one_m_marker(value: &str) -> bool {
    value
        .as_bytes()
        .windows(3)
        .any(|marker| marker.eq_ignore_ascii_case(b"[1m"))
}

fn invalid_marker(field: &str) -> String {
    format!("{field} 的 1M 标记无效；仅模型名末尾的小写 [1m] 可从 Claude Code 配置导入")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_keeps_profile_state_marker_free() {
        assert_eq!(render_model("claude-opus-4-7", true), "claude-opus-4-7[1m]");
        assert_eq!(
            parse_model("claude-opus-4-7[1m]", "主模型", true),
            Ok(("claude-opus-4-7".to_string(), true))
        );
    }

    #[test]
    fn codec_rejects_noncanonical_or_unsupported_wire_markers() {
        assert!(parse_model("claude-opus-4-7[1M]", "主模型", true).is_err());
        assert!(parse_model("claude-haiku-4[1m]", "Haiku 档", false).is_err());
        assert!(parse_model("claude-opus-4-7[1m][1m]", "主模型", true).is_err());
        assert!(parse_model("claude-opus-4-7 [1m]", "主模型", true).is_err());
        assert!(parse_model("claude-opus-4-7[1m ]", "主模型", true).is_err());
    }

    #[test]
    fn ccswitch_codec_normalizes_its_legacy_uppercase_marker() {
        assert_eq!(
            parse_ccswitch_model(Some("claude-opus-4-7[1M]"), "主模型", true),
            Ok((Some("claude-opus-4-7".to_string()), true))
        );
        assert!(parse_ccswitch_model(Some("claude-opus-4-7 [1M]"), "主模型", true).is_err());
        assert!(parse_ccswitch_model(Some("claude-opus-4-7[1M]"), "Haiku 档", false).is_err());
    }
}
