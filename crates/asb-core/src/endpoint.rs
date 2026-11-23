//! Provider URL semantics shared by validation, model discovery and requests.
//!
//! OpenAI-compatible base URLs include the provider's complete API prefix.
//! Anthropic base URLs are service roots, matching Claude Code's SDK contract.

use crate::contracts::{CodexOperation, UpstreamProtocol};
use url::Url;

pub fn validate_base_url(base_url: &str, protocol: UpstreamProtocol) -> Result<(), String> {
    parse_base(base_url, protocol).map(|_| ())
}

pub fn validate_full_url(endpoint: &str) -> Result<(), String> {
    parse_full_url(endpoint).map(|_| ())
}

pub fn upstream_endpoint(base_url: &str, protocol: UpstreamProtocol) -> Result<String, String> {
    if protocol == UpstreamProtocol::GeminiGenerateContent {
        return Err("Gemini 原生请求需要模型和流式模式，请使用 Claude 模型端点解析".into());
    }
    append(base_url, protocol, request_path(protocol))
}

/// Resolves a provider request target while honoring the source application's full-URL
/// flag. Full URLs are used verbatim after validation; no protocol path is
/// appended a second time.
pub fn upstream_endpoint_with_options(
    base_url: &str,
    protocol: UpstreamProtocol,
    is_full_url: bool,
) -> Result<String, String> {
    if is_full_url {
        return parse_full_url(base_url);
    }
    upstream_endpoint(base_url, protocol)
}

/// The native Codex compact endpoint. It is deliberately separate from a
/// generated Responses request: callers must not turn a provider-supported
/// compact operation into a normal model summary.
pub fn compact_endpoint(base_url: &str) -> Result<String, String> {
    append(base_url, UpstreamProtocol::Responses, "/responses/compact")
}

/// Resolves the configured upstream target for a fixed Codex operation.
///
/// Full-URL mode is exact for the primary request, but standalone operations
/// still need their own paths. Only known API shapes are rewritten; opaque
/// full URLs fail closed instead of sending a payload to an unrelated route.
pub fn codex_operation_endpoint(
    base_url: &str, is_full_url: bool, operation: CodexOperation,
) -> Result<String, String> {
    let target_path = operation.upstream_path();
    if !is_full_url {
        return append(base_url, UpstreamProtocol::Responses, target_path);
    }
    validate_full_url(base_url)
        .map_err(|_| "供应商上游请求地址无效".to_string())?;
    let parsed = Url::parse(base_url).map_err(|_| "供应商上游请求地址无效".to_string())?;
    let source_path = parsed.path().trim_end_matches('/').to_ascii_lowercase();
    let suffix = codex_full_url_sources(operation)
        .iter()
        .map(|source| source.upstream_path())
        .find(|suffix| source_path.ends_with(*suffix))
        .ok_or_else(|| {
            format!("无法从完整 Codex 地址推导 {target_path}，请填写 API 根地址或已知操作地址")
        })?;
    let without_query = base_url.split_once('?').map_or(base_url, |(url, _)| url);
    let without_query = without_query.trim_end_matches('/');
    let prefix_len = without_query
        .len()
        .checked_sub(suffix.len())
        .ok_or_else(|| "供应商上游请求地址无效".to_string())?;
    let query = base_url
        .split_once('?')
        .map(|(_, query)| query)
        .filter(|query| !query.is_empty())
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    Ok(format!(
        "{}{target_path}{query}",
        &without_query[..prefix_len]
    ))
}

fn codex_full_url_sources(operation: CodexOperation) -> &'static [CodexOperation] {
    use CodexOperation::*;
    match operation {
        Compact | AlphaSearch => &[Compact, Responses],
        ImageGeneration | ImageEdit => &[ImageGeneration, ImageEdit, ChatCompletions, Compact, Responses],
        ChatCompletions => &[ChatCompletions, Compact, Responses],
        _ => &[],
    }
}

pub fn models_endpoint(base_url: &str, protocol: UpstreamProtocol) -> Result<String, String> {
    let suffix = match protocol {
        UpstreamProtocol::Responses | UpstreamProtocol::ChatCompletions => "/models",
        UpstreamProtocol::AnthropicMessages => "/v1/models",
        UpstreamProtocol::GeminiGenerateContent => {
            return crate::claude_gemini::models_endpoint(base_url, false)
        }
    };
    append(base_url, protocol, suffix)
}

