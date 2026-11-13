use super::declarative::is_http_url;
use super::response::{normalize_readings, parse_response, validate_http_status};
use asb_core::contracts::{ProviderConnectionOptions, UsageReadingInput, UsageSummary};
use rquickjs::{Context, Runtime};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::{Duration, Instant};

const SCRIPT_MEMORY_LIMIT: usize = 2 * 1024 * 1024;
const SCRIPT_STACK_LIMIT: usize = 256 * 1024;
const SCRIPT_EXECUTION_LIMIT: Duration = Duration::from_millis(250);
pub(super) const SCRIPT_PROGRAM_INVALID: &str =
    "用量查询脚本初始化失败，请检查表达式及 request(input)、extract(input) 函数";
const SCRIPT_RUNTIME_UNAVAILABLE: &str = "用量查询脚本运行环境初始化失败";
pub(super) const SCRIPT_EXECUTION_FAILED: &str = "用量查询脚本执行失败";

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
pub(super) struct ScriptRequest {
    pub(super) url: String,
    pub(super) method: String,
    pub(super) headers: BTreeMap<String, String>,
    pub(super) body: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ScriptExtractOutput {
    One(UsageReadingInput),
    Many(Vec<UsageReadingInput>),
}

impl ScriptExtractOutput {
    fn into_readings(self) -> Vec<UsageReadingInput> {
        match self {
            Self::One(reading) => vec![reading],
            Self::Many(readings) => readings,
        }
    }
}

/// A single source evaluation retained across the request and extract calls.
/// Context is declared before Runtime so it drops first.
pub(super) struct ScriptProgram {
    context: Context,
    _runtime: Runtime,
    deadline: Arc<Mutex<Instant>>,
    interrupted: Arc<AtomicBool>,
}

impl ScriptProgram {
    pub(super) fn new(source: &str) -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|_| SCRIPT_RUNTIME_UNAVAILABLE.to_string())?;
        runtime.set_memory_limit(SCRIPT_MEMORY_LIMIT);
        runtime.set_max_stack_size(SCRIPT_STACK_LIMIT);

        let deadline = Arc::new(Mutex::new(Instant::now() + SCRIPT_EXECUTION_LIMIT));
        let interrupt_deadline = Arc::clone(&deadline);
        let interrupted = Arc::new(AtomicBool::new(false));
        let interrupt_flag = Arc::clone(&interrupted);
        runtime.set_interrupt_handler(Some(Box::new(move || match interrupt_deadline.lock() {
            Ok(deadline) => {
                let exceeded = Instant::now() >= *deadline;
                if exceeded { interrupt_flag.store(true, Ordering::Relaxed); }
                exceeded
            },
            Err(_) => true,
        })));

        let context = Context::full(&runtime)
            .map_err(|_| script_error(&interrupted, SCRIPT_RUNTIME_UNAVAILABLE))?;
        let program = Self {
            context,
            _runtime: runtime,
            deadline,
            interrupted,
        };
        program.load(source)?;
        Ok(program)
    }

    fn reset_deadline(&self) {
        self.interrupted.store(false, Ordering::Relaxed);
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
            .map_err(|_| script_error(&self.interrupted, SCRIPT_PROGRAM_INVALID))
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
                const json = JSON.stringify(output, (_key, value) => {{
                    if (typeof value === "number" && !Number.isFinite(value)) {{
                        throw new TypeError("non-finite usage query output");
                    }}
                    return value;
                }});
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
            .map_err(|_| script_error(&self.interrupted, SCRIPT_EXECUTION_FAILED))?;
        serde_json::from_str(&json).map_err(|_| invalid_output.to_string())
    }

    pub(super) fn request(
        &self,
        api_key: &str,
        base_url: Option<&str>,
    ) -> Result<ScriptRequest, String> {
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

    pub(super) fn extract(
        &self,
        body: &serde_json::Value,
        status: u16,
        at: String,
    ) -> Result<UsageSummary, String> {
        validate_http_status(status)?;
        let value = self.call_json(
            "extract",
            &ScriptExtractInput { body, status },
            "用量查询脚本 extract 返回值无效",
            true,
        )?;
        let output: ScriptExtractOutput = serde_json::from_value(value)
            .map_err(|_| "用量查询脚本 extract 返回值无效".to_string())?;
        normalize_readings(output.into_readings(), at)
    }
}

fn script_error(interrupted: &AtomicBool, fallback: &str) -> String {
    if interrupted.load(Ordering::Relaxed) {
        format!("用量查询脚本执行超过 {} 毫秒限额", SCRIPT_EXECUTION_LIMIT.as_millis())
    } else { fallback.to_string() }
}

pub(super) fn parse_script_request(value: serde_json::Value) -> Result<ScriptRequest, String> {
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

pub(super) fn run_script_query(
    source: &str,
    api_key: &str,
    base_url: Option<&str>,
    connection: &ProviderConnectionOptions,
) -> Result<UsageSummary, String> {
    let program = ScriptProgram::new(source)?;
    let request = program.request(api_key, base_url)?;
    let headers = render_script_headers(&request.headers);
    let mut body = request.body.unwrap_or_default().into_bytes();
    crate::upstream_overrides::apply_body_override(&mut body, connection)
        .map_err(|_| "用量查询请求 body 覆盖无效".to_string())?;
    let (status, response_body) = crate::probe::http_request_with_options(
        &request.method,
        &request.url,
        &headers,
        &body,
        connection,
    )?;
    let response = parse_response(status, &response_body)?;
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    program.extract(&response, status, at)
}

#[cfg(test)]
mod tests;
