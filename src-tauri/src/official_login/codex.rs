//! The Codex (ChatGPT) official login: the OpenAI device-code flow used by
//! the public Codex CLI client. The PKCE verifier is issued by the OpenAI
//! device endpoint, so nothing is generated client-side here.

use serde::Deserialize;

use crate::official_login::credentials::{account_id_from_jwt, jwt_payload, CodexTokens};
use crate::official_login::{percent_encode, USER_AGENT};
use crate::probe::http_request;

pub(crate) const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// The user-facing page where the device code is entered.
pub(crate) const DEVICE_VERIFICATION_URL: &str = "https://auth.openai.com/codex/device";
/// The redirect the issued code is bound to; it mirrors the public Codex CLI
/// client registration rather than pointing anywhere local.
const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";

const UNRECOGNIZED_RESPONSE: &str = "登录服务响应无法识别";
const REJECTED_BY_SERVER: &str = "登录服务拒绝了该设备的登录请求";
const EXCHANGE_REJECTED: &str = "登录凭据交换被拒绝";

/// Endpoints as a value so tests point the flow at loopback fakes.
pub(crate) struct CodexOAuthEndpoints {
    pub(crate) usercode_url: String,
    pub(crate) poll_url: String,
    pub(crate) token_url: String,
}

impl Default for CodexOAuthEndpoints {
    fn default() -> Self {
        Self {
            usercode_url: "https://auth.openai.com/api/accounts/deviceauth/usercode".to_string(),
            poll_url: "https://auth.openai.com/api/accounts/deviceauth/token".to_string(),
            token_url: "https://auth.openai.com/oauth/token".to_string(),
        }
    }
}

#[derive(Deserialize)]
struct DeviceCodeBody {
    device_auth_id: Option<String>,
    user_code: Option<String>,
}

#[derive(Deserialize)]
struct DevicePollBody {
    authorization_code: Option<String>,
    code_verifier: Option<String>,
}

#[derive(Deserialize)]
struct TokenBody {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
}

/// One started device-code session: the id used for polling and the code the
/// user enters on the verification page.
pub(crate) struct CodexDeviceCode {
    pub(crate) device_auth_id: String,
    pub(crate) user_code: String,
}

/// One poll step. Transport errors and server hiccups stay pending because a
/// three-second UI poll must survive them; the session expiry bounds the wait.
pub(crate) enum CodexPollOutcome {
    Pending,
    Completed {
        tokens: CodexTokens,
        account_id: Option<String>,
    },
}

/// Asks OpenAI for a device code.
pub(crate) fn request_device_code(
    endpoints: &CodexOAuthEndpoints,
) -> Result<CodexDeviceCode, String> {
    let (status, body) = post_json(endpoints.usercode_url.as_str(), &json_body(CODEX_CLIENT_ID))?;
    if status != 200 {
        return Err(REJECTED_BY_SERVER.to_string());
    }
    let parsed: DeviceCodeBody = serde_json::from_str(&body).map_err(|_| UNRECOGNIZED_RESPONSE)?;
    let device_auth_id = parsed.device_auth_id.filter(|value| !value.is_empty());
    let user_code = parsed.user_code.filter(|value| !value.is_empty());
    match (device_auth_id, user_code) {
        (Some(device_auth_id), Some(user_code)) => Ok(CodexDeviceCode {
            device_auth_id,
            user_code,
        }),
        _ => Err(UNRECOGNIZED_RESPONSE.to_string()),
    }
}

