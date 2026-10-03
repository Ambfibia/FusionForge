use super::super::*;

pub(crate) fn container_asset_bytes_with_source(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    visited: &mut BTreeSet<(usize, i64)>,
) -> Option<ContainerAssetBytesResolution> {
    match container_asset_bytes_with_source_budget(
        env,
        asset_index,
        path_id,
        visited,
        CONTAINER_ASSET_POINTER_LIMIT,
    ) {
        ContainerAssetBytesSearch::Found(resolution) => Some(resolution),
        ContainerAssetBytesSearch::GraphExhausted | ContainerAssetBytesSearch::BudgetExceeded => {
            None
        }
    }
}

/// Exhaustively follows the exact container pointer graph, but accepts bytes
/// only from a real TextAsset that parses as a KFM payload with an exact NIF
/// reference. In particular, Shader.m_Script is source code, never KFM data.
pub(crate) fn kfm_text_asset_bytes_with_source_exact(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    visited: &mut BTreeSet<(usize, i64)>,
) -> ContainerAssetBytesSearch {
    let mut pending = vec![(asset_index, path_id)];
    while let Some((current_asset_index, current_path_id)) = pending.pop() {
        if !visited.insert((current_asset_index, current_path_id)) {
            continue;
        }
        let Some(asset) = env.assets.get(current_asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&current_path_id) else {
            continue;
        };
        let object_type = asset.object_type_name(info);
        let Ok(body) = asset.read_object(current_asset_index, info) else {
            continue;
        };
        if let Some(bytes) = exact_kfm_text_asset_payload(&object_type, &body) {
            return ContainerAssetBytesSearch::Found(ContainerAssetBytesResolution {
                bytes,
                asset_index: current_asset_index,
                path_id: current_path_id,
                object_type,
            });
        }
        let mut pointers = Vec::new();
        collect_value_pointers(&body, &mut pointers);
        let resolved = pointers
            .into_iter()
            .filter_map(|pointer| env.resolve_pointer(&pointer).ok())
            .map(|key| (key.asset, key.path_id))
            .collect::<Vec<_>>();
        for key in resolved.into_iter().rev() {
            if !visited.contains(&key) {
                pending.push(key);
            }
        }
    }
    ContainerAssetBytesSearch::GraphExhausted
}

pub(in super::super) fn container_asset_bytes(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    visited: &mut BTreeSet<(usize, i64)>,
) -> Option<Vec<u8>> {
    container_asset_bytes_with_source(env, asset_index, path_id, visited)
        .map(|resolution| resolution.bytes)
}

pub(in super::super) fn ascii_asset_strings(bytes: &[u8], min_len: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = Vec::new();
    for &byte in bytes {
        let printable = byte.is_ascii_graphic() || byte == b' ' || byte == b'/' || byte == b'\\';
        if printable {
            current.push(byte);
            continue;
        }
        if current.len() >= min_len {
            if let Ok(value) = String::from_utf8(std::mem::take(&mut current)) {
                result.push(value);
            }
        } else {
            current.clear();
        }
    }
    if current.len() >= min_len {
        if let Ok(value) = String::from_utf8(current) {
            result.push(value);
        }
    }
    result
}

pub(in super::super) fn path_stem_token(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    Path::new(&normalized)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
}

pub(in super::super) fn preview_path_matches_tokens(path: &str, tokens: &BTreeSet<String>) -> bool {
    if tokens.is_empty() {
        return false;
    }
    let lower = normalized_asset_path(path);
    let compact = lower.replace(['_', '-', ' ', '/'], "");
    tokens.iter().any(|token| {
        let token_compact = token.replace(['_', '-', ' '], "");
        lower.contains(token) || compact.contains(&token_compact)
    })
}

pub(in super::super) fn matched_wanted_container_path<'a>(
    wanted: &'a BTreeMap<String, String>,
    lower_path: &str,
) -> Option<(&'a str, &'a str)> {
    if let Some((expected, original)) = wanted.get_key_value(lower_path) {
        return Some((expected.as_str(), original.as_str()));
    }
    wanted
        .iter()
        .find(|(expected, _)| lower_path.ends_with(&format!("/{expected}")))
        .map(|(expected, original)| (expected.as_str(), original.as_str()))
}

