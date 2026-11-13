use std::path::{Path, PathBuf};

/// Read failures retain the owning file without retaining credential material.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileStoreError {
    InvalidFile { path: PathBuf, reason: String },
    UnreadableFile { path: PathBuf, reason: String },
    Recovering { path: PathBuf },
    NotFound { id: String },
}

impl ProfileStoreError {
    pub(crate) fn invalid(path: &Path, reason: impl Into<String>) -> Self {
        Self::InvalidFile { path: path.to_path_buf(), reason: reason.into() }
    }

    pub(crate) fn unreadable(path: &Path, error: &std::io::Error) -> Self {
        Self::UnreadableFile { path: path.to_path_buf(), reason: error.to_string() }
    }

    pub(super) fn original_path(mut self, staged: &Path, original: &Path) -> Self {
        let path = match &mut self {
            Self::InvalidFile { path, .. } | Self::UnreadableFile { path, .. }
                | Self::Recovering { path } => path,
            _ => return self,
        };
        if let Ok(relative) = path.strip_prefix(staged) { *path = original.join(relative); }
        self
    }
}

impl std::fmt::Display for ProfileStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFile { path, reason } => write!(formatter,
                "配置校验失败：{}；{}；原数据未改动", path.display(), reason),
            Self::UnreadableFile { path, reason } => write!(formatter,
                "配置不可读：{}；{}；原数据未改动", path.display(), reason),
            Self::Recovering { path } => write!(formatter,
                "应用数据正在恢复，请稍后重试；恢复记录：{}", path.display()),
            Self::NotFound { id } => write!(formatter,
                "找不到 Codex 第三方供应商档案：{id}；请重新加载供应商列表"),
        }
    }
}

pub(super) fn parse_file<T: serde::de::DeserializeOwned>(
    text: &str, path: &Path,
) -> Result<T, ProfileStoreError> {
    serde_json::from_str(text).map_err(|error| json_error(path, &error))
}

pub(super) fn json_error(path: &Path, error: &serde_json::Error) -> ProfileStoreError {
    if error.is_io() {
        return ProfileStoreError::UnreadableFile { path: path.to_path_buf(), reason: error.to_string() };
    }
    ProfileStoreError::invalid(path, json_diagnostic(error))
}

pub(super) fn json_diagnostic(error: &serde_json::Error) -> String {
    let category = match error.classify() {
        serde_json::error::Category::Io => "JSON 读取失败",
        serde_json::error::Category::Syntax => "JSON 语法无效",
        serde_json::error::Category::Data => "JSON 数据不符合当前结构",
        serde_json::error::Category::Eof => "JSON 内容不完整",
    };
    let detail = if error.is_data() { redact_json_values(&error.to_string()) } else { error.to_string() };
    format!("{category}：{detail}")
}

