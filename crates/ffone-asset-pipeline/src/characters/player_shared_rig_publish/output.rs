use super::*;

pub fn publish_player_shared_rigs(
    options: &PlayerSharedRigPublishOptions,
) -> Result<PlayerSharedRigPublishReport> {
    if !options.asset_root.is_dir() {
        return rig_error(format!(
            "native asset root does not exist: {}",
            options.asset_root.display()
        ));
    }
    let dump_metadata =
        fs::metadata(&options.objects_dump).map_err(|error| io_at(&options.objects_dump, error))?;
    if !dump_metadata.is_file() {
        return rig_error("player rig object dump is not a file");
    }
    let dump_bytes =
        fs::read(&options.objects_dump).map_err(|error| io_at(&options.objects_dump, error))?;
    let dump_blake3 = blake3::hash(&dump_bytes).to_hex().to_string();
    drop(dump_bytes);

    let equipment_path = options
        .asset_root
        .join("characters/player/equipment/catalog.json");
    let equipment_bytes =
        fs::read(&equipment_path).map_err(|error| io_at(&equipment_path, error))?;
    let equipment: PlayerEquipmentCatalog =
        serde_json::from_slice(&equipment_bytes).map_err(|source| PipelineError::Json {
            path: equipment_path.display().to_string(),
            source,
        })?;
    if !equipment.standalone_gpu_passed {
        return rig_error("player equipment catalog has not passed its standalone GPU gate");
    }

    let appearance_path = options.asset_root.join(CHARACTER_CREATION_APPEARANCE_PATH);
    let appearance_bytes =
        fs::read(&appearance_path).map_err(|error| io_at(&appearance_path, error))?;
    let appearance: CharacterCreationAppearance = serde_json::from_slice(&appearance_bytes)
        .map_err(|source| PipelineError::Json {
            path: appearance_path.display().to_string(),
            source,
        })?;
    if appearance.schema != CHARACTER_CREATION_APPEARANCE_SCHEMA {
        return rig_error("character-creation appearance schema drifted");
    }
    let avatar_items_path = options
        .asset_root
        .join(CHARACTER_CREATION_AVATAR_ITEMS_PATH);
    let avatar_items_bytes =
        fs::read(&avatar_items_path).map_err(|error| io_at(&avatar_items_path, error))?;
    let avatar_items: CharacterCreationAvatarItems = serde_json::from_slice(&avatar_items_bytes)
        .map_err(|source| PipelineError::Json {
            path: avatar_items_path.display().to_string(),
            source,
        })?;
    if avatar_items.schema != CHARACTER_CREATION_AVATAR_ITEMS_SCHEMA {
        return rig_error("character-creation avatar-item schema drifted");
    }

    let (supplemental_models, mut outputs) = publish_supplemental_creator_sources(options)?;
    let mut creator_models = equipment
        .models
        .iter()
        .map(|model| CreatorModelCandidate {
            gender: None,
            exact_route: model.exact_route.clone(),
            true_name: model.true_name.clone(),
            glb: model.glb.clone(),
            glb_blake3: model.glb_blake3.clone(),
        })
        .collect::<Vec<_>>();
    creator_models.extend(supplemental_models);

    let catalog = DumpCatalog::read(&options.objects_dump)?;
    let (male_contract, male_glb) = publish_gender(
        &catalog,
        &appearance,
        &avatar_items,
        &creator_models,
        &options.asset_root,
        MALE,
    )?;
    let (female_contract, female_glb) = publish_gender(
        &catalog,
        &appearance,
        &avatar_items,
        &creator_models,
        &options.asset_root,
        FEMALE,
    )?;
    let contract = PlayerSharedRigContract {
        schema: PLAYER_SHARED_RIG_SCHEMA.to_owned(),
        status: "native-shared-skeleton-stand1-and-complete-creator-routing-ready".to_owned(),
        source: PlayerRigSourceIdentity {
            object_dump: options.objects_dump.display().to_string(),
            object_dump_bytes: dump_metadata.len(),
            object_dump_blake3: dump_blake3,
            appearance: CHARACTER_CREATION_APPEARANCE_PATH.to_owned(),
            appearance_blake3: blake3::hash(&appearance_bytes).to_hex().to_string(),
            avatar_items: CHARACTER_CREATION_AVATAR_ITEMS_PATH.to_owned(),
            avatar_items_blake3: blake3::hash(&avatar_items_bytes).to_hex().to_string(),
            source_build: equipment.source_build.clone(),
        },
        native_coordinate_contract: exact_native_coordinate_contract(),
        creator_preview_ready: true,
        fake_animation_used: false,
        unity_runtime_required: false,
        genders: vec![male_contract, female_contract],
    };
    let mut contract_bytes =
        serde_json::to_vec_pretty(&contract).map_err(|source| PipelineError::Json {
            path: PLAYER_SHARED_RIG_CONTRACT_PATH.to_owned(),
            source,
        })?;
    contract_bytes.push(b'\n');

    outputs.extend([
        RigOutput {
            relative: MALE_SHARED_SKELETON_GLB_PATH.to_owned(),
            kind: ProjectAssetKind::Model,
            bytes: male_glb,
            source_path: "offline-object-dump:actor/m.kfm".to_owned(),
        },
        RigOutput {
            relative: FEMALE_SHARED_SKELETON_GLB_PATH.to_owned(),
            kind: ProjectAssetKind::Model,
            bytes: female_glb,
            source_path: "offline-object-dump:actor/w.kfm".to_owned(),
        },
        RigOutput {
            relative: PLAYER_SHARED_RIG_CONTRACT_PATH.to_owned(),
            kind: ProjectAssetKind::Data,
            bytes: contract_bytes,
            source_path: "offline-object-dump:player-shared-rig-contract".to_owned(),
        },
    ]);
    for output in &outputs {
        let path = options.asset_root.join(&output.relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
        }
        fs::write(&path, &output.bytes).map_err(|error| io_at(&path, error))?;
    }

    let manifest_path = options.asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    manifest
        .files
        .retain(|entry| !outputs.iter().any(|output| entry.path == output.relative));
    for output in &outputs {
        manifest.files.push(ProjectAssetFile {
            source_path: output.source_path.clone(),
            path: output.relative.clone(),
            kind: output.kind,
            bytes: output.bytes.len() as u64,
            blake3: blake3::hash(&output.bytes).to_hex().to_string(),
        });
    }
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    let mut new_manifest =
        serde_json::to_vec_pretty(&manifest).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    new_manifest.push(b'\n');
    fs::write(&manifest_path, new_manifest).map_err(|error| io_at(&manifest_path, error))?;

    let male = &contract.genders[0];
    let female = &contract.genders[1];
    Ok(PlayerSharedRigPublishReport {
        schema: PLAYER_SHARED_RIG_REPORT_SCHEMA.to_owned(),
        status: contract.status.clone(),
        contract_path: PLAYER_SHARED_RIG_CONTRACT_PATH.to_owned(),
        male_skeleton_glb: MALE_SHARED_SKELETON_GLB_PATH.to_owned(),
        female_skeleton_glb: FEMALE_SHARED_SKELETON_GLB_PATH.to_owned(),
        male_actor_bones: male.nodes.len() as u32,
        female_actor_bones: female.nodes.len() as u32,
        published_animation_clips: contract
            .genders
            .iter()
            .map(|gender| gender.clips.len() as u32)
            .sum(),
        verified_creator_parts: contract
            .genders
            .iter()
            .map(|gender| gender.creator_parts.len() as u32)
            .sum(),
        verified_creator_choices: contract
            .genders
            .iter()
            .map(|gender| gender.creator_choices.len() as u32)
            .sum(),
        verified_skin_palettes: contract
            .genders
            .iter()
            .flat_map(|gender| &gender.creator_parts)
            .map(|part| part.skins.len() as u32)
            .sum(),
        manifest_files: manifest.files.len() as u64,
        creator_preview_ready: true,
        height_shape_runtime_status: "native-bevy-static-scale-and-additive-delta-ready".to_owned(),
    })
}

