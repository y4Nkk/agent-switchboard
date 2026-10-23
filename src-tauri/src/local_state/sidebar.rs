use super::LocalState;
use std::{fs, io::Write};
use uuid::Uuid;

impl LocalState {
    pub fn sidebar_collapsed(&self) -> Result<bool, String> {
        match fs::read(self.root.join("sidebar-collapsed.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("侧栏状态记录无效：{error}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(format!("无法读取侧栏状态：{error}")),
        }
    }

    pub fn set_sidebar_collapsed(&self, collapsed: bool) -> Result<(), String> {
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("无法创建侧栏状态目录：{error}"))?;
        let temporary = self.root.join(format!("sidebar-collapsed.{}.tmp", Uuid::new_v4()));
        let content: &[u8] = if collapsed { b"true" } else { b"false" };
        let write = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)
            .and_then(|mut file| {
                file.write_all(content)?;
                file.sync_all()
            });
        if let Err(error) = write {
            let _ = fs::remove_file(&temporary);
            return Err(format!("无法写入并同步侧栏状态：{error}"));
        }
        if let Err(error) = fs::rename(&temporary, self.root.join("sidebar-collapsed.json")) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("无法原子保存侧栏状态：{error}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_state::{AppSettings, WorkspacePage};

    #[test]
    fn first_read_defaults_to_expanded_without_creating_state() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state");
        let state = LocalState::from_root(path.clone());
        assert!(!state.sidebar_collapsed().unwrap());
        assert!(!path.exists());
    }

    #[test]
    fn both_states_survive_recreation_and_other_preference_writes() {
        let root = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(root.path().to_path_buf());
        for collapsed in [true, false, true] {
            state.set_sidebar_collapsed(collapsed).unwrap();
            state.set_app_settings(&AppSettings::default()).unwrap();
            state.remember_workspace_page(WorkspacePage::Sessions).unwrap();
            let restarted = LocalState::from_root(root.path().to_path_buf());
            assert_eq!(restarted.sidebar_collapsed().unwrap(), collapsed);
            assert_eq!(restarted.last_workspace_page().unwrap(), WorkspacePage::Sessions);
        }
        assert!(fs::read_dir(root.path()).unwrap().all(|entry| {
            entry.unwrap().path().extension().is_some_and(|extension| extension != "tmp")
        }));
    }

    #[test]
    fn invalid_records_are_rejected_without_overwriting_them() {
        let root = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(root.path().to_path_buf());
        let path = root.path().join("sidebar-collapsed.json");
        for invalid in ["", "null", "1", "\"true\"", "{}", "{\"sidebarCollapsed\":true}"] {
            fs::write(&path, invalid).unwrap();
            assert!(state.sidebar_collapsed().is_err());
            assert_eq!(fs::read(&path).unwrap(), invalid.as_bytes());
        }
    }

    #[test]
    fn unreadable_record_is_preserved_and_failed_replace_cleans_its_temporary() {
        let root = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(root.path().to_path_buf());
        let path = root.path().join("sidebar-collapsed.json");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), "original").unwrap();
        assert!(state.sidebar_collapsed().is_err());
        assert!(state.set_sidebar_collapsed(true).is_err());
        assert_eq!(fs::read_to_string(path.join("keep")).unwrap(), "original");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn locked_record_keeps_the_last_saved_state_after_failed_replace() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(root.path().to_path_buf());
        state.set_sidebar_collapsed(true).unwrap();
        let guard = fs::OpenOptions::new().read(true).share_mode(0)
            .open(root.path().join("sidebar-collapsed.json")).unwrap();
        assert!(state.set_sidebar_collapsed(false).is_err());
        drop(guard);
        assert!(state.sidebar_collapsed().unwrap());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
