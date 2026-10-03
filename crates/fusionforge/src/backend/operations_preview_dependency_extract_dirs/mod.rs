use super::super::*;

pub(in super::super) fn preview_name_tokens(paths: &BTreeSet<String>, matched_paths: &[String]) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    for path in paths.iter().chain(matched_paths.iter()) {
        let normalized = normalized_asset_path(path);
        let Some(stem) = Path::new(&normalized)
            .file_stem()
            .and_then(|value| value.to_str())
        else {
            continue;
        };
        let stripped = stem.trim_start_matches("npc_").trim_start_matches("mob_");
        for value in [
            stem.to_string(),
            stripped.to_string(),
            stripped.replace('_', ""),
            format!("npc_{stripped}"),
            format!("mob_{stripped}"),
        ] {
            if value.len() >= 4 {
                tokens.insert(value);
            }
        }
    }
    tokens
}

pub(in super::super) fn add_fuzzy_named_meshes(
    env: &fusionforge::UnityEnvironment,
    tokens: &BTreeSet<String>,
    selected: &mut BTreeSet<(usize, i64)>,
) {
    if tokens.is_empty() {
        return;
    }
    let mut scored = Vec::<(i32, usize, i64)>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "Mesh" {
                continue;
            }
            let Ok(body) = asset.read_object(asset_index, info) else {
                continue;
            };
            let name = fusionforge::object_name(&body).to_ascii_lowercase();
            let compact = name.replace(['_', '-', ' '], "");
            let mut score = 0;
            for token in tokens {
                let token_compact = token.replace(['_', '-', ' '], "");
                if name == *token || compact == token_compact {
                    score += 100;
                } else if name.contains(token) || compact.contains(&token_compact) {
                    score += 35;
                }
            }
            if score > 0 {
                scored.push((score, asset_index, info.path_id));
            }
        }
    }
    scored.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, asset_index, path_id) in scored {
        selected.insert((asset_index, path_id));
    }
}

pub(in super::super) fn native_coordinate_contract_json() -> JsonValue {
    json!({
        "schema": "ffone.native-coordinate-contract.v1",
        "publishedSpace": "gltf-right-handed-y-up",
        "position": "[-unity.x,unity.y,unity.z]",
        "normal": "[-unity.x,unity.y,unity.z]",
        "translation": "[-unity.x,unity.y,unity.z]",
        "rotation": "[unity.x,-unity.y,-unity.z,unity.w]",
        "scale": "unchanged",
        "uv": "[unity.u,1-unity.v]",
        "inverseBindMatrix": "H*unity*H where H=diag(-1,1,1,1)",
        "sourceTriangleWinding": "unity-clockwise-preserved",
        "publishedTriangleWinding": "swap-index-1-and-2-for-gltf-counter-clockwise",
        "windingConversionOwner": "ffone-asset-pipeline",
        "unitScale": "1-unity-unit-equals-1-bevy-unit",
        "originPolicy": "source-root-trs-unchanged-no-auto-centering",
        "autoCentered": false,
        "autoScaled": false,
    })
}

pub(in super::super) fn preview_value_without_fields(value: Option<&JsonValue>, fields: &[&str]) -> JsonValue {
    let Some(value) = value else {
        return JsonValue::Null;
    };
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        for field in fields {
            object.remove(*field);
        }
    }
    value
}

pub(in super::super) fn preview_skin_semantic_value(value: Option<&JsonValue>) -> JsonValue {
    preview_value_without_fields(
        value,
        &[
            "rendererAssetIndex",
            "rendererPathId",
            "rendererTransformPathId",
            "meshFilterAssetIndex",
            "meshFilterPathId",
            "attachmentTransformPathId",
        ],
    )
}

pub(in super::super) fn preview_joint_semantic_key(joint: &JsonValue) -> String {
    serde_json::to_string(&preview_value_without_fields(
        Some(joint),
        &["path", "sourceAssetIndex", "transformPathId"],
    ))
    .unwrap_or_default()
}

