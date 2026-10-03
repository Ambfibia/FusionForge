use super::*;

pub(super) fn find_source_track<'a>(
    kind: &str,
    encoding: &str,
    source_index: usize,
    translations: &'a [JsonValue],
    rotations: &'a [JsonValue],
    scales: &'a [JsonValue],
) -> Option<&'a JsonValue> {
    let tracks = match kind {
        "translation" => translations,
        "rotation" => rotations,
        "scale" => scales,
        _ => return None,
    };
    tracks.iter().find(|track| {
        track.get("sourceEncoding").and_then(JsonValue::as_str) == Some(encoding)
            && track.get("sourceIndex").and_then(JsonValue::as_u64)
                == u64::try_from(source_index).ok()
    })
}

pub(super) fn collect_transforms(
    env: &fusionforge::UnityEnvironment,
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> BTreeMap<ObjectKey, TransformNode> {
    let mut result = BTreeMap::new();
    let mut add_transform = |asset_index: usize,
                             selected_path_id: i64,
                             asset: &fusionforge::Asset,
                             info: &fusionforge::ObjectInfo| {
        if asset.object_type_name(info) != "Transform" {
            return;
        }
        let body = match asset.read_object(asset_index, info) {
            Ok(body) => body,
            Err(error) => {
                if std::env::var_os("FFONE_PROFILE_NPC_LEGACY_SELECTION").is_some() {
                    eprintln!(
                        "legacy NPC transform read failure: asset={} index={} pathId={} error={error}",
                        asset.name, asset_index, selected_path_id,
                    );
                }
                return;
            }
        };
        let key = (asset_index, selected_path_id);
        let game_object = resolved_key(env, body.get("m_GameObject"));
        let name = game_object
            .and_then(|key| read_object(env, key))
            .map(|value| fusionforge::object_name(&value))
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("Transform#{selected_path_id}"));
        result.insert(
            key,
            TransformNode {
                key,
                game_object,
                name,
                parent: resolved_key(env, body.get("m_Father")),
                translation: unity_vec3(body.get("m_LocalPosition"), [0.0, 0.0, 0.0]),
                rotation: unity_quat(body.get("m_LocalRotation"), [0.0, 0.0, 0.0, 1.0]),
                scale: unity_vec3(body.get("m_LocalScale"), [1.0, 1.0, 1.0]),
            },
        );
    };

    if let Some(selected_objects) = selected_objects {
        for &(asset_index, path_id) in selected_objects {
            let Some(asset) = env.assets.get(asset_index) else {
                continue;
            };
            let Some(info) = asset.objects.get(&path_id) else {
                continue;
            };
            // Format-7 NPC import bundles can expose an original PathID as an
            // alias for an object whose storage PathID was remapped while the
            // package was rebuilt. Parent/renderer PPtrs still resolve through
            // the alias, so the exact closure graph must use the selected key
            // rather than ObjectInfo::path_id or the hierarchy splits in two.
            add_transform(asset_index, path_id, asset, info);
        }
    } else {
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                add_transform(asset_index, info.path_id, asset, info);
            }
        }
    }
    result
}

pub(super) fn collect_character_root_candidates(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> BTreeMap<ObjectKey, CharacterRootCandidate> {
    let transform_by_game_object = transforms
        .values()
        .filter_map(|node| node.game_object.map(|game_object| (game_object, node.key)))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = BTreeMap::<ObjectKey, CharacterRootCandidate>::new();
    for_each_selected_object!(env, selected_objects, asset_index, asset, info, {
        if !matches!(
            asset.object_type_name(info).as_str(),
            "SkinnedMeshRenderer" | "MeshFilter"
        ) {
            continue;
        }
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        let Some(mesh) = resolved_key(env, body.get("m_Mesh")) else {
            continue;
        };
        if !selected_meshes.contains(&mesh) {
            continue;
        }
        let Some(game_object) = resolved_key(env, body.get("m_GameObject")) else {
            continue;
        };
        let Some(transform) = transform_by_game_object.get(&game_object).copied() else {
            continue;
        };
        let Some(root) = character_root(transform, transforms) else {
            continue;
        };
        let candidate = candidates.entry(root).or_default();
        candidate.meshes.insert(mesh);
        let mesh_name = read_object(env, mesh).map(|value| fusionforge::object_name(&value));
        for name in [
            transforms.get(&root).map(|node| node.name.as_str()),
            transforms.get(&transform).map(|node| node.name.as_str()),
            mesh_name.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let name = name.trim();
            if !name.is_empty() {
                candidate.searchable_names.insert(name.to_string());
            }
        }
    });
    candidates
}

pub(super) fn collect_skinned_renderers(
    env: &fusionforge::UnityEnvironment,
    selected_meshes: &BTreeSet<ObjectKey>,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    preferred_roots: Option<&BTreeSet<ObjectKey>>,
    selected_objects: Option<&BTreeSet<ObjectKey>>,
) -> Vec<SkinnedRenderer> {
    let transform_by_game_object = transforms
        .values()
        .filter_map(|node| node.game_object.map(|game_object| (game_object, node.key)))
        .collect::<BTreeMap<_, _>>();
    let mut result = Vec::new();
    for_each_selected_object!(env, selected_objects, asset_index, asset, info, {
        if asset.object_type_name(info) != "SkinnedMeshRenderer" {
            continue;
        }
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        let Some(mesh) = resolved_key(env, body.get("m_Mesh")) else {
            continue;
        };
        if !selected_meshes.is_empty() && !selected_meshes.contains(&mesh) {
            continue;
        }
        let game_object = resolved_key(env, body.get("m_GameObject"));
        let transform = game_object.and_then(|key| transform_by_game_object.get(&key).copied());
        if preferred_roots.is_some_and(|roots| {
            transform
                .and_then(|transform| character_root(transform, transforms))
                .is_none_or(|root| !roots.contains(&root))
        }) {
            continue;
        }
        let bones = fusionforge::value_array(body.get("m_Bones"))
            .iter()
            .filter_map(|value| resolved_key(env, Some(value)))
            .collect::<Vec<_>>();
        if bones.is_empty() {
            continue;
        }
        result.push(SkinnedRenderer {
            key: (asset_index, info.path_id),
            mesh,
            transform,
            bones,
            root_bone: resolved_key(env, body.get("m_RootBone")),
            body,
        });
    });
    result
}
