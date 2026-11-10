use asb_core::contracts::{AppKind, SettingsValues};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientExportStatus {
    pub app: AppKind,
    pub configured: bool,
    pub error: Option<String>,
}

pub(crate) fn read(state: &crate::local_state::LocalState, app: AppKind) -> Result<(String, bool), String> {
    let path = state.target(app)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok((text, true)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound =>
            Ok((if app == AppKind::Claude { "{}".into() } else { String::new() }, false)),
        Err(error) => Err(format!("无法读取客户端导出配置：{error}")),
    }
}

pub(crate) fn inspect(state: &crate::local_state::LocalState, app: AppKind) -> ClientExportStatus {
    match read(state, app).and_then(|(text, _)| configured(app, &text)) {
        Ok(configured) => ClientExportStatus { app, configured, error: None },
        Err(error) => ClientExportStatus { app, configured: false, error: Some(error) },
    }
}

pub(crate) fn configured(app: AppKind, current: &str) -> Result<bool, String> {
    match app {
        AppKind::Codex => {
            let document = current.parse::<toml_edit::DocumentMut>().map_err(|_| "Codex 配置不是有效 TOML")?;
            Ok(document.get("otel").and_then(|otel| otel.get("exporter"))
                .and_then(|exporter| exporter.get("otlp-http"))
                .is_some_and(|http| http.get("endpoint").and_then(toml_edit::Item::as_str)
                    == Some(codex_endpoint().as_str())
                    && http.get("protocol").and_then(toml_edit::Item::as_str) == Some("json")))
        }
        AppKind::Claude => {
            let document = claude_document(current)?;
            let env = document.get("env").and_then(Value::as_object);
            Ok(claude_values(true).iter().take(8).all(|(key, value)| env.and_then(|env| env.get(*key))
                .and_then(Value::as_str) == Some(value.as_str()))
                && env.and_then(|env| env.get("BETA_TRACING_ENDPOINT")).and_then(Value::as_str)
                    .is_none_or(str::is_empty))
        }
    }
}

pub(crate) fn render(
    app: AppKind, current: &str, enabled: bool, settings: &mut SettingsValues,
) -> Result<String, String> {
    if !enabled && !configured(app, current)? {
        return Err("客户端当前并非本机采集配置，请重新读取后操作".into());
    }
    match app {
        AppKind::Codex => render_codex(current, enabled),
        AppKind::Claude => render_claude(current, enabled, settings),
    }
}

fn codex_endpoint() -> String { format!("{}/codex/v1/logs", super::ENDPOINT) }

fn render_codex(current: &str, enabled: bool) -> Result<String, String> {
    use toml_edit::{InlineTable, Item, Table, Value as TomlValue, value};
    let mut document = current.parse::<toml_edit::DocumentMut>().map_err(|_| "Codex 配置不是有效 TOML")?;
    let otel = document.entry("otel").or_insert(Item::Table(Table::new())).as_table_like_mut()
        .ok_or("Codex otel 必须是对象，已拒绝覆盖")?;
    let exporter = if enabled {
        let mut http = InlineTable::new();
        http.insert("endpoint", TomlValue::from(codex_endpoint()));
        http.insert("protocol", TomlValue::from("json"));
        let mut exporter = InlineTable::new();
        exporter.insert("otlp-http", TomlValue::InlineTable(http));
        Item::Value(TomlValue::InlineTable(exporter))
    } else { value("none") };
    otel.insert("exporter", exporter);
    if enabled {
        otel.insert("log_user_prompt", value(false));
    }
    Ok(document.to_string())
}

fn claude_document(current: &str) -> Result<Value, String> {
    let document: Value = serde_json::from_str(current).map_err(|_| "Claude 配置不是有效 JSON")?;
    if !document.is_object() { return Err("Claude 配置根节点必须是对象".into()); }
    Ok(document)
}

fn claude_values(enabled: bool) -> Vec<(&'static str, String)> {
    let exporter = if enabled { "otlp" } else { "none" };
    vec![
        ("CLAUDE_CODE_ENABLE_TELEMETRY", if enabled { "1" } else { "0" }.into()),
        ("CLAUDE_CODE_ENHANCED_TELEMETRY_BETA", if enabled { "1" } else { "0" }.into()),
        ("OTEL_LOGS_EXPORTER", exporter.into()),
        ("OTEL_TRACES_EXPORTER", exporter.into()),
        ("OTEL_EXPORTER_OTLP_LOGS_PROTOCOL", "http/json".into()),
        ("OTEL_EXPORTER_OTLP_TRACES_PROTOCOL", "http/json".into()),
        ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", format!("{}/claude/v1/logs", super::ENDPOINT)),
        ("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", format!("{}/claude/v1/traces", super::ENDPOINT)),
        ("BETA_TRACING_ENDPOINT", String::new()),
        ("OTEL_LOG_USER_PROMPTS", "0".into()),
        ("OTEL_LOG_ASSISTANT_RESPONSES", "0".into()),
        ("OTEL_LOG_TOOL_DETAILS", "0".into()),
        ("OTEL_LOG_TOOL_CONTENT", "0".into()),
        ("OTEL_LOG_RAW_API_BODIES", "0".into()),
    ]
}

fn render_claude(current: &str, enabled: bool, settings: &mut SettingsValues) -> Result<String, String> {
    let mut document = claude_document(current)?;
    let env = document.as_object_mut().ok_or("Claude 配置无效")?
        .entry("env").or_insert_with(|| serde_json::json!({})).as_object_mut()
        .ok_or("Claude env 必须是对象，已拒绝覆盖")?;
    for (key, value) in claude_values(enabled) {
        env.insert(key.into(), Value::String(value.clone()));
        // Existing shared intentions must not overwrite the confirmed native exporter on the next switch.
        if let Some(saved) = settings.claude_extra.get_mut("env").and_then(Value::as_object_mut) {
            if saved.contains_key(key) { saved.insert(key.into(), Value::String(value)); }
        }
    }
    serde_json::to_string_pretty(&document).map(|text| format!("{text}\n")).map_err(|e| e.to_string())
}