pub(in super::super) fn staged_preview_sibling_asset_dirs(project: &Path, bundle: &Path) -> Vec<PathBuf> {
    if !bundle.exists() {
        return Vec::new();
    }
    let project_root = fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    let Some(assets_root) = bundle.ancestors().find(|candidate| {
        candidate
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("assets"))
            && candidate.join("_manifest.json").is_file()
    }) else {
        return Vec::new();
    };
    let assets_root_resolved =
        fs::canonicalize(assets_root).unwrap_or_else(|_| assets_root.to_path_buf());
    if assets_root_resolved.strip_prefix(&project_root).is_err() {
        return Vec::new();
    }

    let Ok(text) = fs::read_to_string(assets_root.join("_manifest.json")) else {
        return Vec::new();
    };
    let Ok(document) = serde_json::from_str::<JsonValue>(&text) else {
        return Vec::new();
    };
    let mut source_values = json_string_array(&document, "sourceBundles").unwrap_or_default();
    if let Some(asset_dirs) = document.get("assetDirs").and_then(JsonValue::as_array) {
        for asset_dir in asset_dirs {
            if let Some(path) = json_string(asset_dir, "path") {
                source_values.push(path);
            }
            if let Some(files) = json_string_array(asset_dir, "files") {
                source_values.extend(files);
            }
        }
    }

    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in source_values {
        let resolved = resolve_project_path(project, &value);
        let directory = if resolved.is_dir() {
            resolved
        } else if resolved.is_file() {
            let Some(parent) = resolved.parent() else {
                continue;
            };
            parent.to_path_buf()
        } else {
            continue;
        };
        let directory_resolved = fs::canonicalize(&directory).unwrap_or_else(|_| directory.clone());
        if directory_resolved.strip_prefix(&project_root).is_err() {
            continue;
        }
        let key = directory
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        if seen.insert(key) {
            result.push(directory);
        }
    }
    result
}

pub(in super::super) fn exact_batch_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

pub(in super::super) fn cached_client_file_index(path: &Path) -> Option<Rc<ClientFileIndex>> {
    let metadata = fs::metadata(path).ok()?;
    let bytes = metadata.len();
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let normalized = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    LAST_CLIENT_FILE_INDEX.with(|cached| {
        let mut cached = cached.borrow_mut();
        if let Some((cached_path, cached_bytes, cached_modified, index)) = cached.as_ref() {
            if cached_path == &normalized && *cached_bytes == bytes && *cached_modified == modified
            {
                return Some(Rc::clone(index));
            }
        }
        let text = fs::read_to_string(path).ok()?;
        let index = Rc::new(serde_json::from_str::<ClientFileIndex>(&text).ok()?);
        *cached = Some((normalized, bytes, modified, Rc::clone(&index)));
        Some(index)
    })
}

pub(in super::super) fn pointer_referenced_asset_is_loaded(
    env: &fusionforge::UnityEnvironment,
    pointer: &fusionforge::Pointer,
) -> bool {
    if pointer.file_id == 0 {
        return true;
    }
    // Format-7 assets can encode a recoverable local reference with a
    // negative file id. Prefer the environment's full resolver before the
    // stricter asset-reference table check below.
    if env.resolve_pointer(pointer).is_ok() {
        return true;
    }
    let Some(source) = env.assets.get(pointer.source_asset) else {
        return false;
    };
    let Some(index) = source.pointer_file_index(pointer) else {
        return false;
    };
    let Some(asset_ref) = source.asset_refs.get(index) else {
        return false;
    };
    let names = [&asset_ref.file_path, &asset_ref.asset_path]
        .into_iter()
        .map(|value| normalized_asset_ref_name(value))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if names.is_empty() {
        return true;
    }
    env.assets.iter().any(|asset| {
        let asset_name = normalized_asset_ref_name(&asset.name);
        names.contains(&asset_name)
    })
}

