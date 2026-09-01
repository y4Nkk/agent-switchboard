//! Profile-owned usage-balance queries.
//!
//! Declarative queries retain the fixed GET plus dual-ecosystem credential
//! headers and JSON-Pointer extraction. Script queries evaluate a small
//! JavaScript expression in a fresh, resource-bounded QuickJS context. The
//! script can calculate a request and extract JSON, but receives no host I/O;
//! WinHTTP remains the only process that performs a network request.

use asb_core::contracts::{UsageQuery, UsageReading, UsageSummary};
use rquickjs::{Context, Runtime};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SCRIPT_MEMORY_LIMIT: usize = 2 * 1024 * 1024;
const SCRIPT_STACK_LIMIT: usize = 256 * 1024;
const SCRIPT_EXECUTION_LIMIT: Duration = Duration::from_millis(250);
const SCRIPT_PROGRAM_INVALID: &str =
    "用量查询脚本必须计算为包含 request(input) 与 extract(input) 函数的对象";
const SCRIPT_EXECUTION_FAILED: &str = "用量查询脚本执行失败";

/// Substitutes placeholders in the stored URL. `{{baseUrl}}` loses any
/// trailing slash so `/user/balance` style paths concatenate cleanly.
fn render_url(template: &str, api_key: &str, base_url: Option<&str>) -> String {
    template
        .replace("{{baseUrl}}", base_url.unwrap_or("").trim_end_matches('/'))
        .replace("{{apiKey}}", api_key)
}

/// Reads one number out of the response body via a JSON Pointer such as
/// `data/balance`; numeric strings count, everything else is absent.
fn pointer_number(value: &serde_json::Value, path: &str) -> Option<f64> {
    let trimmed = path.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match value.pointer(&format!("/{trimmed}")) {
        Some(serde_json::Value::Number(number)) => number.as_f64(),
        Some(serde_json::Value::String(text)) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Checks the fixed URL form before it reaches WinHTTP. This deliberately
/// does not return the submitted URL, which could contain `{{apiKey}}`.
fn is_http_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && url.trim() == url
        && !url.chars().any(char::is_control)
}

/// Picks the summary fields out of a parsed declarative response body. A path
/// that leads nowhere simply leaves the field unset; finding none of the
/// configured numbers at all is an error.
fn extract_declarative_summary(
    body: &serde_json::Value,
    remaining_path: Option<&str>,
    used_path: Option<&str>,
    total_path: Option<&str>,
    unit: Option<String>,
    at: String,
) -> Result<UsageSummary, String> {
    let remaining = remaining_path.and_then(|path| pointer_number(body, path));
    let used = used_path.and_then(|path| pointer_number(body, path));
    let total = total_path.and_then(|path| pointer_number(body, path));
    if remaining.is_none() && used.is_none() && total.is_none() {
        return Err("响应中未找到任何配置的用量字段，请检查提取路径".to_string());
    }
    Ok(UsageSummary {
        readings: vec![UsageReading {
            plan_name: None,
            remaining,
            used,
            total,
            unit,
        }],
        at,
    })
}

fn run_declarative_query(
    url_template: &str,
    remaining_path: Option<&str>,
    used_path: Option<&str>,
    total_path: Option<&str>,
    unit: Option<String>,
    api_key: &str,
    base_url: Option<&str>,
) -> Result<UsageSummary, String> {
    let url = render_url(url_template, api_key, base_url);
    if !is_http_url(&url) {
        return Err("查询地址必须是 http(s) URL".to_string());
    }
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let (status, body) = crate::probe::http_get(&url, &crate::probe::auth_headers(api_key))?;
    if status == 401 || status == 403 {
        return Err(format!(
            "服务地址拒绝了 API 密钥（HTTP {status}），请确认密钥仍然有效"
        ));
    }
    if !(200..300).contains(&status) {
        return Err(format!("用量查询返回 HTTP {status}"));
    }
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| "用量响应不是有效 JSON".to_string())?;
    extract_declarative_summary(&value, remaining_path, used_path, total_path, unit, at)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScriptRequestInput<'a> {
    base_url: &'a str,
    api_key: &'a str,
}

#[derive(Serialize)]
struct ScriptExtractInput<'a> {
    body: &'a serde_json::Value,
    status: u16,
}

