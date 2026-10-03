use super::*;

pub fn cook_player_avatar(options: &PlayerAvatarCookOptions) -> Result<PlayerAvatarCookReport> {
    if !options.output_root.is_dir() {
        return player_error(format!(
            "native asset output root does not exist: {}",
            options.output_root.display()
        ));
    }
    let missing_sources = PARTS
        .iter()
        .filter(|spec| !options.part_sources.contains_key(spec.route))
        .map(|spec| spec.route)
        .collect::<Vec<_>>();
    if !missing_sources.is_empty() {
        return player_error(format!(
            "missing exact logical-model sources: {}",
            missing_sources.join(", ")
        ));
    }

    let bundle = file_evidence(&options.bundle)?;
    let objects_dump = file_evidence(&options.objects_dump)?;
    let logical_model_plan = file_evidence(&options.logical_plan)?;
    let catalog = DumpCatalog::read(&options.objects_dump)?;
    let actor_ownership = catalog.route(ACTOR_ROUTE)?;
    if actor_ownership.true_name != "m" || actor_ownership.target_path_id != 147_398 {
        return player_error(format!(
            "Retrobution male base identity drifted: route {} -> {}#{}",
            actor_ownership.exact_route, actor_ownership.true_name, actor_ownership.target_path_id
        ));
    }

    let skeleton = extract_male_skeleton(&catalog, actor_ownership)?;
    let actor_bone_names = skeleton
        .nodes
        .iter()
        .map(|node| node.true_name.clone())
        .collect::<Vec<_>>();
    let skeleton_bytes = pretty_json(&skeleton)?;
    let skeleton_output = SkeletonOutputEvidence {
        path: SKELETON_OUTPUT.to_owned(),
        bytes: skeleton_bytes.len() as u64,
        blake3: blake3::hash(&skeleton_bytes).to_hex().to_string(),
        node_count: skeleton.nodes.len(),
        indexed_animation_clip_count: skeleton.animation_clips.len(),
        status: "skeleton-and-clip-index-complete-animation-channel-conversion-pending".to_owned(),
    };

    let mut parts = Vec::with_capacity(PARTS.len());
    for spec in PARTS {
        parts.push(analyze_part(
            &catalog,
            spec,
            options
                .part_sources
                .get(spec.route)
                .expect("complete source set checked"),
            &actor_bone_names,
        )?);
    }

    let report = PlayerAvatarCookReport {
        schema: PLAYER_AVATAR_COOK_REPORT_SCHEMA.to_owned(),
        profile: TEST_SER_PROFILE.to_owned(),
        status: "blocked-before-assembled-glb".to_owned(),
        complete: false,
        bundle,
        objects_dump,
        logical_model_plan,
        selection: test_ser_selection(),
        native_coordinate_contract: exact_native_coordinate_contract(),
        base_skeleton: skeleton_output,
        parts,
        blockers: vec![
            "The exact 230 AnimationClip objects are indexed but their TRS curves have not yet been projected onto the 133-node native skeleton and roundtrip-audited."
                .to_owned(),
            "The verified transformIndicesM palettes and part inverse-bind matrices have not yet been merged into one shared-skeleton GLB; independent part scenes are not accepted as the finished avatar."
                .to_owned(),
            "Height and shape additive clips are indexed but not yet sampled with Test Ser's height/body values."
                .to_owned(),
            "No GLB is emitted by this stage, so the production asset manifest is intentionally not mutated."
                .to_owned(),
        ],
    };
    let report_bytes = pretty_json(&report)?;
    write_new_output(&options.output_root.join(SKELETON_OUTPUT), &skeleton_bytes)?;
    write_new_output(&options.output_root.join(REPORT_OUTPUT), &report_bytes)?;
    Ok(report)
}

