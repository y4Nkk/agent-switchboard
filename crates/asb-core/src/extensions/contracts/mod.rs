//! Persisted and IPC contracts for the extensions workspace (Skills / MCP).
//!
//! These types are the single owner of the extension vocabulary. The desktop
//! store persists them verbatim, the switch executor consumes plan payloads,
//! and the frontend only ever receives the projection types defined here.
//! Persisted shapes parse strictly: unknown fields are rejected instead of
//! preserved, and display names or native keys are never used as identities.

mod definition;
mod managed;
mod manifest;
mod operation;
mod server;
mod skill;

pub use definition::{ExtensionBinding, ExtensionDefinition, ExtensionPayload, McpMetadata, McpRegistrySource};
pub use managed::{BaselineKind, ManagedBaseline, ManagedBaselineFile, ManagedFileEntry};
pub use manifest::{
    DesiredState, DocumentSyntax, ExtensionKind, ExtensionManifest, ExtensionTarget, SecretValue,
    EXTENSIONS_SCHEMA_VERSION,
};
pub use operation::{
    AppliedExtensionStep, AppliedStep, CapabilityEntry, CapabilityVerification,
    ClientCapabilityReport, DependencyState, ExtensionOperationRecord, ExtensionTargetResult,
    FileState, McpCheckOutcome, McpCheckResult, NativeCondition, ObservedExtension, ObservedOrigin,
    OperationResourceRecord, OperationSnapshot, PlanOperation, ProjectRegistration,
    RollbackSummary, TargetOutcome,
};
pub use server::{CodexServerOptions, FieldEdit, McpDefinition, McpEditRequest, McpFieldEdits};
pub use skill::{CompatibilityNote, SkillDefinition, SkillDependency, SkillManifest, SourceRef};
