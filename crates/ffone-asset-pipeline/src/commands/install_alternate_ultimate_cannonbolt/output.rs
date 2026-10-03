use super::*;

pub(super) fn copy_support_tree(
    source: &Path,
    destination: &Path,
    relative_root: &str,
    artifacts: &mut Vec<ResourceSetArtifact>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(source)
        .map_err(|error| format!("cannot read {}: {error}", source.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot enumerate {}: {error}", source.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "support texture path is not Unicode".to_owned())?;
        if path.is_dir() {
            copy_support_tree(
                &path,
                &destination.join(&name),
                &format!("{relative_root}/{name}"),
                artifacts,
            )?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("png") {
            let bytes = fs::read(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            write_new(&destination.join(&name), &bytes)?;
            artifacts.push(artifact(format!("{relative_root}/{name}"), &bytes));
        }
    }
    Ok(())
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!("refusing to overwrite {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

pub(super) fn write_replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "output has no parent".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let temporary = parent.join(format!(
        ".{}.next-{}",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("output"),
        std::process::id()
    ));
    if temporary.exists() {
        fs::remove_file(&temporary)
            .map_err(|error| format!("cannot clear {}: {error}", temporary.display()))?;
    }
    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| format!("cannot publish {}: {error}", path.display()))
}
