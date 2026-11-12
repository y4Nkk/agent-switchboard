//! Streamable HTTP transport. Each request consumes the same bounded check budget.
use std::collections::BTreeMap;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;
use super::protocol::{self, Connection};
use super::sse::SseDecoder;
use super::{CheckDeadline, CheckError, CheckErrorKind, ProbeOutcome, INITIALIZE_DEADLINE, MAX_OUTPUT_BYTES};

struct HttpTransport {
    client: reqwest::Client,
    url: String,
    headers: HeaderMap,
    session: Option<String>,
    protocol_version: Option<String>,
    bytes_read: usize,
}
struct HttpConnection { runtime: tokio::runtime::Runtime, transport: HttpTransport }

pub(super) fn check_http(
    url: &str, headers: BTreeMap<String, String>, bearer: Option<String>, deadline: &CheckDeadline,
) -> ProbeOutcome {
    let mut connection = match HttpConnection::new(url, headers, bearer) {
        Ok(connection) => connection,
        Err(error) => return ProbeOutcome::for_error(None, error),
    };
    let result = protocol::probe(&mut connection, deadline, INITIALIZE_DEADLINE);
    connection.close(deadline);
    result
}

impl HttpConnection {
    fn new(url: &str, headers: BTreeMap<String, String>, bearer: Option<String>) -> Result<Self, CheckError> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()
            .map_err(|_| CheckError::transport("无法启动 HTTP 检测运行时"))?;
        let client = {
            let _entered = runtime.enter();
            crate::outbound_proxy::apply(reqwest::Client::builder())
                .map_err(|error| CheckError::transport(error.to_string()))?.build()
                .map_err(|_| CheckError::transport("无法创建 HTTP 检测连接"))?
        };
        let mut request_headers = HeaderMap::new();
        for (name, value) in headers {
            request_headers.insert(
                HeaderName::from_bytes(name.as_bytes()).map_err(|_| CheckError::protocol("HTTP 头名称无效"))?,
                HeaderValue::from_str(&value).map_err(|_| CheckError::protocol("HTTP 头内容无效"))?,
            );
        }
        // These headers belong to this protocol session, never user material.
        request_headers.remove("MCP-Session-Id");
        request_headers.remove("MCP-Protocol-Version");
        request_headers.insert(ACCEPT, HeaderValue::from_static("application/json, text/event-stream"));
        request_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(bearer) = bearer {
            request_headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {bearer}"))
                .map_err(|_| CheckError::protocol("HTTP 授权内容无效"))?);
        }
        Ok(Self { runtime, transport: HttpTransport {
            client, url: url.to_string(), headers: request_headers,
            session: None, protocol_version: None, bytes_read: 0,
        } })
    }

    fn close(&self, deadline: &CheckDeadline) {
        if self.transport.session.is_none() { return; }
        let Some(timeout) = deadline.cleanup_timeout() else { return; };
        let request = self.transport.client.delete(&self.transport.url)
            .headers(self.transport.session_headers());
        let _ = self.runtime.block_on(async { tokio::time::timeout(timeout, request.send()).await });
    }
}

impl Connection for HttpConnection {
    fn request(&mut self, request: &Value, deadline: &CheckDeadline) -> Result<Value, CheckError> {
        self.runtime.block_on(self.transport.post(request, deadline))?
            .ok_or_else(|| CheckError::protocol("HTTP 请求未返回 JSON-RPC 响应"))
    }

    fn notify(&mut self, notification: &Value, deadline: &CheckDeadline) -> Result<(), CheckError> {
        self.runtime.block_on(self.transport.post(notification, deadline)).map(|_| ())
    }

    fn set_protocol_version(&mut self, version: &str) {
        self.transport.protocol_version = Some(version.to_string());
    }
}

impl HttpTransport {
    fn session_headers(&self) -> HeaderMap {
        let mut headers = self.headers.clone();
        if let Some(session) = &self.session {
            // The value was validated when initialization received it.
            headers.insert("mcp-session-id", HeaderValue::from_str(session).expect("validated session"));
        }
        if let Some(version) = &self.protocol_version {
            headers.insert("mcp-protocol-version", HeaderValue::from_str(version).expect("negotiated version"));
        }
        headers
    }