pub(in super::super) fn deduplicate_preview_semantic_copies(
    meshes: &mut Vec<JsonValue>,
    materials: &BTreeMap<String, JsonValue>,
    animations: &mut Vec<JsonValue>,
    skeleton: &mut JsonValue,
    warnings: &mut Vec<String>,
) -> PreviewSemanticDedup {
    let original_meshes = meshes.len();
    let mut seen_meshes = BTreeSet::new();
    meshes.retain(|mesh| seen_meshes.insert(preview_mesh_semantic_key(mesh, materials)));

    let original_animations = animations.len();
    let mut seen_animations = BTreeSet::new();
    animations
        .retain(|animation| seen_animations.insert(preview_animation_semantic_key(animation)));

    let mut removed_joints = 0;
    if let Some(joints) = skeleton.get_mut("joints").and_then(JsonValue::as_array_mut) {
        let mut seen_joints = BTreeMap::<String, String>::new();
        let mut conflicting_paths = BTreeSet::new();
        joints.retain(|joint| {
            let Some(path) = joint
                .get("path")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|path| !path.is_empty())
            else {
                return true;
            };
            let normalized_path = path.to_ascii_lowercase();
            let semantic = preview_joint_semantic_key(joint);
            match seen_joints.get(&normalized_path) {
                None => {
                    seen_joints.insert(normalized_path, semantic);
                    true
                }
                Some(existing) if existing == &semantic => {
                    removed_joints += 1;
                    false
                }
                Some(_) => {
                    conflicting_paths.insert(path.to_string());
                    true
                }
            }
        });
        for path in conflicting_paths {
            warnings.push(format!(
                "Skeleton contains conflicting transforms for duplicate path '{path}'; both were preserved."
            ));
        }
    }

    PreviewSemanticDedup {
        meshes: original_meshes.saturating_sub(meshes.len()),
        animations: original_animations.saturating_sub(animations.len()),
        joints: removed_joints,
    }
}

pub(in super::super) fn cache_exact_batch_environment(
    bundle: &Path,
    project: &Path,
    extract_dir: &Path,
    environment: Rc<fusionforge::UnityEnvironment>,
) {
    LAST_EXACT_BATCH_ENVIRONMENT.with(|cached| {
        *cached.borrow_mut() = Some(ExactBatchEnvironment {
            bundle_key: exact_batch_path_key(bundle),
            project_key: exact_batch_path_key(project),
            extract_dir: extract_dir.to_path_buf(),
            environment,
        });
    });
}

pub(in super::super) fn cached_exact_batch_environment(
    bundle: &Path,
    project: &Path,
) -> Option<(PathBuf, Rc<fusionforge::UnityEnvironment>)> {
    let bundle_key = exact_batch_path_key(bundle);
    let project_key = exact_batch_path_key(project);
    LAST_EXACT_BATCH_ENVIRONMENT.with(|cached| {
        cached.borrow().as_ref().and_then(|cached| {
            (cached.bundle_key == bundle_key && cached.project_key == project_key)
                .then(|| (cached.extract_dir.clone(), Rc::clone(&cached.environment)))
        })
    })
}

/// Bundles the caller has named explicitly through
/// `FFONE_EXTRA_DEPENDENCY_BUNDLES`, as a `;`-separated list of paths.
///
/// Automatic dependency discovery keys on `fileId` being an index into the
/// asset's `m_Externals` table. Some builds do not encode it that way - their
/// `PPtr`s carry values such as `571736065` or `-749535231`, which name no
/// external at all - so no dependency can be derived even though the referenced
/// object exists in a sibling bundle. `resolve_pointer` still finds such an
/// object by path id once its bundle is loaded, so naming the bundle is enough.
/// Every listed bundle must exist; a typo has to fail loudly rather than leave
/// the export short of a dependency it appears to have been given.
pub(in super::super) fn explicit_dependency_extract_dirs(project: &Path) -> EditorResult<Vec<PathBuf>> {
    let Some(raw) = std::env::var_os("FFONE_EXTRA_DEPENDENCY_BUNDLES") else {
        return Ok(Vec::new());
    };
    let raw = raw.to_string_lossy().into_owned();
    let mut dirs = Vec::new();
    for entry in raw
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let path = PathBuf::from(entry);
        if !path.is_file() {
            return Err(format!(
                "FFONE_EXTRA_DEPENDENCY_BUNDLES names {entry:?}, which is not a file"
            ));
        }
        dirs.push(extract_bundle_cached(project, &path)?);
    }
    Ok(dirs)
}

