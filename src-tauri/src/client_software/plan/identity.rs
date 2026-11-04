use crate::client_software::types::{Installation, Manager};
use sha2::{Digest, Sha256};
use std::io::Read;

pub(super) fn identity_revision(item: &Installation) -> Result<String, String> {
    let mut hash = Sha256::new();
    for path in [&item.launcher, &item.executable] {
        let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        hash.update(format!("{}:{}:{:?}", path.display(), metadata.len(), metadata.modified().map_err(|e| e.to_string())?));
    }
    let manifest = match item.manager {
        Manager::Npm => Some(item.root.join("package.json")),
        Manager::Homebrew => Some(item.root.join(".metadata/INSTALL_RECEIPT.json")),
        Manager::Native if item.app == asb_core::contracts::AppKind::Codex => item.executable.ancestors().skip(1).take(6).map(|p| p.join("codex-package.json")).find(|p| p.is_file()),
        _ => None,
    };
    if let Some(path) = manifest {
        let mut bytes = Vec::new();
        std::fs::File::open(path).map_err(|e| e.to_string())?.take(1_048_577)
            .read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() > 1_048_576 { return Err("Installation manifest exceeds size limit".into()); }
        hash.update(bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
}
