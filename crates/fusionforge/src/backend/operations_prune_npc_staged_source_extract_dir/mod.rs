use super::super::*;

pub(in super::super) fn stage_npc_clone_from_existing(
    project_dir: String,
    target_bundle_path: String,
    target_asset: String,
    target_path_id: i64,
    source_npc_id: usize,
    mut blueprint: NpcBlueprint,
) -> EditorResult<NpcImportStageResult> {
    run_table_data_task(move || {
        use fusionforge::object_name;

        let project = PathBuf::from(project_dir);
        if !project.exists() || !project.is_dir() {
            return Err(
                EditorError::MissingPath(project.to_string_lossy().to_string()).to_string(),
            );
        }

        let target_bundle = PathBuf::from(&target_bundle_path);
        let target_extract_dir = extract_bundle_cached(&project, &target_bundle)?;
        let target_asset_path = extracted_asset_path(&target_extract_dir, &target_asset)?;
        let target_asset_file = fusionforge::Asset::from_path(&target_asset_path)?;
        let target_info = target_asset_file
            .objects
            .get(&target_path_id)
            .ok_or_else(|| {
                format!(
                    "{}#{} was not found",
                    target_asset_file.name, target_path_id
                )
            })?;
        let mut target_body = if let Some(body) = table_data_patch_body_for_object(
            &project,
            &target_bundle_path,
            &target_asset,
            target_path_id,
        )? {
            body
        } else {
            target_asset_file.read_object(0, target_info)?
        };

        let source_table_snapshot = npc_table_from_body(&target_body)
            .ok_or_else(|| "target TableData object has no m_pNpcTable".to_string())?
            .clone();
        let target_table = npc_table_from_body_mut(&mut target_body)
            .ok_or_else(|| "target TableData object has no m_pNpcTable".to_string())?;
        let npc_id = requested_or_allocated_npc_id(&project, target_table, blueprint.npc_id)?;
        if npc_id == source_npc_id {
            return Err("Clone NPC ID must be different from the source NPC ID.".to_string());
        }
        blueprint.npc_id = npc_id as i64;
        blueprint.template_npc_id = Some(source_npc_id as i64);
        let mut copied_sections =
            copy_npc_table_rows(&source_table_snapshot, target_table, source_npc_id, npc_id)?;
        if apply_blueprint_npc_profile(target_table, npc_id, &blueprint.profile)? {
            copied_sections.push("m_pNpcData".to_string());
        }
        if apply_blueprint_npc_text(target_table, npc_id, &blueprint)? {
            copied_sections.push("m_pNpcStringData".to_string());
            copied_sections.push("m_pNpcBarkerData".to_string());
        }
        copied_sections.sort();
        copied_sections.dedup();

        let patch_name = format!("npc_clone_{}__{}", npc_id, safe_segment(&blueprint.name));
        let document = json!({
            "format": "fftools.tabledata-object.v1",
            "sourceBundle": target_bundle.to_string_lossy(),
            "container": container_name_from_bundle(&target_bundle),
            "asset": target_asset_file.name,
            "pathId": target_path_id,
            "name": object_name(&target_body),
            "objectType": target_asset_file.object_type_name(target_info),
            "value": unity_value_to_json(&target_body),
        });
        let table_data_patch_path = write_table_data_patch_document(
            &project,
            &target_bundle_path,
            &target_asset,
            target_path_id,
            &patch_name,
            &document,
        )?;
        Ok(NpcImportStageResult {
            npc_id,
            table_data_patch_path: table_data_patch_path.to_string_lossy().to_string(),
            import_manifest_path: String::new(),
            bundle_name: String::new(),
            copied_sections,
            warnings: vec![
                "TableData-only NPC clone; no standalone NPC bundle was staged.".to_string(),
            ],
        })
    })
}

pub(in super::super) fn npc_blueprint_source_paths(project: &Path, blueprint: &NpcBlueprint) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    [
        blueprint.model_bundle.as_deref(),
        blueprint.texture_bundle.as_deref(),
        blueprint.icon_bundle.as_deref(),
        blueprint.animation_set.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .filter_map(|value| {
        let path = resolve_project_path(project, value);
        let key = path.to_string_lossy().to_ascii_lowercase();
        if seen.insert(key) {
            Some(path)
        } else {
            None
        }
    })
    .collect()
}