#[derive(Debug, PartialEq)]
struct ScriptRequest {
    url: String,
    method: String,
    headers: BTreeMap<String, String>,
    body: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ScriptExtractOutput {
    One(UsageReading),
    Many(Vec<UsageReading>),
}

impl ScriptExtractOutput {
    fn into_readings(self) -> Vec<UsageReading> {
        match self {
            Self::One(reading) => vec![reading],
            Self::Many(readings) => readings,
        }
    }
}

/// A single source evaluation retained across the request and extract calls.
/// Context is declared before Runtime so it drops first.
struct ScriptProgram {
    context: Context,
    _runtime: Runtime,
    deadline: Arc<Mutex<Instant>>,
}

impl ScriptProgram {
    fn new(source: &str) -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|_| SCRIPT_PROGRAM_INVALID.to_string())?;
        runtime.set_memory_limit(SCRIPT_MEMORY_LIMIT);
        runtime.set_max_stack_size(SCRIPT_STACK_LIMIT);

        let deadline = Arc::new(Mutex::new(Instant::now() + SCRIPT_EXECUTION_LIMIT));
        let interrupt_deadline = Arc::clone(&deadline);
        runtime.set_interrupt_handler(Some(Box::new(move || match interrupt_deadline.lock() {
            Ok(deadline) => Instant::now() >= *deadline,
            Err(_) => true,
        })));

        let context = Context::full(&runtime).map_err(|_| SCRIPT_PROGRAM_INVALID.to_string())?;
        let program = Self {
            context,
            _runtime: runtime,
            deadline,
        };
        program.load(source)?;
        Ok(program)
    }

    fn reset_deadline(&self) {
        if let Ok(mut deadline) = self.deadline.lock() {
            *deadline = Instant::now() + SCRIPT_EXECUTION_LIMIT;
        }
    }

    fn load(&self, source: &str) -> Result<(), String> {
        self.reset_deadline();
        let initialization = format!(
            r#"(() => {{
                const program = ({source});
                if (
                    program === null ||
                    typeof program !== "object" ||
                    Array.isArray(program) ||
                    typeof program.request !== "function" ||
                    typeof program.extract !== "function"
                ) {{
                    throw new TypeError("invalid usage query program");
                }}
                globalThis.__asbUsageQueryProgram = program;
            }})()"#
        );
        self.context
            .with(|ctx| ctx.eval::<(), _>(initialization))
            .map_err(|_| SCRIPT_PROGRAM_INVALID.to_string())
    }

    fn call_json<T: Serialize>(
        &self,
        function_name: &str,
        input: &T,
        invalid_output: &'static str,
        allow_array: bool,
    ) -> Result<serde_json::Value, String> {
        let input = serde_json::to_string(input).map_err(|_| invalid_output.to_string())?;
        let input_literal =
            serde_json::to_string(&input).map_err(|_| invalid_output.to_string())?;
        let function_name =
            serde_json::to_string(function_name).map_err(|_| invalid_output.to_string())?;
        let call = format!(
            r#"(() => {{
                const input = JSON.parse({input_literal});
                const output = globalThis.__asbUsageQueryProgram[{function_name}](input);
                if (
                    output === null ||
                    typeof output !== "object" ||
                    (!{allow_array} && Array.isArray(output)) ||
                    typeof output.then === "function"
                ) {{
                    throw new TypeError("invalid usage query output");
                }}
                const json = JSON.stringify(output);
                if (json === undefined) {{
                    throw new TypeError("unserializable usage query output");
                }}
                return json;
            }})()"#
        );
        self.reset_deadline();
        let json: String = self
            .context
            .with(|ctx| ctx.eval(call))
            .map_err(|_| SCRIPT_EXECUTION_FAILED.to_string())?;
        serde_json::from_str(&json).map_err(|_| invalid_output.to_string())
    }

    fn request(&self, api_key: &str, base_url: Option<&str>) -> Result<ScriptRequest, String> {
        let value = self.call_json(
            "request",
            &ScriptRequestInput {
                base_url: base_url.unwrap_or(""),
                api_key,
            },
            "用量查询脚本 request 返回值无效",
            false,
        )?;
        parse_script_request(value)
    }

    fn extract(
        &self,
        body: &serde_json::Value,
        status: u16,
        at: String,
    ) -> Result<UsageSummary, String> {
        let value = self.call_json(
            "extract",
            &ScriptExtractInput { body, status },
            "用量查询脚本 extract 返回值无效",
            true,
        )?;
        let output: ScriptExtractOutput = serde_json::from_value(value)
            .map_err(|_| "用量查询脚本 extract 返回值无效".to_string())?;
        let readings = output.into_readings();
        if readings.is_empty()
            || readings.iter().any(|reading| {
                reading.remaining.is_none() && reading.used.is_none() && reading.total.is_none()
            })
        {
            return Err("用量查询脚本 extract 的每组结果至少要返回一个数值".to_string());
        }
        Ok(UsageSummary { readings, at })
    }
}

