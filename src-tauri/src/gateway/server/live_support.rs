//! Shared scaffolding for opt-in external Codex verification.
//!
//! Every path, credential and client document used here comes from a temporary
//! directory or the process environment. Preparation always runs the shipped
//! switch transaction, so the client configuration under test is the one the
//! product actually renders. The provider credential reaches the gateway
//! profile only; it is never rendered into a client configuration.

use crate::codex_probe::codex_executable;
use crate::gateway::{GatewayActivation, GatewayController, GatewayProjection};
use crate::local_state::LocalState;
use asb_core::contracts::{
    AppKind, CodexCapabilities, CodexCatalogEntry, CodexChatEffortMode, CodexChatEffortParameter,
    CodexChatReasoning, CodexChatThinkingParameter, CodexEndpoint, CodexProviderDraft,
    CodexReasoningLevel, CodexUpstream, ConfigValue, ResponsesRequestMode, SettingValue,
};
use asb_core::ownership::{default_client_settings, default_provider_parameters};
use asb_switch::io::FsIo;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub(super) const MODEL: &str = "deepseek-flash";
pub(super) const API_KEY_ENV: &str = "ASB_CODEX_LIVE_API_KEY";

pub(super) struct LiveRoute {
    pub(super) label: &'static str,
    pub(super) upstream: CodexUpstream,
    pub(super) endpoint_variable: &'static str,
}

pub(super) const LIVE_ROUTES: [LiveRoute; 3] = [
    LiveRoute {
        label: "responses",
        upstream: CodexUpstream::Responses,
        endpoint_variable: "ASB_CODEX_LIVE_RESPONSES_URL",
    },
    LiveRoute {
        label: "chat",
        upstream: CodexUpstream::ChatCompletions,
        endpoint_variable: "ASB_CODEX_LIVE_CHAT_URL",
    },
    LiveRoute {
        label: "anthropic",
        upstream: CodexUpstream::AnthropicMessages,
        endpoint_variable: "ASB_CODEX_LIVE_ANTHROPIC_URL",
    },
];

pub(super) fn credential() -> String {
    required_environment(API_KEY_ENV)
}

fn required_environment(name: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("external Codex test requires {name}"))
}

fn endpoint(route: &LiveRoute) -> String {
    required_environment(route.endpoint_variable)
}

pub(super) struct LiveSandbox {
    _directory: tempfile::TempDir,
    pub(super) gateway: GatewayController,
    pub(super) projection: GatewayProjection,
    codex_home: PathBuf,
    workdir: PathBuf,
}

impl LiveSandbox {
    pub(super) fn client_base_url(&self) -> String {
        self.projection
            .plan
            .client_base_url()
            .expect("temporary Codex gateway endpoint")
            .to_string()
    }

    pub(super) fn client_token(&self) -> String {
        match &self.projection.activation {
            GatewayActivation::Routed(route) => route.client_token.clone(),
            _ => panic!("temporary Codex route must be activated"),
        }
    }

