use std::{fs, io::ErrorKind, path::Path};
use asb_core::extensions::contracts::{
    ExtensionBinding, ExtensionDefinition, ExtensionPayload, FileState,
    ManagedBaseline, ManagedBaselineFile, ProjectRegistration,
};
use asb_core::extensions::skill::content_digest;
use super::skills::walk_skill_dir;

pub fn binding_file_state(
    binding: &ExtensionBinding, definition: &ExtensionDefinition,
    baseline: Option<&ManagedBaselineFile>, projects: &[ProjectRegistration],
) -> FileState {
    if binding.last_applied_revision.is_none() { return FileState::NotDeployed; }
    match &definition.payload {
        ExtensionPayload::Mcp(_) => mcp_state(binding, definition, baseline, projects),
        ExtensionPayload::Skill(skill) => skill_state(
            binding.locked_digest.as_deref().unwrap_or(&skill.content_digest), baseline,
        ),
    }
}

fn mcp_state(
    binding: &ExtensionBinding, definition: &ExtensionDefinition,
    baseline: Option<&ManagedBaselineFile>, projects: &[ProjectRegistration],
) -> FileState {
    let Some(ManagedBaseline::DocumentEntry {
        target_path, last_written_value, last_document_hash, ..
    }) = baseline.and_then(|file| file.entries.iter()
        .find(|entry| matches!(entry, ManagedBaseline::DocumentEntry { .. }))) else {
        return FileState::NotDeployed;
    };
    let text = match fs::read_to_string(Path::new(target_path)) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return FileState::Missing,
        Err(_) => return FileState::Unreadable,
    };
    // Document hashes are a fast path; unrelated first-party edits do not
    // invalidate the owned entry's baseline.
    let document_unchanged = {
        use sha2::Digest;
        let hash = sha2::Sha256::digest(text.as_bytes()).iter()
            .map(|byte| format!("{byte:02x}")).collect::<String>();
        hash == *last_document_hash
    };
    if !document_unchanged {
        let current = match asb_core::extensions::mcp::managed_entry_text(binding, &text, projects) {
            Ok(current) => current,
            Err(_) => return FileState::Unreadable,
        };
        let unchanged = asb_core::extensions::mcp::managed_entry_unchanged(
            binding.target.client(), current, last_written_value.as_deref(),
        );
        if !unchanged { return FileState::ExternalChange; }
    }
    let config_revisions = definition.mcp_config_revision()..=definition.revision;
    if binding.last_applied_revision.is_some_and(|applied| config_revisions.contains(&applied)) {
        FileState::InSync
    } else {
        FileState::PendingApply
    }
}

fn skill_state(desired_digest: &str, baseline: Option<&ManagedBaselineFile>) -> FileState {
    let Some(ManagedBaseline::Directory { target_dir, last_digest, .. }) = baseline
        .and_then(|file| file.entries.iter().find(|entry| matches!(entry, ManagedBaseline::Directory { .. }))) else {
        return FileState::NotDeployed;
    };
    match walk_skill_dir(Path::new(target_dir)) {
        Ok(entries) if content_digest(&entries) != *last_digest => FileState::ExternalChange,
        Ok(_) if last_digest == desired_digest => FileState::InSync,
        Ok(_) => FileState::PendingApply,
        Err(_) if !Path::new(target_dir).exists() => FileState::Missing,
        Err(_) => FileState::Unreadable,
    }
}

#[cfg(test)]
mod tests;
