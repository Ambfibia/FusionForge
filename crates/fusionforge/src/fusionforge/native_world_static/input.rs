use super::*;

pub(super) fn collect_scene(env: &UnityEnvironment, map_id: &str) -> Result<SceneExtraction, String> {
    let scene_prefix = format!("BuildPlayer-{map_id}");
    let scene_asset_names = env
        .assets
        .iter()
        .filter(|asset| asset.name.starts_with(&scene_prefix))
        .map(|asset| asset.name.clone())
        .collect::<BTreeSet<_>>();
    if scene_asset_names.is_empty() {
        return Err(format!(
            "no serialized scene asset begins with {scene_prefix:?}"
        ));
    }

    let mut transforms = HashMap::<ObjectKey, TransformRecord>::new();
    let mut transform_to_game_object = HashMap::<ObjectKey, ObjectKey>::new();
    let mut game_objects_by_key = HashMap::<ObjectKey, GameObjectRecord>::new();
    let mut game_objects = BTreeMap::<String, GameObjectRecord>::new();
    let mut filter_by_game_object = HashMap::<ObjectKey, Vec<ObjectKey>>::new();
    let mut renderers_by_game_object = HashMap::<ObjectKey, Vec<ObjectKey>>::new();
    let mut colliders_by_game_object = HashMap::<ObjectKey, Vec<ObjectKey>>::new();
    let mut mono_by_game_object = HashMap::<ObjectKey, Vec<ObjectKey>>::new();
    let mut counts = ExportCounts {
        scene_assets: scene_asset_names.len(),
        ..ExportCounts::default()
    };

    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !scene_asset_names.contains(&asset.name) {
            continue;
        }
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "GameObject" {
                continue;
            }
            let key = ObjectKey {
                asset: asset_index,
                path_id: *path_id,
            };
            let body = asset.read_object(asset_index, info).map_err(|err| {
                format!(
                    "could not read scene GameObject {}#{}: {err}",
                    asset.name, path_id
                )
            })?;
            let mut components = Vec::new();
            let mut transform_keys = Vec::new();
            for component in value_array(body.get("m_Component")) {
                let pointer = component_pointer(component).ok_or_else(|| {
                    format!(
                        "{}#{} has a non-PPtr m_Component entry",
                        asset.name, path_id
                    )
                })?;
                let component_key = env.resolve_pointer(pointer).map_err(|err| {
                    format!(
                        "{}#{} component {}/{} cannot resolve: {err}",
                        asset.name, path_id, pointer.file_id, pointer.path_id
                    )
                })?;
                let component_type = object_type(env, component_key)?;
                if component_type == "Transform" {
                    transform_keys.push(component_key);
                }
                match component_type.as_str() {
                    "MeshFilter" => filter_by_game_object
                        .entry(key)
                        .or_default()
                        .push(component_key),
                    "MeshRenderer" | "SkinnedMeshRenderer" => renderers_by_game_object
                        .entry(key)
                        .or_default()
                        .push(component_key),
                    "MeshCollider" => colliders_by_game_object
                        .entry(key)
                        .or_default()
                        .push(component_key),
                    "MonoBehaviour" => mono_by_game_object
                        .entry(key)
                        .or_default()
                        .push(component_key),
                    _ => {}
                }
                components.push(ComponentRecord {
                    key: component_key,
                    object_type: component_type,
                });
            }
            if transform_keys.len() != 1 {
                return Err(format!(
                    "{}#{} has {} Transform components, expected exactly one",
                    asset.name,
                    path_id,
                    transform_keys.len()
                ));
            }
            let transform = transform_keys[0];
            let record = GameObjectRecord {
                key,
                name: object_name(&body),
                active: body
                    .get("m_IsActive")
                    .and_then(UnityValue::as_i64)
                    .unwrap_or(1)
                    != 0,
                layer: body.get("m_Layer").and_then(UnityValue::as_i64),
                tag: body.get("m_Tag").and_then(UnityValue::as_i64),
                components,
                transform,
            };
            let id = source_id(env, key);
            if game_objects.insert(id.clone(), record.clone()).is_some()
                || game_objects_by_key.insert(key, record).is_some()
            {
                return Err(format!("duplicate scene GameObject identity {id}"));
            }
        }
    }
    counts.scene_nodes = game_objects.len();

    for game_object in game_objects.values() {
        let transform_key = game_object.transform;
        let transform = env.read_object(transform_key).map_err(|err| {
            format!(
                "could not read scene Transform {}: {err}",
                source_id(env, transform_key)
            )
        })?;
        let pointed_game_object =
            object_key_from_optional_pointer(env, transform.get("m_GameObject"))?.ok_or_else(
                || {
                    format!(
                        "Transform {} has no m_GameObject",
                        source_id(env, transform_key)
                    )
                },
            )?;
        if pointed_game_object != game_object.key {
            return Err(format!(
                "Transform {} points to {}, expected {}",
                source_id(env, transform_key),
                source_id(env, pointed_game_object),
                source_id(env, game_object.key)
            ));
        }
        let parent = object_key_from_optional_pointer(env, transform.get("m_Father"))?;
        let authored = authored_transform(&transform)?;
        let local = compose_matrix(
            (
                authored.translation[0],
                authored.translation[1],
                authored.translation[2],
            ),
            (
                authored.rotation[0],
                authored.rotation[1],
                authored.rotation[2],
                authored.rotation[3],
            ),
            (authored.scale[0], authored.scale[1], authored.scale[2]),
        );
        if transform_to_game_object
            .insert(transform_key, game_object.key)
            .is_some()
        {
            return Err(format!(
                "duplicate Transform identity {}",
                source_id(env, transform_key)
            ));
        }
        transforms.insert(
            transform_key,
            TransformRecord {
                game_object: game_object.key,
                parent,
                local,
                authored,
            },
        );
    }

    for record in transforms.values() {
        if let Some(parent) = record.parent {
            if !transforms.contains_key(&parent) {
                return Err(format!(
                    "scene Transform {} references parent {} outside the complete scene graph",
                    source_id(env, game_objects_by_key[&record.game_object].transform),
                    source_id(env, parent)
                ));
            }
        } else {
            counts.scene_roots += 1;
        }
    }
    if counts.scene_roots == 0 {
        return Err("scene Transform graph has no root".to_string());
    }

    let mut world_cache = HashMap::<ObjectKey, Matrix4>::new();
    let mut world_visiting = HashSet::new();
    for transform in transforms.keys().copied().collect::<Vec<_>>() {
        world_matrix_for_transform(
            transform,
            &transforms,
            &mut world_cache,
            &mut world_visiting,
        )?;
    }
    let mut active_cache = HashMap::<ObjectKey, bool>::new();
    let mut active_visiting = HashSet::new();
    for game_object in game_objects_by_key.keys().copied().collect::<Vec<_>>() {
        effective_scene_active(
            game_object,
            &game_objects_by_key,
            &transforms,
            &transform_to_game_object,
            &mut active_cache,
            &mut active_visiting,
        )?;
    }

    let mut behaviour_components = Vec::<BehaviourComponent>::new();
    for game_object in game_objects.values() {
        let node = source_id(env, game_object.key);
        for component in &game_object.components {
            if !BEHAVIOUR_COMPONENT_TYPES.contains(&component.object_type.as_str()) {
                continue;
            }
            behaviour_components.push(BehaviourComponent {
                node: node.clone(),
                key: component.key,
                object_type: component.object_type.clone(),
                world_matrix: world_cache[&game_object.transform],
            });
        }
    }
    behaviour_components.sort_by(|left, right| {
        (
            &left.node,
            &left.object_type,
            left.key.asset,
            left.key.path_id,
        )
            .cmp(&(
                &right.node,
                &right.object_type,
                right.key.asset,
                right.key.path_id,
            ))
    });

    let mut hierarchy_nodes = Vec::with_capacity(game_objects.len());
    for game_object in game_objects.values() {
        let transform_record = transforms
            .get(&game_object.transform)
            .ok_or_else(|| "internal missing Transform record".to_string())?;
        let parent_id = transform_record
            .parent
            .map(|parent| {
                transform_to_game_object
                    .get(&parent)
                    .copied()
                    .ok_or_else(|| {
                        format!(
                            "parent Transform {} has no owning GameObject",
                            source_id(env, parent)
                        )
                    })
                    .map(|game_object| source_id(env, game_object))
            })
            .transpose()?;
        let world = *world_cache
            .get(&game_object.transform)
            .ok_or_else(|| "internal world-matrix cache miss".to_string())?;
        hierarchy_nodes.push(json!({
            "id": source_id(env, game_object.key),
            "kind": "sceneGameObject",
            "name": game_object.name,
            "source": source_identity(env, game_object.key)?,
            "parentId": parent_id,
            "activeSelf": game_object.active,
            "activeInHierarchy": active_cache[&game_object.key],
            "layer": game_object.layer,
            "tag": game_object.tag,
            "transform": {
                "source": source_identity(env, game_object.transform)?,
                "localNativeTrs": transform_record.authored,
                "worldNativeMatrix": world,
            },
            "components": game_object.components.iter().map(|component| json!({
                "source": source_identity(env, component.key).expect("validated component identity"),
                "type": component.object_type,
            })).collect::<Vec<_>>(),
        }));
    }

    let mut payloads = Vec::new();
    let mut selected_meshes = BTreeSet::new();
    let mut selected_material_objects = BTreeSet::new();
    let mut used_payload_ids = BTreeSet::new();
    for game_object in game_objects.values() {
        let world_matrix = world_cache[&game_object.transform];
        let active = active_cache[&game_object.key];
        add_game_object_payloads(
            env,
            game_object.key,
            &game_object.name,
            &source_id(env, game_object.key),
            world_matrix,
            active,
            filter_by_game_object.get(&game_object.key),
            renderers_by_game_object.get(&game_object.key),
            colliders_by_game_object.get(&game_object.key),
            &mut payloads,
            &mut selected_meshes,
            &mut selected_material_objects,
            &mut used_payload_ids,
            &mut counts,
            "scene",
        )?;
    }

    for game_object in game_objects.values() {
        let Some(behaviours) = mono_by_game_object.get(&game_object.key) else {
            continue;
        };
        for behaviour_key in behaviours {
            let behaviour = env.read_object(*behaviour_key).map_err(|err| {
                format!(
                    "could not read MonoBehaviour {}: {err}",
                    source_id(env, *behaviour_key)
                )
            })?;
            if let Some(pointer) = behaviour.get("nifObject").and_then(UnityValue::as_pointer) {
                let root = env.resolve_pointer(pointer).map_err(|err| {
                    format!(
                        "MonoBehaviour {} nifObject cannot resolve: {err}",
                        source_id(env, *behaviour_key)
                    )
                })?;
                if object_type(env, root)? != "GameObject" {
                    return Err(format!(
                        "MonoBehaviour {} nifObject resolves to {}, not GameObject",
                        source_id(env, *behaviour_key),
                        object_type(env, root)?
                    ));
                }
                counts.scripted_prefab_roots += 1;
                let mut visited = HashSet::new();
                collect_prefab_instance(
                    env,
                    root,
                    world_cache[&game_object.transform],
                    active_cache[&game_object.key],
                    true,
                    &format!("prefab:{}", source_id(env, *behaviour_key)),
                    Some(source_id(env, game_object.key)),
                    &mut visited,
                    &mut hierarchy_nodes,
                    &mut payloads,
                    &mut selected_meshes,
                    &mut selected_material_objects,
                    &mut used_payload_ids,
                    &mut counts,
                )?;
            }
            collect_combined_meshes(
                env,
                &behaviour,
                *behaviour_key,
                game_object,
                world_cache[&game_object.transform],
                active_cache[&game_object.key],
                &transforms,
                &game_objects_by_key,
                &mut world_cache,
                &mut payloads,
                &mut selected_meshes,
                &mut selected_material_objects,
                &mut used_payload_ids,
                &mut counts,
            )?;
        }
    }

    Ok(SceneExtraction {
        scene_asset_names,
        game_objects,
        transforms,
        transform_to_game_object,
        payloads,
        hierarchy_nodes,
        behaviour_components,
        selected_meshes,
        selected_material_objects,
        counts,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_combined_meshes(
    env: &UnityEnvironment,
    behaviour: &UnityValue,
    behaviour_key: ObjectKey,
    owner: &GameObjectRecord,
    owner_world: Matrix4,
    owner_active: bool,
    transforms: &HashMap<ObjectKey, TransformRecord>,
    game_objects: &HashMap<ObjectKey, GameObjectRecord>,
    world_cache: &mut HashMap<ObjectKey, Matrix4>,
    payloads: &mut Vec<PendingPayload>,
    selected_meshes: &mut BTreeSet<(usize, i64)>,
    selected_material_objects: &mut BTreeSet<(usize, i64)>,
    used_payload_ids: &mut BTreeSet<String>,
    counts: &mut ExportCounts,
) -> Result<(), String> {
    for (combine_index, combine) in value_array(behaviour.get("combine")).iter().enumerate() {
        for (mesh_index, value) in value_array(combine.get("meshes")).iter().enumerate() {
            let pointer = value.as_pointer().ok_or_else(|| {
                format!(
                    "MonoBehaviour {} combine[{combine_index}].meshes[{mesh_index}] is not a PPtr",
                    source_id(env, behaviour_key)
                )
            })?;
            let filter_key = env.resolve_pointer(pointer).map_err(|err| {
                format!(
                    "MonoBehaviour {} combined MeshFilter cannot resolve: {err}",
                    source_id(env, behaviour_key)
                )
            })?;
            if object_type(env, filter_key)? != "MeshFilter" {
                return Err(format!(
                    "MonoBehaviour {} combine pointer resolves to {}, not MeshFilter",
                    source_id(env, behaviour_key),
                    object_type(env, filter_key)?
                ));
            }
            let filter = env.read_object(filter_key)?;
            let game_object_key =
                object_key_from_optional_pointer(env, filter.get("m_GameObject"))?.ok_or_else(
                    || {
                        format!(
                            "combined MeshFilter {} has no GameObject",
                            source_id(env, filter_key)
                        )
                    },
                )?;
            let go_body = env.read_object(game_object_key)?;
            let mut renderers = Vec::new();
            for component in value_array(go_body.get("m_Component")) {
                let Some(pointer) = component_pointer(component) else {
                    continue;
                };
                let key = env.resolve_pointer(pointer)?;
                if matches!(
                    object_type(env, key)?.as_str(),
                    "MeshRenderer" | "SkinnedMeshRenderer"
                ) {
                    renderers.push(key);
                }
            }
            if renderers.is_empty() {
                return Err(format!(
                    "combined MeshFilter {} has no renderer",
                    source_id(env, filter_key)
                ));
            }
            if game_objects.contains_key(&game_object_key) {
                // A renderer owned by the serialized scene was already
                // collected with its exact hierarchy activity and world
                // transform. The legacy `combine` list references that same
                // source geometry; publishing it again would create a
                // duplicate runtime surface.
                continue;
            }
            let (matrix, active, node_id) =
                if let Some(scene_go) = game_objects.get(&game_object_key) {
                    let mut visiting = HashSet::new();
                    let matrix = world_matrix_for_transform(
                        scene_go.transform,
                        transforms,
                        world_cache,
                        &mut visiting,
                    )?;
                    (
                        matrix,
                        scene_go.active && owner_active,
                        source_id(env, game_object_key),
                    )
                } else {
                    (
                        owner_world,
                        owner_active,
                        format!(
                            "combined:{}:{}",
                            source_id(env, behaviour_key),
                            source_id(env, game_object_key)
                        ),
                    )
                };
            add_game_object_payloads(
                env,
                game_object_key,
                &object_name(&go_body),
                &node_id,
                matrix,
                active,
                Some(&vec![filter_key]),
                Some(&renderers),
                None,
                payloads,
                selected_meshes,
                selected_material_objects,
                used_payload_ids,
                counts,
                &format!("combine:{}", source_id(env, behaviour_key)),
            )?;
        }
    }
    let _ = owner;
    Ok(())
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("u32 at offset {offset} is out of bounds"))?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}

/// Resolve a `MonoBehaviour`'s `m_Script` pointer to a stable script identity.
///
/// A null or unresolvable pointer is reported as an unresolved script rather
/// than failing the export: the behaviour state is still exact evidence, and
/// the runtime must refuse to act on a record it cannot classify.
pub(super) fn resolve_script_identity(
    env: &UnityEnvironment,
    body: &UnityValue,
    scripts: &mut BTreeMap<String, JsonValue>,
) -> Result<Option<String>, String> {
    let UnityValue::Pointer(pointer) = body
        .get("m_Script")
        .ok_or_else(|| "MonoBehaviour has no m_Script field".to_string())?
    else {
        return Err("MonoBehaviour m_Script is not a PPtr".to_string());
    };
    if pointer.path_id == 0 {
        return Ok(None);
    }
    let Ok(script_key) = env.resolve_pointer(pointer) else {
        return Ok(None);
    };
    let id = source_id(env, script_key);
    if !scripts.contains_key(&id) {
        let script = env
            .read_object(script_key)
            .map_err(|err| format!("could not read MonoScript {id}: {err}"))?;
        let text = |key: &str| {
            script
                .get(key)
                .and_then(|value| match value {
                    UnityValue::String(value) => Some(value.clone()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        let class_name = {
            let explicit = text("m_ClassName");
            if explicit.is_empty() {
                object_name(&script)
            } else {
                explicit
            }
        };
        scripts.insert(
            id.clone(),
            json!({
                "id": id,
                "className": class_name,
                "namespace": text("m_Namespace"),
                "assemblyName": text("m_AssemblyName"),
                "source": source_json(env, script_key)?,
            }),
        );
    }
    Ok(Some(id))
}

pub(super) fn collect_regular_files(root: &Path) -> Result<Vec<String>, String> {
    let root = canonical_directory(root, "output scan root")?;
    let mut pending = vec![root.clone()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|err| format!("could not enumerate {}: {err}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("could not enumerate {}: {err}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|err| format!("could not inspect {}: {err}", path.display()))?;
            if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
                return Err(format!(
                    "refusing symlink/reparse point in output: {}",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(&root)
                    .map_err(|_| format!("{} escaped output root", path.display()))?;
                files.push(slash_path(relative));
            } else {
                return Err(format!(
                    "refusing non-file/non-directory output entry {}",
                    path.display()
                ));
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn resolve_tile_layout(asset_root: &Path, tile: [i32; 2]) -> Result<TileLayout, String> {
    let registry_relative = "_runtime/world.json";
    let registry_path = join_relative(asset_root, registry_relative)?;
    let registry_bytes = fs::read(&registry_path).map_err(|err| {
        format!(
            "could not read runtime world registry {}: {err}",
            registry_path.display()
        )
    })?;
    let registry: JsonValue = serde_json::from_slice(&registry_bytes).map_err(|err| {
        format!(
            "invalid runtime world registry {}: {err}",
            registry_path.display()
        )
    })?;
    if registry.get("schema").and_then(JsonValue::as_str) != Some("ffone.runtime-world.v1") {
        return Err(format!(
            "runtime world registry {} is not ffone.runtime-world.v1",
            registry_path.display()
        ));
    }
    let entries = registry
        .get("entries")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "runtime world registry has no entries array".to_string())?;
    let mut matches = entries
        .iter()
        .filter(|entry| entry.get("tile") == Some(&json!(tile)));
    let entry = matches
        .next()
        .ok_or_else(|| format!("runtime world registry has no tile {:?}", tile))?;
    if matches.next().is_some() {
        return Err(format!(
            "runtime world registry has more than one tile {:?}",
            tile
        ));
    }
    let scope = entry
        .get("scope")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("runtime world tile {:?} has no scope", tile))?
        .to_string();
    let tile_id = entry
        .get("id")
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("runtime world tile {:?} has no id", tile))?
        .to_string();
    let scene_relative = entry
        .get("scene")
        .and_then(|scene| scene.get("path"))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("runtime world tile {tile_id} has no scene path"))?;
    let source_tile_relative = scene_relative
        .strip_suffix("/scene.json")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!("runtime world tile {tile_id} scene path {scene_relative} is not a tile scene")
        })?
        .to_string();
    let (model_relative_root, static_relative_root) = match scope.as_str() {
        "tutorial" => (
            format!("models/world/tutorial/{tile_id}"),
            format!("world/tutorial/static/tiles/{tile_id}"),
        ),
        "worldMap" => (
            format!("models/world/maps/{tile_id}"),
            format!("world/maps/static/tiles/{tile_id}"),
        ),
        other => return Err(format!("unsupported runtime world scope {other:?}")),
    };
    Ok(TileLayout {
        scope,
        tile_id,
        source_tile_relative,
        model_relative_root,
        static_relative_root,
    })
}

pub(super) fn resolve_case_exact(root: &Path, relative: &str) -> Result<PathBuf, String> {
    validate_relative_path(relative, None)?;
    let mut current = canonical_directory(root, "case-exact root")?;
    for wanted in Path::new(relative).components() {
        let Component::Normal(wanted) = wanted else {
            return Err(format!("invalid relative component in {relative:?}"));
        };
        let entries = fs::read_dir(&current)
            .map_err(|err| format!("could not enumerate {}: {err}", current.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("could not enumerate {}: {err}", current.display()))?;
        let exact = entries
            .iter()
            .find(|entry| entry.file_name() == wanted)
            .map(|entry| entry.path());
        let Some(next) = exact else {
            let folded = wanted.to_string_lossy().to_lowercase();
            if let Some(actual) = entries
                .iter()
                .find(|entry| entry.file_name().to_string_lossy().to_lowercase() == folded)
            {
                return Err(format!(
                    "case mismatch resolving {relative:?}: requested {:?}, actual {:?}",
                    wanted,
                    actual.file_name()
                ));
            }
            return Err(format!(
                "missing case-exact output path {relative:?} below {}",
                current.display()
            ));
        };
        let metadata = fs::symlink_metadata(&next)
            .map_err(|err| format!("could not inspect {}: {err}", next.display()))?;
        if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
            return Err(format!(
                "case-exact path traverses a symlink/reparse point: {}",
                next.display()
            ));
        }
        current = next;
    }
    Ok(current)
}
