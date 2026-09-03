//! Merge-writes the native credential files produced by the official login
//! flows. Tokens exist here only as call arguments; every error is a fixed
//! string so no token, response body, or file content can reach diagnostics.

use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde_json::{json, Map, Value};

use crate::config_store::write_json_atomic;
use asb_switch::sha256_digest;

/// Claude Code stores the plan label next to the token; the community-verified
/// login writes write "max" and Claude Code refreshes/normalizes it itself.
const CLAUDE_SUBSCRIPTION_TYPE: &str = "max";

const SERIALIZE_ERROR: &str = "登录凭据无法序列化";
const WRITE_ERROR: &str = "登录凭据写入失败";
const EXISTING_READ_ERROR: &str = "无法读取现有登录缓存";

/// Tokens from one Codex token exchange. Every field is a credential, so the
/// debug output hides all of them.
pub(crate) struct CodexTokens {
    pub(crate) id_token: Option<String>,
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
}

impl std::fmt::Debug for CodexTokens {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexTokens")
            .field("id_token", &asb_core::redact::REDACTED)
            .field("access_token", &asb_core::redact::REDACTED)
            .field("refresh_token", &asb_core::redact::REDACTED)
            .finish()
    }
}

/// Tokens from one Claude token exchange.
pub(crate) struct ClaudeTokens {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) expires_in: i64,
    pub(crate) scope: String,
}

impl std::fmt::Debug for ClaudeTokens {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClaudeTokens")
            .field("access_token", &asb_core::redact::REDACTED)
            .field("refresh_token", &asb_core::redact::REDACTED)
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .finish()
    }
}

/// Merges the Codex login cache, preserving unknown sibling keys the host may
/// keep. The result always parses as the shape `codex_official_quota` reads.
pub(crate) fn write_codex_auth(
    path: &Path,
    tokens: &CodexTokens,
    account_id: Option<&str>,
) -> Result<(), String> {
    let mut root = existing_object(path)?;
    root.insert("OPENAI_API_KEY".to_string(), Value::Null);
    root.insert("auth_mode".to_string(), json!("chatgpt"));
    let mut token_object = Map::new();
    if let Some(id_token) = tokens.id_token.as_deref() {
        token_object.insert("id_token".to_string(), json!(id_token));
    }
    token_object.insert("access_token".to_string(), json!(tokens.access_token));
    token_object.insert("refresh_token".to_string(), json!(tokens.refresh_token));
    if let Some(account_id) = account_id {
        token_object.insert("account_id".to_string(), json!(account_id));
    }
    root.insert("tokens".to_string(), Value::Object(token_object));
    root.insert("last_refresh".to_string(), json!(rfc3339_now()));
    write_object(path, root)
}

/// Merges the Claude login cache under `claudeAiOauth`, preserving unknown
/// sibling keys Claude Code may keep in the same file.
pub(crate) fn write_claude_credentials(path: &Path, tokens: &ClaudeTokens) -> Result<(), String> {
    let mut root = existing_object(path)?;
    let expires_at = chrono::Utc::now().timestamp_millis() + tokens.expires_in.saturating_mul(1000);
    let scopes: Vec<String> = tokens
        .scope
        .split_whitespace()
        .map(str::to_string)
        .collect();
    root.insert(
        "claudeAiOauth".to_string(),
        json!({
            "accessToken": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "expiresAt": expires_at,
            "scopes": scopes,
            "subscriptionType": CLAUDE_SUBSCRIPTION_TYPE,
        }),
    );
    write_object(path, root)
}

/// Decodes the middle claim segment of one JWT without verifying a signature;
/// the token was received directly over TLS from the vendor token endpoint.
pub(crate) fn jwt_payload(id_token: &str) -> Option<Value> {
    let (_, rest) = id_token.split_once('.')?;
    let (claims, _) = rest.split_once('.')?;
    let bytes = URL_SAFE_NO_PAD.decode(claims).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Extracts the ChatGPT account id from the known claim shapes of a Codex
/// `id_token`. A miss degrades gracefully: quota queries simply omit the
/// account header.
pub(crate) fn account_id_from_jwt(payload: &Value) -> Option<String> {
    payload
        .get("auth")
        .and_then(|auth| auth.get("account_id"))
        .and_then(Value::as_str)
        .or_else(|| payload.get("chatgpt_account_id").and_then(Value::as_str))
        .map(str::to_string)
}

/// One PKCE code verifier: 32 entropy bytes (two UUIDv4s) base64url-encoded to
/// the 43-character minimum of RFC 7636.
pub(crate) fn random_verifier() -> String {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The S256 challenge for one verifier.
pub(crate) fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(sha256_digest(verifier))
}

/// One OAuth `state` nonce.
pub(crate) fn random_state() -> String {
    URL_SAFE_NO_PAD.encode(uuid::Uuid::new_v4().as_bytes())
}

fn write_object(path: &Path, object: Map<String, Value>) -> Result<(), String> {
    let rendered =
        serde_json::to_string_pretty(&Value::Object(object)).map_err(|_| SERIALIZE_ERROR)?;
    write_json_atomic(path, &rendered).map_err(|_| WRITE_ERROR.to_string())
}

/// Reads the current cache as a JSON object. A missing or corrupt file starts
/// from an empty object (the host would reject it anyway); any other read
/// failure aborts the write instead of silently replacing the cache.
fn existing_object(path: &Path) -> Result<Map<String, Value>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => Ok(map),
            _ => Ok(Map::new()),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(_) => Err(EXISTING_READ_ERROR.to_string()),
    }
}

