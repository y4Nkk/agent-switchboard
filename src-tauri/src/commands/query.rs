use super::error::{blocking, operation_error, state, CommandError};
use crate::probe::ProbeResult;
use asb_core::contracts::{ProviderConnectionOptions, UsageQuery, UsageSummary};
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderEndpointsRequest {
    base_url: String,
    upstream_protocol: asb_core::contracts::UpstreamProtocol,
    #[serde(default)]
    connection: ProviderConnectionOptions,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderEndpoints {
    request_url: String,
    models_url: Option<String>,
    models_error: Option<String>,
}

/// Resolves draft addresses without reading configuration or making a request.
#[tauri::command]
pub fn resolve_provider_endpoints(
    request: ProviderEndpointsRequest,
) -> Result<ProviderEndpoints, CommandError> {
    if let Some(native) = &request.connection.claude_native {
        native
            .validate((!request.base_url.is_empty()).then_some(request.base_url.as_str()))
            .map_err(|error| CommandError::new("provider-endpoint-invalid", error))?;
        return Ok(ProviderEndpoints {
            request_url: "由 Claude 原生云 SDK 按模型构造".into(),
            models_url: None,
            models_error: Some("使用云服务的模型目录；不发送普通 /models 请求".into()),
        });
    }
    let invalid = |error| CommandError::new("provider-endpoint-invalid", error);
    let request_url = if request.upstream_protocol == asb_core::UpstreamProtocol::GeminiGenerateContent {
        asb_core::claude_gemini::request_preview(
            &request.base_url,
            request.connection.is_full_url,
            None,
        )
    } else {
        asb_core::endpoint::upstream_endpoint_with_options(
            &request.base_url,
            request.upstream_protocol,
            request.connection.is_full_url,
        )
    }
    .map_err(invalid)?;
    let (models_url, models_error) = match asb_core::endpoint::models_endpoint_for_connection(
        &request.base_url,
        request.upstream_protocol,
        &request.connection,
    ) {
        Ok(url) => (Some(url), None),
        Err(error) => (None, Some(error)),
    };
    Ok(ProviderEndpoints { request_url, models_url, models_error })
}

#[tauri::command]
pub async fn probe_endpoint(url: String) -> Result<ProbeResult, CommandError> {
    blocking(move || {
        crate::probe::probe(&url).map_err(|error| CommandError::new("probe-failed", error))
    })
    .await
}

/// Models (id plus optional vendor) from the provider's configured
/// API root's model-list endpoint. The current editor draft supplies its API key and
/// protocol and optional authentication override. The
/// key is never included in errors or persisted by this command.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderModelsRequest {
    app: asb_core::AppKind,
    url: String,
    api_key: String,
    upstream_protocol: asb_core::contracts::UpstreamProtocol,
    #[serde(default)]
    connection: ProviderConnectionOptions,
    authentication: Option<asb_core::AuthenticationScheme>,
}

#[tauri::command]
pub async fn fetch_provider_models(
    app: tauri::AppHandle,
    request: ProviderModelsRequest,
) -> Result<Vec<crate::probe::ProviderModel>, CommandError> {
    let local = state(&app)?;
    blocking(move || fetch_models(&local, request)).await
}

fn validate_model_fetch_request(request: &ProviderModelsRequest) -> Result<(), CommandError> {
    if request.app == asb_core::AppKind::Codex
        && request.upstream_protocol == asb_core::UpstreamProtocol::GeminiGenerateContent
    {
        return Err(CommandError::keyed(
            "provider-endpoint-invalid",
            "errors.cfg.geminiNativeCodexUnsupported",
            "Gemini Native 不能用于 Codex 模型列表",
        ));
    }
    if request.connection.claude_native.is_some() {
        return Err(CommandError::keyed(
            "claude-native-sdk-only",
            "errors.cfg.claudeNativeNoModelList",
            "原生云 SDK 不提供此通用 HTTP 模型接口，请填写云服务已开通的模型 ID",
        ));
    }
    Ok(())
}

