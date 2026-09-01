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
#[serde(rename_all = "camelCase")]
struct UploadRecord<'a> {
    user_id: &'a str,
    payload: &'a EncryptedBackup,
    updated_at: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    access_token: String,
    user: AuthUser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthUser {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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

pub fn upload(
    state: &LocalState,
    account_password: &str,
    backup_password: &str,
) -> Result<CloudBackupResult, String> {
    let settings = configured_settings(state)?;
    validate_passwords(account_password, backup_password)?;
    let user = authenticate(&settings, account_password)?;
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
    let (status, _) = http_request("POST", &url, &headers, &body)?;
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
    let settings = configured_settings(state)?;
    validate_passwords(account_password, backup_password)?;
    let user = authenticate(&settings, account_password)?;
    let headers = data_headers(&settings, &user.access_token, "return=representation");
    let url = format!(
        "{}/rest/v1/{}?select=payload,updated_at&user_id=eq.{}&limit=1",
        settings.project_url, TABLE, user.id
    );
    let (status, body) = http_request("GET", &url, &headers, &[])?;
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
    if account_password.is_empty() {
        return Err("请输入 Supabase 登录密码".to_string());
    }
    if backup_password.len() < 8 {
        return Err("备份密码至少需要 8 个字符".to_string());
    }
    Ok(())
}

fn authenticate(
    settings: &CloudBackupSettings,
    account_password: &str,
) -> Result<AuthenticatedUser, String> {
    let body = serde_json::to_vec(&serde_json::json!({
        "email": settings.email,
        "password": account_password,
    }))
    .map_err(|_| "Supabase 登录请求序列化失败".to_string())?;
    let headers = format!(
        "apikey: {}\r\nContent-Type: application/json",
        settings.publishable_key
    );
    let url = format!("{}/auth/v1/token?grant_type=password", settings.project_url);
    let (status, body) = http_request("POST", &url, &headers, &body)?;
    if status != 200 {
        return Err("Supabase 登录失败，请检查邮箱、密码和项目设置".to_string());
    }
    let response: AuthResponse = decode_json(&body, "Supabase 登录响应无效")?;
    let id = Uuid::parse_str(&response.user.id)
        .map_err(|_| "Supabase 登录响应无效".to_string())?
        .to_string();
    if response.access_token.is_empty() {
        return Err("Supabase 登录响应无效".to_string());
    }
    Ok(AuthenticatedUser {
        id,
        access_token: response.access_token,
    })
}

fn data_headers(settings: &CloudBackupSettings, access_token: &str, prefer: &str) -> String {
    format!(
        "apikey: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nPrefer: {prefer}",
        settings.publishable_key, access_token
    )
}

fn remote_table_error(status: u16) -> String {
    if status == 404 {
        "云端备份表不可用，请先在 Supabase SQL Editor 执行初始化 SQL".to_string()
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

    #[test]
    fn encrypted_backup_round_trips_the_complete_configuration_snapshot() {
        let snapshot = ConfigurationSnapshot {
            providers: Default::default(),
            common: Default::default(),
            history: Default::default(),
        };

        let encrypted = encrypt(&snapshot, "cloud-backup-password").expect("encrypt");
        let mut cleartext = decrypt(&encrypted, "cloud-backup-password").expect("decrypt");
        let restored: ConfigurationSnapshot =
            serde_json::from_slice(&cleartext).expect("configuration snapshot");
        cleartext.fill(0);

        assert_eq!(restored, snapshot);
        assert_ne!(encrypted.ciphertext, "{}");
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
}
