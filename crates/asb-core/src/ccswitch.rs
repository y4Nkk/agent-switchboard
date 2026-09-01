//! Pure mapping from CC Switch provider rows to import proposals.
//!
//! This module never touches the filesystem or SQLite: the caller reads
//! database rows and injects them. A custom-provider credential becomes the
//! draft API key for profile persistence; diagnostics and warnings carry only
//! field names. Official rows become a credential-free official route and
//! never expose or copy the source login material.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

use crate::contracts::{
    AppKind, ClaudeModelSettings, ModelOptions, ProviderDraft, RouteMode, UsageQuery,
};

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
    /// CC Switch provider metadata. Only the optional usage script is
    /// considered; its source never crosses the scan-response boundary.
    pub meta: Option<String>,
}

/// An importable provider mapped from CC Switch. `key` identifies the source
/// row (`"<app_type>:<id>"`) so the import command can re-resolve it against
/// a fresh scan.
#[derive(Debug, Clone, PartialEq)]
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

/// Claude env keys a profile can represent. Either credential alias supplies
/// the profile API key; the configured `ANTHROPIC_AUTH_TOKEN` is rendered on
/// activation.
const CLAUDE_ENV_KEYS: [&str; 7] = [
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_API_KEY",
];

/// Usage-script keys which represent an independent endpoint or credential.
/// The profile's own `baseUrl` and `apiKey` are the only supported query
/// inputs, so importing one of these would silently change the query's owner.
const USAGE_SCRIPT_INPUT_OVERRIDES: [&str; 7] = [
    "apiKey",
    "baseUrl",
    "accessToken",
    "userId",
    "accessKeyId",
    "secretAccessKey",
    "teamId",
];

const USAGE_SCRIPT_REPRESENTED_KEYS: [&str; 5] =
    ["enabled", "language", "code", "templateType", "timeout"];

/// Converts one enabled CC Switch JavaScript source into the application's
/// native script contract. This is an import-time compilation step: durable
/// profiles never retain a CC Switch query kind or a runtime compatibility
/// branch.
fn compile_usage_script(source: &str) -> String {
    [
        r#"(() => {
  const cc = ("#,
        source,
        r#");
  const object = (value) =>
    value !== null && typeof value === "object" && !Array.isArray(value);
  const substitute = (value, input) =>
    String(value)
      .replaceAll("{{baseUrl}}", String(input.baseUrl || "").replace(/\/+$/, ""))
      .replaceAll("{{apiKey}}", String(input.apiKey || ""));
  const number = (value) => {
    if (typeof value === "number" && Number.isFinite(value)) return value;
    if (typeof value === "string" && value.trim() !== "") {
      const parsed = Number(value);
      if (Number.isFinite(parsed)) return parsed;
    }
    return null;
  };
  const reading = (value) => {
    if (!object(value) || value.isValid === false) {
      throw new TypeError("invalid imported usage result");
    }
    const result = {
      remaining: number(value.remaining),
      used: number(value.used),
      total: number(value.total),
      unit: typeof value.unit === "string" ? value.unit : null,
    };
    if (typeof value.planName === "string" && value.planName.trim() !== "") {
      result.planName = value.planName;
    }
    return result;
  };
  return {
    request(input) {
      if (!object(cc) || !object(cc.request) || typeof cc.extractor !== "function") {
        throw new TypeError("invalid imported usage script");
      }
      const sourceRequest = cc.request;
      const headers = {};
      if (sourceRequest.headers !== undefined) {
        if (!object(sourceRequest.headers)) {
          throw new TypeError("invalid imported usage request");
        }
        for (const name in sourceRequest.headers) {
          if (Object.prototype.hasOwnProperty.call(sourceRequest.headers, name)) {
            headers[name] = substitute(sourceRequest.headers[name], input);
          }
        }
      }
      const request = {
        url: substitute(sourceRequest.url, input),
        method: String(sourceRequest.method || "GET").toUpperCase(),
        headers,
      };
      if (sourceRequest.body !== undefined && sourceRequest.body !== null) {
        const body =
          typeof sourceRequest.body === "string"
            ? sourceRequest.body
            : JSON.stringify(sourceRequest.body);
        request.body = substitute(body, input);
      }
      return request;
    },
    extract(input) {
      if (input.status < 200 || input.status >= 300) {
        throw new TypeError("imported usage request was not successful");
      }
      const extracted = cc.extractor(input.body);
      return Array.isArray(extracted) ? extracted.map(reading) : reading(extracted);
    },
  };
})()"#,
    ]
    .concat()
}

