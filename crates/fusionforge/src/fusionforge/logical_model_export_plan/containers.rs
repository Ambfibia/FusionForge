use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PhysicalIndexedContainerOccurrence {
    pub(super) exact_route: String,
    pub(super) fingerprint: Result<LogicalModelPhysicalFingerprint, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfContainedGameObjectEvidence {
    pub proof: String,
    pub root_game_object: ResolvedObjectKeyEvidence,
    pub root_transform: ResolvedObjectKeyEvidence,
    pub root_name: String,
    pub root_closure_key_count: usize,
    pub preload_core_key_count: usize,
    pub game_object_count: usize,
    pub transform_count: usize,
    pub hierarchy_edge_count: usize,
    pub component_count: usize,
    pub renderer_count: usize,
    pub skinned_mesh_renderer_count: usize,
    pub mesh_count: usize,
    pub material_count: usize,
    pub bone_count: usize,
    pub animation_component_count: usize,
    pub animation_clip_count: usize,
    pub null_parent_transform_count: usize,
    pub core_keys: Vec<ResolvedObjectKeyEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedObjectKeyEvidence {
    pub asset_index: usize,
    pub asset_name: String,
    pub path_id: i64,
    pub object_type: String,
}

#[derive(Debug, Clone)]
pub(super) struct ExactContainerOccurrence {
    pub(super) metadata: super::super::UnityValue,
    pub(super) preload_table: Vec<super::super::UnityValue>,
    pub(super) asset_bundle_asset_name: String,
    pub(super) asset_bundle_path_id: i64,
}

pub(super) fn exact_archive_bundle_paths(
    bundle_index_path: &Path,
) -> Result<BTreeMap<String, Vec<PathBuf>>, String> {
    let source = fs::read_to_string(bundle_index_path)
        .map_err(|err| format!("{}: {err}", bundle_index_path.display()))?;
    let root: JsonValue = serde_json::from_str(&source)
        .map_err(|err| format!("{}: {err}", bundle_index_path.display()))?;
    let mut paths = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for bundle in root
        .get("bundles")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        let Some(bundle_path) = bundle.get("path").and_then(JsonValue::as_str) else {
            continue;
        };
        for extracted in bundle
            .get("extractedFiles")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
        {
            let Some(archive_name) = extracted.get("name").and_then(JsonValue::as_str) else {
                continue;
            };
            paths
                .entry(super::super::unity::normalize_bundle_name(archive_name))
                .or_default()
                .insert(PathBuf::from(bundle_path));
        }
    }
    Ok(paths
        .into_iter()
        .map(|(archive, paths)| (archive, paths.into_iter().collect()))
        .collect())
}

pub(super) fn resolved_object_key_evidence(
    env: &super::super::UnityEnvironment,
    key: (usize, i64),
) -> Result<ResolvedObjectKeyEvidence, KfmResolveError> {
    let asset = env.assets.get(key.0).ok_or_else(|| {
        KfmResolveError::new(
            "kfmResolvedAssetMissing",
            format!("resolved asset index {} is absent", key.0),
        )
    })?;
    let info = asset.objects.get(&key.1).ok_or_else(|| {
        KfmResolveError::new(
            "kfmResolvedObjectMissing",
            format!("resolved object {}#{} is absent", asset.name, key.1),
        )
    })?;
    Ok(ResolvedObjectKeyEvidence {
        asset_index: key.0,
        asset_name: asset.name.clone(),
        path_id: key.1,
        object_type: asset.object_type_name(info),
    })
}

#[derive(Debug)]
pub(super) struct ExactObjectBody {
    pub(super) object_type: String,
    pub(super) class_id: i32,
    pub(super) body: super::super::UnityValue,
}

#[derive(Debug)]
pub(super) struct SelfContainedGameObjectProof {
    pub(super) evidence: SelfContainedGameObjectEvidence,
    pub(super) root_closure: BTreeSet<(usize, i64)>,
}

pub(super) fn exact_object_body(
    env: &super::super::UnityEnvironment,
    key: (usize, i64),
) -> Result<ExactObjectBody, String> {
    let asset = env
        .assets
        .get(key.0)
        .ok_or_else(|| format!("asset index {} is absent", key.0))?;
    let info = asset
        .objects
        .get(&key.1)
        .ok_or_else(|| format!("{}#{} is absent", asset.name, key.1))?;
    let body = asset
        .read_object(key.0, info)
        .map_err(|err| format!("{}#{} is unreadable: {err}", asset.name, key.1))?;
    Ok(ExactObjectBody {
        object_type: asset.object_type_name(info),
        class_id: info.class_id,
        body,
    })
}

pub(super) fn prove_self_contained_game_object(
    env: &super::super::UnityEnvironment,
    container_key: (usize, i64),
    preload_keys: &BTreeSet<(usize, i64)>,
) -> Result<SelfContainedGameObjectProof, String> {
    let root_object = exact_object_body(env, container_key)?;
    if root_object.object_type != "GameObject" {
        return Err(format!(
            "exact container target is {}, not GameObject",
            root_object.object_type
        ));
    }
    let root_name = super::super::object_name(&root_object.body);
    let root_components = root_object
        .body
        .get("m_Component")
        .and_then(super::super::UnityValue::as_array)
        .ok_or_else(|| "root GameObject has no exact m_Component array".to_string())?;
    let mut root_transforms = Vec::new();
    for (index, entry) in root_components.iter().enumerate() {
        let (_, pointer) = exact_component_class_and_pointer(entry).ok_or_else(|| {
            format!("root GameObject m_Component[{index}] is not an exact class/pointer pair")
        })?;
        let key = env.resolve_pointer(pointer).map_err(|err| {
            format!("root GameObject m_Component[{index}] cannot be resolved: {err}")
        })?;
        let key = (key.asset, key.path_id);
        if exact_object_body(env, key)?.object_type == "Transform" {
            root_transforms.push(key);
        }
    }
    root_transforms.sort();
    root_transforms.dedup();
    if root_transforms.len() != 1 {
        return Err(format!(
            "root GameObject must have exactly one Transform component, found {}",
            root_transforms.len()
        ));
    }
    let root_transform = root_transforms[0];

    let root_closure = exact_pointer_closure(env, container_key)?;
    let mut game_objects = BTreeSet::new();
    let mut transforms = BTreeSet::new();
    let mut component_keys = BTreeSet::new();
    let mut component_records = Vec::new();
    let mut runtime_animation_paths = BTreeSet::from([String::new()]);
    let mut hierarchy_edge_count = 0usize;
    let mut pending = vec![(root_transform, None, String::new())];

    while let Some((transform_key, expected_parent, relative_path)) = pending.pop() {
        if !transforms.insert(transform_key) {
            return Err(format!(
                "Transform {}#{} is repeated in the exact hierarchy",
                transform_key.0, transform_key.1
            ));
        }
        let transform = exact_object_body(env, transform_key)?;
        if transform.object_type != "Transform" {
            return Err(format!(
                "hierarchy expected Transform, got {} at {}#{}",
                transform.object_type, transform_key.0, transform_key.1
            ));
        }
        let actual_parent =
            exact_nullable_pointer_key(env, transform.body.get("m_Father"), "Transform.m_Father")?;
        if actual_parent != expected_parent {
            return Err(format!(
                "Transform {}#{} parent mismatch: expected {:?}, got {:?}",
                transform_key.0, transform_key.1, expected_parent, actual_parent
            ));
        }
        let game_object_key = exact_typed_dependency(
            env,
            transform.body.get("m_GameObject"),
            "GameObject",
            "Transform.m_GameObject",
        )?;
        if !game_objects.insert(game_object_key) {
            return Err(format!(
                "GameObject {}#{} is bound to more than one hierarchy Transform",
                game_object_key.0, game_object_key.1
            ));
        }
        let game_object = exact_object_body(env, game_object_key)?;
        let components = game_object
            .body
            .get("m_Component")
            .and_then(super::super::UnityValue::as_array)
            .ok_or_else(|| {
                format!(
                    "GameObject {}#{} has no exact m_Component array",
                    game_object_key.0, game_object_key.1
                )
            })?;
        let mut matching_transform_entries = 0usize;
        for (component_index, entry) in components.iter().enumerate() {
            let (serialized_class_id, pointer) = exact_component_class_and_pointer(entry)
                .ok_or_else(|| {
                    format!(
                        "GameObject {}#{} m_Component[{component_index}] is not an exact class/pointer pair",
                        game_object_key.0, game_object_key.1
                    )
                })?;
            let resolved = env.resolve_pointer(pointer).map_err(|err| {
                format!(
                    "GameObject {}#{} m_Component[{component_index}] cannot be resolved: {err}",
                    game_object_key.0, game_object_key.1
                )
            })?;
            let component_key = (resolved.asset, resolved.path_id);
            let component = exact_object_body(env, component_key)?;
            if i64::from(component.class_id) != serialized_class_id {
                return Err(format!(
                    "GameObject {}#{} component {}#{} class mismatch: serialized {}, object {}",
                    game_object_key.0,
                    game_object_key.1,
                    component_key.0,
                    component_key.1,
                    serialized_class_id,
                    component.class_id
                ));
            }
            let backlink = exact_required_pointer_key(
                env,
                component.body.get("m_GameObject"),
                "Component.m_GameObject",
            )?;
            if backlink != game_object_key {
                return Err(format!(
                    "component {}#{} points back to {:?}, expected GameObject {:?}",
                    component_key.0, component_key.1, backlink, game_object_key
                ));
            }
            if component.object_type == "Transform" {
                if serialized_class_id != 4 || component_key != transform_key {
                    return Err(format!(
                        "GameObject {}#{} has a Transform entry that is not its exact hierarchy Transform",
                        game_object_key.0, game_object_key.1
                    ));
                }
                matching_transform_entries += 1;
            }
            if !component_keys.insert(component_key) {
                return Err(format!(
                    "component {}#{} is shared by multiple GameObjects or listed twice",
                    component_key.0, component_key.1
                ));
            }
            component_records.push(ExactComponentRecord {
                game_object_key,
                key: component_key,
                object_type: component.object_type,
                body: component.body,
            });
        }
        if matching_transform_entries != 1 {
            return Err(format!(
                "GameObject {}#{} must list its hierarchy Transform exactly once, found {matching_transform_entries}",
                game_object_key.0, game_object_key.1
            ));
        }

        let children = transform
            .body
            .get("m_Children")
            .and_then(super::super::UnityValue::as_array)
            .ok_or_else(|| {
                format!(
                    "Transform {}#{} has no exact m_Children array",
                    transform_key.0, transform_key.1
                )
            })?;
        let mut distinct_children = BTreeSet::new();
        for (child_index, child) in children.iter().enumerate() {
            let child_key = exact_required_pointer_key(
                env,
                Some(child),
                &format!("Transform.m_Children[{child_index}]"),
            )?;
            if !distinct_children.insert(child_key) {
                return Err(format!(
                    "Transform {}#{} lists child {:?} more than once",
                    transform_key.0, transform_key.1, child_key
                ));
            }
            let child_transform = exact_object_body(env, child_key)?;
            if child_transform.object_type != "Transform" {
                return Err(format!(
                    "Transform child expected Transform, got {} at {}#{}",
                    child_transform.object_type, child_key.0, child_key.1
                ));
            }
            let child_game_object_key = exact_typed_dependency(
                env,
                child_transform.body.get("m_GameObject"),
                "GameObject",
                "child Transform.m_GameObject",
            )?;
            let child_game_object = exact_object_body(env, child_game_object_key)?;
            let child_name = super::super::object_name(&child_game_object.body);
            let child_path = if relative_path.is_empty() {
                child_name
            } else if child_name.is_empty() {
                relative_path.clone()
            } else {
                format!("{relative_path}/{child_name}")
            };
            runtime_animation_paths.insert(child_path.clone());
            hierarchy_edge_count += 1;
            pending.push((child_key, Some(transform_key), child_path));
        }
    }

    if !game_objects.contains(&container_key) {
        return Err("the exact container GameObject is not the hierarchy root".to_string());
    }

    let mut preload_game_objects = BTreeSet::new();
    let mut preload_transforms = BTreeSet::new();
    for key in preload_keys {
        let object = exact_object_body(env, *key)?;
        match object.object_type.as_str() {
            "GameObject" => {
                preload_game_objects.insert(*key);
            }
            "Transform" => {
                preload_transforms.insert(*key);
            }
            _ => {}
        }
    }
    validate_sparse_preload_hierarchy_membership(
        &preload_game_objects,
        &game_objects,
        &preload_transforms,
        &transforms,
    )?;

    let mut core_keys = BTreeSet::new();
    core_keys.extend(game_objects.iter().copied());
    core_keys.extend(transforms.iter().copied());
    core_keys.extend(component_keys.iter().copied());
    let mut meshes = BTreeSet::new();
    let mut materials = BTreeSet::new();
    let mut bones = BTreeSet::new();
    let mut animation_clips = BTreeSet::new();
    let mut mesh_filters_by_game_object = BTreeMap::<(usize, i64), Vec<(usize, i64)>>::new();
    let mut mesh_renderer_game_objects = Vec::new();
    let mut renderer_count = 0usize;
    let mut skinned_mesh_renderer_count = 0usize;
    let mut animation_component_count = 0usize;

    for component in &component_records {
        match component.object_type.as_str() {
            "MeshFilter" => {
                let mesh = exact_typed_dependency(
                    env,
                    component.body.get("m_Mesh"),
                    "Mesh",
                    "MeshFilter.m_Mesh",
                )?;
                meshes.insert(mesh);
                core_keys.insert(mesh);
                mesh_filters_by_game_object
                    .entry(component.game_object_key)
                    .or_default()
                    .push(mesh);
            }
            "MeshRenderer" | "SkinnedMeshRenderer" => {
                renderer_count += 1;
                let material_values = component
                    .body
                    .get("m_Materials")
                    .and_then(super::super::UnityValue::as_array)
                    .ok_or_else(|| {
                        format!(
                            "{} {}#{} has no exact m_Materials array",
                            component.object_type, component.key.0, component.key.1
                        )
                    })?;
                if material_values.is_empty() {
                    return Err(format!(
                        "{} {}#{} has no material",
                        component.object_type, component.key.0, component.key.1
                    ));
                }
                for (material_index, value) in material_values.iter().enumerate() {
                    let material = exact_typed_dependency(
                        env,
                        Some(value),
                        "Material",
                        &format!("{}.m_Materials[{material_index}]", component.object_type),
                    )?;
                    materials.insert(material);
                    core_keys.insert(material);
                }

                if component.object_type == "MeshRenderer" {
                    mesh_renderer_game_objects.push(component.game_object_key);
                } else {
                    skinned_mesh_renderer_count += 1;
                    let mesh = exact_typed_dependency(
                        env,
                        component.body.get("m_Mesh"),
                        "Mesh",
                        "SkinnedMeshRenderer.m_Mesh",
                    )?;
                    meshes.insert(mesh);
                    core_keys.insert(mesh);
                    let bone_values = component
                        .body
                        .get("m_Bones")
                        .and_then(super::super::UnityValue::as_array)
                        .ok_or_else(|| {
                            format!(
                                "SkinnedMeshRenderer {}#{} has no exact m_Bones array",
                                component.key.0, component.key.1
                            )
                        })?;
                    if bone_values.is_empty() {
                        return Err(format!(
                            "SkinnedMeshRenderer {}#{} has no bones",
                            component.key.0, component.key.1
                        ));
                    }
                    for (bone_index, value) in bone_values.iter().enumerate() {
                        let bone = exact_typed_dependency(
                            env,
                            Some(value),
                            "Transform",
                            &format!("SkinnedMeshRenderer.m_Bones[{bone_index}]"),
                        )?;
                        if !transforms.contains(&bone) {
                            return Err(format!(
                                "SkinnedMeshRenderer bone {:?} is outside the exact root hierarchy",
                                bone
                            ));
                        }
                        bones.insert(bone);
                        core_keys.insert(bone);
                    }
                    let root_bone = component
                        .body
                        .get("m_RootBone")
                        .map(|value| {
                            exact_nullable_pointer_key(
                                env,
                                Some(value),
                                "SkinnedMeshRenderer.m_RootBone",
                            )
                        })
                        .transpose()?
                        .flatten();
                    if let Some(root_bone) = root_bone {
                        if !transforms.contains(&root_bone) {
                            return Err(format!(
                                "SkinnedMeshRenderer root bone {:?} is outside the exact root hierarchy",
                                root_bone
                            ));
                        }
                        bones.insert(root_bone);
                        core_keys.insert(root_bone);
                    }
                }
            }
            "Animation" => {
                animation_component_count += 1;
                let mut clip_keys = BTreeSet::new();
                if let Some(value) = component.body.get("m_Animation") {
                    match value {
                        super::super::UnityValue::Pointer(pointer) if pointer.is_null() => {}
                        super::super::UnityValue::Pointer(_) => {
                            clip_keys.insert(exact_typed_dependency(
                                env,
                                Some(value),
                                "AnimationClip",
                                "Animation.m_Animation",
                            )?);
                        }
                        _ => return Err("Animation.m_Animation is not a pointer".to_string()),
                    }
                }
                if let Some(values) = component
                    .body
                    .get("m_Animations")
                    .and_then(super::super::UnityValue::as_array)
                {
                    for (clip_index, value) in values.iter().enumerate() {
                        let clip = exact_typed_dependency(
                            env,
                            Some(value),
                            "AnimationClip",
                            &format!("Animation.m_Animations[{clip_index}]"),
                        )?;
                        clip_keys.insert(clip);
                    }
                }
                if clip_keys.is_empty() {
                    return Err(format!(
                        "Animation {}#{} has no AnimationClip",
                        component.key.0, component.key.1
                    ));
                }
                for clip in clip_keys {
                    let clip_object = exact_object_body(env, clip)?;
                    let missing_paths = exact_animation_clip_paths(&clip_object.body)
                        .into_iter()
                        .filter(|path| !runtime_animation_paths.contains(path))
                        .collect::<Vec<_>>();
                    if !missing_paths.is_empty() {
                        return Err(format!(
                            "AnimationClip {}#{} targets {} paths outside the exact hierarchy: {}",
                            clip.0,
                            clip.1,
                            missing_paths.len(),
                            missing_paths
                                .into_iter()
                                .take(8)
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    animation_clips.insert(clip);
                    core_keys.insert(clip);
                }
            }
            _ => {}
        }
    }

    for game_object in mesh_renderer_game_objects {
        let filters = mesh_filters_by_game_object
            .get(&game_object)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if filters.len() != 1 {
            return Err(format!(
                "MeshRenderer GameObject {:?} must have exactly one MeshFilter mesh, found {}",
                game_object,
                filters.len()
            ));
        }
    }
    if renderer_count == 0 || meshes.is_empty() {
        return Err("exact GameObject hierarchy has no renderer-backed mesh".to_string());
    }
    if animation_component_count == 0 || animation_clips.is_empty() {
        return Err(
            "exact KFM GameObject hierarchy has no proven legacy animation clips".to_string(),
        );
    }

    let missing_from_root_closure = core_keys
        .difference(&root_closure)
        .copied()
        .collect::<Vec<_>>();
    if !missing_from_root_closure.is_empty() {
        return Err(format!(
            "{} exact model core objects are outside the root pointer closure: {:?}",
            missing_from_root_closure.len(),
            missing_from_root_closure
                .into_iter()
                .take(8)
                .collect::<Vec<_>>()
        ));
    }

    let core_key_evidence = core_keys
        .iter()
        .map(|key| resolved_object_key_evidence(env, *key).map_err(|err| err.detail))
        .collect::<Result<Vec<_>, _>>()?;
    let evidence = SelfContainedGameObjectEvidence {
        proof: "exactGameObjectTransformSubtreeAndRootPointerClosureContainment".to_string(),
        root_game_object: resolved_object_key_evidence(env, container_key)
            .map_err(|err| err.detail)?,
        root_transform: resolved_object_key_evidence(env, root_transform)
            .map_err(|err| err.detail)?,
        root_name,
        root_closure_key_count: root_closure.len(),
        preload_core_key_count: core_keys.intersection(preload_keys).count(),
        game_object_count: game_objects.len(),
        transform_count: transforms.len(),
        hierarchy_edge_count,
        component_count: component_keys.len(),
        renderer_count,
        skinned_mesh_renderer_count,
        mesh_count: meshes.len(),
        material_count: materials.len(),
        bone_count: bones.len(),
        animation_component_count,
        animation_clip_count: animation_clips.len(),
        null_parent_transform_count: 1,
        core_keys: core_key_evidence,
    };
    Ok(SelfContainedGameObjectProof {
        evidence,
        root_closure,
    })
}

pub(super) fn unity_value_kind(value: &super::super::UnityValue) -> &'static str {
    match value {
        super::super::UnityValue::Bool(_) => "bool",
        super::super::UnityValue::Int(_) => "int",
        super::super::UnityValue::UInt(_) => "uint",
        super::super::UnityValue::Float(_) => "float",
        super::super::UnityValue::String(_) => "string",
        super::super::UnityValue::Bytes(_) => "bytes",
        super::super::UnityValue::Array(_) => "array",
        super::super::UnityValue::Object(_) => "object",
        super::super::UnityValue::Pair(_, _) => "pair",
        super::super::UnityValue::Pointer(_) => "pointer",
    }
}

pub(super) fn is_retained_unity_system_pointer(
    env: &super::super::UnityEnvironment,
    pointer: &super::super::Pointer,
) -> bool {
    let Some(source_asset) = env.assets.get(pointer.source_asset) else {
        return false;
    };
    let Some(asset_ref) = usize::try_from(pointer.file_id)
        .ok()
        .and_then(|index| source_asset.asset_refs.get(index))
    else {
        return false;
    };
    [&asset_ref.file_path, &asset_ref.asset_path]
        .into_iter()
        .any(|value| is_unity_system_asset_ref(value))
}
