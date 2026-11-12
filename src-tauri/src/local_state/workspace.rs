use super::{LocalState, WorkspacePage};
use std::fs;
use uuid::Uuid;

impl LocalState {
    /// Browsing position has its own file so preference saves cannot overwrite it.
    pub fn last_workspace_page(&self) -> Result<WorkspacePage, String> {
        match fs::read(self.root.join("last-workspace-page.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("上次访问页面记录无效：{error}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(WorkspacePage::Providers)
            }
            Err(error) => Err(format!("无法读取上次访问页面：{error}")),
        }
    }

    pub fn remember_workspace_page(&self, page: WorkspacePage) -> Result<(), String> {
        let content = serde_json::to_vec(&page).map_err(|error| error.to_string())?;
        fs::create_dir_all(&self.root).map_err(|error| error.to_string())?;
        let temporary = self.root.join(format!("last-workspace-page.{}.tmp", Uuid::new_v4()));
        if let Err(error) = fs::write(&temporary, content) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("无法写入上次访问页面：{error}"));
        }
        if let Err(error) = fs::rename(&temporary, self.root.join("last-workspace-page.json")) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("无法保存上次访问页面：{error}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_state::{AppSettings, LanguagePreference};

    /// The shipped v0.2 file (the current contract minus `language`) upgrades
    /// once at startup: every preference is kept, the language pins to Chinese,
    /// and the file is atomically rewritten into the current contract.
    #[test]
    fn legacy_preferences_upgrade_in_place_with_language_chinese() {
        let root = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(root.path().to_path_buf());
        let mut legacy = serde_json::to_value(AppSettings::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("language");
        let original = serde_json::to_vec(&legacy).unwrap();
        fs::write(state.settings_path(), &original).unwrap();
        assert!(state.get_app_settings().is_err());
        assert_eq!(fs::read(state.settings_path()).unwrap(), original);
        state.prepare_app_settings().unwrap();
        let expected = AppSettings { language: LanguagePreference::ZhCn, ..AppSettings::default() };
        assert_eq!(state.get_app_settings().unwrap(), expected);
        // The upgrade rewrote the file into the current contract, so the next
        // read takes the daily path instead of upgrading again.
        assert_ne!(fs::read(state.settings_path()).unwrap(), original);
        assert_eq!(state.get_app_settings().unwrap(), expected);
        let stored: serde_json::Value =
            serde_json::from_slice(&fs::read(state.settings_path()).unwrap()).unwrap();
        assert_eq!(stored["language"], "zh-CN");
    }

    #[test]
    fn browsing_position_does_not_race_with_preference_snapshots() {
        let root = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(root.path().to_path_buf());
        assert_eq!(state.last_workspace_page().unwrap(), WorkspacePage::Providers);
        assert!(!root.path().join("last-workspace-page.json").exists());
        state.remember_workspace_page(WorkspacePage::Usage).unwrap();
        state.set_app_settings(&AppSettings::default()).unwrap();
        assert_eq!(state.last_workspace_page().unwrap(), WorkspacePage::Usage);
        state.remember_workspace_page(WorkspacePage::Sessions).unwrap();
        assert_eq!(state.last_workspace_page().unwrap(), WorkspacePage::Sessions);
        assert_eq!(state.get_app_settings().unwrap(), AppSettings::default());
    }

    #[test]
    fn unreadable_page_contract_is_rejected_without_rewriting_it() {
        let root = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(root.path().to_path_buf());
        let path = root.path().join("last-workspace-page.json");
        let invalid = br#"{"page":"providers","draft":{}}"#;
        fs::write(&path, invalid).unwrap();
        assert!(state.last_workspace_page().is_err());
        assert_eq!(fs::read(&path).unwrap(), invalid);
    }

    #[test]
    fn unsupported_preferences_are_preserved_without_guessing() {
        let root = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(root.path().to_path_buf());
        let mut older = serde_json::to_value(AppSettings::default()).unwrap();
        older.as_object_mut().unwrap().remove("interfaceScale");
        let original = serde_json::to_vec(&older).unwrap();
        fs::write(state.settings_path(), &original).unwrap();
        assert!(state.get_app_settings().is_err());
        assert_eq!(fs::read(state.settings_path()).unwrap(), original);
        assert!(state.prepare_app_settings().is_err());
        assert_eq!(fs::read(state.settings_path()).unwrap(), original);
    }
}
