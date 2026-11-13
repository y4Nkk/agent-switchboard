//! Codex 通用配置片段的存储层：原始 TOML 文本存放在独立文件
//! `codex/common-fragment.json`（版本 + 文本），修订号是整文件的 SHA-256，
//! 保存走 CAS。启用策略与可视化开关共用 `common-config-policy.json` 的
//! `disabled_profile_ids`——一个开关同时覆盖可视化键与任意片段。
use asb_core::contracts::CodexCommonFragment;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FragmentStore {
    pub version: u8,
    pub text: String,
}
impl Default for FragmentStore {
    fn default() -> Self {
        Self {
            version: 1,
            text: String::new(),
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FragmentView {
    pub text: String,
    pub revision: String,
}
fn path(root: &Path) -> std::path::PathBuf {
    root.join("codex/common-fragment.json")
}
fn load(root: &Path) -> Result<(FragmentStore, String), String> {
    let raw =
        crate::config_store::read_optional(&path(root)).map_err(|_| "Codex 通用配置片段不可读")?;
    let store: FragmentStore = match &raw {
        None => FragmentStore::default(),
        Some(raw) => serde_json::from_str(raw).map_err(|_| "Codex 通用配置片段无效")?,
    };
    if store.version != 1 {
        return Err("Codex 通用配置片段版本无效".into());
    }
    Ok((store, asb_switch::sha256_hex(raw.as_deref().unwrap_or(""))))
}
pub(crate) fn view(root: &Path) -> Result<FragmentView, String> {
    let (store, revision) = load(root)?;
    Ok(FragmentView {
        text: store.text,
        revision,
    })
}
#[allow(dead_code)] // reserved: fragment write path (test-covered)
pub(crate) fn save(
    state: &crate::local_state::LocalState,
    text: &str,
    expected: &str,
) -> Result<FragmentView, String> {
    let root = state.root();
    let _guard = super::LOCK.lock().map_err(|_| "Codex 通用配置片段锁不可用")?;
    let (_, revision) = load(root)?;
    if revision != expected {
        return Err("Codex 通用配置片段已变化，请重新读取".into());
    }
    if !text.trim().is_empty() {
        asb_core::adapter::codex::validate_fragment(text).map_err(|e| e.to_string())?;
        // E02 争写守卫：片段声明的 mcp_servers 键不得与已绑定 Codex 用户
        // 配置的托管 MCP 键相交——否则切换投影（合并/剥离片段）与扩展执行
        // 器（写入/移除条目）会互相覆盖同一字段。
        let declared =
            asb_core::adapter::codex::declared_mcp_server_keys(text).map_err(|e| e.to_string())?;
        if !declared.is_empty() {
            let managed = managed_codex_mcp_keys(state)?;
            if let Some(key) = declared.iter().find(|key| managed.contains(key)) {
                return Err(format!(
                    "通用配置片段声明了 mcp_servers.{key}，与已托管的 Codex MCP 绑定冲突：请先停用并移除该绑定，或为片段改用其他键名"
                ));
            }
        }
    }
    let store = FragmentStore {
        version: 1,
        text: text.to_string(),
    };
    crate::config_store::write_json_atomic(
        &path(root),
        &serde_json::to_string_pretty(&store).map_err(|_| "Codex 通用配置片段无法序列化")?,
    )?;
    view(root)
}

/// 已绑定到 Codex 用户配置（App 作用域）的托管 MCP 键名。项目作用域写的是
/// 另一份文档，与用户级片段无关；停用的绑定同样计数——它的键随时可能被
/// 重新启用，且停用本身也要经过这些键。
#[allow(dead_code)] // reserved: fragment write path (test-covered)
fn managed_codex_mcp_keys(state: &crate::local_state::LocalState) -> Result<Vec<String>, String> {
    let store = crate::extensions::store::ExtensionStore::from_state(state);
    let bindings = store
        .list_bindings()
        .map_err(|_| "Codex 扩展库不可读，无法校验通用配置片段冲突".to_string())?;
    Ok(bindings
        .into_iter()
        .filter(|binding| {
            binding.target.client() == asb_core::AppKind::Codex
                && matches!(
                    binding.target,
                    asb_core::extensions::contracts::ExtensionTarget::App { .. }
                )
        })
        .filter_map(|binding| binding.native_key)
        .collect())
}
/// 解析某一档案的片段计划：空片段返回 None；启用与否由共用策略决定。
pub(crate) fn resolve(root: &Path, profile_id: &str) -> Result<Option<CodexCommonFragment>, String> {
    let (store, _) = load(root)?;
    if store.text.trim().is_empty() {
        return Ok(None);
    }
    let (policy, _) = super::load(root)?;
    Ok(Some(CodexCommonFragment {
        text: store.text,
        enabled: !policy.disabled_profile_ids.contains(profile_id),
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::extensions::contracts::{
        DesiredState, ExtensionBinding, ExtensionTarget, EXTENSIONS_SCHEMA_VERSION,
    };

    #[test]
    fn save_validates_rejects_stale_revisions_and_resolves_per_profile() {
        let directory = tempfile::tempdir().unwrap();
        let state = crate::local_state::LocalState::from_root(directory.path().join("state"));
        let root = state.root();
        let initial = view(root).unwrap();
        assert_eq!(initial.text, "");
        // 受管键被拒绝
        assert!(save(&state, "approval_policy = 'on-request'\n", &initial.revision).is_err());
        // 合法片段保存
        let saved =
            save(&state, "[mcp_servers.tools]\ncommand = 'node'\n", &initial.revision).unwrap();
        assert!(saved.text.contains("mcp_servers"));
        // 修订 CAS
        assert!(save(&state, "[other]\nkey = 1\n", &initial.revision).is_err());
        // 逐档案启用：默认启用；禁用后 enabled=false，文本仍随计划携带以便剥离
        let file = state
            .configuration()
            .create_codex_provider(uuid::Uuid::new_v4().to_string(),
                crate::codex_common::test_draft(),
            )
            .unwrap();
        let fragment = resolve(root, &file.profile.id).unwrap().unwrap();
        assert!(fragment.enabled);
        // set_enabled 的 CAS 针对策略文件修订，与片段文件修订相互独立
        let policy_revision = crate::codex_common::view(&state).unwrap().revision;
        let _disabled_view =
            crate::codex_common::set_enabled(&state, &file.profile.id, false, &policy_revision)
                .unwrap();
        let fragment = resolve(root, &file.profile.id).unwrap().unwrap();
        assert!(!fragment.enabled);
        assert!(fragment.text.contains("mcp_servers"));
        // 清空文本后 resolve 返回 None
        let current = view(root).unwrap();
        save(&state, "", &current.revision).unwrap();
        assert!(resolve(root, &file.profile.id).unwrap().is_none());
    }

    fn write_binding(
        bindings_dir: &Path,
        id: &str,
        target: ExtensionTarget,
        native_key: &str,
        desired: DesiredState,
    ) {
        let binding = ExtensionBinding {
            schema_version: EXTENSIONS_SCHEMA_VERSION,
            id: id.to_string(),
            resource_id: format!("ext-{id}"),
            target,
            native_key: Some(native_key.to_string()),
            deploy_name: None,
            desired,
            locked_digest: None,
            last_applied_revision: Some(1),
            updated_at: "2026-09-16T00:00:00Z".to_string(),
        };
        std::fs::write(
            bindings_dir.join(format!("{id}.json")),
            serde_json::to_string(&binding).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn fragment_save_rejects_mcp_keys_bound_to_codex_user_config() {
        let directory = tempfile::tempdir().unwrap();
        let state = crate::local_state::LocalState::from_root(directory.path().join("state"));
        let root = state.root();
        let bindings_dir = root.join("extensions/bindings");
        std::fs::create_dir_all(&bindings_dir).unwrap();

        let initial = view(root).unwrap();
        write_binding(
            &bindings_dir,
            "bind-codex",
            ExtensionTarget::App {
                client: asb_core::AppKind::Codex,
            },
            "tools",
            DesiredState::Enabled,
        );
        // 托管键被拒绝，且拒绝不消耗 CAS 修订。
        let error =
            save(&state, "[mcp_servers.tools]\ncommand = 'node'\n", &initial.revision)
                .unwrap_err();
        assert!(error.contains("mcp_servers.tools"), "{error}");
        assert!(save(&state, "[mcp_servers.other]\ncommand = 'node'\n", &initial.revision).is_ok());

        // 停用的绑定同样保留键名：它随时可能被重新启用，停用流程本身也要
        // 经过这些条目。
        let after_first = view(root).unwrap();
        write_binding(
            &bindings_dir,
            "bind-disabled",
            ExtensionTarget::App {
                client: asb_core::AppKind::Codex,
            },
            "halted",
            DesiredState::Disabled,
        );
        assert!(save(&state, "[mcp_servers.halted]\ncommand = 'node'\n", &after_first.revision)
            .is_err());

        // Claude 绑定写的是另一个应用的文档；Codex 项目作用域写的是项目级
        // 文档——都与用户级 config.toml 无关，不构成冲突。
        let after_second = view(root).unwrap();
        write_binding(
            &bindings_dir,
            "bind-claude",
            ExtensionTarget::App {
                client: asb_core::AppKind::Claude,
            },
            "claude_tools",
            DesiredState::Enabled,
        );
        write_binding(
            &bindings_dir,
            "bind-project",
            ExtensionTarget::ProjectShared {
                client: asb_core::AppKind::Codex,
                project_id: "proj-1".to_string(),
            },
            "proj_tools",
            DesiredState::Enabled,
        );
        let saved = save(
            &state,
            "[mcp_servers.claude_tools]\ncommand = 'node'\n[mcp_servers.proj_tools]\ncommand = 'node'\n",
            &after_second.revision,
        )
        .unwrap();
        assert!(saved.text.contains("proj_tools"));
    }
}
