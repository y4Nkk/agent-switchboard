use super::FileRevision;
use sha2::{Digest, Sha256};
use std::{fs::File, io::{Read, Seek, SeekFrom, Take}, path::Path};

pub(crate) struct DigestReader<'a> { reader: Take<File>, hash: &'a mut Sha256, pub bytes: u64 }

impl<'a> DigestReader<'a> {
    pub(crate) fn open(path: &Path, start: u64, length: u64, hash: &'a mut Sha256) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|error| error.to_string())?;
        file.seek(SeekFrom::Start(start)).map_err(|error| error.to_string())?;
        Ok(Self { reader: file.take(length), hash, bytes: 0 })
    }
}

impl Read for DigestReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(buffer)?;
        self.hash.update(&buffer[..count]);
        self.bytes += count as u64;
        Ok(count)
    }
}

pub(crate) fn verify(path: &Path, revision: &FileRevision, length: u64, expected: &str,
    checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(), String> {
    checkpoint()?;
    let current = FileRevision::read(path)?;
    if current.created != revision.created || current.bytes < revision.bytes {
        return Err(format!("来源被替换或截断，已跳过本次读取：{}", path.display()));
    }
    let mut hash = Sha256::new();
    extend_hash(path, 0, length, &mut hash, checkpoint)?;
    if format!("{:x}", hash.finalize()) != expected {
        return Err(format!("记录在读取期间被改写，已跳过本次读取：{}", path.display()));
    }
    Ok(())
}

pub(crate) fn extend_hash(path: &Path, start: u64, mut length: u64, hash: &mut Sha256,
    checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(start)).map_err(|error| error.to_string())?;
    let mut buffer = [0; 64 * 1024];
    while length > 0 {
        checkpoint()?;
        let size = length.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..size]).map_err(|error| error.to_string())?;
        hash.update(&buffer[..size]);
        length -= size as u64;
    }
    Ok(())
}
