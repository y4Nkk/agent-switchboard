//! Record CRUD: definitions, bindings, projects, history snapshots,
//! checks, and the journal/backup locations.

use std::fs;
use std::path::PathBuf;

use asb_core::extensions::contracts::{
    ExtensionBinding, ExtensionDefinition, ExtensionManifest, ExtensionPayload, ManagedBaselineFile,
    McpCheckResult, OperationSnapshot, ProjectRegistration, EXTENSIONS_SCHEMA_VERSION,
};
use asb_core::extensions::validate::validate_definition;

use super::commit::save_lock;
use super::*;

#[cfg(test)]
mod checks_tests;

pub(super) fn require_current_schema(
    kind: &str,
    schema_version: u8,
) -> Result<(), ExtensionStoreError> {
    if schema_version == EXTENSIONS_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ExtensionStoreError::Unsupported(format!(
            "{kind}版本 {schema_version} 不是当前版本 {EXTENSIONS_SCHEMA_VERSION}"
        )))
    }
}

fn validate_stored_definition(definition: &ExtensionDefinition) -> Result<(), ExtensionStoreError> {
    validate_definition(definition).map_err(|error| ExtensionStoreError::Unsupported(error.message))
}

pub(super) fn validate_stored_snapshot(
    snapshot: &OperationSnapshot,
) -> Result<(), ExtensionStoreError> {
    require_current_schema("操作快照", snapshot.schema_version)?;
    require_current_schema("操作记录", snapshot.record.schema_version)?;
    for binding in snapshot
        .pre_bindings
        .iter()
        .chain(snapshot.post_bindings.iter())
    {
        require_current_schema("操作快照中的绑定", binding.schema_version)?;
    }
    for (_, baseline) in snapshot
        .pre_baselines
        .iter()
        .chain(snapshot.post_baselines.iter())
    {
        require_current_schema("操作快照中的基线", baseline.schema_version)?;
    }
    Ok(())
}

impl ExtensionStore {
    /// Reads the current manifest strictly. A missing root is a new library;
    /// a present but malformed manifest is a hard error rather than a silent
    /// reset of the generation counter.
    pub fn manifest(&self) -> Result<ExtensionManifest, ExtensionStoreError> {
        let path = self.path(&["manifest.json"]);
        if !path.exists() {
            return Ok(ExtensionManifest {
                schema_version: EXTENSIONS_SCHEMA_VERSION,
                generation: 0,
            });
        }
        let manifest = self.read_json::<ExtensionManifest>(&path)?;
        if manifest.schema_version != EXTENSIONS_SCHEMA_VERSION {
            return Err(ExtensionStoreError::Unsupported(format!(
                "扩展库版本 {} 不是当前版本 {}",
                manifest.schema_version, EXTENSIONS_SCHEMA_VERSION
            )));
        }
        Ok(manifest)
    }

    // --------------------------------------------------------- definitions

