use super::*;
use crate::commands::extensions::sources::{cache_candidates, candidate_id, import_candidate_content};
use crate::extensions::sources::{scan_local_source, SkillCandidate};
use std::{fs, path::Path};

fn candidate(root: &Path, description: &str, commit: &str) -> SkillCandidate {
    fs::write(root.join("SKILL.md"), format!("---\nname: replay-version\ndescription: {description}\n---\n")).unwrap();
    let mut candidate = scan_local_source(root, None).unwrap().remove(0);
    candidate.source_identity = "example/replay-skill".into();
    candidate.subpath = "skills/replay-version".into();
    candidate.resolved_commit = Some(commit.into());
    candidate
}

#[test]
fn upstream_revert_to_stored_bytes_records_the_reviewed_commit() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    let first = candidate(&source, "first version", &"1".repeat(40));
    let second = candidate(&source, "second version", &"2".repeat(40));
    let reverted = candidate(&source, "first version", &"3".repeat(40));
    assert_eq!(first.content_digest, reverted.content_digest);
    assert_ne!(candidate_id(&first), candidate_id(&reverted));
    cache_candidates(vec![first.clone(), second.clone(), reverted.clone()]);

    let store = ExtensionStore::from_root(root.path().join("library"));
    let imported = import_candidate_content(&store, &first, first.name.clone(), None).unwrap();
    let second_version = apply(&store, &imported.id, imported.revision, SkillVersionChange::Upstream {
        candidate_id: candidate_id(&second),
    }).unwrap();
    let new_version = apply(&store, &imported.id, second_version.revision, SkillVersionChange::Upstream {
        candidate_id: candidate_id(&reverted),
    }).unwrap();
    let current = store.get_definition(&imported.id).unwrap().unwrap();
    let ExtensionPayload::Skill(skill) = current.payload else { panic!("not a Skill") };
    assert_eq!(skill.content_digest, first.content_digest);
    assert_eq!(skill.manifest.description.as_deref(), Some("first version"));
    assert_eq!(skill.source.unwrap().resolved_commit, reverted.resolved_commit);

    apply(&store, &imported.id, new_version.revision, SkillVersionChange::Restore { digest: second.content_digest.clone() }).unwrap();
    let restored = store.get_definition(&imported.id).unwrap().unwrap();
    let ExtensionPayload::Skill(skill) = restored.payload else { panic!("not a Skill") };
    assert_eq!(skill.content_digest, second.content_digest);
    assert_eq!(skill.source.unwrap().resolved_commit, reverted.resolved_commit);
}
