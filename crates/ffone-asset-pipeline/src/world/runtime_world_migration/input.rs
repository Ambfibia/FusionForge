use super::*;

pub(super) fn read_json_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })
}

pub(super) fn collect_regular_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    collect_regular_files_from(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

pub(super) fn collect_regular_files_from(
    root: &Path,
    directory: &Path,
    output: &mut Vec<String>,
) -> Result<()> {
    let entries = fs::read_dir(directory)
        .map_err(|error| io_at(directory, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(directory, error))?;
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(entry.path(), error))?;
        if file_type.is_symlink() {
            return invalid(format!(
                "completed world archive contains symlink {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_regular_files_from(root, &entry.path(), output)?;
        } else if file_type.is_file() {
            output.push(normalized_relative(root, &entry.path())?);
        } else {
            return invalid(format!(
                "completed world archive contains non-regular entry {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

pub(super) fn collect_world_json_paths(asset_root: &Path) -> Result<Vec<String>> {
    let world_root = asset_root.join("world");
    let mut paths = Vec::new();
    collect_json_paths_from(asset_root, &world_root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

pub(super) fn collect_json_paths_from(
    asset_root: &Path,
    directory: &Path,
    output: &mut Vec<String>,
) -> Result<()> {
    let entries = fs::read_dir(directory)
        .map_err(|error| io_at(directory, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(directory, error))?;
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(entry.path(), error))?;
        if file_type.is_symlink() {
            return invalid(format!(
                "runtime-world migration refuses symlink {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_json_paths_from(asset_root, &entry.path(), output)?;
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            output.push(normalized_relative(asset_root, &entry.path())?);
        }
    }
    Ok(())
}

pub(super) fn resolve_document_reference(document: &str, reference: &str) -> Result<String> {
    let document = native_path(document)?;
    let parent = document
        .parent()
        .ok_or_else(|| invalid_error("world document has no parent"))?;
    normalized_relative(Path::new(""), &parent.join(native_path(reference)?))
}
