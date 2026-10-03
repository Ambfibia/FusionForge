use super::*;

pub(crate) fn extract_bundle_to_session(path: &Path, session_dir: &Path) -> JsonValue {
    let mut errors = Vec::<String>::new();
    if !path.exists() {
        return json!({
            "path": path,
            "exists": false,
            "buildtool": JsonValue::Null,
            "assets": [],
            "errors": ["File does not exist"],
        });
    }

    let read_result = inspect_bundle(path);
    let extract_result = extract_bundle(path, session_dir);
    let read_ok = read_result.is_ok();
    let extract_ok = extract_result.is_ok();
    if let Err(err) = read_result.as_ref() {
        errors.push(format!("fusionforge read-bundle failed: {err}"));
    }
    if let Err(err) = extract_result.as_ref() {
        errors.push(format!("fusionforge extract-bundle failed: {err}"));
    }

    let files = read_result
        .as_ref()
        .map(|summary| summary.files.clone())
        .or_else(|_| extract_result.as_ref().map(|summary| summary.files.clone()))
        .unwrap_or_default();
    let reported_files = files
        .iter()
        .map(|file| json!({ "name": file.name, "displaySize": display_size(file.size) }))
        .collect::<Vec<_>>();
    let extracted_files = files
        .iter()
        .filter_map(|file| {
            let file_path = session_dir.join(&file.name);
            file_path.is_file().then(|| {
                let size = file_path.metadata().map(|meta| meta.len()).unwrap_or(0);
                json!({ "name": file.name, "path": file_path, "size": size })
            })
        })
        .collect::<Vec<_>>();
    if extracted_files.is_empty() {
        errors.push("No files were extracted by fusionforge.".to_string());
    }

    json!({
        "path": path,
        "exists": true,
        "buildtool": {
            "readExitCode": if read_ok { 0 } else { 1 },
            "extractExitCode": if extract_ok { 0 } else { 1 },
            "readStdout": "",
            "readStderr": read_result.err().unwrap_or_default(),
            "extractStdout": "",
            "extractStderr": extract_result.err().unwrap_or_default(),
            "extractedDir": session_dir,
            "reportedFiles": reported_files,
            "extractedFiles": extracted_files,
        },
        "assets": [],
        "errors": errors,
    })
}

pub(super) fn summarize_game_object(path_id: i64, body: &UnityValue) -> JsonValue {
    json!({
        "reason": "gameObjectName",
        "pathId": path_id,
        "name": object_name(body),
        "active": body.get("m_IsActive").and_then(UnityValue::as_i64),
        "layer": body.get("m_Layer").and_then(UnityValue::as_i64),
        "tag": body.get("m_Tag").and_then(UnityValue::as_i64),
        "componentCount": value_array(body.get("m_Component")).len(),
    })
}

pub(super) fn parse_map_bundle_name(path: &Path) -> Option<(i32, i32, usize, usize)> {
    let stem = path.file_stem()?.to_str()?;
    let parts = stem.split('_').collect::<Vec<_>>();
    if parts.len() != 3 || !parts[0].eq_ignore_ascii_case("Map") {
        return None;
    }
    Some((
        parse_signed_decimal_component(parts[1])?,
        parse_signed_decimal_component(parts[2])?,
        parts[1].trim_start_matches('-').len(),
        parts[2].trim_start_matches('-').len(),
    ))
}

pub(super) fn find_named_bundle(
    name: &str,
    build_root: Option<&Path>,
    fallback_root: &Path,
) -> Option<PathBuf> {
    let direct = fallback_root.join(name);
    if direct.exists() {
        return Some(direct);
    }
    if let Some(build_root) = build_root {
        find_file_recursive(build_root, name)
    } else {
        None
    }
}

pub(super) fn find_map_bundle(tile_id: &str, map_path: &Path, build_root: Option<&Path>) -> Option<PathBuf> {
    find_named_bundle(
        &format!("{tile_id}.unity3d"),
        build_root,
        map_path.parent().unwrap_or_else(|| Path::new(".")),
    )
}

pub(super) fn infer_resource_bundle(map_path: Option<&Path>, build_root: Option<&Path>) -> Option<PathBuf> {
    let map_path = map_path?;
    let (x, y, x_width, y_width) = parse_map_bundle_name(map_path)?;
    let name = format!(
        "DongResources_{}_{}.resourceFile",
        format_signed_decimal_component(x, x_width),
        format_signed_decimal_component(y, y_width)
    );
    find_named_bundle(
        &name,
        build_root,
        map_path.parent().unwrap_or_else(|| Path::new(".")),
    )
}

pub(super) fn direct_bundle_files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut files = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("resourceFile")
                            || ext.eq_ignore_ascii_case("unity3d")
                    })
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

pub(super) fn find_bundle_for_archive(
    archive: &str,
    build_root: Option<&Path>,
    fallback_root: &Path,
    repo_root: &Path,
    archive_indexes: &mut HashMap<PathBuf, HashMap<String, PathBuf>>,
) -> Option<PathBuf> {
    let target = normalize_bundle_name(archive);
    let mut roots = Vec::new();
    if let Some(build_root) = build_root {
        roots.push(build_root.to_path_buf());
    }
    roots.push(fallback_root.to_path_buf());
    for root in roots {
        for pattern_ext in ["resourceFile", "unity3d"] {
            let direct = direct_bundle_files(&root).into_iter().find(|path| {
                path.extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(pattern_ext))
                    && path
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .is_some_and(|stem| normalize_bundle_name(stem) == target)
            });
            if direct.is_some() {
                return direct;
            }
        }
        let index = archive_indexes
            .entry(root.clone())
            .or_insert_with(|| build_archive_index(&root));
        if let Some(path) = index.get(&target) {
            return Some(path.clone());
        }
    }
    find_manifest_bundle_for_archive(&target, build_root, fallback_root, repo_root)
}
