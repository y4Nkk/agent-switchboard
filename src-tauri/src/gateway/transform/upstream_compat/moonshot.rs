//! Moonshot / Kimi Chat Completions `$ref` sibling compatibility.
//!
//! Moonshot rejects `$ref` nodes that carry sibling keywords, which tools
//! from either client may contain. Each such `$ref` moves into `allOf` for that
//! upstream only; every other provider keeps byte-identical tool schemas so
//! prompt-cache prefixes stay intact.

use serde_json::{json, Map, Value};

const MOONSHOT_HOST_SUFFIXES: &[&str] = &["moonshot.cn", "moonshot.ai", "kimi.com"];

/// Schema-valued object keywords whose values are schemas.
const SCHEMA_MAP_KEYWORDS: &[&str] = &[
    "properties",
    "patternProperties",
    "$defs",
    "definitions",
    "dependentSchemas",
    "dependencies",
];

/// Keywords holding a schema or an array of schemas.
const SINGLE_SCHEMA_KEYWORDS: &[&str] = &[
    "additionalProperties",
    "unevaluatedProperties",
    "additionalItems",
    "unevaluatedItems",
    "contains",
    "propertyNames",
    "if",
    "then",
    "else",
    "not",
    "items",
    "contentSchema",
    "prefixItems",
    "allOf",
    "anyOf",
    "oneOf",
];

