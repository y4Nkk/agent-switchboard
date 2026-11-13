use super::ProfileSaveTarget;
use serde_json::json;

#[test]
fn save_targets_require_an_explicit_kind_and_revision_for_updates() {
    let id = "01234567-89ab-cdef-0123-456789abcdef";
    let created: ProfileSaveTarget = serde_json::from_value(json!({
        "kind": "create", "profileId": id,
    })).unwrap();
    assert_eq!(created.profile_id(), id);
    assert!(serde_json::from_value::<ProfileSaveTarget>(json!({
        "kind": "update", "profileId": id,
    })).is_err());
    assert!(serde_json::from_value::<ProfileSaveTarget>(json!({
        "profileId": null, "expectedFileHash": null,
    })).is_err());
    assert!(serde_json::from_value::<ProfileSaveTarget>(json!({
        "kind": "create", "profileId": id, "expectedFileHash": "old-revision",
    })).is_err());
}