pub(super) fn publish_supplemental_creator_sources(
    options: &PlayerSharedRigPublishOptions,
) -> Result<(Vec<CreatorModelCandidate>, Vec<RigOutput>)> {
    let mut candidates = Vec::new();
    let mut outputs = Vec::new();
    for source in &options.supplemental_creator_sources {
        if source.category != "pants"
            || source.gender != PlayerRigGender::Female
            || !matches!(
                source.exact_route.as_str(),
                "wear/f_pants_gothgirl.nif" | "wear/f_pants_stylistdandy.nif"
            )
        {
            return rig_error(format!(
                "unsupported supplemental creator source {:?}",
                source.exact_route
            ));
        }
        let source_bytes =
            fs::read(&source.source).map_err(|error| io_at(&source.source, error))?;
        let source_json: Value =
            serde_json::from_slice(&source_bytes).map_err(|json_source| PipelineError::Json {
                path: source.source.display().to_string(),
                source: json_source,
            })?;
        if source_json.get("schema").and_then(Value::as_str)
            != Some("ffone.logical-model-source.v1")
            || source_json
                .get("exactContainerRoute")
                .and_then(Value::as_str)
                != Some(source.exact_route.as_str())
            || source_json.get("logicalName").and_then(Value::as_str)
                != Some(source.true_name.as_str())
            || source_json
                .get("warnings")
                .and_then(Value::as_array)
                .is_none_or(|warnings| !warnings.is_empty())
        {
            return rig_error(format!(
                "supplemental creator source {:?} is not a warning-free exact source for {:?}",
                source.source, source.exact_route
            ));
        }

        let publish_options =
            LogicalModelPublishOptions::new(&source.source, "characters", &options.asset_root)
                .with_semantic_directories([
                    "player",
                    "equipment",
                    source.category.as_str(),
                    source.true_name.as_str(),
                ])
                .with_semantic_root_layout();
        let prepared = prepare_logical_model(&publish_options, &source_bytes)?;
        let expected_glb = format!(
            "characters/player/equipment/{}/{}/{}.glb",
            source.category, source.true_name, source.true_name
        );
        if prepared.report.contract.legacy_name != source.true_name
            || prepared.report.contract.output_glb != expected_glb
            || !prepared.report.semantic_proof.matched
            || !prepared
                .report
                .contract
                .unresolved_source_features
                .is_empty()
        {
            return rig_error(format!(
                "supplemental creator publication contract failed for {:?}",
                source.exact_route
            ));
        }
        let relative_files = prepared
            .relative_files()
            .map(|path| path.to_path_buf())
            .collect::<Vec<_>>();
        verify_or_write_prepared_logical_model(&options.asset_root, &prepared)?;
        for relative in relative_files {
            let relative_string = relative.to_string_lossy().replace('\\', "/");
            let absolute = options.asset_root.join(&relative);
            let bytes = fs::read(&absolute).map_err(|error| io_at(&absolute, error))?;
            let kind = match relative
                .extension()
                .and_then(|extension| extension.to_str())
            {
                Some("glb") => ProjectAssetKind::Model,
                Some("png") => ProjectAssetKind::Texture,
                Some("json") => ProjectAssetKind::Data,
                other => {
                    return rig_error(format!(
                        "supplemental creator output {:?} has unsupported extension {other:?}",
                        relative
                    ));
                }
            };
            outputs.push(RigOutput {
                relative: relative_string,
                kind,
                bytes,
                source_path: format!(
                    "offline-exact-logical-model-source:{}#{}",
                    source.source.display(),
                    source.exact_route
                ),
            });
        }
        candidates.push(CreatorModelCandidate {
            gender: Some(source.gender),
            exact_route: source.exact_route.clone(),
            true_name: source.true_name.clone(),
            glb: expected_glb,
            glb_blake3: prepared.report.contract.glb_blake3,
        });
    }
    Ok((candidates, outputs))
}

