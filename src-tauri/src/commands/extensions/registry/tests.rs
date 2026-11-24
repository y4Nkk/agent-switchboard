use super::*;
use asb_core::extensions::contracts::SecretValue;
use serde_json::json;

#[test]
fn manual_migration_can_associate_the_new_release_and_resume_same_origin_updates() {
    let old = McpRegistrySource {
        name: "com.example/service".into(),
        version: "1.0".into(),
        endpoint: "https://old.example/mcp".into(),
    };
    let latest = serde_json::from_value(json!({
        "name": old.name, "version": "2.0", "description": "Updated connection",
        "remotes": [{"type": "streamable-http", "url": "https://new.example/mcp", "headers": [
            {"name": "Authorization", "isRequired": true}, {"name": "X-Mode", "isRequired": true}
        ]}]
    })).unwrap();
    let current = McpDefinition::Http { url: old.endpoint.clone(), headers: Default::default(), bearer: None };
    assert!(matches!(classify_update(&current, &old, &latest), RegistryUpdate::Manual { connections, .. } if connections.len() == 1));
    let mut migrated = McpDefinition::Http {
        url: "https://new.example/mcp".into(),
        headers: [("x-mode".into(), SecretValue::Plain { value: "client".into() })].into(),
        bearer: None,
    };
    assert!(matches!(classify_update(&migrated, &old, &latest), RegistryUpdate::Manual { .. }));
    let McpDefinition::Http { bearer, .. } = &mut migrated else { unreachable!() };
    *bearer = Some(SecretValue::EnvRef { name: "EXAMPLE_TOKEN".into() });
    let RegistryUpdate::Available { source: recorded, action: RegistryUpdateAction::AssociateCurrent } =
        classify_update(&migrated, &old, &latest) else { panic!("current connection should match") };
    assert_eq!(recorded.version, "2.0");
    assert_eq!(recorded.endpoint, "https://new.example/mcp");

    let next = serde_json::from_value(json!({
        "name": old.name, "version": "3.0", "description": "Same origin",
        "remotes": [{"type": "streamable-http", "url": "https://new.example/mcp-v3"}]
    })).unwrap();
    assert!(matches!(classify_update(&migrated, &recorded, &next), RegistryUpdate::Available {
        action: RegistryUpdateAction::ReplaceEndpoint, ..
    }));
}