pub(in super::super) fn npc_stage_root(project: &Path, blueprint: &NpcBlueprint) -> PathBuf {
    project.join("npcs").join(format!(
        "{}__{}",
        blueprint.npc_id.max(0),
        safe_segment(&blueprint.name)
    ))
}

pub(in super::super) fn npc_build_work_root(project: &Path, blueprint: &NpcBlueprint) -> PathBuf {
    project.join("cache").join("npc-build-work").join(format!(
        "{}__{}",
        blueprint.npc_id.max(0),
        safe_segment(&blueprint.name)
    ))
}

pub(in super::super) fn npc_build_assets_root(project: &Path, blueprint: &NpcBlueprint) -> PathBuf {
    npc_stage_root(project, blueprint).join("assets")
}

pub(in super::super) fn npc_snapshot_assets_root(project: &Path, blueprint: &NpcBlueprint) -> PathBuf {
    npc_stage_root(project, blueprint).join("bundle-snapshot")
}

pub(in super::super) fn cleanup_npc_build_work_dir(project: &Path, blueprint: &NpcBlueprint) {
    let build_root = npc_build_work_root(project, blueprint);
    if build_root.is_dir() {
        if let Err(err) = fs::remove_dir_all(&build_root) {
            eprintln!(
                "could not remove NPC build work dir {}: {err}",
                build_root.display()
            );
        }
    }
}

pub(in super::super) fn npc_staged_source_dir(asset_stage_root: &Path, source_path: &Path) -> PathBuf {
    let label = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("bundle");
    asset_stage_root.join(safe_segment(label))
}

pub(in super::super) fn npc_staged_source_label(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(
        name.split_once("__")
            .map(|(label, _)| label)
            .unwrap_or(name)
            .to_string(),
    )
}

