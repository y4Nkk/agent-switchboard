use super::{decode, contracts::ReceiverStatus, store};
use asb_core::contracts::AppKind;
use std::{io::Read, path::Path, sync::{Arc, Mutex}};
use tiny_http::{Header, Method, Request, Response};

const MAX_BODY: u64 = 4 * 1024 * 1024;

pub(super) fn receive(mut request: Request, root: &Path, status: &Arc<Mutex<ReceiverStatus>>) {
    let result = decode_request(&mut request).and_then(|batch| {
        let count = store::append(root, &batch.rows).map_err(|error| (503, error))?;
        Ok((count, batch.ignored))
    });
    let (code, body) = match result {
        Ok((accepted, ignored)) => {
            if let Ok(mut value) = status.lock() {
                value.last_export_at_ms = Some(chrono::Utc::now().timestamp_millis().max(0) as u64);
                value.accepted_records += accepted as u64;
                value.ignored_records += ignored;
                value.error = None;
            }
            (200, "{}".to_string())
        }
        Err((code, error)) => {
            if let Ok(mut value) = status.lock() { value.error = Some(error.clone()); }
            log::warn!("原生性能导出接收失败 ({code}): {error}");
            (code, serde_json::json!({"message": error}).to_string())
        }
    };
    let mut response = Response::from_string(body).with_status_code(code);
    if let Ok(header) = Header::from_bytes("Content-Type", "application/json") { response.add_header(header); }
    if let Err(error) = request.respond(response) { log::debug!("原生性能导出响应未送达：{error}"); }
}

fn decode_request(request: &mut Request) -> Result<decode::Batch, (u16, String)> {
    let method = request.method().clone();
    let path = request.url().to_string();
    let content_type = header(request, "Content-Type").to_string();
    let encoding = header(request, "Content-Encoding").to_string();
    decode_export(&method, &path, &content_type, &encoding, request.as_reader())
}

fn decode_export(
    method: &Method, path: &str, content_type: &str, encoding: &str, reader: &mut dyn Read,
) -> Result<decode::Batch, (u16, String)> {
    if method != &Method::Post { return Err((405, "OTLP 只接受 POST".into())); }
    if !matches!(path, "/codex/v1/logs" | "/claude/v1/logs" | "/claude/v1/traces") {
        return Err((404, "OTLP 接收地址不存在".into()));
    }
    if content_type.split(';').next().map(str::trim) != Some("application/json") {
        return Err((415, "原生性能接收器要求 OTLP/HTTP JSON".into()));
    }
    if !matches!(encoding, "" | "identity" | "gzip") {
        return Err((415, "OTLP 压缩格式不受支持".into()));
    }
    let encoded = read_bounded(reader)?;
    let decoded = if encoding == "gzip" {
        read_bounded(&mut flate2::read::GzDecoder::new(encoded.as_slice()))?
    } else { encoded };
    let value = serde_json::from_slice(&decoded).map_err(|_| (400, "OTLP JSON 无效".into()))?;
    match path {
        "/codex/v1/logs" => decode::logs(AppKind::Codex, &value),
        "/claude/v1/logs" => decode::logs(AppKind::Claude, &value),
        _ => decode::traces(&value),
    }.map_err(|error| (400, error))
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;

fn header<'a>(request: &'a Request, key: &str) -> &'a str {
    request.headers().iter().find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(key))
        .map(|header| header.value.as_str()).unwrap_or("")
}

fn read_bounded(reader: &mut dyn Read) -> Result<Vec<u8>, (u16, String)> {
    let mut result = Vec::new();
    reader.take(MAX_BODY + 1).read_to_end(&mut result)
        .map_err(|_| (400, "OTLP 请求体或压缩内容读取失败".into()))?;
    if result.len() as u64 > MAX_BODY { return Err((413, "OTLP 单次导出超过 4 MiB".into())); }
    Ok(result)
}
