use crate::commands::error::CommandError;
use serde::Serialize;
use std::collections::HashSet;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangelogEntry {
    version: String,
    date: Option<String>,
    notes: String,
}

/// The bundled document owns release order and Markdown; only its release
/// boundaries and metadata are interpreted here. Unreleased is never history.
#[tauri::command]
pub(crate) fn changelog_history(app: AppHandle) -> Result<Vec<ChangelogEntry>, CommandError> {
    let path = app.path().resolve("CHANGELOG.md", tauri::path::BaseDirectory::Resource)
        .map_err(|error| CommandError::new("changelog-unavailable", error.to_string()))?;
    let text = std::fs::read_to_string(path)
        .map_err(|error| CommandError::new("changelog-unavailable", error.to_string()))?;
    parse_history(&text)
}

fn parse_history(text: &str) -> Result<Vec<ChangelogEntry>, CommandError> {
    let mut entries = Vec::new();
    let mut current: Option<ChangelogEntry> = None;
    let mut versions = HashSet::new();
    let mut fence = None;
    for line in text.lines() {
        if fence.is_none() && line.starts_with("## [") {
            finish_entry(&mut current, &mut entries);
            current = parse_heading(line)?;
            if let Some(entry) = &current {
                if !versions.insert(entry.version.clone()) {
                    return Err(CommandError::new("changelog-invalid",
                        format!("Duplicate release version: {}", entry.version)));
                }
            }
            continue;
        }
        update_fence(line, &mut fence);
        if let Some(entry) = &mut current {
            entry.notes.push_str(line);
            entry.notes.push('\n');
        }
    }
    finish_entry(&mut current, &mut entries);
    Ok(entries)
}

fn parse_heading(line: &str) -> Result<Option<ChangelogEntry>, CommandError> {
    let (version, suffix) = line.strip_prefix("## [").and_then(|text| text.split_once(']'))
        .ok_or_else(|| CommandError::new("changelog-invalid", format!("Invalid release heading: {line}")))?;
    if version == "Unreleased" { return Ok(None); }
    let version = semver::Version::parse(version)
        .map_err(|error| CommandError::new("changelog-invalid", format!("Invalid version {version}: {error}")))?;
    let date = if suffix.trim().is_empty() {
        None
    } else {
        let value = suffix.strip_prefix(" - ")
            .ok_or_else(|| CommandError::new("changelog-invalid", format!("Invalid release date: {line}")))?;
        Some(chrono::NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
            .map_err(|error| CommandError::new("changelog-invalid", format!("Invalid release date {value}: {error}")))?
            .to_string())
    };
    Ok(Some(ChangelogEntry { version: version.to_string(), date, notes: String::new() }))
}

fn finish_entry(current: &mut Option<ChangelogEntry>, entries: &mut Vec<ChangelogEntry>) {
    if let Some(mut entry) = current.take() {
        if !entry.notes.trim().is_empty() {
            entry.notes = entry.notes.trim_matches('\n').to_string();
            entries.push(entry);
        }
    }
}

// A version-looking line inside a fenced example remains Markdown content.
fn update_fence(line: &str, fence: &mut Option<(u8, usize)>) {
    let text = line.trim_start();
    if line.len() - text.len() > 3 { return; }
    let Some(&marker) = text.as_bytes().first() else { return; };
    if !matches!(marker, b'`' | b'~') { return; }
    let count = text.as_bytes().iter().take_while(|&&byte| byte == marker).count();
    if count < 3 { return; }
    match *fence {
        Some((active, size)) if active == marker && count >= size && text[count..].trim().is_empty() => {
            *fence = None;
        }
        None => *fence = Some((marker, count)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_preserves_release_order_dates_and_markdown() {
        let text = "# Log\n\n## [Unreleased]\n\n- Pending.\n\n## [0.3.30] - 2026-11-12\n\n### Fixes\n\n- **New** entry.\n\n## [0.3.3]\n\n- Older entry.\n";
        let entries = parse_history(text).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, "0.3.30");
        assert_eq!(entries[0].date.as_deref(), Some("2026-11-12"));
        assert_eq!(entries[0].notes, "### Fixes\n\n- **New** entry.");
        assert_eq!(entries[1].version, "0.3.3");
        assert!(entries[1].date.is_none());
        assert_eq!(entries[1].notes, "- Older entry.");
    }

    #[test]
    fn fenced_version_headings_do_not_split_releases() {
        let text = "## [1.0.0]\n\n````md\n## [2.0.0]\n```\n````\n\n~~~md\n## [3.0.0]\n~~~\n";
        let entries = parse_history(text).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].notes.contains("## [2.0.0]"));
        assert!(entries[0].notes.contains("## [3.0.0]"));
    }

    #[test]
    fn body_indentation_is_not_trimmed_into_plain_text() {
        let entries = parse_history("## [1.0.0]\n\n    example()\n").unwrap();
        assert_eq!(entries[0].notes, "    example()");
    }

    #[test]
    fn empty_history_is_distinct_from_invalid_metadata() {
        assert!(parse_history("## [Unreleased]\n\n- Pending.\n\n## [1.0.0]\n").unwrap().is_empty());
        assert!(parse_history("## [invalid]\n- Entry.").is_err());
        assert!(parse_history("## [1.0.0] - 2026-02-30\n- Entry.").is_err());
        assert!(parse_history("## [1.0.0]\n- A\n## [1.0.0]\n- B").is_err());
    }
}
