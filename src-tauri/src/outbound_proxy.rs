//! Outbound proxy policy for application-owned HTTP clients.
//!
//! One owner of the egress contract: none (always direct), system (follow the
//! environment, minus a self-loop guard) or manual (an explicit http/https/
//! socks5/socks5h URL, credentials included). Gateway upstreams, probes and
//! provider requests all build through [`apply`] so a settings change is
//! picked up on the next client build; the revision-cached client registry
//! makes that immediate for long-lived holders.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

mod runtime;
pub(crate) use runtime::{apply, apply_blocking, cached_blocking_client, cached_client, note_gateway_port};

static MUTATIONS: Mutex<()> = Mutex::new(());

const VERSION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OutboundProxyMode {
    #[default]
    None,
    System,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OutboundProxySettings {
    pub version: u64,
    pub mode: OutboundProxyMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub revision: String,
}
impl Default for OutboundProxySettings {
    fn default() -> Self {
        Self {
            version: VERSION,
            mode: OutboundProxyMode::None,
            url: None,
            revision: uuid::Uuid::nil().to_string(),
        }
    }
}

/// One invalid or failed outbound-proxy settings operation. The `Display`
/// text is the scrubbed diagnostic detail; `parts` carries the renderer
/// translation coordinates.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OutboundProxyError {
    #[error("无法读取出站代理设置：{0}")]
    Read(String),
    #[error("出站代理设置文件无效")]
    Unreadable,
    #[error("无法写入出站代理设置：{0}")]
    Write(String),
    #[error("出站代理设置已改变，请重新读取后再保存")]
    RevisionConflict,
    #[error("出站代理设置版本不受支持")]
    UnsupportedVersion,
    #[error("出站代理设置修订无效")]
    InvalidRevision,
    #[error("直连模式不能携带代理地址")]
    DirectUrlForbidden,
    #[error("系统代理模式不能携带代理地址")]
    SystemUrlForbidden,
    #[error("手动代理需要代理地址")]
    ManualUrlRequired,
    #[error("代理地址过长或含有控制字符")]
    UrlTooLong,
    #[error("代理协议必须是 http、https、socks5 或 socks5h：{0}")]
    UrlScheme(String),
    #[error("代理地址无效：{0}")]
    UrlInvalid(String),
}

impl OutboundProxyError {
    pub(crate) fn parts(&self) -> (&'static str, serde_json::Value) {
        match self {
            Self::Read(detail) | Self::Write(detail) => {
                ("errors.outboundProxy.unavailable", serde_json::json!({ "detail": detail }))
            }
            Self::Unreadable => ("errors.outboundProxy.unreadable", serde_json::json!({})),
            Self::RevisionConflict => ("errors.outboundProxy.revisionConflict", serde_json::json!({})),
            Self::UnsupportedVersion => ("errors.outboundProxy.versionUnsupported", serde_json::json!({})),
            Self::InvalidRevision => ("errors.outboundProxy.revisionInvalid", serde_json::json!({})),
            Self::DirectUrlForbidden => ("errors.outboundProxy.directUrlForbidden", serde_json::json!({})),
            Self::SystemUrlForbidden => ("errors.outboundProxy.systemUrlForbidden", serde_json::json!({})),
            Self::ManualUrlRequired => ("errors.outboundProxy.manualUrlRequired", serde_json::json!({})),
            Self::UrlTooLong => ("errors.outboundProxy.urlTooLong", serde_json::json!({})),
            Self::UrlScheme(scheme) => ("errors.outboundProxy.urlScheme", serde_json::json!({ "scheme": scheme })),
            Self::UrlInvalid(detail) => ("errors.outboundProxy.urlInvalid", serde_json::json!({ "detail": detail })),
        }
    }

    /// Read/write failures mean the settings surface is unavailable, not that
    /// the submitted values were invalid.
    pub(crate) fn is_io(&self) -> bool {
        matches!(self, Self::Read(_) | Self::Unreadable | Self::Write(_))
    }
}

