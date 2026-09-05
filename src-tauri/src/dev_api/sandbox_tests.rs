//! Black-box HTTP tests against the production command dispatcher and filesystem.
//! Each child process owns a temporary home, client directories and Tauri app data.
//! Store, adapter and executor are real; credentials are generated dummy values.
//! No external provider is contacted.

use super::*;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tauri::Manager;

const CHILD_ROOT: &str = "ASB_SWITCH_TEST_ROOT";
const ORIGIN: &str = "http://127.0.0.1:1420";

#[test]
fn common_settings_then_new_providers_switch_through_real_commands() {
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        run_workflow(Path::new(&root));
        return;
    }
    for redirected in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let home = root.join("home");
        fs::create_dir_all(home.join("AppData/Roaming")).unwrap();
        fs::create_dir_all(home.join("AppData/Local")).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "dev_api::sandbox_tests::common_settings_then_new_providers_switch_through_real_commands",
            "--nocapture",
        ]);
        command
            .env(CHILD_ROOT, root)
            .env("USERPROFILE", &home)
            .env("HOME", &home)
            .env("APPDATA", home.join("AppData/Roaming"))
            .env("LOCALAPPDATA", home.join("AppData/Local"))
            .env_remove("CODEX_HOME")
            .env_remove("CLAUDE_CONFIG_DIR");
        if redirected {
            command
                .env("CODEX_HOME", root.join("codex-override"))
                .env("CLAUDE_CONFIG_DIR", root.join("claude-override"));
        }
        let mut child = command.spawn().unwrap();
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "sandbox child failed (redirected={redirected})"
                );
                break;
            }
            if started.elapsed() > Duration::from_secs(60) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("sandbox command workflow timed out");
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}

struct Api {
    url: String,
    client: reqwest::blocking::Client,
}

impl Api {
    fn request(&self, command: &str, args: Value) -> Value {
        let response = self
            .client
            .post(&self.url)
            .header("Origin", ORIGIN)
            .header("Content-Type", "application/json")
            .body(json!({ "command": command, "args": args }).to_string())
            .send()
            .expect("sandbox HTTP request");
        assert_eq!(response.status(), 200);
        serde_json::from_str(&response.text().unwrap()).unwrap()
    }

    fn ok(&self, command: &str, args: Value) -> Value {
        let response = self.request(command, args);
        assert_eq!(
            response["kind"], "success",
            "{command}: {}",
            response["error"]
        );
        response["result"].clone()
    }

    fn rejected(&self, command: &str, args: Value, code: &str) {
        let response = self.request(command, args);
        assert_eq!(response["kind"], "failure", "{command} must reject");
        assert_eq!(response["error"]["code"], code);
    }

    fn save_common(&self, app: &str, key: &str, value: Value) -> Value {
        let editor = self.ok("get_common_settings_editor", json!({ "target": app }));
        let mut settings = editor["settings"].clone();
        settings["settings"][key] = json!({ "mode": "explicit", "value": value });
        self.ok(
            "save_common_settings",
            json!({
                "target": app, "settings": settings, "expectedSettingsHash": editor["settingsHash"]
            }),
        )
    }

    fn create(&self, app: &str, name: &str, official: bool, secret: &str) -> Value {
        self.ok("create_profile", json!({ "draft": {
            "app": app, "routeMode": if official { "official" } else { "custom" },
            "name": name, "apiKey": if official { "" } else { secret },
            "baseUrl": if official { Value::Null } else { json!(format!("https://{name}.example.com/v1")) },
            "model": if official { Value::Null } else { json!(format!("{app}-{name}")) }, "websiteUrl": null
        }}))["profile"].clone()
    }

    fn preview(&self, profile: &Value) -> Value {
        self.ok("preview_switch", json!({ "profileId": profile["id"] }))
    }

    fn switch_args(profile: &Value, preview: &Value, confirm: bool) -> Value {
        json!({ "profileId": profile["id"], "expectedHash": preview["contentHash"],
            "expectedRenderedHash": preview["renderedHash"], "confirmWrite": confirm })
    }

