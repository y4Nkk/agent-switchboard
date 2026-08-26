//! Injected filesystem boundary for the switch executor.
//!
//! Production uses [`FsIo`]. Tests wrap `FsIo` with deterministic failure
//! injection so every recovery path can be exercised without touching a real
//! user configuration file.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

pub trait SwitchIo {
    fn read_file(&self, path: &Path) -> io::Result<String>;
    /// Creates a file that must not already exist (lock acquisition).
    fn write_new_file(&self, path: &Path, content: &str) -> io::Result<()>;
    /// Writes or replaces a file's content in place (backups only).
    fn write_file_replace(&self, path: &Path, content: &str) -> io::Result<()>;
    /// Atomic replace: `from` replaces an existing `to`.
    fn rename_replace(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path) -> io::Result<()>;
    fn ensure_dir(&self, path: &Path) -> io::Result<()>;
    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;
    fn now_rfc3339(&self) -> String;
}

pub struct FsIo;

impl SwitchIo for FsIo {
    fn read_file(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn write_new_file(&self, path: &Path, content: &str) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        use std::io::Write;
        file.write_all(content.as_bytes())
    }

    fn write_file_replace(&self, path: &Path, content: &str) -> io::Result<()> {
        fs::write(path, content)
    }

    fn rename_replace(&self, from: &Path, to: &Path) -> io::Result<()> {
        // std::fs::rename on Windows uses MoveFileEx with
        // MOVEFILE_REPLACE_EXISTING: an atomic replacement of `to`.
        fs::rename(from, to)
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn ensure_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(path)? {
            out.push(entry?.path());
        }
        Ok(out)
    }

    fn now_rfc3339(&self) -> String {
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }
}