impl OutboundProxySettings {
    pub(crate) fn validate(&self) -> Result<(), OutboundProxyError> {
        if self.version != VERSION {
            return Err(OutboundProxyError::UnsupportedVersion);
        }
        if uuid::Uuid::parse_str(&self.revision).is_err() {
            return Err(OutboundProxyError::InvalidRevision);
        }
        match self.mode {
            OutboundProxyMode::None => {
                if self.url.is_some() {
                    return Err(OutboundProxyError::DirectUrlForbidden);
                }
            }
            OutboundProxyMode::System => {
                if self.url.is_some() {
                    return Err(OutboundProxyError::SystemUrlForbidden);
                }
            }
            OutboundProxyMode::Manual => {
                let Some(url) = self.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
                    return Err(OutboundProxyError::ManualUrlRequired);
                };
                if url.len() > 2048 || url.chars().any(char::is_control) {
                    return Err(OutboundProxyError::UrlTooLong);
                }
                // This module owns the accepted scheme set; reqwest then owns
                // the rest of the grammar (host, port, credentials).
                const SCHEMES: [&str; 4] = ["http", "https", "socks5", "socks5h"];
                let scheme = url.split("://").next().unwrap_or_default();
                if !SCHEMES.contains(&scheme) {
                    return Err(OutboundProxyError::UrlScheme(scheme.to_string()));
                }
                reqwest::Proxy::all(url)
                    .map_err(|error| OutboundProxyError::UrlInvalid(error.to_string()))?;
            }
        }
        Ok(())
    }
}

pub(crate) fn read(state_root: &Path) -> Result<OutboundProxySettings, OutboundProxyError> {
    let bytes = crate::application_data_io::read(&settings_path(state_root)).map_err(OutboundProxyError::Read)?;
    decode(bytes.as_deref())
}

fn decode(bytes: Option<&[u8]>) -> Result<OutboundProxySettings, OutboundProxyError> {
    let Some(bytes) = bytes else { return Ok(OutboundProxySettings::default()); };
    let settings: OutboundProxySettings = serde_json::from_slice(bytes).map_err(|_| OutboundProxyError::Unreadable)?;
    settings.validate()?;
    Ok(settings)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OutboundProxyDraft {
    pub mode: OutboundProxyMode,
    pub url: Option<String>,
}

pub(crate) struct SettingsView {
    pub settings: Option<OutboundProxySettings>,
    pub revision: String,
    pub issue: Option<OutboundProxyError>,
}

pub(crate) fn view(state_root: &Path) -> Result<SettingsView, OutboundProxyError> {
    let bytes = crate::application_data_io::read(&settings_path(state_root)).map_err(OutboundProxyError::Read)?;
    let revision = bytes.as_deref().map(crate::application_data_io::digest).unwrap_or_else(|| "missing".into());
    match decode(bytes.as_deref()) {
        Ok(settings) => Ok(SettingsView { settings: Some(settings), revision, issue: None }),
        Err(error) => Ok(SettingsView { settings: None, revision, issue: Some(error) }),
    }
}

pub(crate) fn save(state_root: &Path, draft: OutboundProxyDraft, expected_revision: &str)
    -> Result<SettingsView, OutboundProxyError> {
    let settings = OutboundProxySettings { mode: draft.mode, url: draft.url,
        revision: uuid::Uuid::new_v4().to_string(), ..Default::default() };
    settings.validate()?;
    let _guard = MUTATIONS.lock().map_err(|_| OutboundProxyError::Write("出口设置写入锁不可用".into()))?;
    let current = view(state_root)?;
    if current.revision != expected_revision { return Err(OutboundProxyError::RevisionConflict); }
    let path = settings_path(state_root);
    std::fs::create_dir_all(state_root).map_err(|error| OutboundProxyError::Write(error.to_string()))?;
    if current.issue.is_some() {
        let bytes = crate::application_data_io::read(&path).map_err(OutboundProxyError::Read)?.ok_or(OutboundProxyError::RevisionConflict)?;
        let backup = state_root.join(format!("outbound-proxy-backup-{}.json", uuid::Uuid::new_v4()));
        crate::application_data_io::write_new(&backup, &bytes).map_err(OutboundProxyError::Write)?;
        log::info!("原出站设置保留在 {}", backup.display());
    }
    if view(state_root)?.revision != expected_revision { return Err(OutboundProxyError::RevisionConflict); }
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|error| OutboundProxyError::Write(error.to_string()))?;
    crate::application_data_io::replace(&path, &bytes).map_err(OutboundProxyError::Write)?;
    runtime::update(&settings);
    view(state_root)
}

/// Publishes only a validated policy; a failed read also invalidates cached clients.
pub(crate) fn load(state_root: &Path) -> Result<(), OutboundProxyError> {
    let _guard = MUTATIONS.lock().map_err(|_| OutboundProxyError::Read("出口设置锁不可用".into()))?;
    match read(state_root) {
        Ok(settings) => { runtime::update(&settings); Ok(()) }
        Err(error) => { runtime::unavailable(error.clone()); Err(error) }
    }
}

fn settings_path(state_root: &Path) -> PathBuf {
    state_root.join("outbound-proxy.json")
}