    fn switch(&self, profile: &Value, secret: &str) -> Value {
        let preview = self.preview(profile);
        assert!(
            !preview.to_string().contains(secret),
            "preview leaked a fixture secret"
        );
        let outcome = self.ok("execute_switch", Self::switch_args(profile, &preview, true));
        assert!(
            !outcome.to_string().contains(secret),
            "outcome leaked a fixture secret"
        );
        outcome
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn run_workflow(root: &Path) {
    // Only the subprocess environment is redirected; parent/user variables stay intact.
    let home = root.join("home");
    let codex = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or(home.join(".codex"));
    let claude = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or(home.join(".claude"));
    let config = codex.join("config.toml");
    let auth = codex.join("auth.json");
    let settings = claude.join("settings.json");
    let secret = uuid::Uuid::new_v4().to_string();
    let secret_b = uuid::Uuid::new_v4().to_string();
    let oauth = uuid::Uuid::new_v4().to_string();
    let initial_codex = "# host comment\nmodel = \"host-model\"\n[mcp_servers.audit]\ncommand = \"host-only\" # retain bytes\n";
    let initial_claude =
        "{\"permissions\":{\"deny\":[\"Read(.env)\"]},\"env\":{\"HOST_SETTING\":\"keep\"}}";
    write(&config, initial_codex);
    write(
        &auth,
        &json!({"auth_mode":"chatgpt", "tokens":{"access_token":oauth}, "host":"keep"}).to_string(),
    );
    write(&settings, initial_claude);
    let initial_auth = read(&auth);
    // Sentinel default files must stay untouched when explicit directories are used.
    if codex != home.join(".codex") {
        write(&home.join(".codex/config.toml"), "default sentinel");
    }
    if claude != home.join(".claude") {
        write(&home.join(".claude/settings.json"), "default sentinel");
    }

    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    // Tauri joins this absolute identifier to app_data_dir: no real app state is used.
    context.config_mut().identifier = root.join("app-data").to_string_lossy().into_owned();
    let app = tauri::Builder::default()
        .any_thread()
        .build(context)
        .unwrap();
    assert_eq!(app.path().app_data_dir().unwrap(), root.join("app-data"));
    let server = Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/invoke", server.server_addr());
    let handle = app.handle().clone();
    thread::spawn(move || serve(server, handle, ORIGIN.to_string()));
    let api = Api {
        url,
        client: reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap(),
    };

    // First save general settings, then create providers through the real public commands.
    api.save_common("codex", "model_reasoning_effort", json!("high"));
    api.save_common("claude", "effortLevel", json!("high"));
    let codex_a = api.create("codex", "a", false, &secret);
    let codex_b = api.create("codex", "b", false, &secret_b);
    let claude_a = api.create("claude", "a", false, &secret);
    let claude_b = api.create("claude", "b", false, &secret_b);
    assert_eq!(read(&config), initial_codex);
    assert_eq!(read(&settings), initial_claude);
    assert!(read(&auth) == initial_auth);
    let before = api.preview(&codex_a);
    assert!(!before.to_string().contains(&oauth));
    assert_eq!(read(&config), initial_codex);
    assert!(read(&auth) == initial_auth);
    api.rejected(
        "execute_switch",
        Api::switch_args(&codex_a, &before, false),
        "write-not-confirmed",
    );
    assert_eq!(read(&config), initial_codex);
    api.switch(&codex_a, &secret);
    check_codex(&config, &auth, "a", &secret, &oauth);
    assert_eq!(read(&settings), initial_claude);
    api.switch(&claude_a, &secret);
    check_claude(&settings, "a", &secret);
    let a_config = read(&config);
    let a_auth = read(&auth);
    let a_settings = read(&settings);
    api.switch(&codex_b, &secret_b);
    api.switch(&claude_b, &secret_b);
    check_codex(&config, &auth, "b", &secret_b, &oauth);
    check_claude(&settings, "b", &secret_b);
    assert!(!read(&auth).contains(&secret));
    assert!(!read(&settings).contains(&secret));

    for (client, profile) in [("codex", &codex_b), ("claude", &claude_b)] {
        let status = api.ok("config_status", json!({}));
        let status = status
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["app"] == client)
            .unwrap();
        assert_eq!(status["activeProfileId"], profile["id"]);
        assert_eq!(status["syntaxOk"], true);
        api.ok(
            "undo_last_switch",
            json!({"target":client,"confirmWrite":true}),
        );
    }
    assert_eq!(read(&config), a_config);
    assert!(read(&auth) == a_auth);
    assert!(read(&settings) == a_settings);

    // A common-setting edit invalidates a previously accepted projection candidate.
    let stale = api.preview(&codex_b);
    api.save_common("codex", "model_reasoning_effort", json!("xhigh"));
    api.rejected(
        "execute_switch",
        Api::switch_args(&codex_b, &stale, true),
        "preview-stale",
    );
    assert_eq!(read(&config), a_config);
    api.switch(&codex_b, &secret_b);
    assert!(read(&config).contains("model_reasoning_effort = \"xhigh\""));
    let stale = api.preview(&claude_b);
    let changed = format!("{}\n", read(&settings));
    write(&settings, &changed);
    api.rejected(
        "execute_switch",
        Api::switch_args(&claude_b, &stale, true),
        "external-change",
    );
    assert!(read(&settings) == changed);

    // Official routes remove custom endpoints/keys while retaining host data and OAuth.
    let official_codex = api.create("codex", "official", true, &secret);
    let official_claude = api.create("claude", "official", true, &secret);
    api.switch(&official_codex, &secret);
    api.switch(&official_claude, &secret);
    assert!(!read(&config).contains("openai_base_url"));
    let auth_value: Value = serde_json::from_str(&read(&auth)).unwrap();
    assert_eq!(auth_value["auth_mode"], "chatgpt");
    assert!(auth_value["tokens"]["access_token"] == oauth);
    assert!(auth_value["OPENAI_API_KEY"].is_null());
    let claude_value: Value = serde_json::from_str(&read(&settings)).unwrap();
    assert!(claude_value["env"].get("ANTHROPIC_AUTH_TOKEN").is_none());
    assert!(claude_value["env"].get("ANTHROPIC_BASE_URL").is_none());
    let backups = api.ok("list_backups", json!({}));
    assert!(!backups.as_array().unwrap().is_empty());
    for backup in backups.as_array().unwrap() {
        assert!(Path::new(backup["backupPath"].as_str().unwrap()).starts_with(root));
    }
    // A pristine installation must create the real files; undo must restore absence.
    for path in [&config, &auth, &settings] {
        fs::remove_file(path).unwrap();
    }
    api.switch(&codex_a, &secret);
    api.switch(&claude_a, &secret);
    assert!(config.exists() && auth.exists() && settings.exists());
    for client in ["codex", "claude"] {
        api.ok(
            "undo_last_switch",
            json!({"target":client,"confirmWrite":true}),
        );
    }
    assert!(!config.exists() && !auth.exists() && !settings.exists());
    if codex != home.join(".codex") {
        assert_eq!(read(&home.join(".codex/config.toml")), "default sentinel");
    }
    if claude != home.join(".claude") {
        assert_eq!(
            read(&home.join(".claude/settings.json")),
            "default sentinel"
        );
    }
}

fn check_codex(config: &Path, auth: &Path, name: &str, secret: &str, oauth: &str) {
    let text = read(config);
    assert!(text.contains("model_provider = \"openai\""));
    assert!(text.contains(&format!(
        "openai_base_url = \"https://{name}.example.com/v1\""
    )));
    assert!(text.contains(&format!("model = \"codex-{name}\"")));
    assert!(text.contains("model_reasoning_effort = \"high\""));
    assert!(text.contains("[mcp_servers.audit]\ncommand = \"host-only\" # retain bytes"));
    assert!(!text.contains(secret));
    let value: Value = serde_json::from_str(&read(auth)).unwrap();
    assert_eq!(value["auth_mode"], "apikey");
    assert!(value["OPENAI_API_KEY"] == secret);
    assert!(value["tokens"]["access_token"] == oauth);
}

fn check_claude(path: &Path, name: &str, secret: &str) {
    let value: Value = serde_json::from_str(&read(path)).unwrap();
    assert_eq!(value["model"], format!("claude-{name}"));
    assert_eq!(value["effortLevel"], "high");
    assert_eq!(
        value["env"]["ANTHROPIC_BASE_URL"],
        format!("https://{name}.example.com/v1")
    );
    assert!(value["env"]["ANTHROPIC_AUTH_TOKEN"] == secret);
    assert_eq!(value["env"]["HOST_SETTING"], "keep");
    assert_eq!(value["permissions"]["deny"], json!(["Read(.env)"]));
}
