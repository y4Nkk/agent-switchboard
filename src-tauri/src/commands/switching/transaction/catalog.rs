//! The Codex model-catalog artifact paired with a client configuration write.
use super::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct CatalogArtifact {
    path: String,
    before: Option<String>,
    after: Option<String>,
}

impl CatalogArtifact {
    pub(in super::super) fn new(
        path: PathBuf,
        before: Option<String>,
        after: Option<String>,
    ) -> Self {
        Self {
            path: path.to_string_lossy().into_owned(),
            before,
            after,
        }
    }
}

pub(super) fn validate_catalog_target(
    target: &Path,
    catalog: Option<&CatalogArtifact>,
) -> Result<(), String> {
    let Some(catalog) = catalog else {
        return Ok(());
    };
    let parent = target
        .parent()
        .ok_or_else(|| "Codex 配置目录无效".to_string())?;
    let path = Path::new(&catalog.path);
    let file_name = path.file_name().and_then(|name| name.to_str());
    if path.parent() != Some(parent)
        || !file_name.is_some_and(|name| {
            name.starts_with("agent-switchboard-codex-") && name.ends_with(".json")
        })
    {
        return Err("Codex 模型目录事务目标无效".to_string());
    }
    Ok(())
}

pub(super) fn restore_catalog(intent: &SwitchIntent) -> Result<(), String> {
    let Some(catalog) = &intent.catalog else {
        return Ok(());
    };
    let path = Path::new(&catalog.path);
    match &catalog.before {
        Some(content) => crate::config_store::write_json_atomic(path, content),
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("无法恢复 Codex 模型目录文件".to_string()),
        },
    }
}

pub(super) fn verify_catalog_before(intent: &SwitchIntent) -> Result<(), String> {
    verify_catalog_snapshot(intent, |catalog| &catalog.before,
        "Codex 模型目录已变化，保留回填恢复记录")
}

pub(super) fn verify_catalog_after(intent: &SwitchIntent) -> Result<(), String> {
    verify_catalog_snapshot(intent, |catalog| &catalog.after,
        "Codex 模型目录文件与已提交客户端配置不一致，保留事务等待恢复")
}

fn verify_catalog_snapshot(
    intent: &SwitchIntent,
    snapshot: impl FnOnce(&CatalogArtifact) -> &Option<String>,
    message: &str,
) -> Result<(), String> {
    let Some(catalog) = &intent.catalog else {
        return Ok(());
    };
    match (snapshot(catalog), std::fs::read_to_string(&catalog.path)) {
        (Some(expected), Ok(content)) if content == *expected => Ok(()),
        (None, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(message.into()),
    }
}

pub(in super::super) fn stage_catalog(
    state: &LocalState,
    gateway: &GatewayController,
    catalog: Option<&CatalogArtifact>,
) -> Result<(), CommandError> {
    if let Some(catalog) = catalog {
        if let Err(failure) = apply_catalog_artifact(catalog) {
            super::recover(state, gateway).map_err(super::error)?;
            return Err(failure);
        }
    }
    Ok(())
}

pub(in super::super) fn apply_catalog_artifact(
    catalog: &CatalogArtifact,
) -> Result<(), CommandError> {
    match &catalog.after {
        Some(content) => crate::config_store::write_json_atomic(Path::new(&catalog.path), content)
            .map_err(|error| CommandError::new("codex-catalog-write-failed", error)),
        None => match std::fs::remove_file(&catalog.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CommandError::keyed(
                "codex-catalog-remove-failed",
                "errors.sw.ownedCodexCatalogRemoveFailed",
                "无法清理 ASB 管理的 Codex 模型目录文件",
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::GatewayActivationSnapshot;

    fn intent(path: PathBuf, before: Option<&str>, after: Option<&str>) -> SwitchIntent {
        SwitchIntent {
            version: 2,
            app: AppKind::Codex,
            target: path
                .with_file_name("config.toml")
                .to_string_lossy()
                .into_owned(),
            profile_id: None,
            profile_hash: None,
            before_hash: String::new(),
            before_existed: true,
            after_hash: String::new(),
            after_existed: true,
            previous_route: GatewayActivationSnapshot {
                app: AppKind::Codex,
                route: None,
            },
            previous_write: None,
            catalog: Some(CatalogArtifact::new(
                path,
                before.map(str::to_string),
                after.map(str::to_string),
            )),
            codex_backfill: None,
            auth: None,
            operation: WriteOperation::Restore,
            client_settings: None,
        }
    }

    #[test]
    fn catalog_preimage_restores_an_existing_projection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-switchboard-codex-a.json");
        let intent = intent(path.clone(), Some("old"), Some("new"));
        apply_catalog_artifact(intent.catalog.as_ref().unwrap()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");

        restore_catalog(&intent).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    }

    #[test]
    fn catalog_preimage_removes_a_new_projection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-switchboard-codex-a.json");
        let intent = intent(path.clone(), None, Some("new"));
        apply_catalog_artifact(intent.catalog.as_ref().unwrap()).unwrap();
        verify_catalog_after(&intent).unwrap();

        restore_catalog(&intent).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn catalog_cleanup_removes_only_the_recorded_projection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-switchboard-codex-a.json");
        std::fs::write(&path, "owned").unwrap();
        let intent = intent(path.clone(), Some("owned"), None);

        apply_catalog_artifact(intent.catalog.as_ref().unwrap()).unwrap();
        verify_catalog_after(&intent).unwrap();
        assert!(!path.exists());
        restore_catalog(&intent).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "owned");
    }
}
