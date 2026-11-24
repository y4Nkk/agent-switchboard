use std::io::{self, Read};
use std::time::{Duration, Instant};

use super::{HttpFetch, SourceError};

pub(super) const MAX_ARCHIVE_DOWNLOAD: u64 = 64 * 1024 * 1024;
pub(super) const MAX_API_DOWNLOAD: u64 = 8 * 1024 * 1024;
pub(crate) const MAX_DIRECTORY_DOWNLOAD: u64 = 1024 * 1024;

#[derive(Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpTransfer {
    pub http_status: Option<u16>,
    received_bytes: u64,
    total_bytes: Option<u64>,
}

pub fn http_fetch(url: &str) -> Result<(u16, Vec<u8>), String> {
    http_fetch_with_progress(url, None, &|_| {})
}

pub(crate) fn http_fetch_with_progress(
    url: &str,
    deadline: Option<Instant>,
    report: &dyn Fn(HttpTransfer),
) -> Result<(u16, Vec<u8>), String> {
    let parsed = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
    let (limit, service) = match (parsed.scheme(), parsed.host_str()) {
        ("https", Some("api.github.com")) => (MAX_API_DOWNLOAD, "GitHub"),
        ("https", Some("codeload.github.com")) => (MAX_ARCHIVE_DOWNLOAD, "GitHub"),
        ("https", Some("skills.sh")) if parsed.path() == "/api/search" => {
            (MAX_DIRECTORY_DOWNLOAD, "skills.sh")
        }
        ("https", Some("registry.modelcontextprotocol.io")) if parsed.path().starts_with("/v0.1/servers") => {
            (MAX_DIRECTORY_DOWNLOAD, "MCP Registry")
        }
        _ => return Err("来源地址不在支持的 HTTPS 目录中".into()),
    };
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.port().is_some() {
        return Err("来源地址不能包含凭据或自定义端口".into());
    }
    let client = crate::outbound_proxy::cached_blocking_client("extension-sources", |builder| {
        builder
            .user_agent("Agent Switchboard")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("static source HTTP configuration")
    }).map_err(|error| error.to_string())?;
    let mut timeout = Duration::from_secs(if service == "GitHub" { 45 } else { 10 });
    if let Some(deadline) = deadline {
        timeout = timeout.min(deadline.saturating_duration_since(Instant::now()));
        if timeout.is_zero() {
            return Err("本轮仓库扫描的网络等待已达上限，请减少启用的仓库后重试".into());
        }
    }
    report(HttpTransfer::default());
    let response = client
        .get(parsed)
        .timeout(timeout)
        .send()
        .map_err(|error| {
            if error.is_timeout() {
                format!("{service} 请求超时，请检查网络后重试")
            } else {
                format!("{service} 请求失败：{error}")
            }
        })?;
    read_response(response, limit, service, report)
}

fn read_response(
    response: reqwest::blocking::Response,
    limit: u64,
    service: &str,
    report: &dyn Fn(HttpTransfer),
) -> Result<(u16, Vec<u8>), String> {
    let status = response.status().as_u16();
    let mut progress = HttpTransfer {
        http_status: Some(status),
        received_bytes: 0,
        total_bytes: response.content_length(),
    };
    report(progress);
    if progress.total_bytes.is_some_and(|size| size > limit) {
        return Err(format!(
            "{service} 响应超过下载上限 {limit} 字节；请缩小来源范围"
        ));
    }
    let mut response = response.take(limit + 1);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 32 * 1024];
    let mut last_report = Instant::now();
    let read_result = loop {
        match response.read(&mut chunk) {
            Ok(0) => break Ok(()),
            Ok(size) => bytes.extend_from_slice(&chunk[..size]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => break Err(error),
        }
        if last_report.elapsed() >= Duration::from_millis(200) {
            progress.received_bytes = bytes.len() as u64;
            report(progress);
            last_report = Instant::now();
        }
    };
    progress.received_bytes = bytes.len() as u64;
    report(progress);
    read_result.map_err(|error| response_read_error(service, error))?;
    if bytes.len() as u64 > limit {
        return Err(format!(
            "{service} 响应超过下载上限 {limit} 字节；未接受截断内容"
        ));
    }
    Ok((status, bytes))
}

fn response_read_error(service: &str, error: io::Error) -> String {
    let timed_out = error.kind() == io::ErrorKind::TimedOut
        || error.get_ref().and_then(|source| source.downcast_ref::<reqwest::Error>())
            .is_some_and(reqwest::Error::is_timeout);
    if timed_out {
        format!("{service} 响应体读取超时，请检查网络后重试")
    } else {
        format!("读取 {service} 响应体时遇到错误（{error}）")
    }
}

pub(crate) fn fetch_bytes(url: &str, limit: u64, fetch: HttpFetch) -> Result<Vec<u8>, SourceError> {
    let (status, body) = fetch(url).map_err(SourceError::Unreachable)?;
    if body.len() as u64 > limit {
        return Err(SourceError::Rejected(format!(
            "来源响应超过上限 {limit} 字节"
        )));
    }
    if status != 200 {
        let message = serde_json::from_slice::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| value.get("message")?.as_str().map(str::to_owned))
            .unwrap_or_default()
            .chars()
            .take(300)
            .collect::<String>();
        let service = if url.starts_with("https://skills.sh/") {
            "skills.sh"
        } else if url.starts_with("https://registry.modelcontextprotocol.io/") {
            "MCP Registry"
        } else {
            "GitHub"
        };
        let hint = match (service, status) {
            ("skills.sh", 403 | 429) => "；请稍后重试目录搜索",
            ("skills.sh", _) => "；请稍后重试或使用 GitHub 仓库来源",
            ("MCP Registry", _) => "；请检查目录条目状态或稍后重试",
            (_, 403 | 429) => "；请检查 GitHub 访问限制或稍后重试",
            (_, 404) => "；请确认公开仓库、分支或提交存在",
            (_, 301 | 302 | 307 | 308) => "；仓库地址已迁移，请使用新的仓库地址",
            _ => "",
        };
        return Err(SourceError::Unreachable(
            format!("{service} 返回 HTTP {status}{hint} {message}")
                .trim()
                .into(),
        ));
    }
    Ok(body)
}
