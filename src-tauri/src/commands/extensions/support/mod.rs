//! Shared support for the extension commands: process-only caches, identity
//! helpers, error mapping, and the renderer-facing projections several
//! command domains share.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

mod mcp_view;
mod source_view;
pub use mcp_view::McpConnectionView;
pub use source_view::SkillSourceViewDto;
pub(super) use source_view::skill_source_view;

use asb_core::contracts::AppKind;
use asb_core::extensions::contracts::{
    DependencyState, DesiredState, ExtensionBinding, ExtensionDefinition, ExtensionPayload,
    ExtensionTarget, FileState, McpCheckResult, McpMetadata, ObservedExtension, SecretValue,
    SkillDependency, SkillManifest, EXTENSIONS_SCHEMA_VERSION,
};
use asb_core::extensions::plan::ExtensionPlan;
use serde::Serialize;

use crate::commands::error::CommandError;
use crate::extensions::sources;
use crate::extensions::store::ExtensionStore;
use crate::local_state::LocalState;

// ---------------------------------------------------------------- process state

/// Plans awaiting apply. The plan itself is the only write authorization
/// and directly carries the post-apply library state; the commit is
/// pre-staged here and journaled before the executor runs the commit
/// closure.
pub(super) struct PendingPlan {
    pub(super) plan: ExtensionPlan,
    pub(super) operation_id: String,
    pub(super) journal_dir: PathBuf,
    pub(super) commit: crate::extensions::store::LibraryCommit,
}

static PENDING_PLANS: OnceLock<Mutex<BTreeMap<String, PendingPlan>>> = OnceLock::new();

pub(super) fn pending_plans() -> &'static Mutex<BTreeMap<String, PendingPlan>> {
    PENDING_PLANS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Raw discovery facts stay in this process-only cache. The renderer receives
/// an opaque observation id and a safe summary; it never needs a local source
/// path merely to import a discovered Skill.
#[derive(Clone)]
pub(super) struct CachedDiscoveryObservation {
    pub(super) observed: ObservedExtension,
    /// MCP imports verify this digest before parsing, so a click can never
    /// import a document that changed after the displayed discovery pass.
    #[allow(dead_code)] // Read by the dynamically dispatched Tauri MCP-import command.
    pub(super) document_digest: Option<String>,
}

static DISCOVERED_OBSERVATIONS: OnceLock<Mutex<BTreeMap<String, CachedDiscoveryObservation>>> =
    OnceLock::new();

pub(super) fn discovered_observations(
) -> &'static Mutex<BTreeMap<String, CachedDiscoveryObservation>> {
    DISCOVERED_OBSERVATIONS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// The backend facts of one diagnostic that repair planning needs. Real
/// paths and baselines stay out; repair re-derives everything from the
/// library at prepare time.
#[derive(Clone)]
pub(super) struct CachedDiagnostic {
    pub(super) remediation: asb_core::extensions::diagnostics::DiagnosticRemediation,
    /// Set exactly when the subject is a managed binding.
    pub(super) binding_id: Option<String>,
}

/// The most recent discovery scan. A repair request must name this scan;
/// anything else is stale and rejected.
#[derive(Clone)]
pub(super) struct CachedDiscoveryScan {
    pub(super) scan_id: String,
    pub(super) diagnostics: BTreeMap<String, CachedDiagnostic>,
}

static LATEST_DISCOVERY_SCAN: OnceLock<Mutex<Option<CachedDiscoveryScan>>> = OnceLock::new();

pub(super) fn latest_discovery_scan() -> &'static Mutex<Option<CachedDiscoveryScan>> {
    LATEST_DISCOVERY_SCAN.get_or_init(|| Mutex::new(None))
}

/// A placeholder binding row for a restore whose original binding no longer
/// exists. The library commit owns the real pre-state; this row only carries
/// the target required by the executor's typed plan.
pub(super) fn restore_binding_stub(
    definition_id: &str,
    target: &ExtensionTarget,
) -> ExtensionBinding {
    ExtensionBinding {
        schema_version: EXTENSIONS_SCHEMA_VERSION,
        id: new_id("bind"),
        resource_id: definition_id.to_string(),
        target: target.clone(),
        native_key: None,
        deploy_name: None,
        desired: DesiredState::Disabled,
        locked_digest: None,
        last_applied_revision: None,
        updated_at: now(),
    }
}

pub(super) fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub(super) fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

pub(super) fn sha_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn extension_store(state: &LocalState) -> ExtensionStore {
    ExtensionStore::from_state(state)
}

pub(super) fn store_error(error: crate::extensions::store::ExtensionStoreError) -> CommandError {
    match error {
        crate::extensions::store::ExtensionStoreError::RecoveryRequired(message) => {
            CommandError::new("recovery-required", message)
        }
        other => CommandError::new("extension-store", other.to_string()),
    }
}