pub(in super::super) fn prune_npc_staged_source_extract_dir(
    extract_dir: &Path,
    staged_dir: &Path,
    blueprint: &NpcBlueprint,
    hints: &NpcAssetHints,
) -> Result<(), String> {
    let mut assets = Vec::<fusionforge::Asset>::new();
    for file in extracted_files_in_dir(extract_dir) {
        let asset_path = PathBuf::from(&file.path);
        let Some(name) = asset_path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.ends_with(".json") {
            continue;
        }
        let Ok(asset) = fusionforge::Asset::from_path(&asset_path) else {
            continue;
        };
        assets.push(asset);
    }
    if assets.is_empty() {
        return Err(format!(
            "{} had no serialized assets to stage for NPC '{}'",
            extract_dir.display(),
            blueprint.name
        ));
    }

    let env = fusionforge::UnityEnvironment::from_assets(assets);
    let mut required_hints = hints.clone();
    required_hints.model_paths.clear();
    required_hints.texture_paths.clear();
    required_hints.icon_paths.clear();
    let required_container_paths =
        npc_standalone_required_container_paths(blueprint, &required_hints, None);
    let mut selected = BTreeSet::<(usize, i64)>::new();
    let mut queue = VecDeque::<(usize, i64)>::new();
    let mut assetbundle_replacements = BTreeMap::<(usize, i64), fusionforge::UnityValue>::new();
    let mut matched_paths = BTreeSet::<String>::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info)?;
            let preload_table = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
            let mut kept_paths = BTreeSet::<String>::new();
            let mut asset_pointer_overrides = BTreeMap::<String, fusionforge::UnityValue>::new();

            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                if !npc_standalone_container_path_matches(path, &required_container_paths) {
                    continue;
                }
                let Some(pointer) = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                else {
                    continue;
                };
                let mut pointer = pointer.clone();
                let mut pointer_quality =
                    npc_container_pointer_quality(&env, asset_index, path, &pointer);
                if pointer_quality == 0 {
                    if let Some((preload_pointer, preload_quality)) =
                        npc_container_preload_root_pointer(
                            &env,
                            asset_index,
                            path,
                            &preload_table,
                            metadata,
                        )
                    {
                        pointer = preload_pointer;
                        pointer_quality = preload_quality;
                    } else if let Some((named_pointer, named_quality)) =
                        npc_container_named_object_pointer(&env, path)
                    {
                        pointer = named_pointer;
                        pointer_quality = named_quality;
                    } else {
                        continue;
                    }
                }

                kept_paths.insert(path.to_string());
                asset_pointer_overrides.insert(
                    path.to_string(),
                    fusionforge::UnityValue::Pointer(pointer.clone()),
                );
                matched_paths.insert(normalized_asset_path(path));
                add_resolved_pointer_root(&env, &pointer, &mut selected, &mut queue);
                let (start, end) = metadata_preload_range(metadata, preload_table.len());
                if pointer_quality >= 2 && npc_container_should_follow_preload(path) {
                    for preload in &preload_table[start..end] {
                        if let Some(pointer) = preload.as_pointer() {
                            add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
                        }
                    }
                }
            }

            if !kept_paths.is_empty() {
                selected.insert((asset_index, info.path_id));
                assetbundle_replacements.insert(
                    (asset_index, info.path_id),
                    filtered_assetbundle_value(
                        &body,
                        &kept_paths,
                        &BTreeMap::new(),
                        hints,
                        &asset_pointer_overrides,
                    ),
                );
            }
        }
    }

    while let Some((asset_index, path_id)) = queue.pop_front() {
        let Some(asset) = env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        if asset.object_type_name(info) == "AssetBundle" {
            continue;
        }
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        let mut pointers = Vec::new();
        collect_value_pointers(&body, &mut pointers);
        for pointer in pointers {
            add_resolved_pointer_root(&env, &pointer, &mut selected, &mut queue);
        }
    }

    if selected.is_empty() {
        return Err(format!(
            "No matching asset dependency closure found in {} for NPC '{}'",
            extract_dir.display(),
            blueprint.name
        ));
    }

    for (asset_index, asset) in env.assets.iter().enumerate() {
        let keep_ids = selected
            .iter()
            .filter_map(|key| (key.0 == asset_index).then_some(key.1))
            .collect::<BTreeSet<_>>();
        if keep_ids.is_empty() {
            continue;
        }
        let mut replacements = BTreeMap::<i64, Vec<u8>>::new();
        let local_output_path_ids = keep_ids.iter().copied().collect::<BTreeSet<_>>();
        for path_id in &keep_ids {
            let Some(info) = asset.objects.get(path_id) else {
                continue;
            };
            if let Some(value) = assetbundle_replacements.get(&(asset_index, *path_id)) {
                let mut value = value.clone();
                if asset.object_type_name(info) == "AssetBundle" {
                    let empty_external_pointers = BTreeSet::<(i32, i64)>::new();
                    filter_assetbundle_preloads_to_output_paths(
                        &mut value,
                        &local_output_path_ids,
                        &empty_external_pointers,
                    );
                }
                replacements.insert(
                    *path_id,
                    asset.serialize_object_value(asset_index, info, &value)?,
                );
            }
        }
        let data = asset.rebuild_with_object_data_filtered_and_extra(
            Some(&keep_ids),
            &replacements,
            &[],
        )?;
        let target = staged_dir.join(&asset.name);
        fs::write(&target, data).map_err(|err| format!("{}: {err}", target.display()))?;
    }

    if matched_paths.is_empty() {
        return Err(format!(
            "No required container paths matched in {} for NPC '{}'",
            extract_dir.display(),
            blueprint.name
        ));
    }
    Ok(())
}