pub(in super::super) fn preview_dependency_extract_dirs(
    project: &Path,
    bundle: &Path,
    initial_dirs: &[PathBuf],
    wanted: &BTreeSet<String>,
) -> EditorResult<Vec<PathBuf>> {
    // The index is not loaded eagerly. A self-contained bundle resolves its
    // whole closure from its own extract directory and must keep exporting
    // without one; only a bundle that actually reaches outside itself needs the
    // index, and that case is reported below instead of silently resolving
    // nothing.
    let index_path = project.join("cache").join("bundle-index.json");
    let index = cached_client_file_index(&index_path);

    let current_bundle = bundle
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let mut dirs = Vec::new();
    let mut seen_dirs = initial_dirs
        .iter()
        .map(|dir| {
            dir.to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase()
        })
        .collect::<BTreeSet<_>>();

    let mut seen_refs = BTreeSet::new();
    for _ in 0..8 {
        let mut env_dirs = initial_dirs.to_vec();
        env_dirs.extend(dirs.iter().cloned());
        let env = cached_unity_environment_from_extract_dirs(&env_dirs);
        let required_refs = preview_container_dependency_refs(&env, wanted);
        if required_refs.is_empty() {
            break;
        }

        // A bundle that reaches outside itself cannot be resolved without the
        // index. Returning an empty dependency set here used to leave the real
        // failure to surface much later as an unrelated "was not found in
        // loaded assets", with nothing pointing back at the missing index.
        let Some(index) = index.as_ref() else {
            let sample = required_refs
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "{} needs {} cross-bundle reference(s) ({sample}) but {} does not exist; \
                 build it with `fusionforge index-client <source-build-root> <work-dir>`",
                bundle.display(),
                required_refs.len(),
                index_path.display()
            ));
        };

        let mut added = false;
        for dep_ref in required_refs {
            if !seen_refs.insert(dep_ref.clone()) {
                continue;
            }
            let Some(dep_bundle) = index.bundles.iter().find(|candidate| {
                candidate.path.replace('\\', "/").to_ascii_lowercase() != current_bundle
                    && client_bundle_contains_asset_ref(candidate, &dep_ref)
            }) else {
                continue;
            };
            let Some(dir) = dependency_bundle_extract_dir(project, dep_bundle) else {
                continue;
            };
            let key = dir
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            if seen_dirs.insert(key) {
                dirs.push(dir);
                added = true;
            }
        }

        if !added {
            break;
        }
    }
    Ok(dirs)
}

pub(in super::super) fn add_pointer_dependency_root(
    env: &fusionforge::UnityEnvironment,
    pointer: &fusionforge::Pointer,
    refs: &mut BTreeSet<String>,
    selected: &mut BTreeSet<(usize, i64)>,
    queue: &mut VecDeque<(usize, i64)>,
) {
    if pointer.is_null() {
        return;
    }
    if let Some(asset) = env.assets.get(pointer.source_asset) {
        preview_add_external_pointer_ref(asset, pointer, refs);
    }
    if !pointer_referenced_asset_is_loaded(env, pointer) {
        return;
    }
    if let Ok(key) = env.resolve_pointer(pointer) {
        let key = (key.asset, key.path_id);
        if selected.insert(key) {
            queue.push_back(key);
        }
    }
}

pub(in super::super) fn preview_renderer_matches_tokens(
    env: &fusionforge::UnityEnvironment,
    renderer: &fusionforge::UnityValue,
    tokens: &BTreeSet<String>,
) -> bool {
    if tokens.is_empty() {
        return true;
    }
    if renderer
        .get("m_GameObject")
        .and_then(fusionforge::UnityValue::as_pointer)
        .and_then(|pointer| env.resolve_value(pointer).ok())
        .is_some_and(|value| preview_named_value_matches_tokens(&value, tokens))
    {
        return true;
    }
    renderer
        .get("m_Mesh")
        .and_then(fusionforge::UnityValue::as_pointer)
        .and_then(|pointer| env.resolve_value(pointer).ok())
        .is_some_and(|value| preview_named_value_matches_tokens(&value, tokens))
}

