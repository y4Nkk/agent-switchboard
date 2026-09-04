//! Encrypted, user-owned Supabase backups for the application profile store.
//!
//! This module owns the only remote-backup contract: Supabase authenticates
//! the user and enforces row ownership, while an independent user password
//! derives the AES-GCM key that protects the complete profile store before it
//! leaves this device. No Supabase secret/service key, sign-in password, or
//! access token is persisted locally.

use crate::config_store::snapshot::{
    enable_snapshot, read_configuration_snapshot, ConfigurationSnapshot,
};
use crate::local_state::{CloudBackupSettings, LocalState};
use crate::probe::http_request;
use aes_gcm::aead::{
    rand_core::{OsRng, RngCore},
    Aead, KeyInit, Payload,
};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

const TABLE: &str = "agent_switchboard_cloud_backups";
const ENCRYPTION_VERSION: u8 = 1;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 12;
const KEY_LENGTH: usize = 32;
const AAD: &[u8] = b"agent-switchboard-cloud-backup-v1";

/// SQL the user runs once in their own Supabase SQL Editor. It deliberately
/// grants only the operations this client needs and leaves no delete path.
pub const SETUP_SQL: &str = r#"create table if not exists public.agent_switchboard_cloud_backups (
  user_id uuid primary key references auth.users(id) on delete cascade,
  payload jsonb not null,
  updated_at timestamptz not null default now()
);

alter table public.agent_switchboard_cloud_backups enable row level security;
revoke all on table public.agent_switchboard_cloud_backups from anon;
grant select, insert, update on table public.agent_switchboard_cloud_backups to authenticated;

drop policy if exists "read own encrypted backup" on public.agent_switchboard_cloud_backups;
create policy "read own encrypted backup"
on public.agent_switchboard_cloud_backups
for select to authenticated
using ((select auth.uid()) = user_id);

drop policy if exists "insert own encrypted backup" on public.agent_switchboard_cloud_backups;
create policy "insert own encrypted backup"
on public.agent_switchboard_cloud_backups
for insert to authenticated
with check ((select auth.uid()) = user_id);

drop policy if exists "update own encrypted backup" on public.agent_switchboard_cloud_backups;
create policy "update own encrypted backup"
on public.agent_switchboard_cloud_backups
for update to authenticated
using ((select auth.uid()) = user_id)
with check ((select auth.uid()) = user_id);"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudBackupResult {
    pub updated_at: String,
    pub profile_count: usize,
}

