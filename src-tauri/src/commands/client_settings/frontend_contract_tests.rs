use super::*;
use crate::provider_diagnostics::{ProviderDiagnostic, ProviderFailureKind};
use crate::runtime_log::{RuntimeLogAction, RuntimeLogLevel, RuntimeLogSeverity};
use serde::de::{DeserializeOwned, Deserializer, Error, Visitor};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

struct EnumNames(Option<&'static [&'static str]>);

impl<'de> Deserializer<'de> for &mut EnumNames {
    type Error = serde::de::value::Error;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(Self::Error::custom("enum names collected without creating a value"))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self, _name: &'static str, variants: &'static [&'static str], visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0 = Some(variants);
        self.deserialize_any(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

/// Serde supplies the current accepted variants; reserialization verifies
/// the wire spelling rather than maintaining a second Rust variant list.
fn enum_values<T: DeserializeOwned + Serialize>() -> Vec<Value> {
    let mut names = EnumNames(None);
    let _ = T::deserialize(&mut names);
    names.0.expect("a Serde enum must publish its variants").iter().map(|name| {
        let value: T = serde_json::from_value(json!(name)).expect("deserialize current variant");
        serde_json::to_value(value).expect("serialize current variant")
    }).collect()
}

fn settings_contract(app: AppKind) -> Value {
    let specs = editor_catalog(app, SettingOwner::Client);
    json!({
        "app": app,
        "client": { "groups": editor_groups(app, &specs), "specs": specs,
            "directory": directory_catalog(app) },
        "provider": get_provider_parameters_catalog(app),
    })
}

#[test]
fn renderer_consumes_current_serialized_contracts() {
    let diagnostic = ProviderDiagnostic {
        kind: ProviderFailureKind::RateLimit,
        endpoint: "https://example.invalid/v1/responses".into(),
        transport: "http",
        status: Some(429),
        request_id: Some("fixture-request".into()),
        retry_after: Some("120".into()),
        body: None,
        body_truncated: false,
        message: "Rate limited".into(),
    };
    let contract = json!({
        "runtimeActions": enum_values::<RuntimeLogAction>(),
        "runtimeLevels": enum_values::<RuntimeLogLevel>(),
        "runtimeSeverities": enum_values::<RuntimeLogSeverity>(),
        "settings": [settings_contract(AppKind::Codex), settings_contract(AppKind::Claude)],
        "diagnostic": diagnostic,
    });
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("repository root");
    let node = std::env::var_os("ASB_TEST_NODE").unwrap_or_else(|| "node".into());
    let mut command = Command::new(node);
    command.arg("tests/frontend-contract-consumer.ts").current_dir(root)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().expect("frontend contract checks require Node; use npm run ci");
    {
        let mut input = child.stdin.take().expect("contract input");
        input.write_all(&serde_json::to_vec(&contract).expect("serialized contract"))
            .expect("send contract to frontend consumer");
    }
    let output = child.wait_with_output().expect("frontend contract consumer exits");
    assert!(output.status.success(), "frontend contract mismatch:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}