/// Source transports may include a private repository URL or a local path in
/// their low-level error text. Renderer-facing errors expose the failure
/// class only; the full source identity remains inside the source boundary.
pub(super) fn source_error(error: sources::SourceError) -> CommandError {
    if error.is_rejected() {
        CommandError::keyed(
            "source-rejected",
            "errors.extlib.sourceRejected",
            "来源内容未通过完整性或安全校验",
        )
    } else {
        CommandError::keyed(
            "source-unreachable",
            "errors.extlib.sourceUnreachable",
            "来源不可达；请检查网络、地址和访问权限",
        )
    }
}

// ---------------------------------------------------------------- DTOs

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingStatusDto {
    #[serde(flatten)]
    pub(super) binding: ExtensionBinding,
    pub(super) file_state: FileState,
    pub(super) warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatusDto {
    pub(super) name: String,
    pub(super) resource_id: Option<String>,
    pub(super) state: DependencyState,
}

/// A renderer-safe value position. Library definitions retain the full
/// SecretValue internally; no plain value or credential-store handle crosses
/// the IPC boundary back to the renderer.
#[derive(Serialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum SecretValueViewDto {
    EnvRef { name: String },
    Stored,
    Redacted,
}

impl From<SecretValue> for SecretValueViewDto {
    fn from(value: SecretValue) -> Self {
        match value {
            SecretValue::EnvRef { name } => Self::EnvRef { name },
            SecretValue::SecretRef { .. } => Self::Stored,
            SecretValue::Plain { .. } => Self::Redacted,
        }
    }
}

pub(super) fn secret_value_views(
    values: BTreeMap<String, SecretValue>,
) -> BTreeMap<String, SecretValueViewDto> {
    values
        .into_iter()
        .map(|(name, value)| (name, value.into()))
        .collect()
}

/// The read model is deliberately distinct from ExtensionDefinition.
/// Definitions are persisted with connection details so the executor can
/// render client files, whereas the renderer may only receive metadata and
/// redacted operational summaries.
#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ExtensionListItemDto {
    Skill {
        schema_version: u8,
        id: String,
        name: String,
        revision: u64,
        created_at: String,
        updated_at: String,
        content_digest: String,
        manifest: SkillManifest,
        source: Option<SkillSourceViewDto>,
        host_scoped: Option<AppKind>,
        compatibility: Vec<asb_core::extensions::contracts::CompatibilityNote>,
        dependencies: Vec<SkillDependency>,
        bindings: Vec<BindingStatusDto>,
        dependency_states: Vec<DependencyStatusDto>,
        checks: Vec<McpCheckResult>,
    },
    Mcp {
        schema_version: u8,
        id: String,
        name: String,
        revision: u64,
        created_at: String,
        updated_at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mcp_metadata: Option<McpMetadata>,
        #[serde(flatten)]
        connection: McpConnectionView,
        bindings: Vec<BindingStatusDto>,
        dependency_states: Vec<DependencyStatusDto>,
        checks: Vec<McpCheckResult>,
    },
}

pub(super) fn extension_list_item(
    definition: ExtensionDefinition,
    bindings: Vec<BindingStatusDto>,
    dependency_states: Vec<DependencyStatusDto>,
    checks: Vec<McpCheckResult>,
) -> ExtensionListItemDto {
    let ExtensionDefinition {
        schema_version,
        id,
        name,
        mcp_metadata,
        revision,
        created_at,
        updated_at,
        payload,
        ..
    } = definition;
    match payload {
        ExtensionPayload::Skill(skill) => ExtensionListItemDto::Skill {
            schema_version,
            id,
            name,
            revision,
            created_at,
            updated_at,
            content_digest: skill.content_digest,
            manifest: skill.manifest,
            source: skill.source.map(source_view::skill_source_view),
            host_scoped: skill.host_scoped,
            compatibility: skill.compatibility,
            dependencies: skill.dependencies,
            bindings,
            dependency_states,
            checks,
        },
        ExtensionPayload::Mcp(mcp) => ExtensionListItemDto::Mcp {
            schema_version,
            id,
            name,
            mcp_metadata,
            revision,
            created_at,
            updated_at,
            connection: mcp.into(),
            bindings,
            dependency_states,
            checks,
        },
    }
}

/// Mutation responses deliberately contain only the stable identity that the
/// renderer needs to refresh and select the updated row.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionMutationDto {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) revision: u64,
}

pub(super) fn extension_mutation(definition: &ExtensionDefinition) -> ExtensionMutationDto {
    ExtensionMutationDto {
        id: definition.id.clone(),
        name: definition.name.clone(),
        revision: definition.revision,
    }
}


pub(super) fn projection_error(error: asb_core::extensions::ProjectionError) -> CommandError {
    match error {
        asb_core::extensions::ProjectionError::SecretUnavailable(_) => CommandError::keyed(
            "extension-projection",
            "errors.extlib.secretUnavailableRepreview",
            "扩展所需凭据当前不可用；请在系统凭据存储中补充后重新预览",
        ),
        other => CommandError::new("extension-projection", other.to_string()),
    }
}

pub(super) fn adapter_error(error: asb_core::adapter::AdapterError) -> CommandError {
    CommandError::new("extension-projection", error.message)
}