pub(in super::super) fn normalized_asset_ref_name(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

pub(in super::super) fn client_bundle_contains_asset_ref(bundle: &ClientBundleFile, asset_ref: &str) -> bool {
    let target = asset_ref.to_ascii_lowercase();
    normalized_asset_ref_name(&bundle.name) == target
        || Path::new(&bundle.path)
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| normalized_asset_ref_name(name) == target)
        || bundle
            .extracted_files
            .iter()
            .any(|file| normalized_asset_ref_name(&file.name) == target)
        || bundle
            .assets
            .iter()
            .any(|asset| normalized_asset_ref_name(&asset.name) == target)
}

pub(in super::super) fn table_value_at_path<'a>(
    value: &'a fusionforge::UnityValue,
    section_path: &str,
) -> Option<&'a fusionforge::UnityValue> {
    let mut current = value;
    for part in section_path.split('.').filter(|part| !part.is_empty()) {
        current = current.get(part)?;
    }
    Some(current)
}

pub(in super::super) fn table_value_at_path_mut<'a>(
    value: &'a mut fusionforge::UnityValue,
    section_path: &str,
) -> Option<&'a mut fusionforge::UnityValue> {
    let mut current = value;
    for part in section_path.split('.').filter(|part| !part.is_empty()) {
        current = current.get_mut(part)?;
    }
    Some(current)
}

pub(in super::super) fn extracted_asset_path(extracted_dir: &Path, asset_name: &str) -> Result<PathBuf, String> {
    let file_name = Path::new(asset_name)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid asset name: {asset_name}"))?;
    let direct = extracted_dir.join(file_name);
    if direct.is_file() {
        return Ok(direct);
    }
    for entry in
        fs::read_dir(extracted_dir).map_err(|err| format!("{}: {err}", extracted_dir.display()))?
    {
        let path = entry.map_err(|err| err.to_string())?.path();
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(file_name))
        {
            return Ok(path);
        }
    }
    let mut candidates = fs::read_dir(extracted_dir)
        .map_err(|err| format!("{}: {err}", extracted_dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| !value.eq_ignore_ascii_case("cache-meta.json"))
        })
        .collect::<Vec<_>>();
    if file_name.eq_ignore_ascii_case("CustomAssetBundle-TableData") && !candidates.is_empty() {
        return Ok(candidates.remove(0));
    }
    if candidates.len() == 1 {
        return Ok(candidates.remove(0));
    }
    Err(format!(
        "{} was not found in {}",
        file_name,
        extracted_dir.display()
    ))
}

pub(in super::super) fn npc_catalog_links(npc_data: &fusionforge::UnityValue) -> NpcCatalogLinks {
    NpcCatalogLinks {
        npc_name: unity_usize_field(npc_data, "m_iNpcName"),
        comment: unity_usize_field(npc_data, "m_iComment"),
        mesh: unity_usize_field(npc_data, "m_iMesh"),
        icon1: unity_usize_field(npc_data, "m_iIcon1"),
        service_number: unity_usize_field(npc_data, "m_iServiceNumber"),
        barker_number: unity_usize_field(npc_data, "m_iBarkerNumber"),
        active_skill1_string: unity_usize_field(npc_data, "m_iActiveSkill1String"),
        active_skill2_string: unity_usize_field(npc_data, "m_iActiveSkill2String"),
        support_skill_string: unity_usize_field(npc_data, "m_iSupportSkillString"),
        corruption_string: unity_usize_field(npc_data, "m_iCorruptionString"),
        mega_string: unity_usize_field(npc_data, "m_iMegaString"),
    }
}

pub(in super::super) fn npc_catalog_icon_record(
    index: usize,
    icon_row: &fusionforge::UnityValue,
) -> Option<NpcIconRecord> {
    let icon_type = icon_row
        .get("m_iIconType")
        .and_then(fusionforge::UnityValue::as_i64)?;
    let icon_number = icon_row
        .get("m_iIconNumber")
        .and_then(fusionforge::UnityValue::as_i64)?;
    let asset_paths = npc_icon_asset_paths(icon_type, icon_number);
    Some(NpcIconRecord {
        index,
        icon_type,
        icon_number,
        asset_paths,
    })
}

