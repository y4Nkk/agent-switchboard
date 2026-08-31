//! Read-only local discovery.
//!
//! This module never touches the filesystem: the caller injects a read
//! function and the paths to inspect. The report contains parsed facts and
//! warnings only — no raw file content — and anything secret-shaped is
//! redacted.

use crate::contracts::{
    AppKind, ClaudeModelSettings, ImportProposal, ModelOptions, ProviderDraft, RouteState,
};
use toml_edit::Item;

/// Paths the caller wants inspected, as display strings.
pub struct DiscoveryPaths {
    pub codex: String,
    pub claude: String,
}

/// What we found for one file.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredFile {
    pub app: AppKind,
    pub path: String,
    pub exists: bool,
    pub state: DiscoveredState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DiscoveredState {
    Missing,
    ReadError {
        message: String,
    },
    ParseError {
        message: String,
        line: Option<usize>,
    },
    Ok {
        route: RouteState,
        /// Whether the file already contains app-managed keys.
        managed: bool,
        /// Readiness warnings (unsupported shapes, plaintext secrets…).
        warnings: Vec<String>,
        /// Whether this exact configuration shape can be converted into an
        /// app-owned profile without dropping settings.
        importable: bool,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryReport {
    pub codex: DiscoveredFile,
    pub claude: DiscoveredFile,
    pub import_proposals: Vec<ImportProposal>,
}

/// Inspects configuration content and classifies it. `text` is the raw file
/// content, already read by the caller.
pub fn inspect(app: AppKind, path: &str, text: Option<&str>) -> DiscoveredFile {
    let Some(text) = text else {
        return DiscoveredFile {
            app,
            path: path.to_string(),
            exists: false,
            state: DiscoveredState::Missing,
        };
    };

    if let Err(e) = crate::adapter::validate_syntax(app, text) {
        return DiscoveredFile {
            app,
            path: path.to_string(),
            exists: true,
            state: DiscoveredState::ParseError {
                message: e.message,
                line: e.line,
            },
        };
    }

    let route = crate::adapter::route_state(app, text);
    let (managed, mut warnings) = match app {
        AppKind::Codex => inspect_codex(text),
        AppKind::Claude => inspect_claude(text),
    };
    if managed && matches!(route.provider_name, None) {
        warnings.push("存在托管键但未识别到供应商名称".to_string());
    }
    let importable = match app {
        AppKind::Codex => codex_import_is_supported(text, &route),
        AppKind::Claude => claude_import_is_supported(&route),
    };
    if app == AppKind::Codex && route.provider_name.is_some() && !importable {
        if route.wire_api.as_deref() != Some("responses") {
            warnings.push("当前 Codex 供应商未使用 responses 协议，无法导入".to_string());
        } else {
            warnings.push("当前 Codex 供应商包含本应用无法完整表示的设置，无法导入".to_string());
        }
    }
    DiscoveredFile {
        app,
        path: path.to_string(),
        exists: true,
        state: DiscoveredState::Ok {
            route,
            managed,
            warnings,
            importable,
        },
    }
}

fn codex_import_is_supported(text: &str, route: &RouteState) -> bool {
    if route.provider_name.is_none()
        || route.base_url.is_none()
        || route.wire_api.as_deref() != Some("responses")
    {
        return false;
    }
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
        return false;
    };
    let Some(provider_id) = doc
        .as_table()
        .get("model_provider")
        .and_then(Item::as_value)
        .and_then(|value| value.as_str())
    else {
        return false;
    };
    let Some(provider) = doc
        .as_table()
        .get("model_providers")
        .and_then(Item::as_table_like)
        .and_then(|providers| providers.get(provider_id))
        .and_then(Item::as_table_like)
    else {
        return false;
    };
    let has_only_supported_keys = provider
        .iter()
        .all(|(key, _)| matches!(key, "name" | "base_url" | "api_key" | "wire_api"));
    has_only_supported_keys
}

fn claude_import_is_supported(route: &RouteState) -> bool {
    // Official login is not a profile kind; only a custom endpoint is
    // importable. Model tiers alone no longer qualify.
    route.base_url.is_some()
}

fn inspect_codex(text: &str) -> (bool, Vec<String>) {
    let parsed = text.parse::<toml_edit::DocumentMut>();
    let Ok(doc) = parsed else {
        return (false, vec![]); // caller already classified parse errors
    };
    let managed = doc
        .as_table()
        .get("model_providers")
        .and_then(|item| item.as_table_like())
        .map(|t| t.contains_key("asb"))
        .unwrap_or(false);

    (managed, vec![])
}

fn inspect_claude(text: &str) -> (bool, Vec<String>) {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(text) else {
        return (false, vec![]);
    };
    let managed = root
        .pointer("/env/ANTHROPIC_BASE_URL")
        .and_then(|v| v.as_str())
        .is_some();
    let mut warnings = Vec::new();
    if root
        .pointer("/env/ANTHROPIC_AUTH_TOKEN")
        .and_then(|v| v.as_str())
        .is_some()
    {
        warnings.push("settings.json 的 env 中存在明文 ANTHROPIC_AUTH_TOKEN".to_string());
    }
    (managed, warnings)
}

/// Builds an import proposal from a discovered file and its locally-read raw
/// configuration. The API key enters only the returned draft; route state,
/// warnings and diagnostics never carry its value.
pub fn import_proposal(file: &DiscoveredFile, text: Option<&str>) -> Option<ImportProposal> {
    let text = text?;
    let DiscoveredState::Ok {
        route, importable, ..
    } = &file.state
    else {
        return None;
    };
    if !importable {
        return None;
    }
    match file.app {
        AppKind::Codex => {
            let doc = text.parse::<toml_edit::DocumentMut>().ok()?;
            let provider_id = doc
                .as_table()
                .get("model_provider")
                .and_then(Item::as_value)
                .and_then(|value| value.as_str())?;
            let key = doc
                .as_table()
                .get("experimental_bearer_token")
                .and_then(Item::as_value)
                .and_then(|value| value.as_str())?;
            let model_options = route
                .codex_model_options
                .clone()
                .filter(|options| options.context_window.is_some())
                .map(ModelOptions::Codex);
            Some(ImportProposal {
                app: AppKind::Codex,
                draft: ProviderDraft {
                    app: AppKind::Codex,
                    name: route
                        .provider_name
                        .clone()
                        .unwrap_or_else(|| provider_id.to_string()),
                    model: route.model.clone(),
                    base_url: route.base_url.clone(),
                    api_key: key.to_string(),
                    model_options,
                    notes: None,
                    website_url: None,
                },
                basis: "由当前 Codex 自定义供应商配置生成".to_string(),
            })
        }
        AppKind::Claude => {
            let root: serde_json::Value = serde_json::from_str(text).ok()?;
            let key = root
                .pointer("/env/ANTHROPIC_AUTH_TOKEN")
                .or_else(|| root.pointer("/env/ANTHROPIC_API_KEY"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())?;
            let has_tiers = route.haiku_model.is_some()
                || route.sonnet_model.is_some()
                || route.opus_model.is_some()
                || route.available_models.is_some();
            Some(ImportProposal {
                app: AppKind::Claude,
                draft: ProviderDraft {
                    app: AppKind::Claude,
                    name: "当前 Claude 配置".to_string(),
                    model: route.model.clone(),
                    base_url: route.base_url.clone(),
                    api_key: key.to_string(),
                    model_options: has_tiers.then(|| {
                        ModelOptions::Claude(ClaudeModelSettings {
                            haiku_model: route.haiku_model.clone(),
                            sonnet_model: route.sonnet_model.clone(),
                            opus_model: route.opus_model.clone(),
                            available_models: route.available_models.clone(),
                        })
                    }),
                    notes: None,
                    website_url: None,
                },
                basis: "由当前 Claude 配置的模型与服务地址生成".to_string(),
            })
        }
    }
}

fn inspect_read(app: AppKind, path: &str, read: Result<Option<String>, String>) -> DiscoveredFile {
    match read {
        Ok(text) => inspect(app, path, text.as_deref()),
        Err(message) => DiscoveredFile {
            app,
            path: path.to_string(),
            exists: true,
            state: DiscoveredState::ReadError {
                message: crate::adapter::scrub_message(message),
            },
        },
    }
}

/// Runs discovery over injected reads. `read` returns content, a missing-file
/// result, or a safe read error. Nothing is ever written.
pub fn discover(
    paths: &DiscoveryPaths,
    read: impl Fn(&str) -> Result<Option<String>, String>,
) -> DiscoveryReport {
    let codex_text = read(&paths.codex);
    let claude_text = read(&paths.claude);
    let codex = inspect_read(AppKind::Codex, &paths.codex, codex_text.clone());
    let claude = inspect_read(AppKind::Claude, &paths.claude, claude_text.clone());
    let import_proposals = [
        (import_proposal(&codex, codex_text.ok().flatten().as_deref())),
        (import_proposal(&claude, claude_text.ok().flatten().as_deref())),
    ]
    .into_iter()
    .flatten()
    .collect();
    DiscoveryReport {
        codex,
        claude,
        import_proposals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{CLAUDE_JSON, CODEX_TOML};

    #[test]
    fn missing_files_are_reported_not_guessed() {
        let report = discover(
            &DiscoveryPaths {
                codex: "~/.codex/config.toml".into(),
                claude: "~/.claude/settings.json".into(),
            },
            |_| Ok(None),
        );
        assert_eq!(report.codex.state, DiscoveredState::Missing);
        assert_eq!(report.claude.state, DiscoveredState::Missing);
        assert!(report.import_proposals.is_empty());
    }

    #[test]
    fn parse_errors_carry_line_and_scrubbed_message() {
        let report = discover(
            &DiscoveryPaths {
                codex: "c".into(),
                claude: "s".into(),
            },
            |p| {
                if p == "c" {
                    Ok(Some("broken [".into()))
                } else {
                    Ok(None)
                }
            },
        );
        match report.codex.state {
            DiscoveredState::ParseError { line, .. } => assert!(line.is_some()),
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn test_configuration_produces_routes_and_imports_api_keys() {
        let report = discover(
            &DiscoveryPaths {
                codex: "c".into(),
                claude: "s".into(),
            },
            |p| {
                if p == "c" {
                    Ok(Some(CODEX_TOML.to_string()))
                } else {
                    Ok(Some(CLAUDE_JSON.to_string()))
                }
            },
        );
        let DiscoveredState::Ok { route, managed, .. } = &report.codex.state else {
            panic!("codex should be ok");
        };
        assert!(managed);
        assert_eq!(route.provider_name.as_deref(), Some("中继 A"));
        assert_eq!(route.wire_api.as_deref(), Some("responses"));
        let codex_proposal = report
            .import_proposals
            .iter()
            .find(|proposal| proposal.app == AppKind::Codex)
            .expect("codex proposal");
        assert_eq!(codex_proposal.draft.api_key, "TEST_CODEX_IMPORT_KEY");
        let claude_proposal = report
            .import_proposals
            .iter()
            .find(|proposal| proposal.app == AppKind::Claude)
            .expect("claude proposal");
        assert_eq!(claude_proposal.draft.api_key, "TEST_CLAUDE_IMPORT_KEY");
    }

    #[test]
    fn plaintext_token_gets_a_warning() {
        let current = CLAUDE_JSON.replace(
            "\"ANTHROPIC_MODEL\": \"claude-sonnet-4\"",
            "\"ANTHROPIC_AUTH_TOKEN\": \"sk-live-0123456789abcdef\"",
        );
        let file = inspect(AppKind::Claude, "s", Some(&current));
        let DiscoveredState::Ok { warnings, .. } = file.state else {
            panic!("should parse");
        };
        assert!(warnings.iter().any(|w| w.contains("ANTHROPIC_AUTH_TOKEN")));
        // The warning text never contains the token itself.
        assert!(!warnings.iter().any(|w| w.contains("sk-live")));
    }

    #[test]
    fn unsupported_codex_protocol_is_not_imported() {
        let current = CODEX_TOML.replace("wire_api = \"responses\"", "wire_api = \"chat\"");
        let file = inspect(AppKind::Codex, "c", Some(&current));
        let DiscoveredState::Ok { warnings, .. } = &file.state else {
            panic!("should parse");
        };
        assert!(warnings.iter().any(|w| w.contains("responses")));
        assert!(import_proposal(&file, Some(&current)).is_none());
    }

    #[test]
    fn default_responses_protocol_and_codex_run_options_are_imported() {
        let current = CODEX_TOML.replace("wire_api = \"responses\"", "").replace(
            "threads = 8",
            "threads = 8\nmodel_reasoning_effort = \"xhigh\"\nmodel_context_window = 272000",
        );
        let file = inspect(AppKind::Codex, "c", Some(&current));

        let DiscoveredState::Ok {
            route, importable, ..
        } = &file.state
        else {
            panic!("should parse");
        };
        assert!(*importable);
        assert_eq!(route.wire_api.as_deref(), Some("responses"));
        let proposal = import_proposal(&file, Some(&current)).expect("must be importable");
        let Some(ModelOptions::Codex(options)) = proposal.draft.model_options else {
            panic!("Codex run options should be retained");
        };
        // Effort, summary, and verbosity are general settings now; the
        // profile keeps only the context window.
        assert_eq!(options.context_window, Some(272000));
    }

    #[test]
    fn codex_provider_with_extra_settings_is_not_imported() {
        let current = CODEX_TOML.replace(
            "wire_api = \"responses\"",
            "wire_api = \"responses\"\nhttp_headers = { X-Provider = \"example\" }",
        );
        let file = inspect(AppKind::Codex, "c", Some(&current));

        let DiscoveredState::Ok {
            warnings,
            importable,
            ..
        } = &file.state
        else {
            panic!("should parse");
        };
        assert!(!importable);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("无法完整表示")));
        assert!(import_proposal(&file, Some(&current)).is_none());
    }

    #[test]
    fn read_errors_are_distinct_from_missing_files() {
        let report = discover(
            &DiscoveryPaths {
                codex: "c".into(),
                claude: "s".into(),
            },
            |path| {
                if path == "c" {
                    Err("无法读取配置文件".into())
                } else {
                    Ok(None)
                }
            },
        );

        assert!(matches!(
            report.codex.state,
            DiscoveredState::ReadError { .. }
        ));
        assert!(report.import_proposals.is_empty());
    }

    #[test]
    fn claude_model_tiers_without_an_endpoint_are_not_importable() {
        let current = r#"{"env":{"ANTHROPIC_DEFAULT_HAIKU_MODEL":"claude-haiku-4"}}"#;
        let file = inspect(AppKind::Claude, "s", Some(current));
        let DiscoveredState::Ok { importable, .. } = &file.state else {
            panic!("should parse");
        };
        assert!(!*importable);
        assert!(import_proposal(&file, Some(&current)).is_none());
    }
}