pub(in super::super) fn stage_npc_source_extract_dir(
    project: &Path,
    blueprint: &NpcBlueprint,
    source_path: &Path,
    hints: &NpcAssetHints,
    asset_stage_root: &Path,
) -> Result<PathBuf, String> {
    let extract_dir = extract_bundle_cached(project, source_path)?;
    let staged_dir = npc_staged_source_dir(asset_stage_root, source_path);
    if staged_dir.exists() {
        fs::remove_dir_all(&staged_dir)
            .map_err(|err| format!("{}: {err}", staged_dir.display()))?;
    }
    fs::create_dir_all(&staged_dir).map_err(|err| format!("{}: {err}", staged_dir.display()))?;
    if let Err(prune_err) =
        prune_npc_staged_source_extract_dir(&extract_dir, &staged_dir, blueprint, hints)
    {
        if !npc_blueprint_uses_authoring_model_bundle(blueprint) {
            if source_path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(is_npc_shared_dependency_bundle)
            {
                return Err(format!(
                    "{prune_err}; refusing to keep full extracted shared dependency assets for imported NPC staging"
                ));
            }
            let extracted_size = extracted_files_in_dir(&extract_dir)
                .iter()
                .map(|file| file.size)
                .sum::<u64>();
            if extracted_size <= 50_000_000 {
                copy_serialized_assets_into_dir(&extract_dir, &staged_dir).map_err(|copy_err| {
                    format!(
                        "{prune_err}; small-bundle fallback copy from {} also failed: {copy_err}",
                        extract_dir.display()
                    )
                })?;
                return Ok(staged_dir);
            }
            return Err(format!(
                "{prune_err}; refusing to keep full extracted source assets for imported NPC staging"
            ));
        }
        copy_serialized_assets_into_dir(&extract_dir, &staged_dir).map_err(|copy_err| {
            format!(
                "{prune_err}; fallback copy from {} also failed: {copy_err}",
                extract_dir.display()
            )
        })?;
    }
    Ok(staged_dir)
}

pub(in super::super) fn cleanup_generated_authoring_bundles(project: &Path, blueprint: &NpcBlueprint) {
    let generated_root = project.join("npcs").join("generated");
    for path in npc_blueprint_source_paths(project, blueprint) {
        if !path.starts_with(&generated_root) || !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            if !name
                .to_ascii_lowercase()
                .ends_with("__authoring.resourcefile")
            {
                continue;
            }
        }
        if let Err(err) = fs::remove_file(&path) {
            eprintln!(
                "could not remove generated authoring bundle {}: {err}",
                path.display()
            );
        }
    }
    let _ = remove_empty_dirs(&generated_root, &generated_root);
}

pub(in super::super) fn npc_icon_paths_from_table(
    npc_table: &fusionforge::UnityValue,
    npc_id: usize,
) -> BTreeSet<String> {
    let mut hints = NpcAssetHints::default();
    collect_npc_asset_hints_from_table(npc_table, npc_id, &mut hints);
    hints.icon_paths
}

pub(in super::super) fn disable_renderer(value: &mut fusionforge::UnityValue) {
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_Enabled".to_string(),
            fusionforge::UnityValue::Bool(false),
        );
    }
}

pub(in super::super) fn enable_renderer(value: &mut fusionforge::UnityValue) {
    if let Some(object) = value.as_object_mut() {
        object.insert("m_Enabled".to_string(), fusionforge::UnityValue::Bool(true));
    }
}

pub(in super::super) fn set_gameobject_active(value: &mut fusionforge::UnityValue, active: bool) {
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "m_IsActive".to_string(),
            fusionforge::UnityValue::Bool(active),
        );
    }
}

pub(in super::super) fn local_component_pair(
    class_id: i64,
    asset_index: usize,
    path_id: i64,
) -> fusionforge::UnityValue {
    fusionforge::UnityValue::Array(vec![
        fusionforge::UnityValue::Int(class_id),
        unity_local_pointer(asset_index, path_id),
    ])
}

pub(in super::super) fn set_local_pointer_field(
    value: &mut fusionforge::UnityValue,
    field: &str,
    asset_index: usize,
    path_id: i64,
) {
    if let Some(object) = value.as_object_mut() {
        object.insert(field.to_string(), unity_local_pointer(asset_index, path_id));
    }
}

pub(in super::super) fn renderer_gameobject_key(
    env: &fusionforge::UnityEnvironment,
    value: &fusionforge::UnityValue,
) -> Option<(usize, i64)> {
    let pointer = value.get("m_GameObject")?.as_pointer()?;
    let key = env.resolve_pointer(pointer).ok()?;
    Some((key.asset, key.path_id))
}

pub(in super::super) fn transform_gameobject_key(
    env: &fusionforge::UnityEnvironment,
    value: &fusionforge::UnityValue,
) -> Option<(usize, i64)> {
    let pointer = value.get("m_GameObject")?.as_pointer()?;
    let key = env.resolve_pointer(pointer).ok()?;
    Some((key.asset, key.path_id))
}

