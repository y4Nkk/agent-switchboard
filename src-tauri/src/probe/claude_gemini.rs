//! Claude's Google model discovery; never probes a Codex endpoint or cache.
use super::{models::diagnostic_secrets, ProviderModel};
use crate::provider_diagnostics::{network_diagnostic, read_http_diagnostic};
use asb_core::{contracts::ProviderConnectionOptions, AuthenticationScheme, UpstreamProtocol};
use serde_json::Value;
use std::{collections::BTreeSet, io::Read, time::Duration};

fn google_diagnostic_secrets(
    raw_key: &str,
    resolved_key: &str,
    connection: &ProviderConnectionOptions,
) -> Vec<String> {
    let mut secrets = diagnostic_secrets(raw_key, connection);
    if resolved_key != raw_key {
        secrets.push(resolved_key.to_string());
        secrets.sort();
        secrets.dedup();
    }
    secrets
}

pub(super) fn fetch(
    base: &str,
    raw_key: &str,
    selected: Option<AuthenticationScheme>,
    connection: &ProviderConnectionOptions,
) -> Result<Vec<ProviderModel>, String> {
    let endpoint = asb_core::endpoint::models_endpoint_for_connection(
        base,
        UpstreamProtocol::GeminiGenerateContent,
        connection,
    )?;
    let (scheme, key) = asb_core::claude_gemini::credential(raw_key, selected)?;
    let client = crate::outbound_proxy::apply_blocking(reqwest::blocking::Client::builder()).map_err(|error| error.to_string())?
        .user_agent("Agent Switchboard")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "无法初始化 Google 模型列表请求")?;
    let secrets = google_diagnostic_secrets(raw_key, &key, connection);
    let secret_refs = secrets.iter().map(String::as_str).collect::<Vec<_>>();
    let mut models = Vec::new();
    let mut tokens = BTreeSet::new();
    let mut next: Option<String> = None;
    for _ in 0..32 {
        let mut url = reqwest::Url::parse(&endpoint).map_err(|_| "Google 模型列表地址无效")?;
        if let Some(token) = &next {
            url.query_pairs_mut().append_pair("pageToken", token);
        }
        let request = match scheme {
            // Google OAuth callers identify the CLI client; plain API keys
            // never send this marker.
            AuthenticationScheme::Bearer => client
                .get(url.clone())
                .header("x-goog-api-client", "GeminiCLI/1.0")
                .bearer_auth(&key),
            AuthenticationScheme::XGoogApiKey => {
                client.get(url.clone()).header("x-goog-api-key", &key)
            }
            AuthenticationScheme::XApiKey => return Err("Google 不支持 x-api-key 认证".into()),
        };
        let mut request = request
            .build()
            .map_err(|_| "无法构造 Google 模型列表请求")?;
        crate::upstream_overrides::apply_header_overrides(request.headers_mut(), connection);
        let response = client
            .execute(request)
            .map_err(|error| network_diagnostic(url.as_str(), &error).summary())?;
        if !response.status().is_success() {
            return Err(read_http_diagnostic(response, &secret_refs).summary());
        }
        let value = read_page(response)?;
        let (page, token) = parse_page(&value)?;
        for model in page {
            if !models.iter().any(|old: &ProviderModel| old.id == model.id) {
                models.push(model);
            }
        }
        if let Some(token) = token {
            if !tokens.insert(token.clone()) {
                return Err("Google 模型列表返回重复分页标识".into());
            }
            next = Some(token);
        } else {
            return if models.is_empty() {
                Err("Google 未返回可生成内容的模型".into())
            } else {
                Ok(models)
            };
        }
    }
    Err("Google 模型列表超过分页上限，请缩小模型列表范围".into())
}

fn read_page(response: reqwest::blocking::Response) -> Result<Value, String> {
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "读取 Google 模型列表失败")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Google 模型列表超过大小限制".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Google 模型列表不是有效 JSON".into())
}

fn parse_page(value: &Value) -> Result<(Vec<ProviderModel>, Option<String>), String> {
    let items = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or("Google 模型列表缺少 models 数组")?;
    let mut models = Vec::new();
    for item in items {
        if item
            .get("supportedGenerationMethods")
            .is_some_and(|methods| {
                !methods
                    .as_array()
                    .is_some_and(|methods| methods.iter().any(|method| method == "generateContent"))
            })
        {
            continue;
        }
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or("Google 模型缺少 name")?;
        let id = name.strip_prefix("models/").unwrap_or(name);
        asb_core::claude_gemini::request_endpoint(
            "https://generativelanguage.googleapis.com",
            false,
            id,
            false,
        )?;
        models.push(ProviderModel {
            id: id.into(),
            owned_by: Some("Google".into()),
        });
    }
    let token = value
        .get("nextPageToken")
        .filter(|v| !v.is_null())
        .map(|v| v.as_str().ok_or("Google 分页标识必须是字符串"))
        .transpose()?
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok((models, token))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_generation_models_are_returned_without_prefix_or_duplicates() {
        let (models, next) = parse_page(&serde_json::json!({"models":[
            {"name":"models/gemini-3.1-pro","supportedGenerationMethods":["generateContent","countTokens"]},
            {"name":"models/embedding","supportedGenerationMethods":["embedContent"]}
        ],"nextPageToken":"next"})).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gemini-3.1-pro");
        assert_eq!(next.as_deref(), Some("next"));
        assert!(parse_page(&serde_json::json!({"data":[]})).is_err());
    }
}