/// Runs one poll step and, once the server approves the device, immediately
/// exchanges the issued code for tokens.
pub(crate) fn poll_login(
    endpoints: &CodexOAuthEndpoints,
    device_auth_id: &str,
    user_code: &str,
) -> Result<CodexPollOutcome, String> {
    let payload = serde_json::json!({
        "device_auth_id": device_auth_id,
        "user_code": user_code,
    });
    let (status, body) = match post_json(endpoints.poll_url.as_str(), &payload) {
        Ok(outcome) => outcome,
        // One dropped request must not kill a ten-minute login.
        Err(_) => return Ok(CodexPollOutcome::Pending),
    };
    match status {
        200 => {}
        // Still waiting on the user.
        403 | 404 => return Ok(CodexPollOutcome::Pending),
        // Temporary server trouble stays pending, bounded by session expiry.
        status if status >= 500 => return Ok(CodexPollOutcome::Pending),
        400..=499 => return Err(REJECTED_BY_SERVER.to_string()),
        _ => return Err(UNRECOGNIZED_RESPONSE.to_string()),
    }
    let parsed: DevicePollBody = serde_json::from_str(&body).map_err(|_| UNRECOGNIZED_RESPONSE)?;
    let (Some(authorization_code), Some(code_verifier)) =
        (parsed.authorization_code, parsed.code_verifier)
    else {
        return Err(UNRECOGNIZED_RESPONSE.to_string());
    };

    let form = [
        ("grant_type", "authorization_code"),
        ("code", authorization_code.as_str()),
        ("code_verifier", code_verifier.as_str()),
        ("redirect_uri", DEVICE_REDIRECT_URI),
        ("client_id", CODEX_CLIENT_ID),
    ]
    .map(|(key, value)| format!("{key}={}", percent_encode(value)))
    .join("&");
    let headers =
        format!("Content-Type: application/x-www-form-urlencoded\r\nUser-Agent: {USER_AGENT}\r\n");
    let (status, body) = http_request(
        "POST",
        endpoints.token_url.as_str(),
        &headers,
        form.as_bytes(),
    )
    .map_err(|_| EXCHANGE_REJECTED.to_string())?;
    if status != 200 {
        return Err(EXCHANGE_REJECTED.to_string());
    }
    let parsed: TokenBody = serde_json::from_str(&body).map_err(|_| UNRECOGNIZED_RESPONSE)?;
    let (Some(access_token), Some(refresh_token)) = (parsed.access_token, parsed.refresh_token)
    else {
        return Err(UNRECOGNIZED_RESPONSE.to_string());
    };
    let account_id = parsed
        .id_token
        .as_deref()
        .and_then(jwt_payload)
        .as_ref()
        .and_then(account_id_from_jwt);
    Ok(CodexPollOutcome::Completed {
        tokens: CodexTokens {
            id_token: parsed.id_token,
            access_token,
            refresh_token,
        },
        account_id,
    })
}

fn json_body(client_id: &str) -> serde_json::Value {
    serde_json::json!({ "client_id": client_id })
}

