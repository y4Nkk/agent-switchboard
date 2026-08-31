//! Pure mapping from CC Switch provider rows to import proposals.
//!
//! This module never touches the filesystem or SQLite: the caller reads
//! database rows and injects them. Output carries routing facts only — the
//! proposal type has no field capable of holding a secret, and secret-bearing
//! values (`ANTHROPIC_AUTH_TOKEN`, `auth.OPENAI_API_KEY`, OAuth tokens) are
//! inspected only for their presence to classify the routing mode.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::contracts::{AppKind, ClaudeModelSettings, ModelOptions, ProviderDraft};

/// One row of the CC Switch `providers` table, already read by the caller.
#[derive(Debug, Clone)]
pub struct CcSwitchRow {
    pub id: String,
    pub app_type: String,
    pub name: String,
    pub settings_config: String,
    /// Local metadata carried over on import; both columns are nullable.
    pub website_url: Option<String>,
    pub notes: Option<String>,
}

/// An importable provider mapped from CC Switch. `key` identifies the source
/// row (`"<app_type>:<id>"`) so the import command can re-resolve it against
/// a fresh scan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchProposal {
    pub key: String,
    pub app: AppKind,
    pub draft: ProviderDraft,
    /// Field names that exist in the source but are not carried over. Never
    /// contains secret values, only key names.
    pub warnings: Vec<String>,
}

/// A provider that cannot be imported, with a user-facing reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchSkip {
    pub key: String,
    pub app_type: String,
    pub name: String,
    pub reason: String,
}

/// Keys of a Codex managed provider table that a profile can represent.
/// Anything beyond this set would be lost on activation, so such providers
/// are skipped instead of silently narrowed (same rule as `discovery`).
const CODEX_TABLE_KEYS: [&str; 4] = ["name", "base_url", "env_key", "wire_api"];

/// Claude env keys a profile can represent. `ANTHROPIC_AUTH_TOKEN` is listed
/// separately: it is recognized but never imported.
const CLAUDE_ENV_KEYS: [&str; 6] = [
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_AUTH_TOKEN",
];

/// Maps one CC Switch row into a proposal or a skip.
pub fn map_row(row: &CcSwitchRow) -> Result<CcSwitchProposal, CcSwitchSkip> {
    let key = format!("{}:{}", row.app_type, row.id);
    match row.app_type.as_str() {
        "codex" => map_codex(key.clone(), row).map_err(|reason| skip_with(key, row, reason)),
        "claude" => map_claude(key.clone(), row).map_err(|reason| skip_with(key, row, reason)),
        other => Err(skip_with(
            key,
            row,
            format!("客户端 {other} 超出本应用支持范围"),
        )),
    }
}

fn skip_with(key: String, row: &CcSwitchRow, reason: String) -> CcSwitchSkip {
    CcSwitchSkip {
        key,
        app_type: row.app_type.clone(),
        name: row.name.clone(),
        reason,
    }
}

