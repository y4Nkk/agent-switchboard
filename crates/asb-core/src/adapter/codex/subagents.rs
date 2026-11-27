//! Codex `[agents]` default settings: read and render, purely.
//!
//! This module owns exactly three global runtime scalars under `[agents]`. It never
//! reads, maps, writes, or explains a retired alias, and it never touches a
//! role sub-table (`agents.<role>`), role files, or any other host key. Every
//! function here takes text and returns text; file access belongs to
//! `asb-switch`.

use toml_edit::Item;

use crate::adapter::codex::document::{item_at, parse};
use crate::adapter::AdapterError;
use crate::contracts::{
    CodexSubagentKey, CodexSubagentSettings, ConfigValue, SettingValue, MAX_EXACT_CONFIG_INTEGER,
};

fn conflict(message: String) -> AdapterError {
    AdapterError {
        message,
        line: None,
    }
}

fn bad_current_type(key: CodexSubagentKey, expected: &str) -> AdapterError {
    conflict(format!(
        "受管键 {} 的当前值不是{}；请手动修正用户级 config.toml 后再使用本模块",
        key.path(),
        expected
    ))
}

fn read_bool(key: CodexSubagentKey, item: Option<&Item>) -> Result<SettingValue, AdapterError> {
    match item {
        None => Ok(SettingValue::Automatic),
        Some(item) => match item.as_value() {
            Some(toml_edit::Value::Boolean(value)) => Ok(SettingValue::Explicit {
                value: ConfigValue::Bool(*value.value()),
            }),
            Some(_) => Err(bad_current_type(key, "布尔值")),
            None => Err(conflict(format!(
                "受管键 {} 当前是 TOML 表，无法安全覆盖其宿主内容",
                key.path()
            ))),
        },
    }
}

fn read_integer(key: CodexSubagentKey, item: Option<&Item>) -> Result<SettingValue, AdapterError> {
    match item {
        None => Ok(SettingValue::Automatic),
        Some(item) => match item.as_value() {
            Some(toml_edit::Value::Integer(value))
                if *value.value() >= -MAX_EXACT_CONFIG_INTEGER
                    && *value.value() <= MAX_EXACT_CONFIG_INTEGER =>
            {
                Ok(SettingValue::Explicit {
                    value: ConfigValue::Number(*value.value() as f64),
                })
            }
            Some(toml_edit::Value::Integer(_)) => Err(conflict(format!(
                "受管键 {} 超出 ASB 可精确表示的整数范围；请手动修正用户级 config.toml 后再使用本模块",
                key.path()
            ))),
            Some(_) => Err(bad_current_type(key, "整数")),
            None => Err(conflict(format!(
                "受管键 {} 当前是 TOML 表，无法安全覆盖其宿主内容",
                key.path()
            ))),
        },
    }
}

/// Reads the three global runtime scalars from the current user-level document. A
/// missing key yields the automatic intent; anything that cannot be
/// represented exactly is refused instead of silently rewritten.
pub fn read_subagent_settings(text: &str) -> Result<CodexSubagentSettings, AdapterError> {
    let doc = parse(text)?;
    Ok(CodexSubagentSettings {
        enabled: read_bool(
            CodexSubagentKey::Enabled,
            item_at(&doc, &CodexSubagentKey::Enabled.path()),
        )?,
        max_concurrent_threads_per_session: read_integer(
            CodexSubagentKey::MaxConcurrentThreadsPerSession,
            item_at(
                &doc,
                &CodexSubagentKey::MaxConcurrentThreadsPerSession.path(),
            ),
        )?,
        interrupt_message: read_bool(
            CodexSubagentKey::InterruptMessage,
            item_at(&doc, &CodexSubagentKey::InterruptMessage.path()),
        )?,
    })
}

pub(crate) const RETIRED_MAX_THREADS_KEY: &str = "agents.max_threads";

#[cfg(test)]
mod tests {
    use super::*;

    fn render_subagent_settings(current: &str, settings: &CodexSubagentSettings) -> Result<String, AdapterError> {
        crate::adapter::render_client_changes(crate::AppKind::Codex, current, &crate::adapter::ClientConfigurationChanges {
            subagents: CodexSubagentKey::ALL.into_iter().map(|key| (key, settings.get(key).clone())).collect(),
            ..Default::default()
        })
    }

    fn render(current: &str, settings: &CodexSubagentSettings) -> String {
        render_subagent_settings(current, settings).expect("render")
    }

    fn explicit_bool(value: bool) -> SettingValue {
        SettingValue::Explicit {
            value: ConfigValue::Bool(value),
        }
    }

    fn explicit_text(value: &str) -> SettingValue {
        SettingValue::Explicit {
            value: ConfigValue::Str(value.to_string()),
        }
    }