pub(super) fn publish_gender(
    catalog: &DumpCatalog,
    appearance: &CharacterCreationAppearance,
    avatar_items: &CharacterCreationAvatarItems,
    creator_models: &[CreatorModelCandidate],
    asset_root: &Path,
    spec: GenderSpec,
) -> Result<(PlayerGenderRigContract, Vec<u8>)> {
    let (root_path_id, root_name) = catalog.route(spec.route)?;
    if root_path_id != spec.expected_root_path_id || root_name != spec.root_name {
        return rig_error(format!(
            "{} identity drifted: {}#{}",
            spec.route, root_name, root_path_id
        ));
    }
    let (nodes, combiner_path_id, animation_component_path_id) =
        extract_actor_nodes(catalog, root_path_id, spec)?;
    validate_animation_component(
        catalog,
        animation_component_path_id,
        spec.expected_clip_count,
        &spec.clips,
    )?;

    let mut animations = Vec::with_capacity(spec.clips.len());
    let mut clip_contracts = Vec::with_capacity(spec.clips.len());
    for (animation_index, (name, path_id)) in spec.clips.iter().copied().enumerate() {
        let mut clip = decode_clip(catalog, &nodes, spec.root_name, name, path_id)?;
        rebase_legacy_additive_clip(&mut clip, &nodes)?;
        let source_key_count = clip
            .channels
            .iter()
            .map(|channel| u64::from(channel.source_key_count))
            .sum();
        clip_contracts.push(PlayerRigClipContract {
            name: name.to_owned(),
            source_path_id: path_id,
            gltf_animation_index: animation_index as u32,
            channel_count: clip.channels.len() as u32,
            source_key_count,
            duration_seconds_bits: clip.duration.to_bits(),
            playback: if runtime_clip_loops(name) {
                "loop".to_owned()
            } else {
                "clamp".to_owned()
            },
            runtime_status: runtime_clip_status(name).to_owned(),
        });
        animations.push(clip);
    }
    let model = NativeModel {
        schema: ffone_skinned_model::MODEL_SCHEMA.to_owned(),
        name: spec.root_name.to_owned(),
        native_coordinate_contract: exact_native_coordinate_contract(),
        roots: vec![0],
        nodes: nodes
            .iter()
            .map(|node| ModelNode {
                name: node.true_name.clone(),
                legacy_name: None,
                legacy_sibling_ordinal: None,
                parent: node.parent_actor_bone_index,
                translation: node.translation,
                rotation: node.rotation,
                scale: node.scale,
                mesh: None,
                skin: None,
            })
            .collect(),
        meshes: Vec::new(),
        skins: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        samplers: Vec::new(),
        animations,
    };
    let glb = encode_glb(&model)
        .map_err(|error| rig_message(format!("{} GLB encoding failed: {error}", spec.route)))?;
    let document = parse_glb_json(&glb)?;
    validate_published_animations(&document, &spec.clips)?;

    let (creator_choices, creator_models) =
        resolve_creator_models(appearance, avatar_items, creator_models, spec)?;
    let mut creator_parts = Vec::with_capacity(creator_models.len());
    for creator_model in creator_models {
        let exact_route = creator_model.exact_route.as_str();
        let clothes_indices = creator_choices
            .iter()
            .filter(|choice| choice.exact_route == exact_route)
            .map(|choice| actor_skin_combiner_clothes_index(choice.appearance_category))
            .collect::<BTreeSet<_>>();
        let mut clothes_indices_iter = clothes_indices.iter().copied();
        let Some(actor_skin_combiner_clothes_index) = clothes_indices_iter.next() else {
            return rig_error(format!(
                "{:?} creator route {exact_route:?} resolves {} ActorSkinCombiner clothes slots",
                spec.gender,
                clothes_indices.len()
            ));
        };
        if clothes_indices_iter.next().is_some() {
            return rig_error(format!(
                "{:?} creator route {exact_route:?} resolves {} ActorSkinCombiner clothes slots",
                spec.gender,
                clothes_indices.len()
            ));
        }
        let glb_path = asset_root.join(&creator_model.glb);
        let glb_bytes = fs::read(&glb_path).map_err(|error| io_at(&glb_path, error))?;
        if blake3::hash(&glb_bytes).to_hex().as_str() != creator_model.glb_blake3 {
            return rig_error(format!(
                "equipment GLB hash drifted for {}",
                creator_model.glb
            ));
        }
        let legacy = extract_route_remaps(catalog, exact_route, spec)?;
        let skins = audit_glb_remaps(&glb_bytes, &legacy, &nodes, spec.root_name)?;
        creator_parts.push(PlayerRigPartContract {
            exact_route: exact_route.to_owned(),
            true_name: creator_model.true_name,
            glb: creator_model.glb,
            actor_skin_combiner_clothes_index,
            skins,
        });
    }
    let default_creator_part_routes = spec
        .default_parts
        .iter()
        .map(|route| (*route).to_owned())
        .collect::<Vec<_>>();
    for route in &default_creator_part_routes {
        if !creator_parts.iter().any(|part| part.exact_route == *route) {
            return rig_error(format!(
                "{:?} default creator route {route:?} is outside the complete creator contract",
                spec.gender
            ));
        }
    }
    let published_clothes_slots = creator_parts
        .iter()
        .map(|part| part.actor_skin_combiner_clothes_index)
        .collect::<BTreeSet<_>>();
    let expected_clothes_slots = (0_u8..=4).collect::<BTreeSet<_>>();
    if published_clothes_slots != expected_clothes_slots {
        return rig_error(format!(
            "{:?} creator parts do not cover exact ActorSkinCombiner clothes slots 0..=4: {:?}",
            spec.gender, published_clothes_slots
        ));
    }
    let default_clothes_slots = default_creator_part_routes
        .iter()
        .map(|route| {
            creator_parts
                .iter()
                .find(|part| part.exact_route == *route)
                .expect("default route membership was validated")
                .actor_skin_combiner_clothes_index
        })
        .collect::<BTreeSet<_>>();
    if default_clothes_slots != expected_clothes_slots {
        return rig_error(format!(
            "{:?} default creator parts do not select each ActorSkinCombiner clothes slot exactly once: {:?}",
            spec.gender, default_clothes_slots
        ));
    }

    Ok((
        PlayerGenderRigContract {
            gender: spec.gender,
            exact_actor_route: spec.route.to_owned(),
            actor_root_path_id: root_path_id,
            actor_skin_combiner_path_id: combiner_path_id,
            animation_component_path_id,
            skeleton_glb: spec.skeleton_glb.to_owned(),
            nodes,
            clips: clip_contracts,
            creator_parts,
            default_creator_part_routes,
            creator_choices,
            stand1_runtime_ready: true,
            height_shape_assets_published: true,
            height_shape_runtime_status: "native-bevy-static-scale-and-additive-delta-ready"
                .to_owned(),
        },
        glb,
    ))
}