fn rfc3339_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn base64url_decode(value: &str) -> Vec<u8> {
        URL_SAFE_NO_PAD.decode(value).expect("base64url value")
    }

    fn codex_tokens() -> CodexTokens {
        CodexTokens {
            id_token: Some("id-token".to_string()),
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
        }
    }

    #[test]
    fn codex_write_preserves_unknown_sibling_keys_and_repairs_corrupt_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("auth.json");
        fs::write(&path, "{\"host_note\":\"keep\"}").expect("seed");

        write_codex_auth(&path, &codex_tokens(), None).expect("write");
        let merged: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("valid json");
        assert_eq!(merged.get("host_note"), Some(&json!("keep")));
        assert!(merged.get("tokens").expect("tokens")["account_id"].is_null());

        fs::write(&path, "{corrupt").expect("seed corrupt");
        write_codex_auth(&path, &codex_tokens(), None).expect("repairing write");
        serde_json::from_str::<Value>(&fs::read_to_string(&path).expect("read"))
            .expect("repaired file parses");
    }

    #[test]
    fn claude_write_replaces_the_oauth_object_and_keeps_sibling_keys() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join(".credentials.json");
        fs::write(
            &path,
            "{\"claudeAiOauth\":{\"accessToken\":\"old\"},\"host_note\":\"keep\"}",
        )
        .expect("seed");

        write_claude_credentials(
            &path,
            &ClaudeTokens {
                access_token: "at".to_string(),
                refresh_token: "rt".to_string(),
                expires_in: 3600,
                scope: "user:profile user:inference".to_string(),
            },
        )
        .expect("write");

        let merged: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("valid json");
        assert_eq!(merged.get("host_note"), Some(&json!("keep")));
        let oauth = merged.get("claudeAiOauth").expect("oauth object");
        assert_eq!(oauth["accessToken"], json!("at"));
        assert_eq!(oauth["refreshToken"], json!("rt"));
        assert_eq!(oauth["subscriptionType"], json!("max"));
        assert_eq!(oauth["scopes"], json!(["user:profile", "user:inference"]));
        assert!(oauth["expiresAt"].as_i64().expect("epoch millis") > 0);
    }

    #[test]
    fn claude_write_fails_closed_when_the_existing_cache_is_unreadable() {
        let directory = tempfile::tempdir().expect("temporary directory");
        // A directory at the cache path makes the existing-file read fail with
        // a non-NotFound error, which must abort the write instead of
        // silently replacing the cache.
        let blocked = directory.path().join(".credentials.json");
        std::fs::create_dir(&blocked).expect("seed directory");

        let outcome = write_claude_credentials(
            &blocked,
            &ClaudeTokens {
                access_token: "at".to_string(),
                refresh_token: "rt".to_string(),
                expires_in: 3600,
                scope: String::new(),
            },
        );

        assert_eq!(outcome, Err(EXISTING_READ_ERROR.to_string()));
        assert!(blocked.is_dir());
    }

    #[test]
    fn jwt_payload_and_account_id_support_the_known_claim_shapes() {
        let payload = json!({"auth": {"account_id": "acc-1"}, "email": "a@b.c"});
        let segment = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("serialize"));
        let id_token = format!("header.{segment}.signature");

        let decoded = jwt_payload(&id_token).expect("payload");
        assert_eq!(account_id_from_jwt(&decoded).as_deref(), Some("acc-1"));

        let namespaced = json!({"chatgpt_account_id": "acc-2"});
        assert_eq!(account_id_from_jwt(&namespaced).as_deref(), Some("acc-2"));
        assert_eq!(account_id_from_jwt(&json!({})), None);
        assert_eq!(jwt_payload("not-a-jwt"), None);
    }

    #[test]
    fn pkce_and_state_meet_the_rfc7636_shapes() {
        let verifier = random_verifier();
        assert_eq!(verifier.len(), 43);
        assert_eq!(
            base64url_decode(&code_challenge(&verifier)),
            sha256_digest(&verifier)
        );

        let other = random_verifier();
        assert_ne!(verifier, other);
        assert_eq!(base64url_decode(&random_state()).len(), 16);
    }
}