fn contains_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        _ => true,
    }
}

fn has_unsupported_placeholder(source: &str) -> bool {
    let mut remaining = source;
    while let Some(start) = remaining.find("{{") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        let name = after_start[..end].trim();
        if name != "apiKey" && name != "baseUrl" {
            return true;
        }
        remaining = &after_start[end + 2..];
    }
    false
}

/// Resolves the only query-script source supported by CC Switch import.
/// Warnings name source fields only; values and script text never appear in
/// diagnostics or scan data.
fn map_usage_query(meta: Option<&str>, warnings: &mut Vec<String>) -> Option<UsageQuery> {
    let Some(text) = meta.filter(|text| !text.trim().is_empty()) else {
        return None;
    };
    let meta: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(_) => {
            warnings.push("未导入: meta.usage_script（元数据无法解析）".to_string());
            return None;
        }
    };
    let Some(script) = meta.get("usage_script") else {
        return None;
    };
    let Some(script) = script.as_object() else {
        warnings.push("未导入: meta.usage_script（格式无效）".to_string());
        return None;
    };
    if script.get("enabled").and_then(Value::as_bool) != Some(true) {
        warnings.push("未导入: meta.usage_script（脚本已禁用）".to_string());
        return None;
    }
    if !script
        .get("language")
        .and_then(Value::as_str)
        .is_some_and(|language| language.eq_ignore_ascii_case("javascript"))
    {
        warnings.push("未导入: meta.usage_script（仅支持 JavaScript 脚本）".to_string());
        return None;
    }
    let source = match script
        .get("code")
        .and_then(Value::as_str)
        .filter(|source| !source.trim().is_empty())
    {
        Some(source) => source,
        None if script.get("templateType").and_then(Value::as_str).is_some() => {
            warnings.push(
                "未导入: meta.usage_script.templateType（内建模板没有可导入的源码）".to_string(),
            );
            return None;
        }
        None => {
            warnings.push("未导入: meta.usage_script.code（没有可导入的源码）".to_string());
            return None;
        }
    };
    for key in USAGE_SCRIPT_INPUT_OVERRIDES {
        if script.get(key).is_some_and(contains_value) {
            warnings.push(format!("未导入: meta.usage_script.{key}"));
            return None;
        }
    }
    if has_unsupported_placeholder(source) {
        warnings.push("未导入: meta.usage_script.code（包含无对应输入的占位符）".to_string());
        return None;
    }

    let mut unsupported = BTreeSet::new();
    for key in script.keys() {
        if !USAGE_SCRIPT_REPRESENTED_KEYS.contains(&key.as_str())
            && !USAGE_SCRIPT_INPUT_OVERRIDES.contains(&key.as_str())
        {
            unsupported.insert(key);
        }
    }
    for key in unsupported {
        warnings.push(format!("未导入: meta.usage_script.{key}"));
    }
    if script.get("timeout").is_some_and(contains_value) {
        warnings.push("未导入: meta.usage_script.timeout".to_string());
    }
    Some(UsageQuery::Script {
        source: compile_usage_script(source),
    })
}

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
        crate::claude_model::parse_ccswitch_model(text("ANTHROPIC_MODEL"), "主模型", true)?;
    let (haiku, _) = crate::claude_model::parse_ccswitch_model(
        text("ANTHROPIC_DEFAULT_HAIKU_MODEL"),
        "Haiku 档",
        false,
    )?;
    let (sonnet, sonnet_one_m) = crate::claude_model::parse_ccswitch_model(
        text("ANTHROPIC_DEFAULT_SONNET_MODEL"),
        "Sonnet 档",
        true,
    )?;
    let (opus, opus_one_m) = crate::claude_model::parse_ccswitch_model(
        text("ANTHROPIC_DEFAULT_OPUS_MODEL"),
        "Opus 档",
        true,
    )?;

    // Names only; values of unrecognized keys never enter the output.
    let mut warnings = Vec::new();
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

    // A settings.json without a custom endpoint is an official route. Its
    // credentials stay client-owned; only the selectable route is imported.
    let base_url = match base_url {
        Some(url) => url,
        None => {
            return Ok(CcSwitchProposal {
                key,
                app: AppKind::Claude,
                draft: official_draft(AppKind::Claude),
                warnings,
            });
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
    let usage_query = map_usage_query(row.meta.as_deref(), &mut warnings);
    Ok(CcSwitchProposal {
        key,
        app: AppKind::Claude,
        draft: ProviderDraft {
            app: AppKind::Claude,
            route_mode: RouteMode::Custom,
            name: row.name.clone(),
            model,
            base_url: Some(base_url.to_string()),
            api_key: api_key.to_string(),
            model_options,
            notes: row.notes.clone(),
            website_url: row.website_url.clone(),
            usage_query,
        },
        warnings,
    })
}

fn map_codex(key: String, row: &CcSwitchRow) -> Result<CcSwitchProposal, String> {
    let config: Value =
        serde_json::from_str(&row.settings_config).map_err(|e| format!("配置无法解析: {e}"))?;
    let auth = config.get("auth").cloned().unwrap_or(Value::Null);

    // Presence-only checks classify official rows; values are never read.
    let oauth = auth.get("tokens").is_some_and(Value::is_object);
    let api_key = auth.get("OPENAI_API_KEY");
    let official = oauth || api_key.is_none_or(Value::is_null);

    if official {
        return Ok(CcSwitchProposal {
            key,
            app: AppKind::Codex,
            draft: official_draft(AppKind::Codex),
            warnings: vec![],
        });
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
    let mut warnings = Vec::new();
    let usage_query = map_usage_query(row.meta.as_deref(), &mut warnings);
    Ok(CcSwitchProposal {
        key,
        app: AppKind::Codex,
        draft: ProviderDraft {
            app: AppKind::Codex,
            route_mode: RouteMode::Custom,
            name: row.name.clone(),
            model: None,
            base_url: Some(base_url),
            api_key: api_key.to_string(),
            model_options: None,
            notes: row.notes.clone(),
            website_url: row.website_url.clone(),
            usage_query,
        },
        warnings,
    })
}

fn official_draft(app: AppKind) -> ProviderDraft {
    ProviderDraft {
        app,
        route_mode: RouteMode::Official,
        name: match app {
            AppKind::Codex => "Codex 官方登录",
            AppKind::Claude => "Claude 官方登录",
        }
        .to_string(),
        model: None,
        base_url: None,
        api_key: String::new(),
        model_options: None,
        notes: None,
        website_url: None,
        usage_query: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mapping keeps credentials in the backend-only proposal so import can
    // persist them; the scan-response boundary is tested in ccswitch_source.
    const TOKEN: &str = "<placeholder>";

    fn row(app_type: &str, id: &str, name: &str, settings_config: &str) -> CcSwitchRow {
        CcSwitchRow {
            id: id.to_string(),
            app_type: app_type.to_string(),
            name: name.to_string(),
            settings_config: settings_config.to_string(),
            website_url: None,
            notes: None,
            meta: None,
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

    fn enabled_script_meta(source: &str) -> String {
        serde_json::json!({
            "usage_script": {
                "enabled": true,
                "language": "javascript",
                "code": source
            }
        })
        .to_string()
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
        assert!(!outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("ANTHROPIC_AUTH_TOKEN")));
    }

    #[test]
    fn claude_api_key_alias_imports_without_an_unimported_warning() {
        let config = format!(
            r#"{{"env":{{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_API_KEY":"{TOKEN}"}}}}"#
        );
        let outcome = map_row(&row("claude", "id-api-key", "兼容中继", &config)).unwrap();

        assert_eq!(outcome.draft.api_key, TOKEN);
        assert!(!outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("ANTHROPIC_API_KEY")));
    }

    #[test]
    fn custom_usage_script_becomes_the_native_script_contract() {
        let mut source = row("claude", "usage-1", "带用量", &claude_custom());
        source.meta = Some(enabled_script_meta(
            r#"({
                request: {
                    url: "{{baseUrl}}/usage",
                    method: "GET",
                    headers: { Authorization: "Bearer {{apiKey}}" }
                },
                extractor: function(response) {
                    return [
                        { isValid: true, planName: "主套餐", remaining: response.main, unit: "USD" },
                        { isValid: true, planName: "副套餐", remaining: response.extra, unit: "USD" }
                    ];
                }
            })"#,
        ));

        let outcome = map_row(&source).expect("custom provider should map");
        let Some(UsageQuery::Script { source }) = outcome.draft.usage_query else {
            panic!("usage script should be imported");
        };
        assert!(source.contains("const cc = ("));
        assert!(source.contains("cc.extractor(input.body)"));
        assert!(source.contains("planName"));
        assert!(!outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("usage_script")));
    }

    #[test]
    fn disabled_or_template_usage_scripts_are_not_activated_on_import() {
        let mut disabled = row("claude", "usage-2", "禁用脚本", &claude_custom());
        disabled.meta = Some(
            serde_json::json!({
                "usage_script": {
                    "enabled": false,
                    "language": "javascript",
                    "code": "({ request: {}, extractor: function() {} })"
                }
            })
            .to_string(),
        );
        let disabled = map_row(&disabled).expect("provider should still map");
        assert!(disabled.draft.usage_query.is_none());
        assert!(disabled
            .warnings
            .iter()
            .any(|warning| warning.contains("脚本已禁用")));

        let mut template = row("claude", "usage-3", "内建模板", &claude_custom());
        template.meta = Some(
            serde_json::json!({
                "usage_script": {
                    "enabled": true,
                    "language": "javascript",
                    "code": "",
                    "templateType": "balance"
                }
            })
            .to_string(),
        );
        let template = map_row(&template).expect("provider should still map");
        assert!(template.draft.usage_query.is_none());
        assert!(template
            .warnings
            .iter()
            .any(|warning| warning.contains("templateType")));
    }

    #[test]
    fn independent_usage_script_inputs_are_not_mixed_into_the_provider() {
        let mut source = row("claude", "usage-4", "独立凭据", &claude_custom());
        source.meta = Some(
            serde_json::json!({
                "usage_script": {
                    "enabled": true,
                    "language": "javascript",
                    "code": "({ request: {}, extractor: function() {} })",
                    "accessToken": "<placeholder>"
                }
            })
            .to_string(),
        );

        let outcome = map_row(&source).expect("provider should still map");
        assert!(outcome.draft.usage_query.is_none());
        assert!(outcome
            .warnings
            .contains(&"未导入: meta.usage_script.accessToken".to_string()));
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
    fn claude_import_normalizes_ccswitch_uppercase_one_m_model_markers() {
        let config = format!(
            r#"{{"env":{{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"{TOKEN}","ANTHROPIC_MODEL":"claude-opus-4-1[1M]"}}}}"#
        );
        let outcome = map_row(&row("claude", "id-1m", "百万上下文", &config)).unwrap();
        assert_eq!(outcome.draft.model.as_deref(), Some("claude-opus-4-1"));
        let Some(ModelOptions::Claude(settings)) = outcome.draft.model_options else {
            panic!("Claude model settings should be imported");
        };
        assert!(settings.primary_one_m);
    }

    #[test]
    fn claude_official_rows_become_credential_free_routes() {
        let config = format!(r#"{{"env": {{"ANTHROPIC_AUTH_TOKEN": "{TOKEN}"}}}}"#);
        let outcome = map_row(&row("claude", "id-2", "官方", &config)).unwrap();
        assert_eq!(outcome.draft.route_mode, RouteMode::Official);
        assert!(outcome.draft.api_key.is_empty());
        assert!(!format!("{outcome:?}").contains(TOKEN));
    }

    #[test]
    fn claude_malformed_json_skips() {
        let outcome = map_row(&row("claude", "id-3", "坏档", "{oops")).unwrap_err();
        assert!(outcome.reason.contains("无法解析"));
    }

    #[test]
    fn codex_oauth_rows_become_credential_free_routes() {
        let config = format!(
            r#"{{"auth": {{"OPENAI_API_KEY": null, "tokens": {{"refresh_token": "{TOKEN}"}}}}, "config": ""}}"#
        );
        let outcome = map_row(&row("codex", "id-4", "订阅", &config)).unwrap();
        assert_eq!(outcome.draft.route_mode, RouteMode::Official);
        assert!(outcome.draft.api_key.is_empty());
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