    fn explicit_number(value: f64) -> SettingValue {
        SettingValue::Explicit {
            value: ConfigValue::Number(value),
        }
    }

    fn full() -> CodexSubagentSettings {
        CodexSubagentSettings {
            enabled: explicit_bool(true),
            max_concurrent_threads_per_session: explicit_number(3.0),
            interrupt_message: explicit_bool(true),
        }
    }

    #[test]
    fn a_missing_agents_table_reads_as_all_automatic() {
        let settings = read_subagent_settings("model = \"gpt-5\"\n").expect("read");
        assert_eq!(settings, CodexSubagentSettings::automatic());
    }

    #[test]
    fn a_complete_agents_table_reads_every_global_runtime_scalar() {
        let settings = read_subagent_settings(
            "[agents]\nenabled = true\ndefault_subagent_model = \"gpt-5-codex\"\ndefault_subagent_reasoning_effort = \"high\"\nmax_concurrent_threads_per_session = 3\ninterrupt_message = false\n",
        )
        .expect("read");
        assert_eq!(
            settings,
            CodexSubagentSettings {
                enabled: explicit_bool(true),
                max_concurrent_threads_per_session: explicit_number(3.0),
                interrupt_message: explicit_bool(false),
            }
        );
    }

    #[test]
    fn largest_exact_json_integer_round_trips_as_a_toml_integer() {
        let settings = CodexSubagentSettings {
            max_concurrent_threads_per_session: explicit_number(MAX_EXACT_CONFIG_INTEGER as f64),
            ..full()
        };
        let rendered = render("", &settings);
        assert!(rendered.contains(&format!(
            "max_concurrent_threads_per_session = {MAX_EXACT_CONFIG_INTEGER}"
        )));
        assert_eq!(
            read_subagent_settings(&rendered)
                .expect("read exact integer")
                .max_concurrent_threads_per_session,
            explicit_number(MAX_EXACT_CONFIG_INTEGER as f64)
        );
    }

    #[test]
    fn integers_that_cannot_round_trip_through_json_are_refused_on_read() {
        let unsafe_integer = MAX_EXACT_CONFIG_INTEGER + 1;
        let error = read_subagent_settings(&format!(
            "[agents]\nmax_concurrent_threads_per_session = {unsafe_integer}\n"
        ))
        .expect_err("integer above JSON exact range must not be rounded");
        assert!(error.message.contains("可精确表示"), "{error:?}");
    }

    #[test]
    fn minimum_toml_integer_is_refused_without_overflowing_the_reader() {
        let error = read_subagent_settings(&format!(
            "[agents]\nmax_concurrent_threads_per_session = {}\n",
            i64::MIN
        ))
        .expect_err("integer outside the exact range must be refused");
        assert!(error.message.contains("可精确表示"), "{error:?}");
    }

    #[test]
    fn a_partial_table_reads_only_the_present_keys() {
        let settings = read_subagent_settings("[agents]\nenabled = false\n").expect("read");
        assert_eq!(settings.enabled, explicit_bool(false));
        assert_eq!(
            settings.max_concurrent_threads_per_session,
            SettingValue::Automatic
        );
    }

    #[test]
    fn provider_owned_defaults_are_not_read_or_rewritten_by_runtime_settings() {
        let current = concat!(
            "[agents]\n",
            "default_subagent_model = \"provider-model\"\n",
            "default_subagent_reasoning_effort = \"high\"\n",
        );
        assert_eq!(
            read_subagent_settings(current).expect("read runtime settings"),
            CodexSubagentSettings::automatic()
        );
        assert_eq!(
            render(current, &CodexSubagentSettings::automatic()),
            current
        );
    }

    #[test]
    fn wrong_toml_types_are_refused_on_read() {
        for (text, needle) in [
            ("[agents]\nenabled = \"yes\"\n", "布尔值"),
            (
                "[agents]\nmax_concurrent_threads_per_session = 3.5\n",
                "整数",
            ),
        ] {
            let error = read_subagent_settings(text).expect_err("wrong type must fail");
            assert!(error.message.contains(needle), "{error:?}");
        }
    }

    #[test]
    fn a_managed_key_that_is_a_table_is_refused() {
        let error = read_subagent_settings("[agents.enabled]\nnested = true\n")
            .expect_err("table cannot be overwritten");
        assert!(error.message.contains("TOML 表"));
    }

    #[test]
    fn writing_global_runtime_keys_produces_exactly_the_canonical_toml() {
        let rendered = render("", &full());
        assert_eq!(
            rendered,
            "[agents]\nenabled = true\nmax_concurrent_threads_per_session = 3\ninterrupt_message = true\n"
        );
    }

