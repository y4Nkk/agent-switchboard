use super::CodexSubagentRoute;

fn route(profile_id: &str, model: &str) -> CodexSubagentRoute {
    CodexSubagentRoute {
        profile_id: profile_id.to_string(),
        model: model.to_string(),
    }
}

const UUID: &str = "01234567-89ab-cdef-0123-456789abcdef";

#[test]
fn wire_id_round_trips() {
    let original = route(UUID, "gpt-5-codex");
    let parsed = CodexSubagentRoute::parse_wire_id(&original.wire_id()).unwrap();
    assert_eq!(parsed, original);
    assert_eq!(
        original.wire_id(),
        "asb:01234567-89ab-cdef-0123-456789abcdef/gpt-5-codex"
    );
}

#[test]
fn a_model_id_containing_slashes_survives_the_wire_spelling() {
    let original = route(UUID, "meta-llama/Llama-4");
    let parsed = CodexSubagentRoute::parse_wire_id(&original.wire_id()).unwrap();
    assert_eq!(parsed, original);
}

#[test]
fn malformed_wire_spellings_are_rejected() {
    for value in [
        "gpt-5-codex",
        "asb:",
        "asb:gpt-5-codex",
        "asb:/gpt-5-codex",
        "asb:01234567-89ab-cdef-0123-456789abcdef/",
    ] {
        assert!(
            CodexSubagentRoute::parse_wire_id(value).is_none(),
            "{value} must not parse"
        );
    }
}

#[test]
fn canonical_profile_id_shape_is_checked() {
    assert!(route(UUID, "m").has_canonical_profile_id());
    for malformed in [
        "01234567-89ab-cdef-0123-456789abcdeg",
        "0123456789ab-cdef-0123-456789abcdef",
        "01234567-89ab-cdef-0123-456789abcd",
        "",
    ] {
        assert!(
            !route(malformed, "m").has_canonical_profile_id(),
            "{malformed} must not be canonical"
        );
    }
}