/// Serde quotes unexpected string values, while field names and expected
/// types remain useful diagnostics. Unknown enum inputs use backticks.
fn redact_json_values(detail: &str) -> String {
    let enum_detail;
    let detail = if detail.starts_with("unknown variant `") {
        let suffix = detail.rfind("`, expected ")
            .or_else(|| detail.rfind("`, there are no variants"));
        enum_detail = format!("unknown variant [已隐藏值]{}", suffix.map(|index| &detail[index + 1..]).unwrap_or(""));
        &enum_detail
    } else { detail };
    let mut output = String::new();
    let mut characters = detail.chars();
    while let Some(character) = characters.next() {
        if character != '"' {
            output.push(character);
            continue;
        }
        output.push_str("\"[已隐藏值]\"");
        let mut escaped = false;
        for character in characters.by_ref() {
            if escaped { escaped = false; }
            else if character == '\\' { escaped = true; }
            else if character == '"' { break; }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_files_report_path_and_location_without_echoing_values() {
        #[derive(Debug, serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fixture { _number: u32 }
        let path = Path::new("configuration/providers/codex/fixture.json");
        let error = parse_file::<Fixture>(r#"{"_number":"private-credential-value"}"#, path).unwrap_err();
        let detail = error.to_string();
        assert!(detail.contains(&path.display().to_string()));
        assert!(detail.contains("invalid type: string"));
        assert!(detail.contains("expected u32"));
        assert!(detail.contains("line 1"));
        assert!(!detail.contains("private-credential-value"));
        assert!(!detail.contains("查看应用数据检查"));
    }

    #[test]
    fn parser_errors_keep_missing_fields_syntax_and_truncation_causes() {
        #[derive(Debug, serde::Deserialize)]
        struct Fixture { _routes: Vec<String> }
        let path = Path::new("configuration/providers/claude/fixture.json");
        for (text, cause) in [("{}", "missing field `_routes`"),
            ("{\"_routes\" []}", "expected `:`"), ("{\"_routes\": [", "EOF while parsing a list")] {
            let detail = parse_file::<Fixture>(text, path).unwrap_err().to_string();
            assert!(detail.contains(cause), "{detail}");
            assert!(detail.contains("line 1 column"), "{detail}");
            assert!(detail.contains(&path.display().to_string()));
        }
    }

    #[test]
    fn diagnostic_redaction_handles_escaped_strings_and_unknown_enum_values() {
        let path = Path::new("configuration/providers/claude/fixture.json");
        let text = serde_json::to_string("private\"credential\\value").unwrap();
        let error = parse_file::<u32>(&text, path).unwrap_err().to_string();
        assert!(error.contains("expected u32"));
        assert!(!error.contains("credential"));
        #[derive(Debug, serde::Deserialize)]
        enum Mode { Automatic, Explicit }
        let text = serde_json::to_string("private\"`, expected `credential").unwrap();
        let error = parse_file::<Mode>(&text, path).unwrap_err().to_string();
        assert!(error.contains("unknown variant [已隐藏值]"));
        assert!(error.contains("expected `Automatic` or `Explicit`"));
        assert!(!error.contains("private"));
        assert!(!error.contains("credential"));
    }

    #[test]
    fn file_read_errors_keep_the_original_os_diagnostic() {
        let path = Path::new("configuration/client-settings/codex.json");
        let cause = std::io::Error::from_raw_os_error(5);
        let error = ProfileStoreError::unreadable(path, &cause).to_string();
        assert!(error.contains(&cause.to_string()));
        assert!(error.contains(&path.display().to_string()));
    }

    #[test]
    fn staged_failures_point_to_the_retained_original() {
        let error = ProfileStoreError::invalid(Path::new("stage/configuration/history/codex.json"), "bad history")
            .original_path(Path::new("stage/configuration"), Path::new("state/configuration"));
        assert_eq!(error, ProfileStoreError::invalid(Path::new("state/configuration/history/codex.json"), "bad history"));
    }

    #[test]
    fn repair_journal_blocks_background_reads_without_reporting_corruption() {
        let directory = tempfile::tempdir().unwrap();
        let store = super::super::ConfigStore::new(directory.path().to_path_buf());
        let path = store.client_settings_path(asb_core::AppKind::Codex);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = serde_json::to_vec(&asb_core::ownership::default_client_settings(asb_core::AppKind::Codex)).unwrap();
        std::fs::write(&path, &original).unwrap();
        let journal = directory.path().join(super::super::REPAIR_JOURNAL_FILE);
        std::fs::write(&journal, "pending repair").unwrap();
        assert_eq!(store.get_client_settings(asb_core::AppKind::Codex),
            Err(ProfileStoreError::Recovering { path: journal.clone() }));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_file(journal).unwrap();
        store.get_client_settings(asb_core::AppKind::Codex).unwrap();
    }

    #[test]
    fn missing_provider_lookup_does_not_invent_a_corrupt_file() {
        let directory = tempfile::tempdir().unwrap();
        let store = super::super::ConfigStore::new(directory.path().to_path_buf());
        assert_eq!(store.find_codex_provider_file("missing"),
            Err(ProfileStoreError::NotFound { id: "missing".into() }));
        assert_eq!(store.find_codex_provider_with_revision("missing"),
            Err(ProfileStoreError::NotFound { id: "missing".into() }));
        assert!(!store.configuration_dir().exists());
    }
}