    #[test]
    fn automatic_removes_only_its_own_key() {
        let rendered = render(
            "[agents]\nenabled = true\ndefault_subagent_model = \"gpt-5-codex\"\n",
            &CodexSubagentSettings {
                enabled: SettingValue::Automatic,
                ..full()
            },
        );
        assert!(rendered.contains("default_subagent_model = \"gpt-5-codex\""));
        assert!(!rendered.contains("enabled = true"));
    }

    #[test]
    fn all_automatic_drops_the_table_once_it_is_empty() {
        let rendered = render(
            "[agents]\nenabled = true\n",
            &CodexSubagentSettings::automatic(),
        );
        assert!(!rendered.contains("agents"));
    }

    #[test]
    fn role_tables_and_host_keys_survive_all_automatic() {
        let current = concat!(
            "# keep this comment\n",
            "model = \"gpt-5\"\n",
            "[agents]\n",
            "enabled = true\n",
            "unknown_member = \"keep\"\n",
            "[agents.worker]\n",
            "description = \"does the work\"\n",
            "config_file = \"worker.toml\"\n",
        );
        let rendered = render(current, &CodexSubagentSettings::automatic());
        assert!(rendered.contains("# keep this comment"));
        assert!(rendered.contains("model = \"gpt-5\""));
        assert!(rendered.contains("[agents.worker]"));
        assert!(rendered.contains("description = \"does the work\""));
        assert!(rendered.contains("config_file = \"worker.toml\""));
        assert!(rendered.contains("unknown_member = \"keep\""));
        assert!(!rendered.contains("enabled"));
    }

    #[test]
    fn explicit_values_are_written_beside_role_tables() {
        let current = "[agents.worker]\ndescription = \"worker\"\n[other]\nkeep = true\n";
        let rendered = render(current, &full());
        assert!(rendered.contains("[agents.worker]"));
        assert!(rendered.contains("description = \"worker\""));
        assert!(rendered.contains("[other]"));
        assert!(rendered.contains("enabled = true"));
        assert!(rendered.contains("max_concurrent_threads_per_session = 3"));
    }

    #[test]
    fn retired_max_threads_is_removed_without_being_mapped() {
        let settings = read_subagent_settings("[agents]\nmax_threads = 4\n").expect("read");
        assert_eq!(settings, CodexSubagentSettings::automatic());
        let rendered = render("[agents]\nmax_threads = 4\n", &full());
        assert!(!rendered.contains("max_threads"));
        assert!(rendered.contains("max_concurrent_threads_per_session = 3"));
    }

    #[test]
    fn retired_max_threads_is_removed_with_automatic_values() {
        let rendered = render(
            "[agents]\nmax_threads = 4\nunknown_member = \"keep\"\n",
            &CodexSubagentSettings::automatic(),
        );
        assert!(!rendered.contains("max_threads"));
        assert!(rendered.contains("unknown_member = \"keep\""));
    }

    #[test]
    fn invalid_explicit_values_are_refused_before_rendering() {
        let cases: Vec<(&str, CodexSubagentSettings, &str)> = vec![
            (
                "zero concurrency",
                CodexSubagentSettings {
                    max_concurrent_threads_per_session: explicit_number(0.0),
                    ..full()
                },
                "不小于 1",
            ),
            (
                "negative concurrency",
                CodexSubagentSettings {
                    max_concurrent_threads_per_session: explicit_number(-2.0),
                    ..full()
                },
                "不小于 1",
            ),
            (
                "fractional concurrency",
                CodexSubagentSettings {
                    max_concurrent_threads_per_session: explicit_number(1.5),
                    ..full()
                },
                "不小于 1",
            ),
            (
                "huge concurrency",
                CodexSubagentSettings {
                    max_concurrent_threads_per_session: explicit_number(1e30),
                    ..full()
                },
                "不小于 1",
            ),
            (
                "unsafe concurrency",
                CodexSubagentSettings {
                    max_concurrent_threads_per_session: explicit_number(
                        (MAX_EXACT_CONFIG_INTEGER + 1) as f64,
                    ),
                    ..full()
                },
                "不大于",
            ),
            (
                "text as enabled",
                CodexSubagentSettings {
                    enabled: explicit_text("yes"),
                    ..full()
                },
                "true 或 false",
            ),
        ];
        for (name, settings, needle) in cases {
            let error = render_subagent_settings("[agents]\n", &settings).expect_err(name);
            assert!(error.message.contains(needle), "{name}: {error:?}");
        }
    }

    #[test]
    fn rendering_is_idempotent() {
        let once = render("model = \"gpt-5\"\n", &full());
        let twice = render(&once, &full());
        assert_eq!(once, twice);
    }
}