pub(in super::super) fn npc_catalog_record(
    id: usize,
    string_row: &fusionforge::UnityValue,
    links: NpcCatalogLinks,
    icon: Option<NpcIconRecord>,
    barker_id: Option<usize>,
    barker_row: Option<&fusionforge::UnityValue>,
    related_rows: BTreeMap<String, JsonValue>,
) -> NpcCatalogRecord {
    let barker = barker_row
        .filter(|row| npc_row_has_text(row))
        .map(|row| NpcBarkerRecord {
            id: barker_id.unwrap_or(id),
            name: unity_string_field(Some(row), "m_strName"),
            comment: unity_string_field(Some(row), "m_strComment"),
            comment1: unity_string_field(Some(row), "m_strComment1"),
            comment2: unity_string_field(Some(row), "m_strComment2"),
        });
    let name = unity_string_field(Some(string_row), "m_strName")
        .or_else(|| barker.as_ref().and_then(|value| value.name.clone()))
        .unwrap_or_else(|| format!("NPC {id}"));
    NpcCatalogRecord {
        id,
        name,
        internal_name: unity_string_field(Some(string_row), "m_strComment2"),
        links,
        icon,
        comment: unity_string_field(Some(string_row), "m_strComment"),
        comment1: unity_string_field(Some(string_row), "m_strComment1"),
        comment2: unity_string_field(Some(string_row), "m_strComment2"),
        barker,
        string_row: unity_value_to_json(string_row),
        barker_row: barker_row.map(unity_value_to_json),
        related_rows,
    }
}

pub(in super::super) fn inspect_npc_catalog(
    bundle_path: String,
    project_dir: Option<String>,
) -> EditorResult<NpcCatalogInspection> {
    run_table_data_task(move || inspect_npc_catalog_impl(bundle_path, project_dir))
}

pub(in super::super) fn inspect_npc_catalog_impl(
    bundle_path: String,
    project_dir: Option<String>,
) -> EditorResult<NpcCatalogInspection> {
    use fusionforge::{object_name, Asset};

    let bundle = PathBuf::from(&bundle_path);
    if !bundle.is_file() {
        return Err(EditorError::MissingPath(bundle_path).to_string());
    }
    let project = default_table_data_project_dir(project_dir);
    let extract_dir = extract_bundle_cached(&project, &bundle)?;
    for extracted in extracted_files_in_dir(&extract_dir) {
        let asset_path = PathBuf::from(&extracted.path);
        let asset = match Asset::from_path(&asset_path) {
            Ok(asset) => asset,
            Err(_) => continue,
        };
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "MonoBehaviour" {
                continue;
            }
            let body = if let Some(body) =
                table_data_patch_body_for_object(&project, &bundle_path, &asset.name, *path_id)?
            {
                body
            } else {
                asset.read_object(0, info)?
            };
            let Some(npc_table) = npc_table_from_body(&body) else {
                continue;
            };
            let npc_data_rows = npc_table_array(npc_table, "m_pNpcData");
            let string_rows = npc_table_array(npc_table, "m_pNpcStringData");
            let barker_rows = npc_table_array(npc_table, "m_pNpcBarkerData");
            let mut records = Vec::new();
            for (id, npc_data) in npc_data_rows.iter().enumerate() {
                if id == 0 && !npc_related_row_has_data(npc_data) {
                    continue;
                }
                let string_index = unity_usize_field(npc_data, "m_iNpcName").unwrap_or(id);
                let barker_index = unity_usize_field(npc_data, "m_iBarkerNumber");
                let empty_string = fusionforge::UnityValue::Object(BTreeMap::new());
                let string_row = string_rows.get(string_index).unwrap_or(&empty_string);
                let barker_row = barker_index.and_then(|index| barker_rows.get(index));
                if !npc_related_row_has_data(npc_data)
                    && !npc_row_has_text(string_row)
                    && !barker_row.is_some_and(npc_row_has_text)
                {
                    continue;
                }
                let links = npc_catalog_links(npc_data);
                let icon = links.icon1.and_then(|index| {
                    npc_table_array(npc_table, "m_pNpcIconData")
                        .get(index)
                        .and_then(|row| npc_catalog_icon_record(index, row))
                });
                records.push(npc_catalog_record(
                    id,
                    string_row,
                    links,
                    icon,
                    barker_index,
                    barker_row,
                    npc_related_rows(npc_table, id),
                ));
            }
            records.sort_by(|left, right| left.id.cmp(&right.id));
            let mut sections = Vec::new();
            collect_table_sections(npc_table, "m_pNpcTable", &mut sections);
            return Ok(NpcCatalogInspection {
                bundle_path,
                cache_dir: extract_dir.to_string_lossy().to_string(),
                asset: asset.name,
                path_id: *path_id,
                object_name: object_name(&body),
                records,
                sections,
                staged_ids: staged_npc_import_ids(&project).into_iter().collect(),
            });
        }
    }
    Err("No m_pNpcTable MonoBehaviour was found in TableData bundle.".to_string())
}