fn parse_script_request(value: serde_json::Value) -> Result<ScriptRequest, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "用量查询脚本 request 返回值无效".to_string())?;
    let allowed = ["url", "method", "headers", "body"];
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("用量查询脚本 request 返回值无效".to_string());
    }
    let url = object
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "用量查询脚本 request 返回值无效".to_string())?;
    let method = object
        .get("method")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "用量查询脚本 request 返回值无效".to_string())?;
    let headers = match object.get("headers") {
        None => BTreeMap::new(),
        Some(serde_json::Value::Object(headers)) => headers
            .iter()
            .map(|(name, value)| {
                value
                    .as_str()
                    .map(|value| (name.clone(), value.to_string()))
                    .ok_or_else(|| "用量查询脚本 request 返回值无效".to_string())
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?,
        Some(_) => return Err("用量查询脚本 request 返回值无效".to_string()),
    };
    let body = match object.get("body") {
        None => None,
        Some(serde_json::Value::String(body)) => Some(body.clone()),
        Some(_) => return Err("用量查询脚本 request 返回值无效".to_string()),
    };

    if !is_http_url(&url) || !matches!(method.as_str(), "GET" | "POST") {
        return Err("用量查询脚本 request 返回了不受支持的地址或方法".to_string());
    }
    for (name, value) in &headers {
        let valid_name = !name.is_empty()
            && name.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'!' | b'#'
                            | b'$'
                            | b'%'
                            | b'&'
                            | b'\''
                            | b'*'
                            | b'+'
                            | b'-'
                            | b'.'
                            | b'^'
                            | b'_'
                            | b'`'
                            | b'|'
                            | b'~'
                    )
            });
        if !valid_name || value.contains(['\r', '\n']) {
            return Err("用量查询脚本 request 包含无效请求头".to_string());
        }
    }
    Ok(ScriptRequest {
        url,
        method,
        headers,
        body,
    })
}