    /// Runs the real Codex CLI inside this sandbox.
    pub(super) fn run_codex(&self, prompt: &str, timeout: Duration) -> Output {
        let auth = self.codex_home.join("auth.json");
        let auth_before = fs::read(&auth).expect("read isolated official login fixture");
        let mut command = Command::new(codex_executable());
        command.env_clear();
        for name in [
            "PATH",
            "SYSTEMROOT",
            "WINDIR",
            "COMSPEC",
            "PATHEXT",
            "TEMP",
            "TMP",
        ] {
            if let Some(value) = env::var_os(name) {
                command.env(name, value);
            }
        }
        for name in ["HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA"] {
            command.env(name, self._directory.path());
        }
        let mut child = command
            .args(["exec", "--ephemeral", "--skip-git-repo-check", prompt])
            .current_dir(&self.workdir)
            .env("CODEX_HOME", &self.codex_home)
            .env_remove("OPENAI_API_KEY")
            .env_remove("OPENAI_BASE_URL")
            .env_remove("OPENAI_API_BASE")
            .env_remove("CODEX_API_KEY")
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start isolated Codex CLI");
        let deadline = Instant::now() + timeout;
        loop {
            if child.try_wait().expect("poll Codex CLI").is_some() {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("isolated Codex CLI did not finish within {timeout:?}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let output = child.wait_with_output().expect("collect Codex CLI output");
        assert_eq!(
            fs::read(&auth).expect("read isolated official login fixture"),
            auth_before,
            "the Codex client must not rewrite its official login store"
        );
        output
    }

    pub(super) fn shutdown(self) {
        self.gateway.shutdown();
    }
}

/// Creates a temporary profile, activates it through the shipped switch
/// transaction and returns the sandbox with an isolated Codex home.
pub(super) fn prepare_live_sandbox(route: &LiveRoute, api_key: &str) -> LiveSandbox {
    let directory = tempfile::tempdir().expect("temporary live Codex sandbox");
    let root = directory.path();
    let state = LocalState::from_root(root.join("state"));
    let record = state
        .configuration()
        .create_codex_provider(live_draft(route, api_key))
        .expect("create temporary specialized Codex provider");
    let file = state
        .configuration()
        .find_codex_provider_file(&record.profile.id)
        .expect("load temporary specialized Codex provider");
    let gateway = GatewayController::start(&state);
    let projection = gateway
        .project_codex(&file, default_client_settings(AppKind::Codex), None)
        .expect("project temporary Codex route");
    assert!(
        projection.plan.profile.requires_gateway(),
        "every third-party Codex profile must route through the local gateway"
    );
    let codex_home = root.join("codex-home");
    let workdir = root.join("work");
    fs::create_dir_all(&codex_home).expect("create temporary Codex home");
    fs::create_dir_all(&workdir).expect("create temporary working directory");
    let auth = codex_home.join("auth.json");
    let fixture_auth = fake_official_auth();
    fs::write(&auth, &fixture_auth).expect("write isolated official login fixture");
    let target = codex_home.join("config.toml");
    // `chatgpt_base_url` is pinned to a dead loopback port so an isolated run
    // can never reach the real account service with its fixture login.
    fs::write(
        &target,
        "cli_auth_credentials_store = \"file\"\nchatgpt_base_url = \"http://127.0.0.1:9/isolated\"\n",
    )
    .expect("write isolated Codex baseline");
    let catalog = projection
        .codex_catalog
        .as_ref()
        .expect("Codex projection must carry its catalog artifact");
    fs::write(codex_home.join(&catalog.file_name), &catalog.content)
        .expect("write isolated model catalog");
    apply_projection(&gateway, &projection, root, &target, api_key);
    assert_eq!(
        fs::read(&auth).expect("read isolated official login fixture"),
        fixture_auth,
        "the switch transaction must not write the official login store"
    );
    LiveSandbox {
        _directory: directory,
        gateway,
        projection,
        codex_home,
        workdir,
    }
}

fn apply_projection(
    gateway: &GatewayController,
    projection: &GatewayProjection,
    root: &Path,
    target: &Path,
    api_key: &str,
) {
    let backup_dir = root.join("backups");
    let io = FsIo;
    let preview =
        asb_switch::read_preview(&io, target, &projection.plan, &backup_dir.to_string_lossy())
            .expect("preview isolated Codex switch");
    let committed_gateway = gateway.clone();
    let committed_projection = projection.clone();
    asb_switch::execute(
        &io,
        &asb_switch::SwitchRequest {
            target,
            plan: &projection.plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
        move |_| committed_gateway.commit(&committed_projection, || Ok(())),
    )
    .expect("execute isolated Codex switch");
    let rendered = fs::read_to_string(target).expect("read temporary Codex configuration");
    assert!(rendered.contains("model_provider = \"openai\""));
    assert!(rendered.contains("/codex/asb_codex_"));
    assert!(!rendered.contains("model_providers"));
    assert!(
        !rendered.contains(api_key),
        "the client configuration must never contain the provider credential"
    );
}

fn live_draft(route: &LiveRoute, api_key: &str) -> CodexProviderDraft {
    let chat_reasoning = if route.upstream == CodexUpstream::ChatCompletions {
        CodexChatReasoning::Configured {
            thinking_parameter: CodexChatThinkingParameter::None,
            separate_output: false,
            effort_parameter: CodexChatEffortParameter::ReasoningEffort,
            effort_mode: CodexChatEffortMode::LowHigh,
        }
    } else {
        CodexChatReasoning::Unsupported
    };
    // The external provider serves model traffic only. Disabling the client's
    // web search keeps every route on the model contract under test instead of
    // asking a third party for an OpenAI-hosted tool.
    let mut parameters = default_provider_parameters(AppKind::Codex);
    parameters.settings.insert(
        "web_search".to_string(),
        SettingValue::Explicit {
            value: ConfigValue::Str("disabled".to_string()),
        },
    );
    CodexProviderDraft {
        name: format!("external Codex {} sandbox", route.label),
        endpoint: CodexEndpoint(endpoint(route)),
        api_key: api_key.to_string(),
        authentication: None,
        connection: Default::default(),
        upstream: route.upstream,
        request_mode: ResponsesRequestMode::Standard,
        default_model: MODEL.to_string(),
        catalog: vec![CodexCatalogEntry {
            id: MODEL.to_string(),
            context_window: 128_000,
            max_output_tokens: 4_096,
            function_tools: true,
            custom_tools: true,
            tool_search: true,
            reasoning: true,
            default_reasoning_level: CodexReasoningLevel::High,
            supported_reasoning_levels: vec![CodexReasoningLevel::None, CodexReasoningLevel::High],
            images: false,
            compact: false,
            display_name: None,
            description: None,
            base_instructions: None,
            supports_parallel_tool_calls: None,
        }],
        model_routes: Vec::new(),
        subagent_route: None,
        capabilities: CodexCapabilities {
            responses: true,
            compact: false,
            models: false,
            chat_completions: false,
            alpha_search: false,
            image_generation: false,
            image_edit: false,
            function_tools: true,
            custom_tools: true,
            tool_search: true,
            reasoning: true,
            chat_reasoning,
        },
        parameters,
        notes: None,
        website_url: None,
        usage_query: None,
    }
}

fn fake_official_auth() -> Vec<u8> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let claims = json!({"exp":chrono::Utc::now().timestamp()+86400,"email":"fixture@example.invalid",
        "https://api.openai.com/auth":{"chatgpt_account_id":"fixture-account","chatgpt_plan_type":"plus"}});
    let jwt = format!(
        "{}.{}.fixture",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    );
    json!({"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"id_token":jwt,
        "access_token":"fixture-official-access","refresh_token":"fixture-refresh","account_id":"fixture-account"},
        "last_refresh":chrono::Utc::now().to_rfc3339()}).to_string().into_bytes()
}

pub(super) fn failure_summary(body: &str, api_key: &str) -> String {
    let error = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .or_else(|| value.pointer("/error/error"))
                .cloned()
        })
        .unwrap_or_else(|| Value::String(body.to_string()));
    let mut summary = crate::provider_diagnostics::redact_text(&error.to_string(), &[api_key]);
    if summary.len() > 1_024 {
        summary.truncate(1_024);
        summary.push_str(" [truncated]");
    }
    summary
}