pub(in super::super) fn table_data_patch_path_for_object(
    project: &Path,
    bundle_path: &str,
    asset: &str,
    path_id: i64,
) -> Result<Option<PathBuf>, String> {
    let manifest_path = project.join("tabledata").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }
    let manifest = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(&manifest_path).map_err(|err| EditorError::Io(err).to_string())?,
    )
    .unwrap_or_else(|_| json!({ "entries": [] }));
    let container = container_name_from_bundle(Path::new(bundle_path));
    let Some(entries) = manifest.get("entries").and_then(JsonValue::as_array) else {
        return Ok(None);
    };
    for entry in entries {
        if entry.get("container").and_then(JsonValue::as_str) == Some(container.as_str())
            && entry.get("asset").and_then(JsonValue::as_str) == Some(asset)
            && entry.get("pathId").and_then(JsonValue::as_i64) == Some(path_id)
        {
            let Some(file) = entry.get("file").and_then(JsonValue::as_str) else {
                continue;
            };
            return Ok(Some(
                project.join("tabledata").join(file.replace('/', "\\")),
            ));
        }
    }
    Ok(None)
}

pub(in super::super) fn manifest_entry_file(entry: &JsonValue) -> Option<String> {
    entry
        .get("file")
        .and_then(JsonValue::as_str)
        .map(|value| value.replace('\\', "/"))
}

pub(in super::super) fn table_data_manifest_patch_paths(project: &Path) -> Result<Vec<PathBuf>, String> {
    let table_data_root = project.join("tabledata");
    let manifest_path = table_data_root.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }
    let manifest = serde_json::from_str::<JsonValue>(
        &fs::read_to_string(&manifest_path).map_err(|err| EditorError::Io(err).to_string())?,
    )
    .unwrap_or_else(|_| json!({ "entries": [] }));
    let Some(entries) = manifest.get("entries").and_then(JsonValue::as_array) else {
        return Ok(Vec::new());
    };
    entries
        .iter()
        .filter_map(manifest_entry_file)
        .map(|file| path_from_normalized_relative(&table_data_root, &file))
        .collect()
}