    pub fn list_definitions(&self) -> Result<Vec<ExtensionDefinition>, ExtensionStoreError> {
        let dir = self.path(&["definitions"]);
        let mut definitions = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(definitions),
            Err(error) => {
                return Err(ExtensionStoreError::Unreadable(format!(
                    "{}：{error}",
                    dir.display()
                )))
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|error| ExtensionStoreError::Unreadable(error.to_string()))?
                .path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let definition = self.read_json::<ExtensionDefinition>(&path)?;
                validate_stored_definition(&definition)?;
                definitions.push(definition);
            }
        }
        definitions.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Ok(definitions)
    }

    pub fn get_definition(
        &self,
        id: &str,
    ) -> Result<Option<ExtensionDefinition>, ExtensionStoreError> {
        let path = self.path(&["definitions", &format!("{id}.json")]);
        if !path.exists() {
            return Ok(None);
        }
        let definition = self.read_json(&path)?;
        validate_stored_definition(&definition)?;
        Ok(Some(definition))
    }

    /// Creates a new definition and advances the library generation in the
    /// same critical section. There is no blind upsert path: callers that
    /// edit an existing definition must supply its observed revision.
    pub fn create_definition(
        &self,
        definition: &ExtensionDefinition,
    ) -> Result<(), ExtensionStoreError> {
        validate_stored_definition(definition)?;
        let _guard = save_lock();
        let path = self.path(&["definitions", &format!("{}.json", definition.id)]);
        if path.exists() {
            return Err(ExtensionStoreError::Conflict(format!(
                "扩展 {} 已存在；请刷新后编辑",
                definition.id
            )));
        }
        let manifest_before = self.manifest_state_locked()?;
        self.write_json(&path, definition)?;
        if let Err(error) = self.bump_generation_locked() {
            return self.metadata_failure(
                "创建扩展定义",
                error,
                self.restore_file_and_manifest_locked::<ExtensionDefinition>(
                    &path,
                    None,
                    manifest_before.as_ref(),
                ),
            );
        }
        Ok(())
    }

    /// Replaces an existing definition only when the caller still holds the
    /// revision it previewed, then advances the generation atomically.
    pub fn update_definition(
        &self,
        definition: &ExtensionDefinition,
        expected_revision: u64,
    ) -> Result<(), ExtensionStoreError> {
        validate_stored_definition(definition)?;
        let _guard = save_lock();
        let path = self.path(&["definitions", &format!("{}.json", definition.id)]);
        let current = self.read_json::<ExtensionDefinition>(&path)?;
        validate_stored_definition(&current)?;
        if current.revision != expected_revision {
            return Err(ExtensionStoreError::Conflict(
                "扩展已被其他窗口修改；请刷新后重试".to_string(),
            ));
        }
        if expected_revision.checked_add(1) != Some(definition.revision) {
            return Err(ExtensionStoreError::Conflict(
                "扩展修订号不连续；请重新生成编辑草稿".to_string(),
            ));
        }
        let mut next = definition.clone();
        let same_mcp_config = matches!(&next.payload, ExtensionPayload::Mcp(_))
            && next.name == current.name
            && next.payload == current.payload;
        next.mcp_config_revision = same_mcp_config.then(|| current.mcp_config_revision());
        let manifest_before = self.manifest_state_locked()?;
        self.write_json(&path, &next)?;
        if let Err(error) = self.bump_generation_locked() {
            return self.metadata_failure(
                "更新扩展定义",
                error,
                self.restore_file_and_manifest_locked(
                    &path,
                    Some(&current),
                    manifest_before.as_ref(),
                ),
            );
        }
        Ok(())
    }

    // ------------------------------------------------------------ bindings

    pub fn list_bindings(&self) -> Result<Vec<ExtensionBinding>, ExtensionStoreError> {
        let dir = self.path(&["bindings"]);
        let mut bindings = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(bindings),
            Err(error) => {
                return Err(ExtensionStoreError::Unreadable(format!(
                    "{}：{error}",
                    dir.display()
                )))
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|error| ExtensionStoreError::Unreadable(error.to_string()))?
                .path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let binding = self.read_json::<ExtensionBinding>(&path)?;
                require_current_schema("绑定", binding.schema_version)?;
                bindings.push(binding);
            }
        }
        Ok(bindings)
    }

    // ----------------------------------------------------------- baselines

    /// The complete baseline file of one binding: every entry the
    /// application owns at its target.
    pub fn get_baseline_file(
        &self,
        binding_id: &str,
    ) -> Result<Option<ManagedBaselineFile>, ExtensionStoreError> {
        let path = self.path(&["baselines", &format!("{binding_id}.json")]);
        if !path.exists() {
            return Ok(None);
        }
        let baseline: ManagedBaselineFile = self.read_json(&path)?;
        require_current_schema("基线", baseline.schema_version)?;
        Ok(Some(baseline))
    }

    // -------------------------------------------------------------- projects

    pub fn list_projects(&self) -> Result<Vec<ProjectRegistration>, ExtensionStoreError> {
        let dir = self.path(&["projects"]);
        let mut projects = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(projects),
            Err(error) => {
                return Err(ExtensionStoreError::Unreadable(format!(
                    "{}：{error}",
                    dir.display()
                )))
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|error| ExtensionStoreError::Unreadable(error.to_string()))?
                .path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let project = self.read_json::<ProjectRegistration>(&path)?;
                require_current_schema("项目登记", project.schema_version)?;
                projects.push(project);
            }
        }
        projects.sort_by(|a, b| a.root.cmp(&b.root));
        Ok(projects)
    }

    /// Persists a new project registration with one generation change.
    pub fn create_project(&self, project: &ProjectRegistration) -> Result<(), ExtensionStoreError> {
        require_current_schema("项目登记", project.schema_version)?;
        let _guard = save_lock();
        let path = self.path(&["projects", &format!("{}.json", project.id)]);
        if path.exists() {
            return Err(ExtensionStoreError::Conflict(format!(
                "项目 {} 已存在",
                project.id
            )));
        }
        let manifest_before = self.manifest_state_locked()?;
        self.write_json(&path, project)?;
        if let Err(error) = self.bump_generation_locked() {
            return self.metadata_failure(
                "注册项目",
                error,
                self.restore_file_and_manifest_locked::<ProjectRegistration>(
                    &path,
                    None,
                    manifest_before.as_ref(),
                ),
            );
        }
        Ok(())
    }

    // --------------------------------------------------------------- history

    /// History files are persisted [`OperationSnapshot`]s: the record plus
    /// the library's pre/post state, so restoring a specific operation
    /// replays durable facts instead of guessing from current baselines.
    pub fn append_snapshot(&self, snapshot: &OperationSnapshot) -> Result<(), ExtensionStoreError> {
        validate_stored_snapshot(snapshot)?;
        let _guard = save_lock();
        self.write_json(
            &self.path(&["history", &format!("{}.json", snapshot.record.id)]),
            snapshot,
        )
    }

    pub fn get_snapshot(
        &self,
        operation_id: &str,
    ) -> Result<Option<OperationSnapshot>, ExtensionStoreError> {
        let path = self.path(&["history", &format!("{operation_id}.json")]);
        if !path.exists() {
            return Ok(None);
        }
        let snapshot = self.read_json(&path)?;
        validate_stored_snapshot(&snapshot)?;
        Ok(Some(snapshot))
    }

    pub fn list_history(&self) -> Result<Vec<OperationSnapshot>, ExtensionStoreError> {
        let dir = self.path(&["history"]);
        let mut snapshots = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(snapshots),
            Err(error) => {
                return Err(ExtensionStoreError::Unreadable(format!(
                    "{}：{error}",
                    dir.display()
                )))
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|error| ExtensionStoreError::Unreadable(error.to_string()))?
                .path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let snapshot = self.read_json::<OperationSnapshot>(&path)?;
                validate_stored_snapshot(&snapshot)?;
                snapshots.push(snapshot);
            }
        }
        snapshots.sort_by(|a, b| b.record.created_at.cmp(&a.record.created_at));
        Ok(snapshots)
    }

    // ---------------------------------------------------------------- checks

    /// Stores one current-revision result per target. A probe that outlives
    /// its definition cannot overwrite a newer result or recreate deleted data.
    pub fn save_check_if_current(&self, result: &McpCheckResult) -> Result<bool, ExtensionStoreError> {
        let _guard = save_lock();
        if !self.get_definition(&result.definition_id)?
            .is_some_and(|definition| definition.revision == result.definition_revision) {
            return Ok(false);
        }
        let path = self.path(&["checks", &format!("{}.json", result.definition_id)]);
        let mut existing: Vec<McpCheckResult> = if path.exists() {
            self.read_json(&path)?
        } else {
            Vec::new()
        };
        existing.retain(|previous| previous.target != result.target);
        existing.push(result.clone());
        existing.sort_by(|a, b| b.checked_at.cmp(&a.checked_at));
        self.write_json(&path, &existing)?;
        Ok(true)
    }

    pub fn list_checks(&self) -> Result<Vec<McpCheckResult>, ExtensionStoreError> {
        let dir = self.path(&["checks"]);
        let mut results = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(results),
            Err(error) => {
                return Err(ExtensionStoreError::Unreadable(format!(
                    "{}：{error}",
                    dir.display()
                )))
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|error| ExtensionStoreError::Unreadable(error.to_string()))?
                .path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                results.extend(self.read_json::<Vec<McpCheckResult>>(&path)?);
            }
        }
        Ok(results)
    }

    // -------------------------------------------------------- skill library

    /// Persists one immutable content version and returns its directory.
    /// Published versions are content-addressed and never overwritten in
    /// place: the tree is staged, verified against the caller's digest,
    /// then published; an already-present version is verified and kept.

    pub fn transaction_dir(&self, operation_id: &str) -> PathBuf {
        self.path(&["transactions", operation_id])
    }

    /// Whether a durable library receipt proves this operation committed.
    /// Callers use this before replaying an incomplete executor journal.
    pub fn has_commit_marker(&self, operation_id: &str) -> Result<bool, ExtensionStoreError> {
        let path = self.commit_marker_path(operation_id);
        if !path.exists() {
            return Ok(false);
        }
        let marker: LibraryCommitMarker = self.read_json(&path)?;
        if marker.schema_version != EXTENSIONS_SCHEMA_VERSION || marker.operation_id != operation_id
        {
            return Err(ExtensionStoreError::Unsupported(format!(
                "操作 {operation_id} 的扩展库提交凭据无效"
            )));
        }
        Ok(true)
    }

    // --------------------------------------------------------- batch commit

    /// Applies one operation's complete library change under a single lock:
    /// bindings upsert, baseline files, binding deletions, the persisted
    /// operation snapshot, and the generation bump. A normal I/O error is
    /// compensated from the commit's pre-state before it leaves this method;
    /// partial library state is never reported as an ordinary failed apply.

    pub fn pending_transactions(&self) -> Vec<PathBuf> {
        let dir = self.path(&["transactions"]);
        let mut pending = Vec::new();
        let Ok(entries) = fs::read_dir(&dir) else {
            return pending;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            }
        }
        pending.sort();
        pending
    }

    /// Backup directory for extension operations, beside the provider
    /// backups under the existing backup root.
    pub fn backup_root(&self, operation_id: &str) -> PathBuf {
        self.extension_backup_root.join(operation_id)
    }
}
