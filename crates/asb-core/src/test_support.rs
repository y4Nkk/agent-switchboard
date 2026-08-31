//! Test-only configuration samples.
//!
//! These constants are used only by automated tests. They contain no real
//! credentials or private endpoints, and deliberately include host-owned
//! fields so tests prove unknown keys survive a switch untouched.

/// Codex `config.toml` test sample: host-owned top-level keys, a host-owned
/// provider table, comments, and the app-managed `model_providers.asb` table.
pub const CODEX_TOML: &str = r#"# host-owned Codex configuration (test sample)
history_persistence = "save-all"
threads = 8
model = "gpt-5.1"
model_provider = "asb"
experimental_bearer_token = "TEST_CODEX_IMPORT_KEY"

[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
wire_api = "responses"

[model_providers.asb]
name = "中继 A"
base_url = "https://relay-a.internal/v1"
wire_api = "responses"

[projects."F:\\work\\sample"]
trusted = true
"#;

/// Claude Code `settings.json` test sample: host-owned permission blocks and
/// status line plus app-owned routing keys.
pub const CLAUDE_JSON: &str = r#"{
  "permissions": {
    "allow": [
      "Bash(npm run test:*)"
    ],
    "deny": []
  },
  "statusLine": {
    "type": "command",
    "command": "node status.js"
  },
  "env": {
    "ANTHROPIC_BASE_URL": "https://relay-a.internal",
    "ANTHROPIC_AUTH_TOKEN": "TEST_CLAUDE_IMPORT_KEY",
    "ANTHROPIC_MODEL": "claude-sonnet-4",
    "ANTHROPIC_SMALL_FAST_MODEL": "claude-3-5-haiku-latest"
  },
  "model": "claude-sonnet-4"
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_carry_no_real_credentials() {
        for (name, text) in [("codex", CODEX_TOML), ("claude", CLAUDE_JSON)] {
            let lower = text.to_ascii_lowercase();
            assert!(
                !lower.contains("sk-"),
                "{name} test sample contains a secret-shaped value"
            );
            assert!(
                !lower.contains("bearer "),
                "{name} test sample contains an auth header"
            );
        }
    }

    #[test]
    fn samples_contain_host_owned_fields() {
        assert!(CODEX_TOML.contains("threads"));
        assert!(CODEX_TOML.contains("[model_providers.openai]"));
        assert!(CLAUDE_JSON.contains("permissions"));
        assert!(CLAUDE_JSON.contains("statusLine"));
    }
}
