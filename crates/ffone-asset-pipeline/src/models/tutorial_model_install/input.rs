use super::*;

pub(super) fn collect_uris(value: &Value, output: &mut Vec<String>) -> Result<()> {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_uris(value, output)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key == "uri" {
                    match value {
                        Value::String(uri) => output.push(uri.clone()),
                        Value::Null => {}
                        _ => return invalid("GLB URI field is neither a string nor null"),
                    }
                }
                collect_uris(value, output)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn find_exact_evidence(roots: &[EvidenceRoot], selection: &str) -> Result<EvidenceProof> {
    let (json_path, png_path) = gpu_evidence_relative_paths(selection).map_err(|error| {
        invalid_error(format!(
            "cannot derive GPU sidecars for {selection:?}: {error}"
        ))
    })?;
    let json_relative = slash_path(&json_path);
    let png_relative = slash_path(&png_path);
    let mut matches = Vec::new();
    for root in roots {
        let json = exact_index_path(&root.files, &json_relative)?;
        let png = exact_index_path(&root.files, &png_relative)?;
        match (json, png) {
            (None, None) => {}
            (Some(_), None) | (None, Some(_)) => {
                return invalid(format!(
                    "partial GPU evidence pair for {selection:?} in {}",
                    root.canonical.display()
                ));
            }
            (Some(json), Some(png)) => {
                matches.push(EvidenceProof {
                    root: root.canonical.clone(),
                    json_relative: json_relative.clone(),
                    json_bytes: read_regular_relative(&root.canonical, json)?,
                    png_relative: png_relative.clone(),
                    png_bytes: read_regular_relative(&root.canonical, png)?,
                });
            }
        }
    }
    if matches.len() != 1 {
        return invalid(format!(
            "selected model {selection:?} must have exactly one GPU JSON+PNG pair across all evidence roots; found {}",
            matches.len()
        ));
    }
    Ok(matches.pop().expect("one evidence match"))
}

pub(super) fn read_regular_relative(root: &Path, relative: &str) -> Result<Vec<u8>> {
    validate_relative(relative)?;
    let mut path = root.to_path_buf();
    let parts = relative.split('/').collect::<Vec<_>>();
    for (index, component) in parts.iter().enumerate() {
        path.push(component);
        let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
        if metadata.file_type().is_symlink() {
            return invalid(format!("installer input path contains symlink {path:?}"));
        }
        if index + 1 == parts.len() {
            if !metadata.is_file() {
                return invalid(format!("installer input is not a regular file {path:?}"));
            }
        } else if !metadata.is_dir() {
            return invalid(format!(
                "installer input parent is not a directory {path:?}"
            ));
        }
    }
    fs::read(&path).map_err(|error| io_at(&path, error))
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| invalid_error("GLB header is truncated"))
}