pub(in super::super) fn preview_named_value_matches_tokens(
    value: &fusionforge::UnityValue,
    tokens: &BTreeSet<String>,
) -> bool {
    if tokens.is_empty() {
        return true;
    }
    let name = fusionforge::object_name(value).to_ascii_lowercase();
    let compact = name.replace(['_', '-', ' '], "");
    tokens.iter().any(|token| {
        let token_compact = token.replace(['_', '-', ' '], "");
        name.contains(token) || compact.contains(&token_compact)
    })
}

pub(in super::super) fn preview_add_external_pointer_ref(
    asset: &fusionforge::Asset,
    pointer: &fusionforge::Pointer,
    refs: &mut BTreeSet<String>,
) {
    if pointer.is_null() || pointer.file_id == 0 {
        return;
    }
    let Some(index) = asset.pointer_file_index(pointer) else {
        return;
    };
    let Some(asset_ref) = asset.asset_refs.get(index) else {
        return;
    };
    for value in [&asset_ref.file_path, &asset_ref.asset_path] {
        let normalized = normalized_asset_ref_name(value);
        if !normalized.is_empty() && normalized != "library/unity default resources" {
            refs.insert(normalized);
        }
    }
}

pub(in super::super) fn run_table_data_task<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    std::thread::Builder::new()
        .name("ff-tabledata-worker".to_string())
        .stack_size(256 * 1024 * 1024)
        .spawn(task)
        .map_err(|err| err.to_string())?
        .join()
        .map_err(|_| "TableData worker panicked".to_string())?
}

pub(in super::super) fn table_row_fields(row: &fusionforge::UnityValue) -> Vec<String> {
    let Some(object) = row.as_object() else {
        return Vec::new();
    };
    object
        .keys()
        .filter(|key| !is_unity_metadata_field(key))
        .take(48)
        .cloned()
        .collect()
}

pub(in super::super) fn table_section_fields(rows: &[fusionforge::UnityValue]) -> Vec<String> {
    let mut fields = BTreeSet::new();
    for row in rows.iter().take(16) {
        for field in table_row_fields(row) {
            fields.insert(field);
        }
        if fields.len() >= 48 {
            break;
        }
    }
    fields.into_iter().take(48).collect()
}

pub(in super::super) fn count_table_scalars(value: &fusionforge::UnityValue, strings: &mut usize, numbers: &mut usize) {
    match value {
        fusionforge::UnityValue::String(value) => {
            if !value.is_empty() {
                *strings += 1;
            }
        }
        fusionforge::UnityValue::Int(_)
        | fusionforge::UnityValue::UInt(_)
        | fusionforge::UnityValue::Float(_) => *numbers += 1,
        fusionforge::UnityValue::Array(values) => {
            for value in values {
                count_table_scalars(value, strings, numbers);
            }
        }
        fusionforge::UnityValue::Object(values) => {
            for value in values.values() {
                count_table_scalars(value, strings, numbers);
            }
        }
        fusionforge::UnityValue::Pair(left, right) => {
            count_table_scalars(left, strings, numbers);
            count_table_scalars(right, strings, numbers);
        }
        _ => {}
    }
}

pub(in super::super) fn npc_row_has_text(row: &fusionforge::UnityValue) -> bool {
    [
        "m_strName",
        "m_strComment",
        "m_strComment1",
        "m_strComment2",
    ]
    .iter()
    .any(|key| unity_string_field(Some(row), key).is_some())
}

pub(in super::super) fn npc_table_from_body(body: &fusionforge::UnityValue) -> Option<&fusionforge::UnityValue> {
    body.get("m_pNpcTable")
}

pub(in super::super) fn npc_table_from_body_mut(
    body: &mut fusionforge::UnityValue,
) -> Option<&mut fusionforge::UnityValue> {
    body.get_mut("m_pNpcTable")
}

pub(in super::super) fn npc_table_array<'a>(
    npc_table: &'a fusionforge::UnityValue,
    section: &str,
) -> &'a [fusionforge::UnityValue] {
    npc_table
        .get(section)
        .and_then(fusionforge::UnityValue::as_array)
        .unwrap_or(&[])
}

