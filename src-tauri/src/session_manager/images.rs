use base64::Engine;
use serde_json::{Map, Value};
use std::fs::File;
use std::io::Read;

pub(super) fn references(value: &Value) -> Vec<String> {
    let mut output = Vec::new();
    collect(value, &mut output);
    output
}

fn collect(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::Array(values) => for value in values { collect(value, output); },
        Value::Object(object) => {
            let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
            if matches!(kind, "imageGeneration" | "image_generation_call") {
                if let Some(image) = generated_image(object) { output.push(image); return; }
                if let Some(path) = object.get("savedPath").and_then(Value::as_str).filter(|path| !path.is_empty()) {
                    output.push(path.to_string());
                }
            }
            if matches!(kind, "image" | "input_image" | "image_url" | "imageView" | "image_view") {
                let uri = object.get("image_url").or_else(|| object.get("url"))
                    .and_then(|value| value.as_str().or_else(|| value.get("url").and_then(Value::as_str)));
                let source = object.get("source");
                let encoded = source.and_then(|value| value.get("data")).and_then(Value::as_str)
                    .zip(source.and_then(|value| value.get("media_type")).and_then(Value::as_str))
                    .map(|(data, mime)| format!("data:{mime};base64,{data}"));
                let local = object.get("imagePath").or_else(|| object.get("path")).and_then(Value::as_str);
                if let Some(uri) = uri.filter(|uri| uri.starts_with("data:image/")).map(str::to_string)
                    .or(encoded).or_else(|| local.map(str::to_string)) { output.push(uri); }
            }
            for value in object.values() { if value.is_object() || value.is_array() { collect(value, output); } }
        }
        _ => {}
    }
}

pub(super) fn generated_image(object: &Map<String, Value>) -> Option<String> {
    let result = object.get("result")?.as_str()?;
    if result.starts_with("data:image/") { return Some(result.to_string()); }
    let header = base64::engine::general_purpose::STANDARD.decode(result.get(..16)?).ok()?;
    Some(format!("data:{};base64,{result}", mime(&header)?))
}

fn mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { Some("image/png") }
    else if bytes.starts_with(&[0xff,0xd8,0xff]) { Some("image/jpeg") }
    else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") { Some("image/gif") }
    else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]) { Some("image/webp") }
    else if bytes.starts_with(b"BM") { Some("image/bmp") }
    else { None }
}

pub(super) fn data_uri(bytes: &[u8]) -> Result<String, String> {
    let mime = mime(bytes).ok_or("已保留图片格式不受支持")?;
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

pub(super) fn resolve(references: &[String]) -> (Vec<String>, Vec<String>) {
    let mut images = Vec::new();
    let mut issues = Vec::new();
    let mut total_bytes = 0;
    for reference in references {
        let resolved = if reference.starts_with("data:image/") { Ok(reference.clone()) } else { load_file(reference) };
        match resolved {
            Ok(image) if total_bytes + image.len() <= super::records::MAX_RECORD_BYTES as usize => {
                total_bytes += image.len(); images.push(image);
            }
            Ok(_) => issues.push("图片附件合计超过 32 MiB 单次读取上限；文本仍可阅读".into()),
            Err(error) => issues.push(error),
        }
    }
    (images, issues)
}

fn load_file(path: &str) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("图片附件不可读 {path}: {error}"))?;
    let mut header = [0u8; 12];
    let count = file.read(&mut header).map_err(|error| error.to_string())?;
    let bytes = &header[..count];
    let mime = mime(bytes).ok_or_else(|| format!("图片附件格式不受支持: {path}"))?;
    let mut data = bytes.to_vec();
    file.take(24 * 1024 * 1024 + 1).read_to_end(&mut data).map_err(|error| error.to_string())?;
    if data.len() > 24 * 1024 * 1024 { return Err(format!("图片附件超过 24 MiB 文件读取上限: {path}")); }
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(data)))
}

pub(super) fn copy_text(message: &super::SessionMessage) -> Result<String, String> {
    let (images, issues) = resolve(&message.images);
    if !issues.is_empty() { return Err(issues.join("；")); }
    Ok(std::iter::once(message.content.clone()).chain(images.into_iter().map(|uri| format!("![会话图片]({uri})")))
        .collect::<Vec<_>>().join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_pixels_remain_available_without_the_saved_image_file() {
        let encoded = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGN8AAAAASUVORK5CYII=";
        let item = serde_json::json!({"type":"imageGeneration","id":"image","result":encoded,
            "revisedPrompt":"retained prompt","savedPath":"missing-original.png"});
        let message = super::super::parser::message_from(&item).unwrap();
        assert_eq!(message.images, vec![format!("data:image/png;base64,{encoded}")]);
        assert!(message.content.contains("retained prompt"));
        assert!(!message.content.contains(encoded));
        let (images, issues) = resolve(&message.images);
        assert!(issues.is_empty());
        assert_eq!(images.len(), 1);
        assert!(copy_text(&message).unwrap().contains(encoded));
    }
}