/// Resolves the one explicit model-discovery endpoint. A full request URL is
/// never guessed or reused as a model-list URL: it must have `models_url`.
pub fn models_endpoint_for_connection(
    base_url: &str,
    protocol: UpstreamProtocol,
    connection: &crate::contracts::ProviderConnectionOptions,
) -> Result<String, String> {
    if let Some(url) = &connection.models_url {
        validate_full_url(url)?;
        return Ok(url.clone());
    }
    if connection.is_full_url {
        return Err("完整请求 URL 不能推导模型列表地址；请填写模型列表 URL".into());
    }
    if protocol == UpstreamProtocol::GeminiGenerateContent {
        return crate::claude_gemini::models_endpoint(base_url, false);
    }
    models_endpoint(base_url, protocol)
}

fn request_path(protocol: UpstreamProtocol) -> &'static str {
    match protocol {
        UpstreamProtocol::Responses => "/responses",
        UpstreamProtocol::ChatCompletions => "/chat/completions",
        UpstreamProtocol::AnthropicMessages => "/v1/messages",
        UpstreamProtocol::GeminiGenerateContent => ":generateContent",
    }
}

fn parse_service_base(base_url: &str) -> Result<Url, String> {
    let invalid = || "服务地址必须是无凭据、查询参数或片段的完整 HTTP(S) 地址".to_string();
    let authority = base_url.split_once("://").map(|(_, rest)| rest);
    if base_url.trim() != base_url
        || base_url.chars().any(char::is_control)
        || base_url.contains('\\')
        || authority.is_none_or(|rest| rest.is_empty() || rest.starts_with('/'))
    {
        return Err(invalid());
    }
    let url = Url::parse(base_url).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url)
}

fn parse_base(base_url: &str, protocol: UpstreamProtocol) -> Result<Url, String> {
    let url = parse_service_base(base_url)?;
    if url
        .path()
        .trim_end_matches('/')
        .ends_with(request_path(protocol))
    {
        return Err(format!(
            "服务地址应填写{}，不要包含完整请求端点 {}",
            if protocol == UpstreamProtocol::AnthropicMessages {
                "服务根地址"
            } else {
                "API 根地址（含服务商要求的前缀）"
            },
            request_path(protocol),
        ));
    }
    // The Anthropic contract takes the service root: `/v1` belongs to the
    // request path. A version-prefixed base would silently double to
    // `/v1/v1/messages`, so it is rejected instead of guessed away.
    if protocol == UpstreamProtocol::AnthropicMessages
        && version_prefix_root(url.path()).is_some()
    {
        return Err("服务地址应填写服务根地址，不要包含 /v1 版本前缀".into());
    }
    Ok(url)
}

fn version_prefix_root(path: &str) -> Option<&str> {
    path.trim_end_matches('/').strip_suffix("/v1")
}

/// Repairs one terminal `/v1` on an otherwise valid Anthropic service base.
/// The original URL syntax and the resulting root use the normal validators;
/// unrelated malformed inputs must not be normalized by repair.
pub fn strip_anthropic_version_prefix(base_url: &str) -> Option<String> {
    let mut url = parse_service_base(base_url).ok()?;
    let path = version_prefix_root(url.path())?.to_owned();
    url.set_path(if path.is_empty() { "/" } else { &path });
    let root = url.to_string();
    validate_base_url(&root, UpstreamProtocol::AnthropicMessages).ok()?;
    Some(root)
}

fn parse_full_url(endpoint: &str) -> Result<String, String> {
    let invalid = || "完整服务地址必须是无凭据、片段或控制字符的 HTTP(S) 地址".to_string();
    if endpoint.trim() != endpoint
        || endpoint.chars().any(char::is_control)
        || endpoint.contains('\\')
    {
        return Err(invalid());
    }
    let url = Url::parse(endpoint).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url.to_string())
}