pub(in super::super) fn blueprint_asset_stem(value: &str) -> Option<String> {
    let normalized = value.trim().replace('\\', "/");
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("null") {
        return None;
    }
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
    let stem = file_name
        .rsplit_once('.')
        .filter(|(_, extension)| extension.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .map(|(stem, _)| stem)
        .unwrap_or(file_name)
        .trim();
    (!stem.is_empty()).then(|| stem.to_ascii_lowercase())
}

pub(in super::super) fn copy_npc_row_to_index(
    source_table: &fusionforge::UnityValue,
    target_table: &mut fusionforge::UnityValue,
    section: &str,
    source_index: usize,
    target_index: usize,
) -> Result<bool, String> {
    let Some(source_row) = npc_copy_source_row(source_table, section, source_index).cloned() else {
        return Ok(false);
    };
    let Some(target_rows) = npc_target_rows_mut(target_table, section) else {
        return Ok(false);
    };
    let empty = empty_npc_row_like(&source_row);
    while target_rows.len() <= target_index {
        target_rows.push(empty.clone());
    }
    target_rows[target_index] = source_row;
    Ok(true)
}

pub(in super::super) fn write_npc_row_at_index(
    target_table: &mut fusionforge::UnityValue,
    section: &str,
    index: usize,
    row: fusionforge::UnityValue,
) -> Result<(), String> {
    let rows = npc_target_rows_mut(target_table, section)
        .ok_or_else(|| format!("m_pNpcTable.{section} is not an array"))?;
    let empty = empty_npc_row_like(&row);
    while rows.len() <= index {
        rows.push(empty.clone());
    }
    rows[index] = row;
    Ok(())
}

pub(in super::super) fn blueprint_asset_value(value: Option<&String>, fallback: &str) -> String {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| fallback.to_string())
}

#[derive(Debug, Clone)]
pub(in super::super) struct NpcIconAssetPath {
    pub(in super::super) prefix: String,
    pub(in super::super) number: i64,
}

pub(in super::super) fn npc_icon_runtime_asset_path(icon_type: i64, icon_number: i64) -> Option<String> {
    if icon_number < 0 {
        return None;
    }
    npc_icon_prefix_for_type(icon_type).map(|prefix| format!("icons/{prefix}_{icon_number:02}.png"))
}

pub(in super::super) fn parse_npc_icon_asset_path(path: &str) -> Option<NpcIconAssetPath> {
    let file_name = path
        .replace('\\', "/")
        .rsplit('/')
        .next()?
        .trim()
        .to_ascii_lowercase();
    let stem = file_name
        .strip_suffix(".png")
        .or_else(|| file_name.strip_suffix(".dds"))
        .unwrap_or(&file_name);
    let (prefix, number) = stem.rsplit_once('_')?;
    npc_icon_type_for_prefix(prefix)?;
    let number = number.parse::<i64>().ok()?;
    Some(NpcIconAssetPath {
        prefix: prefix.to_string(),
        number,
    })
}

pub(in super::super) fn preferred_npc_icon_asset_path(hints: &NpcAssetHints) -> Option<String> {
    hints
        .icon_paths
        .iter()
        .find(|path| parse_npc_icon_asset_path(path).is_some())
        .cloned()
}

pub(in super::super) fn retargeted_npc_icon_asset_path(
    blueprint: &NpcBlueprint,
    hints: &NpcAssetHints,
) -> Option<String> {
    let source_icon = preferred_npc_icon_asset_path(hints).or_else(|| {
        blueprint
            .icon_asset
            .as_deref()
            .map(normalized_asset_path)
            .filter(|path| parse_npc_icon_asset_path(path).is_some())
    })?;
    let icon = parse_npc_icon_asset_path(&source_icon)?;
    (blueprint.npc_id >= 0).then(|| format!("icons/{}_{:02}.png", icon.prefix, blueprint.npc_id))
}

pub(in super::super) fn staged_manifest_icon_paths(manifest: &JsonValue, blueprint: &NpcBlueprint) -> BTreeSet<String> {
    [
        blueprint.icon_asset.clone(),
        json_nested_string(manifest, &["resources", "iconTexture", "asset"]),
        blueprint
            .generated_icon
            .as_ref()
            .map(|icon| icon.template_asset_path.clone()),
    ]
    .into_iter()
    .flatten()
    .map(|path| normalized_asset_path(&path))
    .filter(|path| !path.is_empty() && parse_npc_icon_asset_path(path).is_some())
    .collect()
}

pub(in super::super) fn staged_manifest_icon_bundle_path(
    manifest: &JsonValue,
    blueprint: &NpcBlueprint,
) -> Option<String> {
    json_nested_string(manifest, &["resources", "iconTexture", "bundle"])
        .or_else(|| blueprint.icon_bundle.clone())
        .map(|path| path.trim().replace('\\', "/"))
        .filter(|path| !path.is_empty())
}