/// Whether the resolved upstream base URL points at a Moonshot / Kimi host.
/// Fails closed on unparsable URLs: no host, no rewrite.
pub(super) fn upstream_requires_ref_sibling_all_of(base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url.trim()) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    MOONSHOT_HOST_SUFFIXES.iter().any(|suffix| {
        host == *suffix
            || host
                .strip_suffix(suffix)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

/// Rewrites schema positions in the final Chat request without entering data.
pub(super) fn rewrite_chat_tools(body: &mut Value) -> Result<bool, String> {
    let Some(tools) = body.get_mut("tools").and_then(Value::as_array_mut) else {
        return Ok(false);
    };
    let mut changed = false;
    for tool in tools.iter_mut() {
        if let Some(parameters) = tool
            .get_mut("function")
            .and_then(|function| function.get_mut("parameters"))
        {
            changed |= wrap_ref_siblings(parameters)? > 0;
        }
    }
    Ok(changed)
}

/// Walks a JSON Schema through schema-valued keywords only and moves every
/// `$ref` with sibling keywords into `allOf`. Data-valued keywords
/// (`default`, `examples`, `enum`, `const`) and unknown `x-…` extensions are
/// never entered, so a literal `$ref` key inside them stays untouched.
fn wrap_ref_siblings(schema: &mut Value) -> Result<usize, String> {
    let Value::Object(map) = schema else {
        return Ok(0);
    };
    let mut rewritten = 0;
    if map.len() > 1 && map.get("$ref").is_some_and(Value::is_string) {
        move_ref_into_all_of(map)?;
        rewritten += 1;
    }
    for (key, child) in map.iter_mut() {
        let key = key.as_str();
        if SCHEMA_MAP_KEYWORDS.contains(&key) {
            if let Value::Object(entries) = child {
                for entry in entries.values_mut() {
                    rewritten += wrap_ref_siblings(entry)?;
                }
            }
        } else if SINGLE_SCHEMA_KEYWORDS.contains(&key) {
            match child {
                Value::Array(entries) => {
                    for entry in entries {
                        rewritten += wrap_ref_siblings(entry)?;
                    }
                }
                other => rewritten += wrap_ref_siblings(other)?,
            }
        }
    }
    Ok(rewritten)
}

fn move_ref_into_all_of(map: &mut Map<String, Value>) -> Result<(), String> {
    if map.get("allOf").is_some_and(|value| !value.is_array()) {
        return Err("Moonshot 工具 schema 的 allOf 必须是数组，未丢弃原有约束".into());
    }
    let Some(reference) = map.remove("$ref") else {
        return Ok(());
    };
    let branch = json!({ "$ref": reference });
    match map.get_mut("allOf") {
        Some(Value::Array(branches)) => branches.push(branch),
        _ => {
            map.insert("allOf".to_string(), Value::Array(vec![branch]));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Codex `automation_update` reduced to the two positions Moonshot
    /// rejected: a property `$ref` with a description and a `$defs` entry
    /// `$ref` with `type`/`minLength` siblings.
    fn desktop_like_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "$ref": "#/$defs/__schema20", "description": "Prompt to run" },
                "mode": { "type": "string", "enum": ["fast", "slow"] }
            },
            "required": ["prompt"],
            "$defs": {
                "__schema20": { "$ref": "#/$defs/__schema2", "type": "string", "minLength": 1 },
                "__schema2": { "type": "string" }
            }
        })
    }

    #[test]
    fn gate_matches_moonshot_and_kimi_hosts_only() {
        for url in [
            "https://api.moonshot.cn/v1",
            "https://api.moonshot.ai/v1/",
            "https://api.kimi.com/coding/v1",
            "https://EU.API.KIMI.COM/coding/v1",
        ] {
            assert!(upstream_requires_ref_sibling_all_of(url), "{url}");
        }
        for url in [
            "https://api.deepseek.com/v1",
            "https://open.bigmodel.cn/api/paas/v4",
            "https://not-moonshot.cn.example.test/v1",
        ] {
            assert!(!upstream_requires_ref_sibling_all_of(url), "{url}");
        }
        assert!(!upstream_requires_ref_sibling_all_of("::::"));
    }

    #[test]
    fn ref_siblings_move_into_allof_only_where_present() {
        let body = json!({
            "tools": [
                { "type": "function", "function": { "name": "automation_update", "parameters": desktop_like_schema() } },
                { "type": "function", "function": { "name": "clean", "parameters": { "type": "object" } } }
            ]
        });
        let mut rewritten = body;
        assert!(rewrite_chat_tools(&mut rewritten).unwrap());
        let parameters = &rewritten["tools"][0]["function"]["parameters"];
        assert_eq!(
            parameters["properties"]["prompt"]["allOf"][0]["$ref"],
            json!("#/$defs/__schema20")
        );
        assert_eq!(
            parameters["properties"]["prompt"]["description"],
            json!("Prompt to run")
        );
        assert_eq!(
            parameters["$defs"]["__schema20"]["allOf"][0]["$ref"],
            json!("#/$defs/__schema2")
        );
        assert_eq!(parameters["$defs"]["__schema20"]["type"], json!("string"));
        // The untouched tool stays byte-identical in shape.
        assert_eq!(
            rewritten["tools"][1]["function"]["parameters"],
            json!({"type": "object"})
        );
    }

    #[test]
    fn data_valued_keywords_are_never_entered() {
        let mut schema = json!({
            "default": { "$ref": "kept-with-sibling", "type": "string" },
            "examples": [{ "$ref": "also-kept" }],
            "properties": { "real": { "$ref": "#/x", "description": "moved" } }
        });
        assert_eq!(wrap_ref_siblings(&mut schema).unwrap(), 1);
        assert!(schema["default"].get("$ref").is_some());
        assert_eq!(schema["examples"][0]["$ref"], json!("also-kept"));
        assert!(schema["properties"]["real"].get("$ref").is_none());
    }

    #[test]
    fn schema_keywords_and_existing_allof_keep_every_constraint() {
        let node = json!({"$ref":"#/x", "allOf":[{"minLength":2}], "description":"kept"});
        for keyword in ["prefixItems", "items", "allOf", "anyOf", "oneOf"] {
            let mut schema = json!({});
            schema[keyword] = json!([node.clone()]);
            assert_eq!(wrap_ref_siblings(&mut schema).unwrap(), 1);
            assert_eq!(schema[keyword][0]["allOf"], json!([{"minLength":2}, {"$ref":"#/x"}]));
        }
        for keyword in ["unevaluatedProperties", "unevaluatedItems", "contentSchema"] {
            let mut schema = json!({});
            schema[keyword] = node.clone();
            assert_eq!(wrap_ref_siblings(&mut schema).unwrap(), 1);
            assert_eq!(schema[keyword]["allOf"], json!([{"minLength":2}, {"$ref":"#/x"}]));
        }
    }
}
