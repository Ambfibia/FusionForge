//! Final-output-only installation: validate the entire write set before mutation.
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

pub fn install(root: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<(), String> {
    install_with_permission(root, files, false)
}

/// Explicit replacement uses memory preimages and writes only final paths.
/// Rollback handles ordinary I/O errors; power-loss atomicity is not promised.
pub fn install_with_permission(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
    replace_existing: bool,
) -> Result<(), String> {
    process(root, files, replace_existing, false)
}

/// Run the same conflict/dependency preflight without creating directories or files.
pub fn check(root: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<(), String> {
    process(root, files, false, true)
}

pub fn check_with_permission(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
    replace_existing: bool,
) -> Result<(), String> {
    process(root, files, replace_existing, true)
}

fn process(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
    replace_existing: bool,
    check_only: bool,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    let mut pending = Vec::new();
    for (relative, bytes) in files {
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(format!("invalid output path {relative:?}"));
        }
        for component in relative.components() {
            let Component::Normal(name) = component else {
                unreachable!()
            };
            let name = name.to_string_lossy();
            let base = name
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || (base.len() == 4
                    && (base.starts_with("COM") || base.starts_with("LPT"))
                    && matches!(base.as_bytes()[3], b'1'..=b'9'));
            if name.contains([':', '\\', '<', '>', '"', '|', '?', '*'])
                || name.ends_with([' ', '.'])
                || name.chars().any(|c| c.is_control())
                || reserved
            {
                return Err(format!("unsafe output name {name}"));
            }
        }
        if !seen.insert(relative.to_string_lossy().to_lowercase()) {
            return Err(format!("duplicate output {relative:?}"));
        }
        let path = root.join(relative);
        for ancestor in path.ancestors() {
            if let Ok(metadata) = fs::symlink_metadata(ancestor) {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return Err(format!("output traverses a reparse point: {ancestor:?}"));
                    }
                }
                if metadata.file_type().is_symlink() {
                    return Err(format!("output traverses a link: {ancestor:?}"));
                }
            }
        }
        let before = match fs::read(&path) {
            Ok(existing) if &existing == bytes => continue,
            Ok(existing) if replace_existing => Some(existing),
            Ok(_) => return Err(format!("output conflict; preserving user file {path:?}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("{path:?}: {e}")),
        };
        pending.push((path, bytes, before));
    }
    // Resolve GLB image/buffer dependencies against the complete proposed write set.
    for (relative, bytes) in files {
        if relative.extension().and_then(|x| x.to_str()) != Some("glb") {
            continue;
        }
        if bytes.len() < 20 || &bytes[..4] != b"glTF" || &bytes[16..20] != b"JSON" {
            return Err("invalid final GLB header".into());
        }
        let size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value =
            serde_json::from_slice(bytes.get(20..20 + size).ok_or("invalid GLB JSON size")?)
                .map_err(|e| e.to_string())?;
        for collection in ["images", "buffers"] {
            for object in json[collection].as_array().into_iter().flatten() {
                let Some(uri) = object["uri"].as_str() else {
                    continue;
                };
                if uri.starts_with("data:") {
                    continue;
                }
                let uri = Path::new(uri);
                if uri.is_absolute() || uri.to_string_lossy().contains(':') {
                    return Err("unsupported external GLB URI".into());
                }
                let mut dependency = relative.parent().unwrap_or(Path::new("")).to_path_buf();
                for component in uri.components() {
                    match component {
                        Component::Normal(part) => dependency.push(part),
                        Component::CurDir => {}
                        Component::ParentDir if dependency.pop() => {}
                        _ => return Err("GLB dependency escapes native output root".into()),
                    }
                }
                if !files.iter().any(|(path, _)| path == &dependency)
                    && !root.join(&dependency).is_file()
                {
                    return Err(format!("missing final GLB dependency {dependency:?}"));
                }
            }
        }
    }
    if check_only {
        return Ok(());
    }
    let mut created: Vec<(PathBuf, Option<Vec<u8>>)> = Vec::new();
    let result = (|| {
        for (path, bytes, before) in pending {
            if fs::read(&path).ok() != before {
                return Err(format!("output changed after preflight: {path:?}"));
            }
            let parent = path.parent().ok_or("output has no parent")?;
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            let mut options = OpenOptions::new();
            options.write(true);
            if before.is_some() {
                options.truncate(true);
            } else {
                options.create_new(true);
            }
            let mut file = options.open(&path).map_err(|e| format!("{path:?}: {e}"))?;
            created.push((path, before));
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if result.is_err() {
        for (path, before) in created.iter().rev() {
            match before {
                Some(bytes) => fs::write(path, bytes),
                None => fs::remove_file(path),
            }
            .map_err(|e| format!("rollback {path:?}: {e}"))?;
        }
    }
    result
}

#[cfg(test)]
mod tests;