fn post_json(url: &str, payload: &serde_json::Value) -> Result<(u16, String), String> {
    let headers =
        format!("Content-Type: application/json\r\nAccept: application/json\r\nUser-Agent: {USER_AGENT}\r\n");
    http_request("POST", url, &headers, payload.to_string().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::official_login::fake_oauth::FakeServer;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;

    fn endpoints(server: &FakeServer) -> CodexOAuthEndpoints {
        CodexOAuthEndpoints {
            usercode_url: format!("http://{}/usercode", server.address),
            poll_url: format!("http://{}/poll", server.address),
            token_url: format!("http://{}/token", server.address),
        }
    }

    fn body(id_token_claims: &str) -> Vec<String> {
        vec![
            "{\"device_auth_id\":\"device-1\",\"user_code\":\"CODE-1234\"}".to_string(),
            ":404:{}".to_string(),
            "{\"authorization_code\":\"auth-code\",\"code_verifier\":\"server-verifier\"}"
                .to_string(),
            format!(
                "{{\"access_token\":\"access-token\",\"refresh_token\":\"refresh-token\",\"id_token\":\"h.{id_token_claims}.s\"}}"
            ),
        ]
    }

    #[test]
    fn device_flow_requests_match_the_openai_shape_and_completes() {
        let claims = URL_SAFE_NO_PAD.encode("{\"auth\":{\"account_id\":\"acc-9\"}}");
        let id_token = format!("h.{claims}.s");
        let server = FakeServer::start(&body(&claims));
        let endpoints = endpoints(&server);

        let device = request_device_code(&endpoints).expect("device code");
        assert_eq!(device.user_code, "CODE-1234");

        assert!(matches!(
            poll_login(&endpoints, &device.device_auth_id, &device.user_code).expect("poll"),
            CodexPollOutcome::Pending
        ));
        let outcome = poll_login(&endpoints, &device.device_auth_id, &device.user_code)
            .expect("poll reaches completion");
        let CodexPollOutcome::Completed { tokens, account_id } = outcome else {
            panic!("expected completion");
        };
        assert_eq!(tokens.access_token, "access-token");
        assert_eq!(tokens.refresh_token, "refresh-token");
        assert_eq!(tokens.id_token.as_deref(), Some(id_token.as_str()));
        assert_eq!(account_id.as_deref(), Some("acc-9"));

        let usercode_request = server.request(0);
        assert!(usercode_request.starts_with("POST /usercode HTTP/1.1\r\n"));
        assert!(usercode_request.contains("{\"client_id\":\"app_EMoamEEZ73f0CkXaXp7hrann\"}"));
        let first_poll = server.request(1);
        assert!(
            first_poll.contains("{\"device_auth_id\":\"device-1\",\"user_code\":\"CODE-1234\"}")
        );
        server.request(2);
        let exchange = server.request(3);
        assert!(exchange.starts_with("POST /token HTTP/1.1\r\n"));
        assert!(exchange.contains("grant_type=authorization_code"));
        assert!(exchange.contains("code=auth-code"));
        assert!(exchange.contains("code_verifier=server-verifier"));
        assert!(exchange.contains(&format!(
            "redirect_uri={}",
            percent_encode(DEVICE_REDIRECT_URI)
        )));
        assert!(exchange.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"));
    }

    #[test]
    fn pending_and_transient_statuses_stay_pending_while_rejection_fails() {
        let server = FakeServer::start(&[
            "{\"device_auth_id\":\"device-1\",\"user_code\":\"CODE-1234\"}".to_string(),
            ":403:{}".to_string(),
            ":500:{}".to_string(),
            "{\"error\":\"unexpected\"}".to_string(),
            ":400:{\"error\":\"expired_token\"}".to_string(),
        ]);
        let endpoints = endpoints(&server);

        let device = request_device_code(&endpoints).expect("device code");
        assert!(matches!(
            poll_login(&endpoints, &device.device_auth_id, &device.user_code).expect("poll"),
            CodexPollOutcome::Pending
        ));
        assert!(matches!(
            poll_login(&endpoints, &device.device_auth_id, &device.user_code).expect("poll"),
            CodexPollOutcome::Pending
        ));
        assert!(matches!(
            poll_login(&endpoints, &device.device_auth_id, &device.user_code),
            Err(message) if message == UNRECOGNIZED_RESPONSE
        ));
        assert!(matches!(
            poll_login(&endpoints, &device.device_auth_id, &device.user_code),
            Err(message) if message == REJECTED_BY_SERVER
        ));
    }

    #[test]
    fn incomplete_responses_are_rejected_with_fixed_messages() {
        let server = FakeServer::start(&["{\"user_code\":\"CODE\"}".to_string()]);
        assert!(matches!(
            request_device_code(&endpoints(&server)),
            Err(message) if message == UNRECOGNIZED_RESPONSE
        ));

        let partial_exchange = FakeServer::start(&[
            "{\"device_auth_id\":\"device-1\",\"user_code\":\"CODE-1234\"}".to_string(),
            "{\"authorization_code\":\"auth-code\",\"code_verifier\":\"v\"}".to_string(),
            "{\"access_token\":\"only-access\"}".to_string(),
        ]);
        let endpoints = endpoints(&partial_exchange);
        let device = request_device_code(&endpoints).expect("device code");
        assert!(matches!(
            poll_login(&endpoints, &device.device_auth_id, &device.user_code),
            Err(message) if message == UNRECOGNIZED_RESPONSE
        ));
    }
}