pub(in super::super) fn npc_related_row_has_data(value: &fusionforge::UnityValue) -> bool {
    match value {
        fusionforge::UnityValue::String(value) => {
            let trimmed = value.trim();
            !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("null")
        }
        fusionforge::UnityValue::Array(values) => values.iter().any(npc_related_row_has_data),
        fusionforge::UnityValue::Object(values) => values.values().any(npc_related_row_has_data),
        fusionforge::UnityValue::Pair(left, right) => {
            npc_related_row_has_data(left) || npc_related_row_has_data(right)
        }
        _ => false,
    }
}

pub(in super::super) fn add_npc_related_row(
    rows: &mut BTreeMap<String, JsonValue>,
    npc_table: &fusionforge::UnityValue,
    section: &str,
    index: usize,
    label: &str,
) {
    let Some(row) = npc_table_array(npc_table, section)
        .get(index)
        .filter(|row| npc_related_row_has_data(row) || *row != &empty_npc_row_like(row))
    else {
        return;
    };
    rows.insert(
        format!("{section}[{label}={index}]"),
        unity_value_to_json(row),
    );
}

pub(in super::super) fn npc_related_rows(npc_table: &fusionforge::UnityValue, id: usize) -> BTreeMap<String, JsonValue> {
    let mut rows = BTreeMap::new();
    if let Some(npc_data) = npc_table_array(npc_table, "m_pNpcData").get(id) {
        rows.insert("m_pNpcData".to_string(), unity_value_to_json(npc_data));
        if let Some(index) = unity_usize_field(npc_data, "m_iMesh") {
            add_npc_related_row(&mut rows, npc_table, "m_pNpcMeshData", index, "m_iMesh");
        }
        if let Some(index) = unity_usize_field(npc_data, "m_iIcon1") {
            add_npc_related_row(&mut rows, npc_table, "m_pNpcIconData", index, "m_iIcon1");
        }
        if let Some(index) = unity_usize_field(npc_data, "m_iServiceNumber") {
            add_npc_related_row(
                &mut rows,
                npc_table,
                "m_pNpcServiceData",
                index,
                "m_iServiceNumber",
            );
        }
        for key in [
            "m_iNpcName",
            "m_iComment",
            "m_iActiveSkill1String",
            "m_iActiveSkill2String",
            "m_iSupportSkillString",
            "m_iCorruptionString",
            "m_iMegaString",
        ] {
            if let Some(index) = unity_usize_field(npc_data, key) {
                add_npc_related_row(&mut rows, npc_table, "m_pNpcStringData", index, key);
            }
        }
        if let Some(index) = unity_usize_field(npc_data, "m_iBarkerNumber") {
            add_npc_related_row(
                &mut rows,
                npc_table,
                "m_pNpcBarkerData",
                index,
                "m_iBarkerNumber",
            );
        }
    }
    rows
}

pub(in super::super) fn inspect_npc_source_build(
    source_dir: String,
    target_project_dir: Option<String>,
) -> EditorResult<NpcSourceBuildInspection> {
    run_table_data_task(move || {
        let source_path = PathBuf::from(&source_dir);
        if !source_path.exists() || !source_path.is_dir() {
            return Err(EditorError::MissingPath(source_dir).to_string());
        }
        let import_project = npc_source_import_project_dir(&source_path, target_project_dir);
        let index = index_client_project(
            source_path.to_string_lossy().to_string(),
            import_project.to_string_lossy().to_string(),
        )?;
        let table_bundle = index
            .bundles
            .iter()
            .find(|bundle| bundle.name.eq_ignore_ascii_case("TableData.resourceFile"))
            .or_else(|| {
                index
                    .bundles
                    .iter()
                    .find(|bundle| bundle.name.to_ascii_lowercase().contains("tabledata"))
            })
            .ok_or_else(|| {
                format!(
                    "No TableData.resourceFile was found in {}",
                    index.source_dir
                )
            })?;
        let table_data_bundle = table_bundle.path.clone();
        let catalog =
            inspect_npc_catalog_impl(table_data_bundle.clone(), Some(index.project_dir.clone()))?;
        Ok(NpcSourceBuildInspection {
            index,
            table_data_bundle,
            catalog,
        })
    })
}

pub(in super::super) fn blueprint_profile_protected_field(field: &str) -> bool {
    matches!(
        field,
        "m_iNpcNumber" | "m_iNpcName" | "m_iComment" | "m_iMesh" | "m_iIcon1" | "m_iBarkerNumber"
    )
}