fn append(base_url: &str, protocol: UpstreamProtocol, suffix: &str) -> Result<String, String> {
    let mut url = parse_base(base_url, protocol)?;
    let path = format!("{}{suffix}", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_paths_preserve_the_declared_api_prefix_without_guessing_versions() {
        for (base, prefix) in [
            ("https://example.test", "https://example.test"),
            ("https://example.test/", "https://example.test"),
            ("https://example.test/v1/", "https://example.test/v1"),
            ("https://example.test/v2", "https://example.test/v2"),
            (
                "https://example.test/proxy/openai/",
                "https://example.test/proxy/openai",
            ),
            ("http://[::1]:9876/api", "http://[::1]:9876/api"),
            (
                "https://example.test/tenant%20name",
                "https://example.test/tenant%20name",
            ),
        ] {
            for (protocol, path) in [
                (UpstreamProtocol::Responses, "responses"),
                (UpstreamProtocol::ChatCompletions, "chat/completions"),
            ] {
                assert_eq!(
                    upstream_endpoint(base, protocol).unwrap(),
                    format!("{prefix}/{path}")
                );
                assert_eq!(
                    models_endpoint(base, protocol).unwrap(),
                    format!("{prefix}/models")
                );
            }
        }
    }

    #[test]
    fn anthropic_paths_match_the_client_service_root_contract() {
        let protocol = UpstreamProtocol::AnthropicMessages;
        assert_eq!(
            upstream_endpoint("https://example.test/proxy/", protocol).unwrap(),
            "https://example.test/proxy/v1/messages"
        );
        assert_eq!(
            models_endpoint("https://example.test/proxy/", protocol).unwrap(),
            "https://example.test/proxy/v1/models"
        );
    }

    #[test]
    fn anthropic_version_prefixed_bases_are_rejected_instead_of_doubled() {
        let protocol = UpstreamProtocol::AnthropicMessages;
        for base in ["https://example.test/v1", "https://example.test/v1/"] {
            assert!(validate_base_url(base, protocol).is_err());
            assert!(upstream_endpoint(base, protocol).is_err());
            assert!(models_endpoint(base, protocol).is_err());
        }
        // A non-v1 trailing segment is an explicit proxy prefix, not a
        // doubled version: the declared root is preserved.
        assert_eq!(
            upstream_endpoint("https://example.test/v1beta", protocol).unwrap(),
            "https://example.test/v1beta/v1/messages"
        );
    }

    #[test]
    fn version_prefix_repair_returns_valid_roots_for_current_request_paths() {
        let protocol = UpstreamProtocol::AnthropicMessages;
        for (base, expected_root, prefix) in [
            (
                "https://example.test/v1",
                "https://example.test/",
                "https://example.test",
            ),
            (
                "https://example.test/v1/",
                "https://example.test/",
                "https://example.test",
            ),
            (
                "https://example.test/proxy/v1",
                "https://example.test/proxy",
                "https://example.test/proxy",
            ),
            (
                "https://example.test/proxy/v1///",
                "https://example.test/proxy",
                "https://example.test/proxy",
            ),
        ] {
            assert!(validate_base_url(base, protocol).is_err());
            assert!(upstream_endpoint(base, protocol).is_err());
            assert!(models_endpoint(base, protocol).is_err());
            let root = strip_anthropic_version_prefix(base).unwrap();
            assert_eq!(root, expected_root);
            assert!(validate_base_url(&root, protocol).is_ok());
            assert_eq!(
                upstream_endpoint(&root, protocol).unwrap(),
                format!("{prefix}/v1/messages")
            );
            assert_eq!(
                models_endpoint(&root, protocol).unwrap(),
                format!("{prefix}/v1/models")
            );
        }
        for base in [
            "https://example.test",
            "https://example.test/v1beta",
            "https://example.test/api/v2",
            "https://v1",
        ] {
            assert_eq!(strip_anthropic_version_prefix(base), None);
        }
    }

    #[test]
    fn version_prefix_repair_rejects_other_invalid_url_shapes() {
        for base in [
            " https://example.test/v1",
            "https://example.test/v1 ",
            "https://exa\nmple.test/v1",
            "https://example.test/v1\t",
            "https://example.test\\proxy/v1",
            "https://user:password@example.test/v1",
            "https://example.test/v1?key=value",
            "https://example.test/v1#fragment",
            "https:///example.test/v1",
            "https:example.test/v1",
            "ftp://example.test/v1",
            "https://example.test/v1/v1",
            "https://example.test/v1/messages/v1",
        ] {
            assert!(validate_base_url(base, UpstreamProtocol::AnthropicMessages).is_err());
            assert_eq!(strip_anthropic_version_prefix(base), None, "{base:?}");
        }
    }

    #[test]
    fn request_endpoints_are_not_accepted_as_base_url_aliases() {
        for protocol in [
            UpstreamProtocol::Responses,
            UpstreamProtocol::ChatCompletions,
            UpstreamProtocol::AnthropicMessages,
        ] {
            let endpoint = upstream_endpoint("https://example.test/api", protocol).unwrap();
            assert!(validate_base_url(&endpoint, protocol).is_err());
            assert!(validate_base_url(&format!("{endpoint}/"), protocol).is_err());
        }
    }

    #[test]
    fn compact_endpoint_uses_the_declared_responses_api_root() {
        assert_eq!(
            compact_endpoint("https://example.test/tenant/v1").unwrap(),
            "https://example.test/tenant/v1/responses/compact"
        );
    }

    #[test]
    fn codex_operation_endpoints_are_siblings_of_the_declared_api_root() {
        assert_eq!(
            codex_operation_endpoint("https://example.test/tenant/v1", false, CodexOperation::ImageGeneration).unwrap(),
            "https://example.test/tenant/v1/images/generations"
        );
        assert_eq!(codex_operation_endpoint("https://example.test/tenant/v1/responses?tenant=a", true,
            CodexOperation::ImageGeneration).unwrap(), "https://example.test/tenant/v1/images/generations?tenant=a");
        assert!(codex_operation_endpoint("https://example.test/v1/opaque", true, CodexOperation::ImageGeneration).is_err());
    }

    #[test]
    fn invalid_addresses_are_rejected_without_echoing_sensitive_input() {
        for base in [
            "https://",
            "https:///path",
            "https:example.test",
            "https://example.test\\path",
            "ftp://example.test",
            "https://user:secret@example.test",
            "https://example.test?token=secret",
            "https://example.test#secret",
            " https://example.test",
            "https://example.test/\nsecret",
        ] {
            let error = validate_base_url(base, UpstreamProtocol::Responses).unwrap_err();
            assert!(!error.contains("secret"));
        }
    }

    #[test]
    fn full_urls_are_not_extended_with_protocol_paths() {
        assert_eq!(
            upstream_endpoint_with_options(
                "https://example.test/custom/messages",
                UpstreamProtocol::AnthropicMessages,
                true,
            )
            .unwrap(),
            "https://example.test/custom/messages"
        );
        let full_url = "https://example.test/custom/messages";
        let connection = crate::contracts::ProviderConnectionOptions {
            is_full_url: true,
            ..Default::default()
        };
        assert!(models_endpoint_for_connection(
            full_url,
            UpstreamProtocol::AnthropicMessages,
            &connection,
        )
        .is_err());
        let connection = crate::contracts::ProviderConnectionOptions {
            is_full_url: true,
            models_url: Some("https://example.test/v1/models".into()),
            ..Default::default()
        };
        assert_eq!(
            models_endpoint_for_connection(full_url, UpstreamProtocol::AnthropicMessages, &connection)
                .unwrap(),
            "https://example.test/v1/models"
        );
    }

    #[test]
    fn full_urls_preserve_safe_query_and_reject_credentials_or_fragments() {
        assert_eq!(
            upstream_endpoint_with_options(
                "https://example.test/messages?tenant=one%20two",
                UpstreamProtocol::AnthropicMessages,
                true,
            )
            .unwrap(),
            "https://example.test/messages?tenant=one%20two"
        );
        for endpoint in [
            "https://user:secret@example.test/messages",
            "https://example.test/messages#secret",
        ] {
            let error =
                upstream_endpoint_with_options(endpoint, UpstreamProtocol::AnthropicMessages, true)
                    .unwrap_err();
            assert!(!error.contains("secret"));
        }
    }
}
