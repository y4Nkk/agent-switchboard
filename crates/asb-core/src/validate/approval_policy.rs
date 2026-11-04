use crate::contracts::ConfigValue;

pub const REQUIRED_FIELDS: &[&str] = &["sandbox_approval", "rules", "mcp_elicitations"];
pub const OPTIONAL_FIELDS: &[&str] = &["request_permissions", "skill_approval"];

pub fn valid(value: &ConfigValue) -> bool {
    if let ConfigValue::Str(value) = value {
        return matches!(value.as_str(), "on-request" | "never");
    }
    let ConfigValue::Object(policy) = value else { return false; };
    let Some(ConfigValue::Object(fields)) = policy.get("granular") else { return false; };
    policy.len() == 1
        && REQUIRED_FIELDS.iter().all(|key| fields.contains_key(*key))
        && fields.iter().all(|(key, value)| {
            (REQUIRED_FIELDS.contains(&key.as_str()) || OPTIONAL_FIELDS.contains(&key.as_str()))
                && matches!(value, ConfigValue::Bool(_))
        })
}