fn fetch_managed_claude_models(
    local: &crate::local_state::LocalState,
    request: &ProviderModelsRequest,
) -> Result<Option<Vec<crate::probe::ProviderModel>>, CommandError> {
    if request.app != asb_core::AppKind::Claude {
        return Ok(None);
    }
    let account = crate::claude_auth::ClaudeAuth::shared(local.root())
        .resolve(&request.connection)
        .map_err(|message| CommandError::new("claude-account-unavailable", message))?;
    let Some(account) = account else {
        return Ok(None);
    };
    crate::claude_auth::models::fetch_for_provider(
        &account,
        request.upstream_protocol,
        &request.connection,
    )
    .map(Some)
    .map_err(|message| CommandError::new("models-fetch-failed", message))
}

fn fetch_models(
    local: &crate::local_state::LocalState,
    request: ProviderModelsRequest,
) -> Result<Vec<crate::probe::ProviderModel>, CommandError> {
    validate_model_fetch_request(&request)?;
    if let Some(models) = fetch_managed_claude_models(local, &request)? {
        return Ok(models);
    }
    crate::probe::fetch_models(
        &request.url,
        &request.api_key,
        request.upstream_protocol,
        request.authentication,
        &request.connection,
    )
    // The provider diagnostic already redacts credentials. Generic token
    // scrubbing would erase legitimate endpoint URLs and request ids.
    .map_err(|message| CommandError {
        code: "models-fetch-failed",
        message,
        message_key: None,
        params: None,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UsageQueryRequest {
    query: UsageQuery,
    api_key: String,
    base_url: Option<String>,
    upstream_protocol: asb_core::contracts::UpstreamProtocol,
    #[serde(default)]
    connection: ProviderConnectionOptions,
    authentication: Option<asb_core::AuthenticationScheme>,
}

#[tauri::command]
pub async fn test_usage_query(request: UsageQueryRequest) -> Result<UsageSummary, CommandError> {
    blocking(move || {
        crate::usage_query::run_usage_query_with_connection(
            &request.query,
            &request.api_key,
            request.base_url.as_deref(),
            request.upstream_protocol,
            request.authentication,
            &request.connection,
        )
        .map_err(|error| CommandError::new("usage-query-failed", error))
    })
    .await
}

/// Runs the persisted query of one provider and records the successful
/// credential-free summary for the custom tray panel. This is the forced,
/// immediate path (the refresh button); scheduled queries share the same
/// executor. The renderer passes only the client and stable profile id;
/// this boundary owns the query and API key.
#[tauri::command]
pub async fn query_profile_usage(
    app: tauri::AppHandle,
    target: asb_core::contracts::AppKind,
    profile_id: String,
) -> Result<(), CommandError> {
    let state = state(&app)?;
    let gate = app.state::<super::ConfigWriteGate>().inner().clone();
    let result = blocking(move || {
        crate::usage_query::scheduler::execute_once(&state, target, &profile_id, true, &gate)
            .map(|_| ())
    })
    .await;
    crate::tray::refresh(&app);
    result
}

/// Reads the last completed query, including failure and retained readings.
/// `null` means this query has never completed. No network access.
#[tauri::command]
pub async fn read_profile_usage(
    app: tauri::AppHandle,
    target: asb_core::contracts::AppKind,
    profile_id: String,
) -> Result<Option<asb_core::contracts::UsageSnapshot>, CommandError> {
    let state = state(&app)?;
    let gate = app.state::<super::ConfigWriteGate>().inner().clone();
    gate.run(move || {
        let profile = crate::usage_query::scheduler::usage_profile(&state, target, &profile_id)
            .map_err(|error| operation_error("profile-not-found", error))?;
        crate::usage_cache::get(&state, &profile)
            .map_err(|error| CommandError::new("usage-cache-read-failed", error))
    })
    .await
}

#[cfg(test)]
mod endpoint_tests {
    use super::*;

    #[test]
    fn endpoint_preview_uses_the_same_api_root_for_requests_and_model_discovery() {
        let result = resolve_provider_endpoints(ProviderEndpointsRequest {
            base_url: "https://example.test/openai/v2/".into(),
            upstream_protocol: asb_core::contracts::UpstreamProtocol::Responses,
            connection: Default::default(),
        })
        .unwrap();
        assert_eq!(
            result.request_url,
            "https://example.test/openai/v2/responses"
        );
        assert_eq!(result.models_url.as_deref(), Some("https://example.test/openai/v2/models"));
        assert_eq!(result.models_error, None);
    }

    #[test]
    fn full_request_url_reports_missing_model_list_url_without_invalidating_request_preview() {
        let result = resolve_provider_endpoints(ProviderEndpointsRequest {
            base_url: "https://example.test/v1/responses".into(),
            upstream_protocol: asb_core::contracts::UpstreamProtocol::Responses,
            connection: ProviderConnectionOptions {
                is_full_url: true,
                ..Default::default()
            },
        })
        .unwrap();
        assert_eq!(result.request_url, "https://example.test/v1/responses");
        assert_eq!(result.models_url, None);
        assert_eq!(
            result.models_error.as_deref(),
            Some("完整请求 URL 不能推导模型列表地址；请填写模型列表 URL"),
        );
    }

    #[test]
    fn endpoint_preview_rejects_credentials_and_obsolete_endpoint_aliases() {
        for base in [
            "https://user:secret@example.test",
            "https://example.test/v1/responses",
        ] {
            let error = resolve_provider_endpoints(ProviderEndpointsRequest {
                base_url: base.into(),
                upstream_protocol: asb_core::contracts::UpstreamProtocol::Responses,
                connection: Default::default(),
            })
            .unwrap_err();
            assert_eq!(error.code, "provider-endpoint-invalid");
            assert!(!error.message.contains("secret"));
        }
        assert!(serde_json::from_value::<ProviderEndpointsRequest>(serde_json::json!({
            "baseUrl": "https://example.test", "upstreamProtocol": "responses", "apiKey": "secret"
        })).is_err());
    }

    #[test]
    fn model_list_command_keeps_upstream_details_without_generic_scrubbing() {
        let upstream = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}/tenant/openai", upstream.server_addr());
        let server = std::thread::spawn(move || {
            let request = upstream
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
                .unwrap();
            let path = request.url().to_string();
            request
                .respond(
                    tiny_http::Response::from_string(
                        "model-list access denied; isolated-model-key",
                    )
                    .with_status_code(401)
                    .with_header(
                        tiny_http::Header::from_bytes(
                            "X-Request-Id",
                            "upstream_request_abcdefghijklmnopqrstuvwxyz",
                        )
                        .unwrap(),
                    ),
                )
                .unwrap();
            path
        });
        let directory = tempfile::tempdir().unwrap();
        let local = crate::local_state::LocalState::from_root(directory.path().join("state"));
        let error = fetch_models(
            &local,
            ProviderModelsRequest {
                app: asb_core::AppKind::Claude,
                authentication: None,
                url: base.clone(),
                api_key: "isolated-model-key".into(),
                upstream_protocol: asb_core::contracts::UpstreamProtocol::Responses,
                connection: Default::default(),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "models-fetch-failed");
        assert!(error.message.contains("HTTP 401"));
        assert!(error.message.contains(&format!("{base}/models")));
        assert!(error
            .message
            .contains("upstream_request_abcdefghijklmnopqrstuvwxyz"));
        assert!(error.message.contains("model-list access denied"));
        assert!(!error.message.contains("isolated-model-key"));
        assert_eq!(server.join().unwrap(), "/tenant/openai/models");
    }
}