#[derive(Debug, Serialize)]
struct UploadRecord<'a> {
    user_id: &'a str,
    payload: &'a EncryptedBackup,
    updated_at: &'a str,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    #[serde(rename = "access_token")]
    access_token: String,
    user: AuthUser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct RemoteRecord {
    payload: EncryptedBackup,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EncryptedBackup {
    version: u8,
    salt: String,
    nonce: String,
    ciphertext: String,
}

struct AuthenticatedUser {
    id: String,
    access_token: String,
}

/// Returns the persisted public connection coordinates, if the user has
/// configured their own Supabase project.
pub fn settings(state: &LocalState) -> Result<Option<CloudBackupSettings>, String> {
    state.get_cloud_backup_settings()
}

/// Stores connection coordinates only. Passwords are command arguments and
/// intentionally never enter this local state file.
pub fn save_settings(state: &LocalState, settings: CloudBackupSettings) -> Result<(), String> {
    state.set_cloud_backup_settings(&settings)
}

/// Validates the current, unsaved connection draft without creating or
/// replacing a remote backup. A successful result proves that the configured
/// Auth user can read only its own cloud-backup row.
pub fn test_connection(
    settings: &CloudBackupSettings,
    account_password: &str,
) -> Result<(), String> {
    test_connection_with_request(settings, account_password, http_request)
}

pub fn upload(
    state: &LocalState,
    account_password: &str,
    backup_password: &str,
) -> Result<CloudBackupResult, String> {
    upload_with_request(state, account_password, backup_password, &http_request)
}

fn upload_with_request<F>(
    state: &LocalState,
    account_password: &str,
    backup_password: &str,
    request: &F,
) -> Result<CloudBackupResult, String>
where
    F: Fn(&str, &str, &str, &[u8]) -> Result<(u16, String), String>,
{
    let settings = configured_settings(state)?;
    validate_passwords(account_password, backup_password)?;
    let user = authenticate_with_request(&settings, account_password, request)?;
    let snapshot =
        read_configuration_snapshot(&state.configuration()).map_err(|error| error.to_string())?;
    let payload = encrypt(&snapshot, backup_password)?;
    let updated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let body = serde_json::to_vec(&UploadRecord {
        user_id: &user.id,
        payload: &payload,
        updated_at: &updated_at,
    })
    .map_err(|_| "云端备份序列化失败".to_string())?;
    let headers = data_headers(
        &settings,
        &user.access_token,
        "resolution=merge-duplicates,return=minimal",
    );
    let url = format!(
        "{}/rest/v1/{}?on_conflict=user_id",
        settings.project_url, TABLE
    );
    let (status, _) = request("POST", &url, &headers, &body)?;
    if status != 200 && status != 201 {
        return Err(remote_table_error(status));
    }
    Ok(CloudBackupResult {
        updated_at,
        profile_count: snapshot.provider_count(),
    })
}

pub fn restore(
    state: &LocalState,
    account_password: &str,
    backup_password: &str,
) -> Result<CloudBackupResult, String> {
    restore_with_request(state, account_password, backup_password, &http_request)
}

fn restore_with_request<F>(
    state: &LocalState,
    account_password: &str,
    backup_password: &str,
    request: &F,
) -> Result<CloudBackupResult, String>
where
    F: Fn(&str, &str, &str, &[u8]) -> Result<(u16, String), String>,
{
    let settings = configured_settings(state)?;
    validate_passwords(account_password, backup_password)?;
    let user = authenticate_with_request(&settings, account_password, request)?;
    let headers = data_headers(&settings, &user.access_token, "return=representation");
    let url = format!(
        "{}/rest/v1/{}?select=payload,updated_at&user_id=eq.{}&limit=1",
        settings.project_url, TABLE, user.id
    );
    let (status, body) = request("GET", &url, &headers, &[])?;
    if status != 200 {
        return Err(remote_table_error(status));
    }
    let records: Vec<RemoteRecord> = decode_json(&body, "云端备份响应无效")?;
    let record = records
        .into_iter()
        .next()
        .ok_or_else(|| "云端没有可恢复的备份".to_string())?;
    let mut cleartext = decrypt(&record.payload, backup_password)?;
    let restored = restore_snapshot(&state, &cleartext);
    cleartext.fill(0);
    let snapshot = restored?;
    Ok(CloudBackupResult {
        updated_at: record.updated_at,
        profile_count: snapshot.provider_count(),
    })
}

/// Restores a decrypted snapshot through the same staging, verification, and
/// directory-swap path as the legacy migration. The live layout survives a
/// failed restore untouched.
fn restore_snapshot(state: &LocalState, cleartext: &[u8]) -> Result<ConfigurationSnapshot, String> {
    let snapshot: ConfigurationSnapshot = serde_json::from_slice(cleartext)
        .map_err(|_| "云端备份不是当前支持的配置数据格式".to_string())?;
    enable_snapshot(&state.configuration(), &snapshot)?;
    Ok(snapshot)
}

fn configured_settings(state: &LocalState) -> Result<CloudBackupSettings, String> {
    state
        .get_cloud_backup_settings()?
        .ok_or_else(|| "请先保存 Supabase 云端备份设置".to_string())
}

fn validate_passwords(account_password: &str, backup_password: &str) -> Result<(), String> {
    validate_account_password(account_password)?;
    if backup_password.len() < 8 {
        return Err("备份密码至少需要 8 个字符".to_string());
    }
    Ok(())
}

fn validate_account_password(account_password: &str) -> Result<(), String> {
    if account_password.is_empty() {
        return Err("请输入项目 Auth 登录密码".to_string());
    }
    Ok(())
}

fn test_connection_with_request<F>(
    settings: &CloudBackupSettings,
    account_password: &str,
    request: F,
) -> Result<(), String>
where
    F: Fn(&str, &str, &str, &[u8]) -> Result<(u16, String), String>,
{
    settings.validate()?;
    validate_account_password(account_password)?;
    let user = authenticate_with_request(settings, account_password, &request)?;
    verify_remote_backup_table_with_request(settings, &user, &request)
}

fn authenticate_with_request<F>(
    settings: &CloudBackupSettings,
    account_password: &str,
    request: &F,
) -> Result<AuthenticatedUser, String>
where
    F: Fn(&str, &str, &str, &[u8]) -> Result<(u16, String), String>,
{
    let body = serde_json::to_vec(&serde_json::json!({
        "email": settings.email,
        "password": account_password,
    }))
    .map_err(|_| "项目 Auth 登录请求序列化失败".to_string())?;
    let headers = format!(
        "apikey: {}\r\nContent-Type: application/json",
        settings.publishable_key
    );
    let url = format!("{}/auth/v1/token?grant_type=password", settings.project_url);
    let (status, body) = request("POST", &url, &headers, &body)?;
    if status != 200 {
        return Err("项目 Auth 登录失败，请检查邮箱、密码和项目设置".to_string());
    }
    let response: AuthResponse = decode_json(&body, "项目 Auth 登录响应无效")?;
    let id = Uuid::parse_str(&response.user.id)
        .map_err(|_| "项目 Auth 登录响应无效".to_string())?
        .to_string();
    if response.access_token.is_empty() {
        return Err("项目 Auth 登录响应无效".to_string());
    }
    Ok(AuthenticatedUser {
        id,
        access_token: response.access_token,
    })
}

fn verify_remote_backup_table_with_request<F>(
    settings: &CloudBackupSettings,
    user: &AuthenticatedUser,
    request: &F,
) -> Result<(), String>
where
    F: Fn(&str, &str, &str, &[u8]) -> Result<(u16, String), String>,
{
    let headers = data_headers(settings, &user.access_token, "return=minimal");
    let url = format!(
        "{}/rest/v1/{}?select=user_id&user_id=eq.{}&limit=1",
        settings.project_url, TABLE, user.id
    );
    let (status, _) = request("GET", &url, &headers, &[])?;
    if status != 200 {
        return Err(remote_table_error(status));
    }
    Ok(())
}

fn data_headers(settings: &CloudBackupSettings, access_token: &str, prefer: &str) -> String {
    format!(
        "apikey: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nPrefer: {prefer}",
        settings.publishable_key, access_token
    )
}

fn remote_table_error(status: u16) -> String {
    if status == 404 {
        "云端备份表不可用，请确认已启用 Data API 并在 Supabase SQL Editor 执行初始化 SQL"
            .to_string()
    } else if status == 401 || status == 403 {
        "云端备份表权限不足，请在 Supabase SQL Editor 重新执行初始化 SQL".to_string()
    } else {
        format!("Supabase 云端备份请求失败（HTTP {status}）")
    }
}

fn decode_json<T: DeserializeOwned>(body: &str, message: &str) -> Result<T, String> {
    serde_json::from_str(body).map_err(|_| message.to_string())
}

fn encrypt(snapshot: &ConfigurationSnapshot, password: &str) -> Result<EncryptedBackup, String> {
    let mut cleartext =
        serde_json::to_vec(snapshot).map_err(|_| "配置快照序列化失败".to_string())?;
    let mut salt = [0u8; SALT_LENGTH];
    let mut nonce = [0u8; NONCE_LENGTH];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let mut key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "无法初始化备份加密器".to_string())?;
    let encrypted = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &cleartext,
                aad: AAD,
            },
        )
        .map_err(|_| "无法加密云端备份".to_string());
    key.fill(0);
    cleartext.fill(0);
    let ciphertext = encrypted?;
    Ok(EncryptedBackup {
        version: ENCRYPTION_VERSION,
        salt: STANDARD_NO_PAD.encode(salt),
        nonce: STANDARD_NO_PAD.encode(nonce),
        ciphertext: STANDARD_NO_PAD.encode(ciphertext),
    })
}