pub(in super::super) fn template_transform_z_offset(
    env: &fusionforge::UnityEnvironment,
    transform_key: (usize, i64),
) -> f64 {
    env.assets
        .get(transform_key.0)
        .and_then(|asset| {
            asset
                .objects
                .get(&transform_key.1)
                .and_then(|info| asset.read_object(transform_key.0, info).ok())
        })
        .and_then(|body| {
            body.get("m_LocalPosition")
                .and_then(|position| position.get("z"))
                .and_then(fusionforge::UnityValue::as_f64)
        })
        .unwrap_or(0.0)
}

pub(in super::super) fn compensate_static_template_transform(
    mesh: &mut fusionforge::modding::ImportedMesh,
    z_offset: f64,
) {
    for vertex in &mut mesh.vertices {
        vertex.0 = -vertex.0;
        vertex.1 = -vertex.1;
        vertex.2 -= z_offset;
    }
    for normal in &mut mesh.normals {
        normal.0 = -normal.0;
        normal.1 = -normal.1;
    }
}

pub(in super::super) fn clean_mul_vec3(a: (f64, f64, f64), b: (f64, f64, f64)) -> (f64, f64, f64) {
    (a.0 * b.0, a.1 * b.1, a.2 * b.2)
}

pub(in super::super) fn clean_add_vec3(a: (f64, f64, f64), b: (f64, f64, f64)) -> (f64, f64, f64) {
    (a.0 + b.0, a.1 + b.1, a.2 + b.2)
}

pub(in super::super) fn clean_quat_mul(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    (
        a.3 * b.0 + a.0 * b.3 + a.1 * b.2 - a.2 * b.1,
        a.3 * b.1 - a.0 * b.2 + a.1 * b.3 + a.2 * b.0,
        a.3 * b.2 + a.0 * b.1 - a.1 * b.0 + a.2 * b.3,
        a.3 * b.3 - a.0 * b.0 - a.1 * b.1 - a.2 * b.2,
    )
}

pub(in super::super) fn clean_rotate_vec3(rotation: (f64, f64, f64, f64), value: (f64, f64, f64)) -> (f64, f64, f64) {
    let qv = (value.0, value.1, value.2, 0.0);
    let q_conj = (-rotation.0, -rotation.1, -rotation.2, rotation.3);
    let rotated = clean_quat_mul(clean_quat_mul(rotation, qv), q_conj);
    (rotated.0, rotated.1, rotated.2)
}

