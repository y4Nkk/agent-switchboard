use std::io::Read;
use std::time::Duration;

use super::{HttpFetch, SourceError};

pub(super) const MAX_ARCHIVE_DOWNLOAD: u64 = 64 * 1024 * 1024;
pub(super) const MAX_API_DOWNLOAD: u64 = 8 * 1024 * 1024;
pub(crate) const MAX_DIRECTORY_DOWNLOAD: u64 = 1024 * 1024;

pub fn http_fetch(url: &str) -> Result<(u16, Vec<u8>), String> {
    let parsed = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
    let (limit, service) = match (parsed.scheme(), parsed.host_str()) {
        ("https", Some("api.github.com")) => (MAX_API_DOWNLOAD, "GitHub"),
        ("https", Some("codeload.github.com")) => (MAX_ARCHIVE_DOWNLOAD, "GitHub"),
        ("https", Some("skills.sh")) if parsed.path() == "/api/search" => {
            (MAX_DIRECTORY_DOWNLOAD, "skills.sh")
        }
        _ => return Err("仅接受 GitHub 或 skills.sh 的 HTTPS 来源地址".into()),
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
    let timeout = Duration::from_secs(if service == "skills.sh" { 10 } else { 45 });
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
    let status = response.status().as_u16();
    if response.content_length().is_some_and(|size| size > limit) {
        return Err(format!(
            "{service} 响应超过下载上限 {limit} 字节；请缩小来源范围"
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{service} 响应读取失败：{error}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!(
            "{service} 响应超过下载上限 {limit} 字节；未接受截断内容"
        ));
    }
    Ok((status, bytes))
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
        } else {
            "GitHub"
        };
        let hint = match (service, status) {
            ("skills.sh", 403 | 429) => "；请稍后重试目录搜索",
            ("skills.sh", _) => "；请稍后重试或使用 GitHub 仓库来源",
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
