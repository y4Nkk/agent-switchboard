//! Read-only local discovery.
//!
//! This module never touches the filesystem: the caller injects a read
//! function and the paths to inspect. The report contains parsed facts and
//! warnings only — no raw file content — and anything secret-shaped is
//! redacted.

use crate::contracts::{
    AppKind, ClaudeModelSettings, ImportProposal, ModelOptions, ProviderDraft, RouteMode,
    RouteState,
};
use toml_edit::Item;

/// Paths the caller wants inspected, as display strings.
pub struct DiscoveryPaths {
    pub codex: String,
    pub codex_auth: String,
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

impl DiscoveryReport {
    /// The copy persisted in the app-owned discovery cache: identical display
    /// facts with credentials cleared. Import always re-derives the draft from
    /// the live files, so a cached proposal never needs the secret.
    pub fn cached_display(&self) -> DiscoveryReport {
        let mut copy = self.clone();
        for proposal in &mut copy.import_proposals {
            proposal.draft.api_key = String::new();
        }
        copy
    }
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
    let claude_import_error = (app == AppKind::Claude)
        .then(|| claude_import_model_fields(&route).err())
        .flatten();
    let importable = if route.route_mode == RouteMode::Official {
        true
    } else {
        match app {
            AppKind::Codex => codex_import_is_supported(text, &route),
            AppKind::Claude => route.base_url.is_some() && claude_import_error.is_none(),
        }
    };
    if app == AppKind::Codex && route.route_mode == RouteMode::Custom && !importable {
        warnings.push("当前 Codex 配置无法作为供应商档案导入".to_string());
    }
    if app == AppKind::Claude && route.route_mode == RouteMode::Custom {
        if let Some(error) = claude_import_error {
            warnings.push(format!("当前 Claude 配置无法作为供应商档案导入：{error}"));
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
    if route.route_mode != RouteMode::Custom || route.base_url.is_none() {
        return false;
    }
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
        return false;
    };
    let provider_id = doc
        .as_table()
        .get("model_provider")
        .and_then(Item::as_value)
        .and_then(|value| value.as_str());
    provider_id == Some(crate::adapter::codex::OFFICIAL_PROVIDER)
}

fn codex_auth_api_key(text: Option<&str>) -> Option<String> {
    let root: serde_json::Value = serde_json::from_str(text?).ok()?;
    (root.get("auth_mode").and_then(serde_json::Value::as_str) == Some("apikey"))
        .then(|| {
            root.get("OPENAI_API_KEY")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

/// Converts only the externally valid Claude wire spelling into the profile
/// contract. The resulting profile never carries a `[1m]` suffix in a model
/// string; enabled context lives in the explicit boolean fields.
fn claude_import_model_fields(
    route: &RouteState,
) -> Result<(Option<String>, Option<ModelOptions>), String> {
    let (model, primary_one_m) =
        crate::claude_model::parse_optional_model(route.model.as_deref(), "主模型", true)?;
    let (haiku_model, _) =
        crate::claude_model::parse_optional_model(route.haiku_model.as_deref(), "Haiku 档", false)?;
    let (sonnet_model, sonnet_one_m) = crate::claude_model::parse_optional_model(
        route.sonnet_model.as_deref(),
        "Sonnet 档",
        true,
    )?;
    let (opus_model, opus_one_m) =
        crate::claude_model::parse_optional_model(route.opus_model.as_deref(), "Opus 档", true)?;
    let available_models = match route.available_models.as_ref() {
        Some(models) => {
            for model in models {
                crate::claude_model::parse_model(model, "可选模型列表", false)?;
            }
            Some(models.clone())
        }
        None => None,
    };
    let has_settings = primary_one_m
        || haiku_model.is_some()
        || sonnet_model.is_some()
        || sonnet_one_m
        || opus_model.is_some()
        || opus_one_m
        || available_models.is_some();
    let model_options = has_settings.then(|| {
        ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m,
            haiku_model,
            sonnet_model,
            sonnet_one_m,
            opus_model,
            opus_one_m,
            available_models,
        })
    });
    Ok((model, model_options))
}

fn inspect_codex(text: &str) -> (bool, Vec<String>) {
    let parsed = text.parse::<toml_edit::DocumentMut>();
    let Ok(doc) = parsed else {
        return (false, vec![]); // caller already classified parse errors
    };
    let provider_id = doc
        .as_table()
        .get("model_provider")
        .and_then(Item::as_value)
        .and_then(|value| value.as_str())
        .unwrap_or(crate::adapter::codex::OFFICIAL_PROVIDER);
    let managed = provider_id == crate::adapter::codex::OFFICIAL_PROVIDER
        && doc
            .as_table()
            .get("openai_base_url")
            .and_then(Item::as_value)
            .and_then(|value| value.as_str())
            .is_some();

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
pub fn import_proposal(
    file: &DiscoveredFile,
    text: Option<&str>,
    codex_auth: Option<&str>,
) -> Option<ImportProposal> {
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
    if route.route_mode == RouteMode::Official {
        let name = match file.app {
            AppKind::Codex => "Codex 官方登录",
            AppKind::Claude => "Claude 官方登录",
        };
        return Some(ImportProposal {
            app: file.app,
            draft: ProviderDraft {
                app: file.app,
                route_mode: RouteMode::Official,
                name: name.to_string(),
                model: None,
                base_url: None,
                api_key: String::new(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
            },
            basis: format!("由当前 {name} 状态生成；凭据继续由客户端管理"),
        });
    }
    match file.app {
        AppKind::Codex => {
            let key = codex_auth_api_key(codex_auth)?;
            let model_options = route
                .codex_model_options
                .clone()
                .filter(|options| options.context_window.is_some())
                .map(ModelOptions::Codex);
            Some(ImportProposal {
                app: AppKind::Codex,
                draft: ProviderDraft {
                    app: AppKind::Codex,
                    route_mode: RouteMode::Custom,
                    name: route
                        .provider_name
                        .clone()
                        .unwrap_or_else(|| "当前 Codex 配置".to_string()),
                    model: route.model.clone(),
                    base_url: route.base_url.clone(),
                    api_key: key,
                    model_options,
                    notes: None,
                    website_url: None,
                    usage_query: None,
                },
                basis: "由当前 Codex 可转换配置生成".to_string(),
            })
        }
        AppKind::Claude => {
            let root: serde_json::Value = serde_json::from_str(text).ok()?;
            let key = root
                .pointer("/env/ANTHROPIC_AUTH_TOKEN")
                .or_else(|| root.pointer("/env/ANTHROPIC_API_KEY"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())?;
            let (model, model_options) = claude_import_model_fields(route).ok()?;
            Some(ImportProposal {
                app: AppKind::Claude,
                draft: ProviderDraft {
                    app: AppKind::Claude,
                    route_mode: RouteMode::Custom,
                    name: "当前 Claude 配置".to_string(),
                    model,
                    base_url: route.base_url.clone(),
                    api_key: key.to_string(),
                    model_options,
                    notes: None,
                    website_url: None,
                    usage_query: None,
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
    let codex_auth = read(&paths.codex_auth);
    let claude_text = read(&paths.claude);
    let mut codex = inspect_read(AppKind::Codex, &paths.codex, codex_text.clone());
    if let DiscoveredState::Ok {
        route,
        warnings,
        importable,
        ..
    } = &mut codex.state
    {
        if route.route_mode == RouteMode::Custom
            && codex_auth_api_key(codex_auth.as_ref().ok().and_then(|text| text.as_deref()))
                .is_none()
        {
            *importable = false;
            warnings.push("当前 Codex API-key 登录缓存不可导入".to_string());
        }
    }
    let claude = inspect_read(AppKind::Claude, &paths.claude, claude_text.clone());
    let import_proposals = [
        (import_proposal(
            &codex,
            codex_text.ok().flatten().as_deref(),
            codex_auth.ok().flatten().as_deref(),
        )),
        (import_proposal(&claude, claude_text.ok().flatten().as_deref(), None)),
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
    use crate::test_support::{CLAUDE_JSON, CODEX_AUTH_JSON, CODEX_TOML};

    #[test]
    fn missing_files_are_reported_not_guessed() {
        let report = discover(
            &DiscoveryPaths {
                codex: "~/.codex/config.toml".into(),
                codex_auth: "~/.codex/auth.json".into(),
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
                codex_auth: "a".into(),
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
                codex_auth: "a".into(),
                claude: "s".into(),
            },
            |p| {
                if p == "c" {
                    Ok(Some(CODEX_TOML.to_string()))
                } else if p == "a" {
                    Ok(Some(CODEX_AUTH_JSON.to_string()))
                } else {
                    Ok(Some(CLAUDE_JSON.to_string()))
                }
            },
        );
        let DiscoveredState::Ok { route, managed, .. } = &report.codex.state else {
            panic!("codex should be ok");
        };
        assert!(managed);
        assert_eq!(route.route_mode, RouteMode::Custom);
        assert_eq!(route.provider_name, None);
        assert_eq!(route.wire_api, None);
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

        // Only the cache copy is redacted; the live report keeps its keys.
        let cached = report.cached_display();
        assert_eq!(cached.codex, report.codex);
        assert_eq!(cached.claude, report.claude);
        for proposal in &cached.import_proposals {
            assert_eq!(proposal.draft.api_key, "");
        }
        for proposal in &report.import_proposals {
            assert!(!proposal.draft.api_key.is_empty());
        }
    }

    #[test]
    fn importing_claude_configuration_decodes_lowercase_one_m_into_semantic_state() {
        let current = r#"{"env":{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"TEST_CLAUDE_IMPORT_KEY","ANTHROPIC_MODEL":"claude-opus-4-1[1m]","ANTHROPIC_DEFAULT_SONNET_MODEL":"claude-sonnet-4-6[1m]","ANTHROPIC_DEFAULT_OPUS_MODEL":"claude-opus-4-1[1m]"}}"#;
        let file = inspect(AppKind::Claude, "s", Some(current));
        let proposal = import_proposal(&file, Some(current), None).expect("must be importable");

        assert_eq!(proposal.draft.model.as_deref(), Some("claude-opus-4-1"));
        let Some(ModelOptions::Claude(settings)) = proposal.draft.model_options.as_ref() else {
            panic!("Claude model settings should be imported");
        };
        assert!(settings.primary_one_m);
        assert_eq!(settings.sonnet_model.as_deref(), Some("claude-sonnet-4-6"));
        assert!(settings.sonnet_one_m);
        assert_eq!(settings.opus_model.as_deref(), Some("claude-opus-4-1"));
        assert!(settings.opus_one_m);
        assert!(proposal.draft.validate().is_ok());
    }

    #[test]
    fn importing_claude_configuration_rejects_uppercase_one_m_before_import() {
        let current = r#"{"env":{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"TEST_CLAUDE_IMPORT_KEY","ANTHROPIC_MODEL":"claude-opus-4-1[1M]"}}"#;
        let file = inspect(AppKind::Claude, "s", Some(current));
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
            .any(|warning| warning.contains("1M 标记无效")));
        assert!(import_proposal(&file, Some(current), None).is_none());
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
    fn non_openai_codex_provider_is_not_imported() {
        let current = r#"
model_provider = "gateway"
experimental_bearer_token = "TEST_CODEX_IMPORT_KEY"
"#;
        let file = inspect(AppKind::Codex, "c", Some(current));
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
            .any(|w| w.contains("无法作为供应商档案导入")));
        assert!(import_proposal(&file, Some(current), None).is_none());
    }

    #[test]
    fn default_responses_protocol_and_codex_run_options_are_imported() {
        let current = CODEX_TOML.replace(
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
        assert_eq!(route.wire_api, None);
        let proposal = import_proposal(&file, Some(&current), Some(CODEX_AUTH_JSON))
            .expect("must be importable");
        let Some(ModelOptions::Codex(options)) = proposal.draft.model_options else {
            panic!("Codex run options should be retained");
        };
        // Effort, summary, and verbosity are general settings now; the
        // profile keeps only the context window.
        assert_eq!(options.context_window, Some(272000));
    }

    #[test]
    fn codex_official_route_is_imported_without_reading_a_token() {
        let current = "model = \"gpt-5\"\nthreads = 8\n";
        let file = inspect(AppKind::Codex, "c", Some(&current));

        let DiscoveredState::Ok {
            warnings,
            importable,
            ..
        } = &file.state
        else {
            panic!("should parse");
        };
        assert!(*importable);
        assert!(warnings.is_empty());
        let proposal =
            import_proposal(&file, Some(&current), None).expect("official route imports");
        assert_eq!(proposal.draft.route_mode, RouteMode::Official);
        assert!(proposal.draft.api_key.is_empty());
        assert!(proposal.draft.base_url.is_none());
    }

    #[test]
    fn read_errors_are_distinct_from_missing_files() {
        let report = discover(
            &DiscoveryPaths {
                codex: "c".into(),
                codex_auth: "a".into(),
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
    fn claude_official_route_discards_custom_model_tiers_on_import() {
        let current = r#"{"env":{"ANTHROPIC_DEFAULT_HAIKU_MODEL":"claude-haiku-4"}}"#;
        let file = inspect(AppKind::Claude, "s", Some(current));
        let DiscoveredState::Ok { importable, .. } = &file.state else {
            panic!("should parse");
        };
        assert!(*importable);
        let proposal = import_proposal(&file, Some(current), None).expect("official route imports");
        assert_eq!(proposal.draft.route_mode, RouteMode::Official);
        assert!(proposal.draft.model.is_none());
    }
}