pub(in super::super) fn empty_npc_row_like(value: &fusionforge::UnityValue) -> fusionforge::UnityValue {
    match value {
        fusionforge::UnityValue::Bool(_) => fusionforge::UnityValue::Bool(false),
        fusionforge::UnityValue::Int(_) => fusionforge::UnityValue::Int(0),
        fusionforge::UnityValue::UInt(_) => fusionforge::UnityValue::UInt(0),
        fusionforge::UnityValue::Float(_) => fusionforge::UnityValue::Float(0.0),
        fusionforge::UnityValue::String(_) => fusionforge::UnityValue::String(String::new()),
        fusionforge::UnityValue::Bytes(_) => fusionforge::UnityValue::Bytes(Vec::new()),
        fusionforge::UnityValue::Array(_) => fusionforge::UnityValue::Array(Vec::new()),
        fusionforge::UnityValue::Object(values) => fusionforge::UnityValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), empty_npc_row_like(value)))
                .collect(),
        ),
        fusionforge::UnityValue::Pair(left, right) => fusionforge::UnityValue::Pair(
            Box::new(empty_npc_row_like(left)),
            Box::new(empty_npc_row_like(right)),
        ),
        fusionforge::UnityValue::Pointer(_) => {
            fusionforge::UnityValue::Pointer(fusionforge::Pointer {
                source_asset: 0,
                file_id: 0,
                path_id: 0,
            })
        }
    }
}

pub(in super::super) fn npc_table_occupied_ids(npc_table: &fusionforge::UnityValue) -> BTreeSet<usize> {
    let mut ids = BTreeSet::new();
    for section in ["m_pNpcStringData", "m_pNpcBarkerData"] {
        for (id, row) in npc_table_array(npc_table, section).iter().enumerate() {
            if npc_row_has_text(row) {
                ids.insert(id);
            }
        }
    }
    ids
}

pub(in super::super) fn requested_or_allocated_npc_id(
    project: &Path,
    target_table: &fusionforge::UnityValue,
    requested: i64,
) -> Result<usize, String> {
    if requested <= 0 {
        return Ok(allocate_import_npc_id(project, target_table));
    }
    let npc_id = usize::try_from(requested)
        .map_err(|_| format!("NPC ID {requested} is outside the supported range"))?;
    let mut used = npc_table_occupied_ids(target_table);
    used.extend(staged_npc_import_ids(project));
    if used.contains(&npc_id) {
        return Err(format!(
            "NPC ID {npc_id} is already used or reserved by a staged NPC import"
        ));
    }
    Ok(npc_id)
}

pub(in super::super) fn requested_or_existing_staged_npc_id(
    project: &Path,
    target_table: &fusionforge::UnityValue,
    requested: i64,
    name: &str,
) -> Result<usize, String> {
    if requested <= 0 {
        return requested_or_allocated_npc_id(project, target_table, requested);
    }
    let npc_id = usize::try_from(requested)
        .map_err(|_| format!("NPC ID {requested} is outside the supported range"))?;
    let expected_file = format!("{}__{}.npc-import.json", npc_id, safe_segment(name));
    let expected_manifest = project
        .join("npcs")
        .join(format!("{}__{}", npc_id, safe_segment(name)))
        .join(&expected_file);
    if expected_manifest.is_file() {
        return Ok(npc_id);
    }
    requested_or_allocated_npc_id(project, target_table, requested)
}

pub(in super::super) fn blueprint_icon_number(blueprint: &NpcBlueprint, target_id: usize) -> i64 {
    if blueprint.npc_id >= 0 {
        blueprint.npc_id
    } else {
        target_id as i64
    }
}

pub(in super::super) fn npc_target_rows_mut<'a>(
    target_table: &'a mut fusionforge::UnityValue,
    section: &str,
) -> Option<&'a mut Vec<fusionforge::UnityValue>> {
    target_table
        .as_object_mut()?
        .get_mut(section)?
        .as_array_mut()
}

pub(in super::super) fn blank_npc_row_from_section(
    target_table: &fusionforge::UnityValue,
    section: &str,
) -> fusionforge::UnityValue {
    npc_table_array(target_table, section)
        .first()
        .map(empty_npc_row_like)
        .unwrap_or_else(|| fusionforge::UnityValue::Object(BTreeMap::new()))
}
