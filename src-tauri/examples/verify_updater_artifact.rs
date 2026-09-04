use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use std::{env, fs, path::Path};

fn decode_base64(value: &str, label: &str) -> Result<String, String> {
    let bytes = STANDARD
        .decode(value)
        .map_err(|error| format!("{label} is not valid Base64: {error}"))?;
    String::from_utf8(bytes).map_err(|error| format!("{label} is not valid UTF-8: {error}"))
}

fn verify_payload(
    payload: &[u8],
    encoded_signature: &str,
    encoded_public_key: &str,
) -> Result<(), String> {
    let public_key = PublicKey::decode(&decode_base64(
        encoded_public_key,
        "embedded updater public key",
    )?)
    .map_err(|error| format!("embedded updater public key is invalid: {error}"))?;
    let signature = Signature::decode(&decode_base64(encoded_signature, "updater signature")?)
        .map_err(|error| format!("updater signature is invalid: {error}"))?;
    public_key
        .verify(payload, &signature, true)
        .map_err(|error| format!("updater signature verification failed: {error}"))
}

fn embedded_public_key() -> Result<String, String> {
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
        .map_err(|error| format!("could not read Tauri updater configuration: {error}"))?;
    config
        .pointer("/plugins/updater/pubkey")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "Tauri updater public key is missing".to_owned())
}

fn verify_artifact(artifact: &Path, signature: &Path) -> Result<(), String> {
    let payload = fs::read(artifact).map_err(|error| {
        format!(
            "could not read updater artifact {}: {error}",
            artifact.display()
        )
    })?;
    let signature = fs::read_to_string(signature).map_err(|error| {
        format!(
            "could not read updater signature {}: {error}",
            signature.display()
        )
    })?;
    verify_payload(&payload, &signature, &embedded_public_key()?)
}

fn run() -> Result<(), String> {
    let mut arguments = env::args_os().skip(1);
    let artifact = arguments
        .next()
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "usage: verify_updater_artifact <artifact> <artifact.sig>".to_owned())?;
    let signature = arguments
        .next()
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "usage: verify_updater_artifact <artifact> <artifact.sig>".to_owned())?;
    if arguments.next().is_some() {
        return Err("usage: verify_updater_artifact <artifact> <artifact.sig>".to_owned());
    }

    verify_artifact(&artifact, &signature)?;
    println!("Verified updater signature: {}", artifact.display());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const SIGNATURE: &str = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";

    fn encoded(value: &str) -> String {
        STANDARD.encode(value)
    }

    #[test]
    fn accepts_a_matching_signature_and_rejects_a_changed_payload() {
        let public_key = encoded(&format!("untrusted comment: test\n{PUBLIC_KEY}\n"));
        let signature = encoded(SIGNATURE);

        assert!(verify_payload(b"test", &signature, &public_key).is_ok());
        assert!(verify_payload(b"changed", &signature, &public_key).is_err());
    }
}
