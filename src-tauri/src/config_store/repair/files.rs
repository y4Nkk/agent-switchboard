use std::{collections::BTreeMap, fs, path::{Path, PathBuf}};

pub(super) fn json_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{} 不可读：{error}", dir.display())),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).display().to_string()
}

pub(super) fn file_tree(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!("配置目录不是普通目录：{}", root.display()));
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.to_string()),
        _ => {}
    }
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_files(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) -> Result<(), String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && dir == root => return Ok(()),
        Err(error) => return Err(format!("{} 不可读：{error}", dir.display())),
    };
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        let kind = fs::symlink_metadata(&path).map_err(|error| error.to_string())?.file_type();
        if kind.is_symlink() { return Err(format!("配置目录含符号链接，无法安全修复：{}", path.display())); }
        if kind.is_dir() { collect_files(root, &path, files)?; }
        else if kind.is_file() {
            files.insert(path.strip_prefix(root).map_err(|error| error.to_string())?.to_path_buf(),
                fs::read(&path).map_err(|error| error.to_string())?);
        } else { return Err(format!("配置目录含不支持的条目：{}", path.display())); }
    }
    Ok(())
}
pub(super) fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|error| error.to_string())?;
    if !source.exists() { return Ok(()); }
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let destination = target.join(path.file_name().ok_or("无效配置文件名")?);
        let kind = fs::symlink_metadata(&path).map_err(|error| error.to_string())?.file_type();
        if kind.is_dir() { copy_tree(&path, &destination)?; }
        else if kind.is_file() { fs::copy(&path, &destination).map_err(|error| error.to_string())?; }
        else { return Err(format!("配置目录含不支持的条目：{}", path.display())); }
    }
    Ok(())
}