pub(in super::super) fn clean_trs_matrix4(
    translation: (f64, f64, f64),
    rotation: (f64, f64, f64, f64),
    scale: (f64, f64, f64),
) -> [[f64; 4]; 4] {
    let (x, y, z, w) = rotation;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    [
        [
            (1.0 - 2.0 * (yy + zz)) * scale.0,
            (2.0 * (xy - wz)) * scale.1,
            (2.0 * (xz + wy)) * scale.2,
            translation.0,
        ],
        [
            (2.0 * (xy + wz)) * scale.0,
            (1.0 - 2.0 * (xx + zz)) * scale.1,
            (2.0 * (yz - wx)) * scale.2,
            translation.1,
        ],
        [
            (2.0 * (xz - wy)) * scale.0,
            (2.0 * (yz + wx)) * scale.1,
            (1.0 - 2.0 * (xx + yy)) * scale.2,
            translation.2,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(in super::super) fn clean_multiply_matrix4(left: [[f64; 4]; 4], right: [[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut out = [[0.0_f64; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            out[row][col] = (0..4)
                .map(|index| left[row][index] * right[index][col])
                .sum();
        }
    }
    out
}

pub(in super::super) fn clean_invert_affine_matrix4(matrix: [[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let a = matrix[0][0];
    let b = matrix[0][1];
    let c = matrix[0][2];
    let d = matrix[1][0];
    let e = matrix[1][1];
    let f = matrix[1][2];
    let g = matrix[2][0];
    let h = matrix[2][1];
    let i = matrix[2][2];
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    assert!(det.abs() > f64::EPSILON, "non-invertible affine matrix");
    let inv_det = 1.0 / det;
    let inv3 = [
        [
            (e * i - f * h) * inv_det,
            (c * h - b * i) * inv_det,
            (b * f - c * e) * inv_det,
        ],
        [
            (f * g - d * i) * inv_det,
            (a * i - c * g) * inv_det,
            (c * d - a * f) * inv_det,
        ],
        [
            (d * h - e * g) * inv_det,
            (b * g - a * h) * inv_det,
            (a * e - b * d) * inv_det,
        ],
    ];
    let tx = matrix[0][3];
    let ty = matrix[1][3];
    let tz = matrix[2][3];
    [
        [
            inv3[0][0],
            inv3[0][1],
            inv3[0][2],
            -(inv3[0][0] * tx + inv3[0][1] * ty + inv3[0][2] * tz),
        ],
        [
            inv3[1][0],
            inv3[1][1],
            inv3[1][2],
            -(inv3[1][0] * tx + inv3[1][1] * ty + inv3[1][2] * tz),
        ],
        [
            inv3[2][0],
            inv3[2][1],
            inv3[2][2],
            -(inv3[2][0] * tx + inv3[2][1] * ty + inv3[2][2] * tz),
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(in super::super) fn clean_quat_from_rotation_matrix3(matrix: [[f64; 3]; 3]) -> (f64, f64, f64, f64) {
    let trace = matrix[0][0] + matrix[1][1] + matrix[2][2];
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        (
            (matrix[2][1] - matrix[1][2]) / s,
            (matrix[0][2] - matrix[2][0]) / s,
            (matrix[1][0] - matrix[0][1]) / s,
            0.25 * s,
        )
    } else if matrix[0][0] > matrix[1][1] && matrix[0][0] > matrix[2][2] {
        let s = (1.0 + matrix[0][0] - matrix[1][1] - matrix[2][2]).sqrt() * 2.0;
        (
            0.25 * s,
            (matrix[0][1] + matrix[1][0]) / s,
            (matrix[0][2] + matrix[2][0]) / s,
            (matrix[2][1] - matrix[1][2]) / s,
        )
    } else if matrix[1][1] > matrix[2][2] {
        let s = (1.0 + matrix[1][1] - matrix[0][0] - matrix[2][2]).sqrt() * 2.0;
        (
            (matrix[0][1] + matrix[1][0]) / s,
            0.25 * s,
            (matrix[1][2] + matrix[2][1]) / s,
            (matrix[0][2] - matrix[2][0]) / s,
        )
    } else {
        let s = (1.0 + matrix[2][2] - matrix[0][0] - matrix[1][1]).sqrt() * 2.0;
        (
            (matrix[0][2] + matrix[2][0]) / s,
            (matrix[1][2] + matrix[2][1]) / s,
            0.25 * s,
            (matrix[1][0] - matrix[0][1]) / s,
        )
    }
}

pub(in super::super) fn clean_decompose_affine_matrix4(
    matrix: [[f64; 4]; 4],
) -> ((f64, f64, f64), (f64, f64, f64, f64), (f64, f64, f64)) {
    let translation = (matrix[0][3], matrix[1][3], matrix[2][3]);
    let column0 = (matrix[0][0], matrix[1][0], matrix[2][0]);
    let column1 = (matrix[0][1], matrix[1][1], matrix[2][1]);
    let column2 = (matrix[0][2], matrix[1][2], matrix[2][2]);
    let scale = (
        (column0.0 * column0.0 + column0.1 * column0.1 + column0.2 * column0.2).sqrt(),
        (column1.0 * column1.0 + column1.1 * column1.1 + column1.2 * column1.2).sqrt(),
        (column2.0 * column2.0 + column2.1 * column2.1 + column2.2 * column2.2).sqrt(),
    );
    let rotation = clean_quat_from_rotation_matrix3([
        [
            matrix[0][0] / scale.0,
            matrix[0][1] / scale.1,
            matrix[0][2] / scale.2,
        ],
        [
            matrix[1][0] / scale.0,
            matrix[1][1] / scale.1,
            matrix[1][2] / scale.2,
        ],
        [
            matrix[2][0] / scale.0,
            matrix[2][1] / scale.1,
            matrix[2][2] / scale.2,
        ],
    ]);
    (translation, rotation, scale)
}