fn map_claude(key: String, row: &CcSwitchRow) -> Result<CcSwitchProposal, String> {
    let config: Value =
        serde_json::from_str(&row.settings_config).map_err(|e| format!("配置无法解析: {e}"))?;
    let env = config.get("env").cloned().unwrap_or(Value::Null);
    let env = env.as_object().cloned().unwrap_or_default();

    let text = |name: &str| {
        env.get(name)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
    };
    let base_url = text("ANTHROPIC_BASE_URL");
    let (model, primary_one_m) =
        crate::claude_model::parse_optional_model(text("ANTHROPIC_MODEL"), "主模型", true)?;
    let (haiku, _) = crate::claude_model::parse_optional_model(
        text("ANTHROPIC_DEFAULT_HAIKU_MODEL"),
        "Haiku 档",
        false,
    )?;
    let (sonnet, sonnet_one_m) = crate::claude_model::parse_optional_model(
        text("ANTHROPIC_DEFAULT_SONNET_MODEL"),
        "Sonnet 档",
        true,
    )?;
    let (opus, opus_one_m) = crate::claude_model::parse_optional_model(
        text("ANTHROPIC_DEFAULT_OPUS_MODEL"),
        "Opus 档",
        true,
    )?;

    // Names only; values of unrecognized keys never enter the output.
    let mut warnings = Vec::new();
    if env.contains_key("ANTHROPIC_AUTH_TOKEN") {
        warnings.push("凭据 env.ANTHROPIC_AUTH_TOKEN 不导入".to_string());
    }
    for name in env.keys() {
        if !CLAUDE_ENV_KEYS.contains(&name.as_str()) {
            warnings.push(format!("未导入: env.{name}"));
        }
    }
    for name in config.as_object().map(|o| o.keys()).into_iter().flatten() {
        if name != "env" {
            warnings.push(format!("未导入: {name}"));
        }
    }

    // A settings.json without a custom endpoint means the client is on its
    // official login, which this app no longer manages as a profile.
    let base_url = match base_url {
        Some(url) => url,
        None => {
            return Err("官方登录配置不导入；客户端自身的登录即为官方路由".to_string());
        }
    };
    let api_key = text("ANTHROPIC_AUTH_TOKEN")
        .or_else(|| text("ANTHROPIC_API_KEY"))
        .ok_or_else(|| "缺少 ANTHROPIC_AUTH_TOKEN 或 ANTHROPIC_API_KEY".to_string())?;
    let model_options = (primary_one_m
        || haiku.is_some()
        || sonnet.is_some()
        || sonnet_one_m
        || opus.is_some()
        || opus_one_m)
        .then(|| {
            ModelOptions::Claude(ClaudeModelSettings {
                primary_one_m,
                haiku_model: haiku,
                sonnet_model: sonnet,
                sonnet_one_m,
                opus_model: opus,
                opus_one_m,
                available_models: None,
            })
        });
    Ok(CcSwitchProposal {
        key,
        app: AppKind::Claude,
        draft: ProviderDraft {
            app: AppKind::Claude,
            name: row.name.clone(),
            model,
            base_url: Some(base_url.to_string()),
            api_key: api_key.to_string(),
            model_options,
            notes: row.notes.clone(),
            website_url: row.website_url.clone(),
        },
        warnings,
    })
}