fn decrypt(payload: &EncryptedBackup, password: &str) -> Result<Vec<u8>, String> {
    if payload.version != ENCRYPTION_VERSION {
        return Err("云端备份加密版本不受支持".to_string());
    }
    let salt = decode_fixed(&payload.salt, SALT_LENGTH, "云端备份数据无效")?;
    let nonce = decode_fixed(&payload.nonce, NONCE_LENGTH, "云端备份数据无效")?;
    let ciphertext = STANDARD_NO_PAD
        .decode(&payload.ciphertext)
        .map_err(|_| "云端备份数据无效".to_string())?;
    let mut key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "无法初始化备份解密器".to_string())?;
    let cleartext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: AAD,
            },
        )
        .map_err(|_| "备份密码不正确或云端备份已损坏".to_string());
    key.fill(0);
    cleartext
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LENGTH], String> {
    let params = Params::new(19 * 1024, 2, 1, Some(KEY_LENGTH))
        .map_err(|_| "无法配置备份密钥派生".to_string())?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LENGTH];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| "无法派生备份加密密钥".to_string())?;
    Ok(key)
}

fn decode_fixed(value: &str, expected_length: usize, message: &str) -> Result<Vec<u8>, String> {
    let decoded = STANDARD_NO_PAD
        .decode(value)
        .map_err(|_| message.to_string())?;
    if decoded.len() == expected_length {
        Ok(decoded)
    } else {
        Err(message.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{
        AppKind, ClaudeModelSettings, CodexModelSettings, CommonSettingValue, ConfigValue,
        ConfigWriteRecord, ModelOptions, ProviderDraft, RouteMode, UsageQuery, WriteOperation,
    };
    use std::cell::RefCell;

    fn connection_settings() -> CloudBackupSettings {
        CloudBackupSettings {
            project_url: "https://example.supabase.co".to_string(),
            publishable_key: "sb_publishable_example".to_string(),
            email: "backup@example.com".to_string(),
        }
    }

    #[test]
    fn upload_and_restore_round_trip_the_complete_configuration_snapshot() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = LocalState::from_root(directory.path().join("source-state"));
        source
            .set_cloud_backup_settings(&connection_settings())
            .expect("source connection settings");
        let source_store = source.configuration();
        let codex = source_store
            .create_provider(ProviderDraft {
                app: AppKind::Codex,
                route_mode: RouteMode::Custom,
                name: "Codex relay".to_string(),
                model: Some("gpt-5.6-codex".to_string()),
                base_url: Some("https://codex-relay.example/v1".to_string()),
                api_key: "fixture-codex-value".to_string(),
                model_options: Some(ModelOptions::Codex(CodexModelSettings {
                    context_window: Some(272_000),
                })),
                notes: Some("primary coding route".to_string()),
                website_url: Some("https://codex-relay.example".to_string()),
                usage_query: Some(UsageQuery::Declarative {
                    url: "{{baseUrl}}/usage".to_string(),
                    remaining_path: Some("data/remaining".to_string()),
                    used_path: Some("data/used".to_string()),
                    total_path: Some("data/total".to_string()),
                    unit: Some("credits".to_string()),
                    refresh_interval_minutes: 15,
                }),
            })
            .expect("codex provider");
        source_store
            .create_provider(ProviderDraft {
                app: AppKind::Codex,
                route_mode: RouteMode::Official,
                name: "Codex official".to_string(),
                model: None,
                base_url: None,
                api_key: String::new(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
            })
            .expect("codex official provider");
        let claude = source_store
            .create_provider(ProviderDraft {
                app: AppKind::Claude,
                route_mode: RouteMode::Custom,
                name: "Claude relay".to_string(),
                model: Some("claude-opus-4-1".to_string()),
                base_url: Some("https://claude-relay.example".to_string()),
                api_key: "fixture-claude-value".to_string(),
                model_options: Some(ModelOptions::Claude(ClaudeModelSettings {
                    primary_one_m: true,
                    haiku_model: Some("claude-haiku-4".to_string()),
                    sonnet_model: Some("claude-sonnet-4-6".to_string()),
                    sonnet_one_m: true,
                    opus_model: Some("claude-opus-4-1".to_string()),
                    opus_one_m: true,
                    available_models: Some(vec![
                        "claude-haiku-4".to_string(),
                        "claude-opus-4-1".to_string(),
                    ]),
                })),
                notes: Some("primary analysis route".to_string()),
                website_url: Some("https://claude-relay.example/docs".to_string()),
                usage_query: None,
            })
            .expect("claude provider");
        let codex_common = source_store
            .get_common_settings(AppKind::Codex)
            .expect("codex common settings");
        let mut codex_settings = codex_common.settings;
        codex_settings.settings.insert(
            "model_reasoning_effort".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Str("xhigh".to_string()),
            },
        );
        source_store
            .save_common_settings(AppKind::Codex, codex_settings, &codex_common.settings_hash)
            .expect("save codex common settings");
        for (profile, at) in [
            (&codex.profile, "2026-09-04T12:00:00Z"),
            (&claude.profile, "2026-09-04T12:01:00Z"),
        ] {
            source_store
                .record_config_write(ConfigWriteRecord {
                    app: profile.app,
                    profile_id: Some(profile.id.clone()),
                    profile_name: Some(profile.name.clone()),
                    content_hash: "a".repeat(64),
                    backup_id: format!("backup-{}", profile.id),
                    at: at.to_string(),
                    operation: WriteOperation::Projection,
                })
                .expect("switch history");
        }
        let snapshot = read_configuration_snapshot(&source_store).expect("source snapshot");
        assert_eq!(snapshot.providers[&AppKind::Codex].len(), 2);
        assert_eq!(snapshot.providers[&AppKind::Claude].len(), 1);
        assert_eq!(snapshot.history[&AppKind::Codex].len(), 1);
        assert_eq!(snapshot.history[&AppKind::Claude].len(), 1);

        let requests = RefCell::new(Vec::new());
        let uploaded = upload_with_request(
            &source,
            "project-auth-password",
            "cloud-backup-password",
            &|method, url, headers, body| {
                let index = requests.borrow().len();
                requests.borrow_mut().push((
                    method.to_string(),
                    url.to_string(),
                    headers.to_string(),
                    body.to_vec(),
                ));
                match index {
                    0 => Ok((
                        200,
                        r#"{"access_token":"session-value","user":{"id":"c9d2eeb1-425e-4f9d-8ff4-bd27e52103fb"}}"#
                            .to_string(),
                    )),
                    1 => Ok((201, String::new())),
                    _ => panic!("unexpected upload request"),
                }
            },
        )
        .expect("upload");
        assert_eq!(uploaded.profile_count, 3);
        let requests = requests.into_inner();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].0, "POST");
        assert!(requests[1]
            .1
            .ends_with("/rest/v1/agent_switchboard_cloud_backups?on_conflict=user_id"));
        let upload_body: serde_json::Value =
            serde_json::from_slice(&requests[1].3).expect("encrypted upload JSON");
        assert_eq!(
            upload_body["user_id"],
            "c9d2eeb1-425e-4f9d-8ff4-bd27e52103fb"
        );
        assert!(upload_body.get("userId").is_none());
        let upload_text = String::from_utf8(requests[1].3.clone()).expect("upload text");
        assert!(!upload_text.contains("fixture-codex-value"));
        assert!(!upload_text.contains("fixture-claude-value"));
        let payload: EncryptedBackup =
            serde_json::from_value(upload_body["payload"].clone()).expect("encrypted payload");

        let target = LocalState::from_root(directory.path().join("target-state"));
        target
            .set_cloud_backup_settings(&connection_settings())
            .expect("target connection settings");
        target
            .configuration()
            .create_provider(ProviderDraft {
                app: AppKind::Codex,
                route_mode: RouteMode::Custom,
                name: "stale route".to_string(),
                model: None,
                base_url: Some("https://stale.example".to_string()),
                api_key: "stale-value".to_string(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
            })
            .expect("stale target provider");
        let restore_response = serde_json::json!([{
            "payload": payload,
            "updated_at": uploaded.updated_at,
        }])
        .to_string();
        let restore_requests = RefCell::new(0usize);
        let restored = restore_with_request(
            &target,
            "project-auth-password",
            "cloud-backup-password",
            &|_, _, _, _| {
                let index = *restore_requests.borrow();
                *restore_requests.borrow_mut() += 1;
                match index {
                    0 => Ok((
                        200,
                        r#"{"access_token":"session-value","user":{"id":"c9d2eeb1-425e-4f9d-8ff4-bd27e52103fb"}}"#
                            .to_string(),
                    )),
                    1 => Ok((200, restore_response.clone())),
                    _ => panic!("unexpected restore request"),
                }
            },
        )
        .expect("restore");
        assert_eq!(restored.profile_count, 3);
        assert_eq!(*restore_requests.borrow(), 2);
        assert_eq!(
            read_configuration_snapshot(&target.configuration()).expect("restored snapshot"),
            snapshot
        );
    }

    #[test]
    fn incorrect_backup_password_never_decrypts_the_snapshot() {
        let snapshot = ConfigurationSnapshot {
            providers: Default::default(),
            common: Default::default(),
            history: Default::default(),
        };
        let encrypted = encrypt(&snapshot, "cloud-backup-password").expect("encrypt");

        assert_eq!(
            decrypt(&encrypted, "wrong-password").expect_err("wrong password"),
            "备份密码不正确或云端备份已损坏"
        );
    }

    #[test]
    fn setup_sql_enforces_authenticated_row_ownership() {
        assert!(SETUP_SQL.contains("enable row level security"));
        assert!(SETUP_SQL.contains("to authenticated"));
        assert!(SETUP_SQL.contains("(select auth.uid()) = user_id"));
        assert!(!SETUP_SQL.contains("service_role"));
    }

    #[test]
    fn backup_records_use_the_database_column_names_over_data_api() {
        let payload = EncryptedBackup {
            version: ENCRYPTION_VERSION,
            salt: "salt".to_string(),
            nonce: "nonce".to_string(),
            ciphertext: "ciphertext".to_string(),
        };
        let record = serde_json::to_value(UploadRecord {
            user_id: "c9d2eeb1-425e-4f9d-8ff4-bd27e52103fb",
            payload: &payload,
            updated_at: "2026-09-04T12:00:00.000Z",
        })
        .expect("upload record serializes");

        assert_eq!(record["user_id"], "c9d2eeb1-425e-4f9d-8ff4-bd27e52103fb");
        assert_eq!(record["updated_at"], "2026-09-04T12:00:00.000Z");
        assert!(record.get("userId").is_none());
        assert!(record.get("updatedAt").is_none());

        let remote: RemoteRecord = serde_json::from_value(serde_json::json!({
            "payload": payload,
            "updated_at": "2026-09-04T12:00:00.000Z",
        }))
        .expect("Supabase record deserializes");
        assert_eq!(remote.updated_at, "2026-09-04T12:00:00.000Z");
    }

    #[test]
    fn connection_test_authenticates_and_reads_only_the_callers_backup_row() {
        let calls = RefCell::new(Vec::new());
        test_connection_with_request(&connection_settings(), "account-password", |method, url, headers, body| {
            let index = calls.borrow().len();
            calls.borrow_mut().push((
                method.to_string(),
                url.to_string(),
                headers.to_string(),
                body.to_vec(),
            ));
            match index {
                0 => Ok((
                    200,
                    r#"{"access_token":"session-value","user":{"id":"c9d2eeb1-425e-4f9d-8ff4-bd27e52103fb"}}"#
                        .to_string(),
                )),
                1 => Ok((200, "[]".to_string())),
                _ => panic!("unexpected request"),
            }
        })
        .expect("connection succeeds");

        let calls = calls.into_inner();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "POST");
        assert_eq!(
            calls[0].1,
            "https://example.supabase.co/auth/v1/token?grant_type=password"
        );
        assert_eq!(
            calls[0].2,
            "apikey: sb_publishable_example\r\nContent-Type: application/json"
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&calls[0].3).expect("auth request JSON"),
            serde_json::json!({
                "email": "backup@example.com",
                "password": "account-password",
            })
        );
        assert_eq!(calls[1].0, "GET");
        assert_eq!(
            calls[1].1,
            "https://example.supabase.co/rest/v1/agent_switchboard_cloud_backups?select=user_id&user_id=eq.c9d2eeb1-425e-4f9d-8ff4-bd27e52103fb&limit=1"
        );
        assert!(calls[1].2.contains("Authorization: Bearer session-value"));
        assert!(calls[1].3.is_empty());
    }

    #[test]
    fn connection_test_reports_a_missing_or_unavailable_backup_table() {
        let request_count = RefCell::new(0usize);
        let error = test_connection_with_request(
            &connection_settings(),
            "account-password",
            |_, _, _, _| {
                let index = *request_count.borrow();
                *request_count.borrow_mut() += 1;
                if index == 0 {
                    Ok((
                        200,
                        r#"{"access_token":"session-value","user":{"id":"c9d2eeb1-425e-4f9d-8ff4-bd27e52103fb"}}"#
                            .to_string(),
                    ))
                } else {
                    Ok((404, String::new()))
                }
            },
        )
        .expect_err("missing table is not a successful connection");

        assert_eq!(
            error,
            "云端备份表不可用，请确认已启用 Data API 并在 Supabase SQL Editor 执行初始化 SQL"
        );
        assert_eq!(*request_count.borrow(), 2);
    }
}