fn render_script_headers(headers: &BTreeMap<String, String>) -> String {
    headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn run_script_query(
    source: &str,
    api_key: &str,
    base_url: Option<&str>,
) -> Result<UsageSummary, String> {
    let program = ScriptProgram::new(source)?;
    let request = program.request(api_key, base_url)?;
    let headers = render_script_headers(&request.headers);
    let body = request.body.as_deref().unwrap_or("").as_bytes();
    let (status, response_body) =
        crate::probe::http_request(&request.method, &request.url, &headers, body)?;
    let response: serde_json::Value =
        serde_json::from_str(&response_body).map_err(|_| "用量响应不是有效 JSON".to_string())?;
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    program.extract(&response, status, at)
}

/// Validates every field that becomes durable state. The core validates the
/// shared tagged shape; script mode additionally evaluates the source once to
/// prove it produces both required functions before LocalState writes it.
pub(crate) fn validate_persisted(query: &UsageQuery) -> Result<(), String> {
    asb_core::validate::validate_usage_query(query).map_err(|error| error.to_string())?;
    if let UsageQuery::Script { source } = query {
        ScriptProgram::new(source).map(|_| ())?;
    }
    Ok(())
}

/// Runs one usage query against the provider endpoint. The credential travels
/// only in the declarative mode's established dual auth headers, or into the
/// script's explicit `request` input. It is never echoed in errors or the
/// returned summary.
pub fn run_usage_query(
    query: &UsageQuery,
    api_key: &str,
    base_url: Option<&str>,
) -> Result<UsageSummary, String> {
    asb_core::validate::validate_usage_query(query).map_err(|error| error.to_string())?;
    match query {
        UsageQuery::Declarative {
            url,
            remaining_path,
            used_path,
            total_path,
            unit,
        } => run_declarative_query(
            url,
            remaining_path.as_deref(),
            used_path.as_deref(),
            total_path.as_deref(),
            unit.clone(),
            api_key,
            base_url,
        ),
        UsageQuery::Script { source } => run_script_query(source, api_key, base_url),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declarative(
        url: &str,
        remaining: Option<&str>,
        used: Option<&str>,
        total: Option<&str>,
    ) -> UsageQuery {
        UsageQuery::Declarative {
            url: url.to_string(),
            remaining_path: remaining.map(str::to_string),
            used_path: used.map(str::to_string),
            total_path: total.map(str::to_string),
            unit: None,
        }
    }

    const SCRIPT: &str = r#"({
        request({ baseUrl, apiKey }) {
            return {
                url: baseUrl + "/usage",
                method: "POST",
                headers: { Authorization: "Bearer " + apiKey },
                body: "{}"
            };
        },
        extract({ body, status }) {
            return { remaining: body.balance, used: status, unit: "credits" };
        }
    })"#;

    #[test]
    fn placeholders_substitute_and_base_url_loses_trailing_slash() {
        let rendered = render_url(
            "{{baseUrl}}/user/balance?key={{apiKey}}",
            "sk-x",
            Some("https://relay.example/v1/"),
        );
        assert_eq!(rendered, "https://relay.example/v1/user/balance?key=sk-x");
    }

    #[test]
    fn pointer_reads_numbers_numeric_strings_and_array_cells() {
        let body: serde_json::Value = serde_json::from_str(
            r#"{"data":{"balance":12.5,"used":"3.25","tiers":[{"quota":7}]}}"#,
        )
        .expect("body");
        assert_eq!(pointer_number(&body, "data/balance"), Some(12.5));
        assert_eq!(pointer_number(&body, "/data/used"), Some(3.25));
        assert_eq!(pointer_number(&body, "data/tiers/0/quota"), Some(7.0));
        assert_eq!(pointer_number(&body, "data/missing"), None);
        assert_eq!(pointer_number(&body, ""), None);
    }

    #[test]
    fn declarative_summary_fills_configured_fields_only() {
        let body: serde_json::Value =
            serde_json::from_str(r#"{"balance":9.5,"used":0.5}"#).expect("body");
        let summary = extract_declarative_summary(
            &body,
            Some("balance"),
            None,
            Some("total"),
            Some("USD".into()),
            "t".into(),
        )
        .expect("summary");
        assert_eq!(summary.readings.len(), 1);
        let reading = &summary.readings[0];
        assert_eq!(reading.remaining, Some(9.5));
        assert_eq!(reading.used, None);
        assert_eq!(reading.total, None);
        assert_eq!(reading.unit.as_deref(), Some("USD"));
    }

    #[test]
    fn declarative_validation_happens_before_any_request() {
        let empty_url = declarative("  ", Some("a"), None, None);
        assert!(run_usage_query(&empty_url, "sk", None).is_err());
        let no_paths = declarative("https://x", None, None, None);
        assert!(run_usage_query(&no_paths, "sk", None).is_err());
    }

    #[test]
    fn script_keeps_request_and_extract_in_one_bounded_runtime() {
        let program = ScriptProgram::new(SCRIPT).expect("script program");
        let request = program
            .request("test-api-key", Some("https://relay.example/v1"))
            .expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://relay.example/v1/usage");
        assert_eq!(
            request.headers.get("Authorization"),
            Some(&"Bearer test-api-key".to_string())
        );
        let response = serde_json::json!({ "balance": 12.5 });
        let summary = program
            .extract(&response, 201, "t".to_string())
            .expect("extract");
        assert_eq!(summary.readings.len(), 1);
        let reading = &summary.readings[0];
        assert_eq!(reading.remaining, Some(12.5));
        assert_eq!(reading.used, Some(201.0));
        assert_eq!(reading.unit.as_deref(), Some("credits"));
    }

    #[test]
    fn script_extract_preserves_multiple_named_readings() {
        let program = ScriptProgram::new(
            r#"({
                request() { return { url: "https://relay.example", method: "GET" }; },
                extract() {
                    return [
                        { planName: "主套餐", remaining: 12, unit: "USD" },
                        { planName: "附加套餐", used: 3, total: 10, unit: "USD" }
                    ];
                }
            })"#,
        )
        .expect("script program");

        let summary = program
            .extract(&serde_json::json!({}), 200, "t".to_string())
            .expect("multiple readings");
        assert_eq!(summary.readings.len(), 2);
        assert_eq!(summary.readings[0].plan_name.as_deref(), Some("主套餐"));
        assert_eq!(summary.readings[0].remaining, Some(12.0));
        assert_eq!(summary.readings[1].plan_name.as_deref(), Some("附加套餐"));
        assert_eq!(summary.readings[1].used, Some(3.0));
        assert_eq!(summary.readings[1].total, Some(10.0));
    }

    #[test]
    fn imported_ccswitch_script_uses_profile_inputs_and_keeps_each_plan() {
        let row = asb_core::ccswitch::CcSwitchRow {
            id: "cc-usage".to_string(),
            app_type: "claude".to_string(),
            name: "导入测试".to_string(),
            settings_config: r#"{
                "env": {
                    "ANTHROPIC_BASE_URL": "https://relay.example/v1",
                    "ANTHROPIC_AUTH_TOKEN": "<placeholder>"
                }
            }"#
            .to_string(),
            website_url: None,
            notes: None,
            meta: Some(
                serde_json::json!({
                    "usage_script": {
                        "enabled": true,
                        "language": "javascript",
                        "code": r#"({
                            request: {
                                url: "{{baseUrl}}/balance",
                                method: "GET",
                                headers: { Authorization: "Bearer {{apiKey}}" }
                            },
                            extractor: function(response) {
                                return [
                                    { isValid: true, planName: "主套餐", remaining: response.main, unit: "USD" },
                                    { isValid: true, planName: "附加套餐", total: response.total, used: response.used, unit: "USD" }
                                ];
                            }
                        })"#
                    }
                })
                .to_string(),
            ),
        };
        let proposal = asb_core::ccswitch::map_row(&row).expect("mapped provider");
        let Some(UsageQuery::Script { source }) = proposal.draft.usage_query else {
            panic!("query script should import");
        };

        let program = ScriptProgram::new(&source).expect("native script");
        let request = program
            .request("profile-key", Some("https://relay.example/v1/"))
            .expect("request");
        assert_eq!(request.url, "https://relay.example/v1/balance");
        assert_eq!(
            request.headers.get("Authorization"),
            Some(&"Bearer profile-key".to_string())
        );
        let summary = program
            .extract(
                &serde_json::json!({ "main": 12, "used": 3, "total": 20 }),
                200,
                "t".to_string(),
            )
            .expect("summary");
        assert_eq!(summary.readings.len(), 2);
        assert_eq!(summary.readings[0].plan_name.as_deref(), Some("主套餐"));
        assert_eq!(summary.readings[0].remaining, Some(12.0));
        assert_eq!(summary.readings[1].plan_name.as_deref(), Some("附加套餐"));
        assert_eq!(summary.readings[1].used, Some(3.0));
        assert_eq!(summary.readings[1].total, Some(20.0));
    }

    #[test]
    fn script_validation_requires_both_functions_without_leaking_source() {
        let query = UsageQuery::Script {
            source: "({ request() {} })".to_string(),
        };
        let error = validate_persisted(&query).expect_err("missing extract");
        assert_eq!(error, SCRIPT_PROGRAM_INVALID);
        assert!(!error.contains("request()"));
    }

    #[test]
    fn script_request_rejects_invalid_method_urls_and_header_lines() {
        for value in [
            serde_json::json!({ "url": "file:///x", "method": "GET" }),
            serde_json::json!({ "url": "https://x", "method": "PATCH" }),
            serde_json::json!({ "url": "https://x", "method": "GET", "headers": { "X-Test": "ok\r\nInjected: yes" } }),
        ] {
            assert!(parse_script_request(value).is_err());
        }
    }

    #[test]
    fn script_extract_requires_one_numeric_value() {
        let program = ScriptProgram::new(
            r#"({
                request() { return { url: "https://relay.example", method: "GET" }; },
                extract() { return { unit: "credits" }; }
            })"#,
        )
        .expect("script program");
        assert!(program
            .extract(&serde_json::json!({}), 200, "t".to_string())
            .is_err());
    }

    #[test]
    fn scripts_have_no_host_network_or_process_globals() {
        let program = ScriptProgram::new(
            r#"({
                request() {
                    if (
                        typeof fetch !== "undefined" ||
                        typeof process !== "undefined" ||
                        typeof module !== "undefined" ||
                        typeof require !== "undefined" ||
                        typeof fs !== "undefined" ||
                        typeof os !== "undefined" ||
                        typeof std !== "undefined" ||
                        typeof Deno !== "undefined" ||
                        typeof Bun !== "undefined"
                    ) throw new Error("host capability");
                    return { url: "https://relay.example", method: "GET" };
                },
                extract() { return { remaining: 1 }; }
            })"#,
        )
        .expect("script program");
        assert!(program.request("key", None).is_ok());
    }

    #[test]
    fn script_errors_do_not_echo_api_keys() {
        let secret = "api-key-that-must-not-escape";
        let program = ScriptProgram::new(
            r#"({
                request({ apiKey }) { throw new Error(apiKey); },
                extract() { return { remaining: 1 }; }
            })"#,
        )
        .expect("script program");

        let error = program
            .request(secret, None)
            .expect_err("request must fail");
        assert_eq!(error, SCRIPT_EXECUTION_FAILED);
        assert!(!error.contains(secret));
    }

    #[test]
    fn nonterminating_source_is_interrupted() {
        let started = Instant::now();
        assert!(ScriptProgram::new("(() => { while (true) {} })()").is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
