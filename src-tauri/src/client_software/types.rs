use asb_core::contracts::{AppKind, LocalizedMessage};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Form { Cli, Desktop, Extension, Bundled }

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Manager { Native, Npm, Homebrew, Winget, System, Manual, Host }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    pub id: String,
    pub app: AppKind,
    pub form: Form,
    pub manager: Manager,
    pub launcher: PathBuf,
    pub executable: PathBuf,
    pub root: PathBuf,
    pub version: Option<String>,
    pub version_text: Option<String>,
    pub active: bool,
    pub channel: String,
    pub detail: Option<String>,
    // Execution coordinates never cross the renderer boundary.
    #[serde(skip)]
    pub owner: Option<Owner>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
    Native,
    Npm { node: PathBuf, npm: PathBuf, prefix: PathBuf, package: String, registry: Option<String> },
    Homebrew { brew: PathBuf, cask: String },
    Winget { package: String, scope: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub source: String,
    pub checked_at: String,
    pub stale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub channel: String,
    pub automatic_disabled: bool,
    pub updates_disabled: bool,
    pub minimum: Option<String>,
    pub maximum: Option<String>,
    pub issue: Option<String>,
    pub revision: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Software {
    #[serde(flatten)]
    pub installation: Installation,
    pub official: Option<Release>,
    pub available: Option<Release>,
    pub comparison: Comparison,
    pub policy: Policy,
    pub update_allowed: bool,
    pub reason: Option<String>,
    pub guide_url: String,
    pub release_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Comparison { Unknown, Current, ChannelBehind, UpdateAvailable, Ahead }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: String,
    pub installations: Vec<Software>,
    pub issues: Vec<String>,
    pub task: Option<UpdateTask>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePlan {
    pub id: String,
    pub installation_id: String,
    pub app: AppKind,
    pub launcher: PathBuf,
    pub manager: Manager,
    pub from_version: String,
    pub target_version: String,
    pub exact_target: bool,
    pub channel: String,
    pub proxy_mode: String,
    pub expires_at: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Phase { Preparing, RunningUpdater, Verifying, Completed, Failed, NeedsAttention, Cancelled }

impl Phase {
    pub fn active(self) -> bool { matches!(self, Self::Preparing | Self::RunningUpdater | Self::Verifying) }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTask {
    pub id: String,
    pub revision: u64,
    pub plan: UpdatePlan,
    pub phase: Phase,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub actual_version: Option<String>,
    pub cancellable: bool,
    pub log: Vec<String>,
    pub error: Option<LocalizedMessage>,
    pub process_id: Option<u32>,
    pub process_birth: Option<String>,
}

pub fn guide(app: AppKind) -> &'static str {
    match app {
        AppKind::Codex => "https://developers.openai.com/codex/cli",
        AppKind::Claude => "https://code.claude.com/docs/en/setup",
    }
}

pub fn installation_guide(item: &Installation) -> &'static str {
    match (item.app, item.form) {
        (AppKind::Codex, Form::Desktop) => "https://developers.openai.com/codex/app",
        (AppKind::Codex, Form::Extension) => "https://developers.openai.com/codex/ide",
        (AppKind::Claude, Form::Desktop) => "https://claude.ai/download",
        (AppKind::Claude, Form::Extension) => "https://code.claude.com/docs/en/vs-code",
        _ => guide(item.app),
    }
}
