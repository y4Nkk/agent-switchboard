use std::path::Path;

const FIXED_LOCATIONS: &[&str] = &[
    "profiles.json", "configuration-upgrade.json", "configuration-repair.json",
    "configuration/common", "configuration/client-settings",
    "configuration/client-settings/codex.json", "configuration/client-settings/claude.json",
    "configuration/history/codex.json", "configuration/history/claude.json",
    "configuration/providers/codex", "configuration/providers/claude",
    "configuration/providers/codex/official", "configuration/save-journal.json",
];

pub(super) fn is_valid(location: &str) -> bool {
    if FIXED_LOCATIONS.contains(&location) { return true; }
    let directories = ["configuration/providers/codex/", "configuration/providers/claude/",
        "configuration/providers/codex/official/"];
    directories.iter().any(|prefix| location.strip_prefix(prefix)
        .and_then(|name| name.strip_suffix(".json"))
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()))
}

pub(crate) fn location(path: &Path) -> Option<String> {
    let parts = path.components().map(|part| part.as_os_str().to_str()).collect::<Option<Vec<_>>>()?;
    let candidate = match parts.iter().rposition(|part| *part == "configuration") {
        Some(index) => parts[index..].join("/"),
        None => (*parts.last()?).to_string(),
    };
    is_valid(&candidate).then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_retain_only_owned_structure_and_uuid_file_names() {
        let id = "00000000-0000-0000-0000-000000000001";
        let relative = format!("configuration/providers/codex/official/{id}.json");
        assert_eq!(location(&Path::new("private-user/state").join(&relative)), Some(relative));
        assert_eq!(location(Path::new("private-user/state/configuration/client-settings/codex.json")),
            Some("configuration/client-settings/codex.json".into()));
        for value in ["../auth.json", "configuration/providers/codex/sk-private.json",
            "configuration/providers/codex/../auth.json", "private-user/.codex/auth.json"] {
            assert!(location(Path::new(value)).is_none());
            assert!(!is_valid(value));
        }
    }
}