    async fn post(&mut self, payload: &Value, deadline: &CheckDeadline) -> Result<Option<Value>, CheckError> {
        let timeout = deadline.request_timeout()?;
        let result = tokio::select! {
            _ = deadline.cancelled() => Err(CheckError::cancelled()),
            result = tokio::time::timeout(timeout, self.exchange(payload)) => match result {
                Ok(result) => result,
                Err(_) => Err(CheckError::timed_out()),
            },
        };
        if result.as_ref().err().is_some_and(|error| matches!(error.kind,
            CheckErrorKind::Cancelled | CheckErrorKind::TimedOut))
            && payload.get("method").and_then(Value::as_str) != Some("initialize") {
            if let (Some(id), Some(timeout)) = (payload.get("id"), deadline.cleanup_timeout()) {
                let notification = serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/cancelled",
                    "params": { "requestId": id, "reason": "Connection check stopped" } });
                if let Ok(body) = serde_json::to_vec(&notification) {
                    let request = self.client.post(&self.url).headers(self.session_headers()).body(body);
                    let _ = tokio::time::timeout(timeout, request.send()).await;
                }
            }
        }
        result
    }

    async fn exchange(&mut self, payload: &Value) -> Result<Option<Value>, CheckError> {
        let body = serde_json::to_vec(payload).map_err(|_| CheckError::protocol("请求无法编码"))?;
        let mut response = self.client.post(&self.url).headers(self.session_headers()).body(body)
            .send().await.map_err(http_error)?;
        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            return Err(CheckError { kind: CheckErrorKind::Authentication, message: "端点要求原生客户端授权".into() });
        }
        let Some(id) = payload.get("id") else {
            return if status == 202 { Ok(None) } else {
                Err(CheckError::protocol(format!("服务未接受通知（HTTP {status}）")))
            };
        };
        if status != 200 {
            return Err(CheckError::transport(format!("HTTP 请求失败（{status}）")));
        }
        if payload.get("method").and_then(Value::as_str) == Some("initialize") {
            if let Some(session) = response.headers().get("MCP-Session-Id") {
                let session = session.to_str().ok().filter(|value| !value.is_empty()
                    && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte)))
                    .ok_or_else(|| CheckError::protocol("服务返回了无效的会话标识"))?;
                self.session = Some(session.to_string());
            }
        }
        let content_type = response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok())
            .unwrap_or_default().split(';').next().unwrap_or_default().trim().to_string();
        if !["application/json", "text/event-stream"].iter().any(|kind| content_type.eq_ignore_ascii_case(kind)) {
            return Err(CheckError::protocol("HTTP 响应类型不是 JSON 或 SSE"));
        }
        let stream = content_type.eq_ignore_ascii_case("text/event-stream");
        let mut decoder = SseDecoder::default();
        let mut body = Vec::new();
        while let Some(bytes) = response.chunk().await.map_err(http_error)? {
            self.bytes_read = self.bytes_read.saturating_add(bytes.len());
            if self.bytes_read > MAX_OUTPUT_BYTES { return Err(CheckError::output_limit()); }
            if stream {
                for message in decoder.push(&bytes)? {
                    if protocol::is_response(&message, id) { return Ok(Some(message)); }
                    if let Some(reply) = protocol::server_request_reply(&message) {
                        self.reply_to_server(&reply).await?;
                    }
                }
            } else {
                body.extend_from_slice(&bytes);
            }
        }
        if stream { return Err(CheckError::closed()); }
        let response: Value = serde_json::from_slice(&body)
            .map_err(|_| CheckError::protocol("HTTP 响应不是有效的 JSON"))?;
        if !protocol::is_response(&response, id) {
            return Err(CheckError::protocol("HTTP 响应与请求编号或 JSON-RPC 格式不符"));
        }
        Ok(Some(response))
    }

    async fn reply_to_server(&self, payload: &Value) -> Result<(), CheckError> {
        let body = serde_json::to_vec(payload).map_err(|_| CheckError::protocol("响应无法编码"))?;
        let response = self.client.post(&self.url).headers(self.session_headers()).body(body)
            .send().await.map_err(http_error)?;
        if response.status().as_u16() != 202 {
            return Err(CheckError::protocol("服务未接受客户端 JSON-RPC 响应"));
        }
        Ok(())
    }
}

fn http_error(error: reqwest::Error) -> CheckError {
    if error.is_timeout() { CheckError::timed_out() }
    else { CheckError::transport("HTTP 连接或响应读取失败") }
}

#[cfg(test)]
mod tests;
