//! Bounded authentication HTTP. Credential-bearing response bodies never enter diagnostics.

use serde_json::Value;
use std::{io::Read, time::Duration};

pub(super) fn json_request(
    method: reqwest::Method,
    url: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> Result<Value, String> {
    let (status, value) = json_response(method, url, headers, body)?;
    if !(200..300).contains(&status) {
        return Err(status_error(status));
    }
    Ok(value)
}

pub(super) fn json_response(
    method: reqwest::Method,
    url: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> Result<(u16, Value), String> {
    let client = crate::outbound_proxy::apply_blocking(reqwest::blocking::Client::builder()).map_err(|error| error.to_string())?
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "无法初始化 Claude 认证连接")?;
    let mut request = client
        .request(method, url)
        .header("user-agent", "Agent-Switchboard-Claude");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request
        .body(body)
        .send()
        .map_err(|_| "Claude 认证服务连接失败；请检查网络后重试")?;
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| "Claude 认证响应读取失败")?;
    if bytes.len() > 1_048_576 {
        return Err("Claude 认证响应超过大小限制".into());
    }
    let value = serde_json::from_slice(&bytes)
        .or_else(|error| {
            if !(200..300).contains(&status) {
                Ok(Value::Null)
            } else {
                Err(error)
            }
        })
        .map_err(|_| "Claude 认证服务响应格式无效，未更新已有凭据")?;
    Ok((status, value))
}

pub(super) fn status_error(status: u16) -> String {
    match status {
        401 | 403 => "Claude 托管账号授权已失效或无订阅权限，请重新登录".into(),
        429 => "Claude 认证服务请求过于频繁，请稍后重试".into(),
        code => format!("Claude 认证服务返回 HTTP {code}，未更新已有凭据"),
    }
}

pub(super) fn form_body(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut encoded = reqwest::Url::parse("https://localhost/").expect("static URL");
    encoded
        .query_pairs_mut()
        .extend_pairs(fields.iter().copied());
    encoded.query().unwrap_or_default().as_bytes().to_vec()
}