pub(super) fn analyze_part(
    catalog: &DumpCatalog,
    spec: &PartSpec,
    source_path: &Path,
    actor_bone_names: &[String],
) -> Result<PlayerPartEvidence> {
    let ownership = catalog.route(spec.route)?;
    let source = file_evidence(source_path)?;
    let bytes = fs::read(source_path).map_err(|error| io_at(source_path, error))?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|source_error| PipelineError::Json {
            path: source_path.display().to_string(),
            source: source_error,
        })?;
    if document.get("schema").and_then(Value::as_str) != Some("ffone.logical-model-source.v1")
        || document.get("exactContainerRoute").and_then(Value::as_str) != Some(spec.route)
        || document.get("logicalName").and_then(Value::as_str) != Some(ownership.true_name.as_str())
    {
        return player_error(format!(
            "{} is not the exact source for {} -> {}",
            source_path.display(),
            spec.route,
            ownership.true_name
        ));
    }
    let expected_contract = serde_json::to_value(exact_native_coordinate_contract())
        .map_err(|error| player_message(format!("coordinate contract JSON failed: {error}")))?;
    if document.get("nativeCoordinateContract") != Some(&expected_contract) {
        return player_error(format!(
            "{} has a contradictory native coordinate contract",
            source_path.display()
        ));
    }
    let meshes = array_field(&document, "meshes")?;
    if meshes.is_empty() {
        return player_error(format!("{} contains no meshes", source_path.display()));
    }
    let warnings = document
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| player_message("source warning is not a string"))
        })
        .collect::<Result<Vec<_>>>()?;
    let warning_classified = !warnings.is_empty()
        && warnings
            .iter()
            .all(|warning| classified_unity_alias_warning(spec.route, warning));
    if !warnings.is_empty() && !warning_classified {
        return player_error(format!(
            "{} contains unresolved warnings: {}",
            source_path.display(),
            warnings.join("; ")
        ));
    }

    #[derive(Clone, Copy)]
    struct SourceSkin {
        joints: usize,
        inverse_binds: usize,
    }
    let mut renderers = BTreeMap::<i64, SourceSkin>::new();
    for mesh in meshes {
        let Some(skin) = mesh.get("skin").filter(|value| !value.is_null()) else {
            continue;
        };
        let renderer_path_id = skin
            .get("rendererPathId")
            .and_then(Value::as_i64)
            .ok_or_else(|| player_message("source skin has no rendererPathId"))?;
        let joints = optional_array_len(skin, "jointPaths");
        let inverse_binds = optional_array_len(skin, "inverseBindMatrices");
        if joints == 0 || joints != inverse_binds {
            return player_error(format!(
                "{} renderer {} joint/IBM mismatch {joints}/{inverse_binds}",
                spec.route, renderer_path_id
            ));
        }
        match renderers.insert(
            renderer_path_id,
            SourceSkin {
                joints,
                inverse_binds,
            },
        ) {
            Some(previous)
                if previous.joints != joints || previous.inverse_binds != inverse_binds =>
            {
                return player_error(format!(
                    "{} repeats renderer {} with contradictory skin counts",
                    spec.route, renderer_path_id
                ));
            }
            _ => {}
        }
    }
    if spec.participation == "combined_skinned_mesh" && renderers.is_empty() {
        return player_error(format!("{} has no skinned renderer", spec.route));
    }

    let mut renderer_remaps = Vec::with_capacity(renderers.len());
    for (renderer_path_id, source_skin) in renderers {
        let renderer = catalog.get(renderer_path_id)?;
        if renderer.object_type != "SkinnedMeshRenderer" {
            return player_error(format!(
                "{} source renderer {} is {}",
                spec.route, renderer_path_id, renderer.object_type
            ));
        }
        let renderer_value = catalog.value(renderer_path_id)?;
        let game_object_path_id = pointer_path_id(
            renderer_value
                .get("m_GameObject")
                .ok_or_else(|| player_message("renderer has no m_GameObject"))?,
        )?;
        let game_object = catalog.get(game_object_path_id)?;
        let renderer_bone_count = optional_array_len(&renderer_value, "m_Bones");
        let game_object_value = catalog.value(game_object_path_id)?;
        let index_tables = component_path_ids(&game_object_value)?
            .into_iter()
            .filter_map(|path_id| {
                catalog
                    .value(path_id)
                    .ok()
                    .filter(|value| value.get("transformIndicesM").is_some())
                    .map(|value| (path_id, value))
            })
            .collect::<Vec<_>>();
        let [(index_table_path_id, index_table)] = index_tables.as_slice() else {
            return player_error(format!(
                "{} renderer {} GameObject {} has {} ActorWearIndexTable components",
                spec.route,
                renderer_path_id,
                game_object.name,
                index_tables.len()
            ));
        };
        let actor_bone_indices = array_field(index_table, "transformIndicesM")?
            .iter()
            .map(|value| {
                let index = value
                    .as_u64()
                    .ok_or_else(|| player_message("transformIndicesM entry is not unsigned"))?;
                usize::try_from(index)
                    .map_err(|_| player_message("transformIndicesM entry exceeds usize"))
            })
            .collect::<Result<Vec<_>>>()?;
        if actor_bone_indices.len() != renderer_bone_count
            || actor_bone_indices.len() != source_skin.joints
        {
            return player_error(format!(
                "{} renderer {} palette mismatch: renderer={}, source={}, transformIndicesM={}",
                spec.route,
                renderer_path_id,
                renderer_bone_count,
                source_skin.joints,
                actor_bone_indices.len()
            ));
        }
        let actor_bone_names_for_renderer = actor_bone_indices
            .iter()
            .map(|index| {
                actor_bone_names.get(*index).cloned().ok_or_else(|| {
                    player_message(format!(
                        "{} renderer {} transformIndicesM index {} is outside actorBones",
                        spec.route, renderer_path_id, index
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        renderer_remaps.push(RendererRemapEvidence {
            renderer_path_id,
            renderer_game_object_path_id: game_object_path_id,
            renderer_true_name: game_object.name.clone(),
            actor_wear_index_table_path_id: *index_table_path_id,
            renderer_bone_count,
            source_joint_count: source_skin.joints,
            inverse_bind_matrix_count: source_skin.inverse_binds,
            actor_bone_indices,
            actor_bone_names: actor_bone_names_for_renderer,
        });
    }

    Ok(PlayerPartEvidence {
        part: spec.part.to_owned(),
        participation: spec.participation.to_owned(),
        ownership,
        source,
        planned_semantic_glb: spec.semantic_glb.to_owned(),
        source_mesh_count: meshes.len(),
        source_material_count: document
            .get("materials")
            .and_then(Value::as_object)
            .map_or(0, serde_json::Map::len),
        source_texture_count: document
            .get("textures")
            .and_then(Value::as_object)
            .map_or(0, serde_json::Map::len),
        source_skeleton_joint_count: document
            .pointer("/skeleton/joints")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        source_warnings: warnings,
        legacy_alias_warning_classified: warning_classified,
        renderer_remaps,
        status: if spec.participation == "combined_skinned_mesh" {
            "source-and-shared-palette-remap-verified-glb-pending"
        } else {
            "rigid-source-verified-glb-pending"
        }
        .to_owned(),
    })
}

pub(super) fn file_evidence(path: &Path) -> Result<FileEvidence> {
    let metadata = fs::metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.is_file() {
        return player_error(format!("input is not a file: {}", path.display()));
    }
    let mut file = File::open(path).map_err(|error| io_at(path, error))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| io_at(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(FileEvidence {
        path: path.display().to_string(),
        bytes: metadata.len(),
        blake3: hasher.finalize().to_hex().to_string(),
    })
}

pub(super) fn pretty_json(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        player_message(format!("could not serialize player cook output: {error}"))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn array_field<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| player_message(format!("{field} is missing or not an array")))
}

pub(super) fn optional_array_len(value: &Value, field: &str) -> usize {
    value
        .get(field)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

pub(super) fn usize_field(value: &Value, field: &str) -> Result<usize> {
    let raw = value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| player_message(format!("{field} is missing or not unsigned")))?;
    usize::try_from(raw).map_err(|_| player_message(format!("{field} exceeds usize")))
}

pub(super) fn vec3_field(value: &Value, field: &str) -> Result<[f64; 3]> {
    let vector = value
        .get(field)
        .ok_or_else(|| player_message(format!("{field} is missing")))?;
    Ok([
        finite_number(vector, "x", field)?,
        finite_number(vector, "y", field)?,
        finite_number(vector, "z", field)?,
    ])
}

pub(super) fn quat_field(value: &Value, field: &str) -> Result<[f64; 4]> {
    let quaternion = value
        .get(field)
        .ok_or_else(|| player_message(format!("{field} is missing")))?;
    Ok([
        finite_number(quaternion, "x", field)?,
        finite_number(quaternion, "y", field)?,
        finite_number(quaternion, "z", field)?,
        finite_number(quaternion, "w", field)?,
    ])
}

pub(super) fn finite_number(value: &Value, component: &str, field: &str) -> Result<f64> {
    value
        .get(component)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| player_message(format!("{field}.{component} is not finite")))
}

pub(super) fn native_rotation(value: [f64; 4]) -> Result<[f64; 4]> {
    let norm = value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || (norm - 1.0).abs() > 1.0e-4 {
        return player_error(format!("source quaternion is not unit length: {norm}"));
    }
    Ok([value[0], -value[1], -value[2], value[3]])
}

pub(super) fn player_message(detail: impl Into<String>) -> PipelineError {
    PipelineError::PlayerAvatarCook(detail.into())
}
