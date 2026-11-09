use super::{Error, Result, TARGET};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs::{self, File}, io::{BufRead, BufReader, Read, Write}, path::Path};

pub(super) struct Header { pub id: String, pub provider: Option<String>, value: Value, bom: bool, ending: &'static [u8] }

pub(super) fn header(reader: &mut impl BufRead) -> Result<Header> {
    let mut bytes = Vec::new();
    reader.take(super::super::records::MAX_RECORD_BYTES + 1).read_until(b'\n', &mut bytes)?;
    if bytes.len() as u64 > super::super::records::MAX_RECORD_BYTES { return Err("Codex session metadata exceeds 32 MiB".into()); }
    let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
    let value: Value = serde_json::from_slice(if bom { &bytes[3..] } else { &bytes }).map_err(|error| error.to_string())?;
    if value["type"] != "session_meta" { return Err("Codex transcript has no leading session_meta record".into()); }
    let id = value["payload"]["id"].as_str().filter(|id| !id.is_empty()).ok_or("Codex session metadata has no identity")?.to_string();
    let provider = match &value["payload"]["model_provider"] {
        Value::Null => None, Value::String(value) => Some(value.clone()),
        _ => return Err("Codex session provider must be a string or absent".into()),
    };
    let ending: &'static [u8] = if bytes.ends_with(b"\r\n") { b"\r\n" } else if bytes.ends_with(b"\n") { b"\n" } else { b"" };
    Ok(Header { id, provider, value, bom, ending })
}

pub(super) fn inspect(path: &Path) -> Result<Header> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(super::super::record_file::decode(path, file)?);
    header(&mut reader)
}

pub(super) fn digest(path: &Path) -> Result<String> {
    let result = (|| -> std::io::Result<String> {
        let mut file = File::open(path)?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            let length = file.read(&mut buffer)?;
            if length == 0 { break; }
            hash.update(&buffer[..length]);
        }
        Ok(format!("{:x}", hash.finalize()))
    })();
    result.map_err(|error| Error::Failure(format!("{}: {error}", path.display())))
}

pub(super) fn candidate(source: &Path, target: &Path, id: &str, provider: Option<&str>) -> Result<()> {
    let mut reader = BufReader::new(super::super::record_file::decode(source, File::open(source)?)?);
    let mut meta = header(&mut reader)?;
    if meta.id != id || meta.provider.as_deref() != provider { return Err(Error::Changed); }
    meta.value["payload"]["model_provider"] = Value::String(TARGET.into());
    let mut output = File::create(target)?;
    if super::super::record_file::compressed(source) {
        let mut encoder = zstd::stream::write::Encoder::new(&mut output, 3)?;
        write(&mut encoder, &meta, &mut reader)?;
        encoder.finish()?;
    } else { write(&mut output, &meta, &mut reader)?; }
    output.set_permissions(fs::metadata(source)?.permissions())?;
    output.sync_all()?;
    Ok(())
}

fn write(output: &mut impl Write, meta: &Header, body: &mut impl Read) -> Result<()> {
    if meta.bom { output.write_all(&[0xef, 0xbb, 0xbf])?; }
    serde_json::to_writer(&mut *output, &meta.value).map_err(|error| error.to_string())?;
    output.write_all(meta.ending)?;
    std::io::copy(body, output)?;
    Ok(())
}

pub(super) fn temporary(target: &Path, id: &str) -> std::path::PathBuf {
    let mut temporary = target.as_os_str().to_os_string();
    temporary.push(format!(".{id}.asb-transfer"));
    temporary.into()
}

pub(super) fn clear_temporary(target: &Path, id: &str) -> Result<()> {
    let path = temporary(target, id);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Ok(metadata) if metadata.is_file() && !super::super::sources::is_link(&metadata) => fs::remove_file(path)?,
        Ok(_) => return Err(Error::Changed),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub(super) fn replace(material: &Path, target: &Path, expected: &str, published: &str, id: &str) -> Result<()> {
    if super::inventory::regular(target)? != target || digest(target)? != expected { return Err(Error::Changed); }
    let temporary = temporary(target, id);
    let mut output = File::options().write(true).create_new(true).open(&temporary)?;
    let result = (|| {
        std::io::copy(&mut File::open(material)?, &mut output)?;
        output.set_permissions(fs::metadata(material)?.permissions())?;
        output.sync_all()?;
        drop(output);
        if digest(&temporary)? != published {
            return Err(Error::Failure(format!("Publication integrity check failed: {}", target.display())));
        }
        if super::inventory::regular(target)? != target || digest(target)? != expected { return Err(Error::Changed); }
        crate::storage::publish::replace(&temporary, target)?;
        Ok(())
    })();
    if result.is_err() && temporary.is_file() { let _ = fs::remove_file(temporary); }
    result
}