fn map_codex(key: String, row: &CcSwitchRow) -> Result<CcSwitchProposal, String> {
    let config: Value =
        serde_json::from_str(&row.settings_config).map_err(|e| format!("配置无法解析: {e}"))?;
    let auth = config.get("auth").cloned().unwrap_or(Value::Null);

    // Presence-only checks classify official rows; values are never read.
    // Official login is no longer a profile kind, so such rows are skipped.
    let oauth = auth.get("tokens").is_some_and(Value::is_object);
    let api_key = auth.get("OPENAI_API_KEY");
    let official = oauth || api_key.is_none_or(Value::is_null);

    if official {
        return Err("官方登录配置不导入；客户端自身的登录即为官方路由".to_string());
    }

    let toml_text = config
        .get("config")
        .and_then(Value::as_str)
        .ok_or_else(|| "缺少供应商配置表".to_string())?;
    let document: toml_edit::DocumentMut = toml_text
        .parse()
        .map_err(|_| "供应商 TOML 无法解析".to_string())?;
    let providers = document
        .get("model_providers")
        .and_then(|item| item.as_table())
        .ok_or_else(|| "缺少 model_providers 表".to_string())?;
    if providers.len() != 1 {
        return Err(format!(
            "model_providers 含 {} 个表,无法确定导入对象",
            providers.len()
        ));
    }
    let table = providers
        .iter()
        .next()
        .map(|(_, item)| item.as_table())
        .flatten()
        .ok_or("model_providers 表项不是表")?;
    for entry in table.iter() {
        if !CODEX_TABLE_KEYS.contains(&entry.0) {
            return Err(format!("供应商表包含无法表示的键 {},激活时会丢失", entry.0));
        }
    }
    let field = |name: &str| {
        table
            .get(name)
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let base_url = field("base_url").ok_or_else(|| "缺少 base_url".to_string())?;
    let wire_api = field("wire_api").unwrap_or_else(|| "responses".to_string());
    if wire_api != "responses" {
        return Err(format!("wire_api = {wire_api} 不受支持"));
    }
    let api_key = auth
        .get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "缺少 OPENAI_API_KEY".to_string())?;
    Ok(CcSwitchProposal {
        key,
        app: AppKind::Codex,
        draft: ProviderDraft {
            app: AppKind::Codex,
            name: row.name.clone(),
            model: None,
            base_url: Some(base_url),
            api_key: api_key.to_string(),
            model_options: None,
            notes: row.notes.clone(),
            website_url: row.website_url.clone(),
        },
        warnings: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic shapes only. Secret-looking slots hold an obvious placeholder
    // that is also asserted to never leak into proposals.
    const TOKEN: &str = "<placeholder>";

    fn row(app_type: &str, id: &str, name: &str, settings_config: &str) -> CcSwitchRow {
        CcSwitchRow {
            id: id.to_string(),
            app_type: app_type.to_string(),
            name: name.to_string(),
            settings_config: settings_config.to_string(),
            website_url: None,
            notes: None,
        }
    }

    fn claude_custom() -> String {
        format!(
            r#"{{
                "env": {{
                    "ANTHROPIC_BASE_URL": "https://relay.internal",
                    "ANTHROPIC_AUTH_TOKEN": "{TOKEN}",
                    "ANTHROPIC_MODEL": "claude-x",
                    "ANTHROPIC_DEFAULT_OPUS_MODEL": "opus-x",
                    "ANTHROPIC_DEFAULT_SONNET_MODEL": "sonnet-x",
                    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "haiku-x",
                    "CLAUDE_CODE_SUBAGENT_MODEL": "sub-x"
                }},
                "permissions": {{"defaultMode": "auto"}}
            }}"#
        )
    }

    #[test]
    fn claude_custom_imports_its_api_key_and_routing() {
        let outcome = map_row(&row("claude", "id-1", "中继 A", &claude_custom())).unwrap();
        assert_eq!(outcome.key, "claude:id-1");
        assert_eq!(outcome.draft.api_key, TOKEN);
        assert_eq!(
            outcome.draft.base_url.as_deref(),
            Some("https://relay.internal")
        );
        assert!(outcome
            .warnings
            .contains(&"未导入: permissions".to_string()));
    }

    #[test]
    fn claude_import_decodes_lowercase_one_m_model_markers_into_semantic_state() {
        let config = format!(
            r#"{{"env":{{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"{TOKEN}","ANTHROPIC_MODEL":"claude-opus-4-1[1m]","ANTHROPIC_DEFAULT_SONNET_MODEL":"claude-sonnet-4-6[1m]","ANTHROPIC_DEFAULT_OPUS_MODEL":"claude-opus-4-1[1m]"}}}}"#
        );
        let outcome = map_row(&row("claude", "id-1m", "百万上下文", &config)).unwrap();

        assert_eq!(outcome.draft.model.as_deref(), Some("claude-opus-4-1"));
        let Some(ModelOptions::Claude(settings)) = outcome.draft.model_options.as_ref() else {
            panic!("Claude model settings should be imported");
        };
        assert!(settings.primary_one_m);
        assert_eq!(settings.sonnet_model.as_deref(), Some("claude-sonnet-4-6"));
        assert!(settings.sonnet_one_m);
        assert_eq!(settings.opus_model.as_deref(), Some("claude-opus-4-1"));
        assert!(settings.opus_one_m);
        assert!(outcome.draft.validate().is_ok());
    }

    #[test]
    fn claude_import_rejects_uppercase_one_m_model_markers() {
        let config = format!(
            r#"{{"env":{{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"{TOKEN}","ANTHROPIC_MODEL":"claude-opus-4-1[1M]"}}}}"#
        );
        let skipped = map_row(&row("claude", "id-1m", "百万上下文", &config)).unwrap_err();

        assert!(skipped.reason.contains("1M 标记无效"));
    }

    #[test]
    fn claude_official_rows_are_skipped() {
        let config = format!(r#"{{"env": {{"ANTHROPIC_AUTH_TOKEN": "{TOKEN}"}}}}"#);
        let outcome = map_row(&row("claude", "id-2", "官方", &config)).unwrap_err();
        assert!(outcome.reason.contains("官方登录"));
        assert!(!format!("{outcome:?}").contains(TOKEN));
    }

    #[test]
    fn claude_malformed_json_skips() {
        let outcome = map_row(&row("claude", "id-3", "坏档", "{oops")).unwrap_err();
        assert!(outcome.reason.contains("无法解析"));
    }

    #[test]
    fn codex_oauth_rows_are_skipped() {
        let config = format!(
            r#"{{"auth": {{"OPENAI_API_KEY": null, "tokens": {{"refresh_token": "{TOKEN}"}}}}, "config": ""}}"#
        );
        let outcome = map_row(&row("codex", "id-4", "订阅", &config)).unwrap_err();
        assert!(outcome.reason.contains("官方登录"));
        assert!(!format!("{outcome:?}").contains(TOKEN));
    }

    #[test]
    fn codex_custom_imports_api_key_from_auth() {
        let config = r#"{"auth": {"OPENAI_API_KEY": "<placeholder>"},
            "config": "[model_providers.relay]\nname = \"Relay\"\nbase_url = \"https://api.relay.internal\"\nenv_key = \"RELAY_KEY\"\nwire_api = \"responses\"\n"}"#;
        let outcome = map_row(&row("codex", "id-5", "中继 B", config)).unwrap();
        assert_eq!(outcome.draft.api_key, TOKEN);
        assert_eq!(
            outcome.draft.base_url.as_deref(),
            Some("https://api.relay.internal")
        );
    }

    #[test]
    fn codex_custom_without_wire_api_defaults_to_responses() {
        let config = r#"{"auth": {"OPENAI_API_KEY": "<placeholder>"},
            "config": "[model_providers.r]\nbase_url = \"https://x.internal\"\n"}"#;
        let outcome = map_row(&row("codex", "id-6", "缺省", config)).unwrap();
        assert_eq!(outcome.draft.api_key, TOKEN);
        assert_eq!(
            outcome.draft.base_url.as_deref(),
            Some("https://x.internal")
        );
    }

    #[test]
    fn codex_unsupported_wire_api_skips() {
        let config = r#"{"auth": {"OPENAI_API_KEY": "<placeholder>"},
            "config": "[model_providers.r]\nbase_url = \"https://x.internal\"\nwire_api = \"chat\"\n"}"#;
        let outcome = map_row(&row("codex", "id-7", "聊天式", config)).unwrap_err();
        assert!(outcome.reason.contains("wire_api"));
    }

    #[test]
    fn codex_extra_table_key_skips_because_activation_would_lose_it() {
        let config = r#"{"auth": {"OPENAI_API_KEY": "<placeholder>"},
            "config": "[model_providers.r]\nbase_url = \"https://x.internal\"\nhttp_headers = {\"X-Up\" = \"1\"}\n"}"#;
        let outcome = map_row(&row("codex", "id-8", "带头", config)).unwrap_err();
        assert!(outcome.reason.contains("http_headers"));
    }

    #[test]
    fn codex_multiple_provider_tables_skips() {
        let config = r#"{"auth": {"OPENAI_API_KEY": "<placeholder>"},
            "config": "[model_providers.a]\nbase_url = \"https://a.internal\"\n[model_providers.b]\nbase_url = \"https://b.internal\"\n"}"#;
        let outcome = map_row(&row("codex", "id-9", "多表", config)).unwrap_err();
        assert!(outcome.reason.contains("2 个表"));
    }

    #[test]
    fn codex_broken_toml_skips() {
        let config = r#"{"auth": {"OPENAI_API_KEY": "<placeholder>"}, "config": "[oops"}"#;
        let outcome = map_row(&row("codex", "id-10", "坏档", config)).unwrap_err();
        assert!(outcome.reason.contains("TOML"));
    }

    #[test]
    fn codex_custom_without_base_url_skips() {
        let config = r#"{"auth": {"OPENAI_API_KEY": "<placeholder>"}, "config": "[model_providers.r]\nname = \"R\"\n"}"#;
        let outcome = map_row(&row("codex", "id-11", "无地址", config)).unwrap_err();
        assert!(outcome.reason.contains("base_url"));
    }

    #[test]
    fn unsupported_client_skips() {
        let outcome = map_row(&row("gemini", "id-12", "双子", "{}")).unwrap_err();
        assert!(outcome.reason.contains("gemini"));
        let outcome = map_row(&row("claude-desktop", "id-13", "桌面", "{}")).unwrap_err();
        assert!(outcome.reason.contains("claude-desktop"));
    }
}
