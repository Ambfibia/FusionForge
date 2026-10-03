use super::*;

pub(super) fn link_scene_instance(
    env: &UnityEnvironment,
    map_asset_names: &BTreeSet<String>,
    terrain: &TerrainIdentity,
    source: &TileSource,
) -> Result<SceneLink, String> {
    let mut colliders = BTreeMap::<(String, i64), (ObjectKey, UnityValue, Pointer)>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !map_asset_names.contains(&asset.name) {
            continue;
        }
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "TerrainCollider" {
                continue;
            }
            let body = asset.read_object(asset_index, info).map_err(|err| {
                format!(
                    "could not read TerrainCollider {}#{}: {err}",
                    asset.name, path_id
                )
            })?;
            let Some(pointer) = body
                .get("m_TerrainData")
                .and_then(UnityValue::as_pointer)
                .cloned()
            else {
                continue;
            };
            let Ok(resolved) = env.resolve_pointer(&pointer) else {
                continue;
            };
            let Some(resolved_asset) = env.assets.get(resolved.asset) else {
                continue;
            };
            if resolved_asset.name == terrain.asset_name && resolved.path_id == terrain.path_id {
                colliders.insert(
                    (asset.name.clone(), *path_id),
                    (
                        ObjectKey {
                            asset: asset_index,
                            path_id: *path_id,
                        },
                        body,
                        pointer,
                    ),
                );
            }
        }
    }
    if colliders.len() != 1 {
        return Err(format!(
            "expected exactly one map TerrainCollider linked to {}#{}, found {}",
            terrain.asset_name,
            terrain.path_id,
            colliders.len()
        ));
    }
    let (_, (collider_key, collider_body, terrain_pointer)) = colliders.into_iter().next().unwrap();
    let game_object_pointer = collider_body
        .get("m_GameObject")
        .and_then(UnityValue::as_pointer)
        .ok_or_else(|| {
            format!(
                "TerrainCollider pathId {} has no m_GameObject",
                collider_key.path_id
            )
        })?;
    let game_object_key = env
        .resolve_pointer(game_object_pointer)
        .map_err(|err| format!("could not resolve TerrainCollider GameObject: {err}"))?;
    let game_object = env.read_object(game_object_key).map_err(|err| {
        format!(
            "could not read TerrainCollider GameObject pathId {}: {err}",
            game_object_key.path_id
        )
    })?;
    let game_object_name = object_name(&game_object);
    let mut transforms = Vec::<(ObjectKey, UnityValue)>::new();
    let mut terrain_components = Vec::<(ObjectKey, UnityValue, Pointer, Pointer)>::new();
    let mut component_documents = Vec::<JsonValue>::new();
    let mut sidecars = Vec::<SceneSidecar>::new();
    let mut component_sidecar_names = BTreeMap::<String, String>::new();
    let mut semantic_blockers = Vec::<(&'static str, String)>::new();
    let mut gameplay_attributes = None;

    for (component_index, component) in value_array(game_object.get("m_Component"))
        .iter()
        .enumerate()
    {
        let pointer = component_pointer(component).ok_or_else(|| {
            format!(
                "Terrain owner GameObject {} component {component_index} has no pointer",
                game_object_key.path_id
            )
        })?;
        let key = env.resolve_pointer(pointer).map_err(|err| {
            format!("could not resolve Terrain owner component {component_index}: {err}")
        })?;
        let asset = env
            .assets
            .get(key.asset)
            .ok_or_else(|| format!("component asset index {} is missing", key.asset))?;
        let info = asset
            .objects
            .get(&key.path_id)
            .ok_or_else(|| format!("component {}#{} is missing", asset.name, key.path_id))?;
        let body = asset.read_object(key.asset, info).map_err(|err| {
            format!(
                "could not read component {}#{}: {err}",
                asset.name, key.path_id
            )
        })?;
        let object_type = asset.object_type_name(info);
        let script_pointer = body
            .get("m_Script")
            .and_then(UnityValue::as_pointer)
            .cloned();
        let script_name = script_pointer
            .as_ref()
            .and_then(|script| resolved_object_name(env, script));
        let linked_terrain_pointer = body
            .get("m_TerrainData")
            .and_then(UnityValue::as_pointer)
            .cloned();
        let links_exact_terrain = linked_terrain_pointer
            .as_ref()
            .is_some_and(|pointer| pointer_resolves_to(env, pointer, terrain.key));

        let role = if object_type == "Transform" {
            transforms.push((key, body.clone()));
            "transform"
        } else if key == collider_key {
            "terrainCollider"
        } else if links_exact_terrain {
            let pointer = linked_terrain_pointer.clone().unwrap();
            terrain_components.push((
                key,
                body.clone(),
                pointer,
                script_pointer.clone().unwrap_or(Pointer {
                    file_id: 0,
                    path_id: 0,
                    source_asset: key.asset,
                }),
            ));
            "terrainRenderer"
        } else if script_name.as_deref() == Some("MapAttributeTable") {
            "mapAttributeTable"
        } else if script_name.as_deref() == Some("TerrainDetailManager") {
            "terrainDetailManager"
        } else {
            semantic_blockers.push((
                "unresolvedTerrainOwnerComponentRole",
                format!(
                    "owner component {}#{} ({object_type}, script={:?}) has no proven semantic role",
                    asset.name, key.path_id, script_name
                ),
            ));
            "unresolved"
        };

        let raw = asset.object_raw_data(info).map_err(|err| {
            format!(
                "could not read raw component {}#{}: {err}",
                asset.name, key.path_id
            )
        })?;
        let parsed = serde_json::to_vec_pretty(&unity_to_json(&body))
            .map_err(|err| format!("could not encode component {}: {err}", key.path_id))?;
        let (raw_data, parsed_data) = match component_sidecar_stem(role) {
            Some(stem) => {
                register_case_folded_sidecar_stem(&mut component_sidecar_names, stem)?;
                let raw_path = format!("components/{stem}.raw.bin");
                let parsed_path = format!("components/{stem}.parsed.json");
                sidecars.push(SceneSidecar {
                    relative_path: raw_path.clone(),
                    bytes: raw.to_vec(),
                });
                sidecars.push(SceneSidecar {
                    relative_path: parsed_path.clone(),
                    bytes: parsed.clone(),
                });
                (
                    json!({
                        "path": raw_path,
                        "byteLength": raw.len(),
                        "blake3": hash_bytes(raw),
                    }),
                    json!({
                        "path": parsed_path,
                        "blake3": hash_bytes(&parsed),
                    }),
                )
            }
            None => (
                json!({
                    "status": "blockedUnresolvedSemanticRole",
                    "byteLength": raw.len(),
                    "blake3": hash_bytes(raw),
                }),
                json!({
                    "status": "blockedUnresolvedSemanticRole",
                    "blake3": hash_bytes(&parsed),
                }),
            ),
        };

        let mut component_document = json!({
            "componentIndex": component_index,
            "componentPointer": pointer_json(pointer),
            "assetName": asset.name,
            "pathId": key.path_id,
            "classId": info.class_id,
            "typeId": info.type_id,
            "typeName": object_type,
            "role": role,
            "scriptPointer": script_pointer.as_ref().map(pointer_json),
            "scriptTrueName": script_name,
            "rawData": raw_data,
            "parsedData": parsed_data,
        });
        if role == "terrainDetailManager" {
            component_document["scriptEvidence"] = json!({
                "workspacePath": "work/ilspy-b8c3-firstpass/TerrainDetailManager.cs",
                "sha256": "a49200ffa26c056768b01d101b0e21507ff6490fed52e1164518feb0c91cffd0",
                "fact": "SetupRenderOptions mutates the sibling Terrain render settings",
            });
        }
        if role == "mapAttributeTable" {
            let attributes = exact_byte_payload(body.get("attributes")).ok_or_else(|| {
                format!(
                    "MapAttributeTable {} has no exact byte attributes payload",
                    key.path_id
                )
            })?;
            let (width, height) = terrain_dimensions(env, terrain.key)?;
            let expected = (width as usize)
                .checked_mul(height as usize)
                .ok_or_else(|| "MapAttributeTable dimensions overflow usize".to_string())?;
            if attributes.len() != expected {
                return Err(format!(
                    "MapAttributeTable {} has {} bytes, expected {}x{}={expected}",
                    key.path_id,
                    attributes.len(),
                    width,
                    height
                ));
            }
            let png = encode_gray8_png(width, height, &attributes)?;
            let raw_json = serde_json::to_vec_pretty(&unity_to_json(&body))
                .map_err(|err| format!("could not encode MapAttributeTable: {err}"))?;
            sidecars.push(SceneSidecar {
                relative_path: "gameplay/attributes.png".to_string(),
                bytes: png.clone(),
            });
            sidecars.push(SceneSidecar {
                relative_path: "gameplay/attributes.bin".to_string(),
                bytes: attributes.clone(),
            });
            sidecars.push(SceneSidecar {
                relative_path: "gameplay/attributes.raw.json".to_string(),
                bytes: raw_json.clone(),
            });
            let contract = json!({
                "path": "gameplay/attributes.png",
                "width": width,
                "height": height,
                "sampleCount": attributes.len(),
                "pngColorType": "Gray8",
                "rawEncoding": "u8",
                "pixelOrder": "row-major: index = row * width + column",
                "rawPath": "gameplay/attributes.bin",
                "rawBlake3": hash_bytes(&attributes),
                "pngBlake3": hash_bytes(&png),
                "rawParsedDocument": {
                    "path": "gameplay/attributes.raw.json",
                    "blake3": hash_bytes(&raw_json),
                },
                "source": {
                    "pointerFileId": pointer.file_id,
                    "pointerPathId": pointer.path_id,
                    "resolvedAssetName": asset.name,
                    "resolvedPathId": key.path_id,
                    "scriptPointer": script_pointer.as_ref().map(pointer_json),
                    "scriptTrueName": "MapAttributeTable",
                    "decompiledEvidence": {
                        "workspacePath": "work/ilspy-b8c3-firstpass/MapAttributeTable.cs",
                        "sha256": "046526af51207b759c3ccfc238dd5ca564979c6895e652114595a4dffe986e7b",
                        "indexFormula": "attributes[x + z * heightmapWidth]",
                    }
                }
            });
            component_document["gameplayAttributes"] = contract.clone();
            gameplay_attributes = Some(contract);
        }
        component_documents.push(component_document);
    }

    if transforms.len() != 1 {
        return Err(format!(
            "TerrainCollider GameObject pathId {} has {} Transform components",
            game_object_key.path_id,
            transforms.len()
        ));
    }
    if terrain_components.len() != 1 {
        return Err(format!(
            "Terrain owner GameObject pathId {} has {} non-collider components whose m_TerrainData resolves to {}#{}",
            game_object_key.path_id,
            terrain_components.len(),
            terrain.asset_name,
            terrain.path_id
        ));
    }
    let (transform_key, transform) = transforms.into_iter().next().unwrap();
    let (terrain_component_key, terrain_component, terrain_component_pointer, _) =
        terrain_components.into_iter().next().unwrap();
    let unity_translation = strict_vec3(
        transform.get("m_LocalPosition"),
        "TerrainCollider owner m_LocalPosition",
    )?;
    let unity_rotation = strict_quaternion(
        transform.get("m_LocalRotation"),
        "TerrainCollider owner m_LocalRotation",
    )?;
    let unity_scale = strict_vec3(
        transform.get("m_LocalScale"),
        "TerrainCollider owner m_LocalScale",
    )?;
    let native_translation = unity_translation_to_native(unity_translation);
    let native_rotation = unity_rotation_to_native(unity_rotation);
    let parent_transform_path_id = transform
        .get("m_Father")
        .and_then(UnityValue::as_pointer)
        .map(|pointer| pointer.path_id);
    let collider_asset_name = env.assets[collider_key.asset].name.clone();
    let transform_asset_name = env.assets[transform_key.asset].name.clone();
    let terrain_resolved = env
        .resolve_pointer(&terrain_pointer)
        .map_err(|err| format!("could not re-resolve linked TerrainData: {err}"))?;
    let terrain_resolved_asset_name = env.assets[terrain_resolved.asset].name.clone();
    let (root_chain, map_tile_root_transform_path_id, root_chain_blake3) =
        resolved_transform_chain(env, transform_key, &transform)?;
    let terrain_component_asset_name = env.assets[terrain_component_key.asset].name.clone();
    let terrain_render_contract = terrain_render_contract(&terrain_component)?;
    let document = json!({
        "schema": SCENE_INSTANCE_SCHEMA,
        "tileId": source.tile_id,
        "terrainData": {
            "trueName": terrain.true_name,
            "sourceAssetName": terrain.asset_name,
            "sourcePathId": terrain.path_id,
        },
        "mapScene": source.map_scene,
        "linkage": {
            "terrainCollider": {
                "assetName": collider_asset_name,
                "pathId": collider_key.path_id,
                "contract": terrain_collider_contract(&collider_body)?,
            },
            "terrainDataPointer": {
                "fileId": terrain_pointer.file_id,
                "pathId": terrain_pointer.path_id,
                "resolvedAssetName": terrain_resolved_asset_name,
                "resolvedPathId": terrain_resolved.path_id,
            },
            "gameObject": {
                "assetName": env.assets[game_object_key.asset].name,
                "pathId": game_object_key.path_id,
                "trueName": game_object_name,
            },
            "transform": {
                "assetName": transform_asset_name,
                "pathId": transform_key.path_id,
                "parentTransformPathId": parent_transform_path_id,
            },
            "terrainComponent": {
                "assetName": terrain_component_asset_name,
                "pathId": terrain_component_key.path_id,
                "terrainDataPointer": pointer_json(&terrain_component_pointer),
                "renderContract": terrain_render_contract,
            },
        },
        "ownerComponents": component_documents,
        "coordinateContract": {
            "space": "native",
            "basis": "H=diag(-1,1,1)",
            "unitScale": "1-unity-unit-equals-1-bevy-unit",
        },
        "localTransform": {
            "translation": native_translation,
            "rotation": native_rotation,
            "scale": unity_scale,
        },
        "rootChain": root_chain,
        "mapTileRootTransformPathId": map_tile_root_transform_path_id,
        "rootChainBlake3": root_chain_blake3,
        "terrainDataLocalGeometryOnly": true,
        "transformIsNotBakedIntoHeightmap": true,
        "applicationOrder": "terrainDataLocalVertex -> owner localTransform -> each rootChain parent in owner-to-root order",
    });
    Ok(SceneLink {
        document,
        summary: SceneInstanceSummary {
            terrain_collider_path_id: collider_key.path_id,
            game_object_path_id: game_object_key.path_id,
            transform_path_id: transform_key.path_id,
            parent_transform_path_id,
            map_tile_root_transform_path_id,
            root_chain_blake3,
            native_local_translation: native_translation,
        },
        sidecars,
        gameplay_attributes,
        semantic_blockers,
    })
}

pub(super) fn pointer_resolves_to(env: &UnityEnvironment, pointer: &Pointer, expected: ObjectKey) -> bool {
    env.resolve_pointer(pointer)
        .is_ok_and(|key| key == expected)
}

pub(super) fn pointer_json(pointer: &Pointer) -> JsonValue {
    json!({
        "fileId": pointer.file_id,
        "pathId": pointer.path_id,
    })
}

pub(super) fn any_pointer(value: &UnityValue) -> Option<&Pointer> {
    match value {
        UnityValue::Pointer(pointer) => Some(pointer),
        _ => None,
    }
}

pub(super) fn resolved_transform_chain(
    env: &UnityEnvironment,
    owner_key: ObjectKey,
    owner_body: &UnityValue,
) -> Result<(JsonValue, i64, String), String> {
    let mut nodes = Vec::<JsonValue>::new();
    let mut order = Vec::<i64>::new();
    let mut visited = HashSet::<ObjectKey>::new();
    visited.insert(owner_key);
    let owner_father = owner_body
        .get("m_Father")
        .and_then(UnityValue::as_pointer)
        .cloned();
    let Some(owner_father) = owner_father.filter(|pointer| !pointer.is_null()) else {
        let canonical_bytes = serde_json::to_vec(&nodes)
            .map_err(|err| format!("could not hash root chain: {err}"))?;
        let chain_hash = hash_bytes(&canonical_bytes);
        return Ok((
            json!({
                "order": "immediateParentToRoot",
                "includesOwnerTransform": false,
                "compositionOrderTransformPathIds": order,
                "nodes": nodes,
                "mapTileRootTransformPathId": owner_key.path_id,
                "canonicalJsonBlake3": chain_hash,
            }),
            owner_key.path_id,
            chain_hash,
        ));
    };
    let mut key = env
        .resolve_pointer(&owner_father)
        .map_err(|err| format!("could not resolve owner Transform parent: {err}"))?;
    let mut body = env.read_object(key).map_err(|err| {
        format!(
            "could not read owner Transform parent {}: {err}",
            key.path_id
        )
    })?;
    loop {
        if !visited.insert(key) {
            return Err(format!(
                "Transform parent cycle detected at asset {} pathId {}",
                key.asset, key.path_id
            ));
        }
        if visited.len() > 256 {
            return Err("Transform parent chain exceeds 256 nodes".to_string());
        }
        let asset = env
            .assets
            .get(key.asset)
            .ok_or_else(|| format!("Transform asset index {} is missing", key.asset))?;
        let translation = strict_vec3(body.get("m_LocalPosition"), "Transform m_LocalPosition")?;
        let rotation = strict_quaternion(body.get("m_LocalRotation"), "Transform m_LocalRotation")?;
        let scale = strict_vec3(body.get("m_LocalScale"), "Transform m_LocalScale")?;
        let native_translation = unity_translation_to_native(translation);
        let native_rotation = unity_rotation_to_native(rotation);
        let game_object_pointer = body
            .get("m_GameObject")
            .and_then(UnityValue::as_pointer)
            .ok_or_else(|| format!("Transform {} has no m_GameObject", key.path_id))?;
        let game_object_key = env
            .resolve_pointer(game_object_pointer)
            .map_err(|err| format!("could not resolve Transform GameObject: {err}"))?;
        let game_object_name = env
            .read_object(game_object_key)
            .map(|value| object_name(&value))
            .map_err(|err| format!("could not read Transform GameObject: {err}"))?;
        let father = body
            .get("m_Father")
            .and_then(UnityValue::as_pointer)
            .cloned();
        nodes.push(json!({
            "assetName": asset.name,
            "transformPathId": key.path_id,
            "gameObject": {
                "assetName": env.assets[game_object_key.asset].name,
                "pathId": game_object_key.path_id,
                "trueName": game_object_name,
            },
            "unityLocalTransform": {
                "translation": translation,
                "rotation": rotation,
                "scale": scale,
            },
            "nativeLocalTransform": {
                "translation": native_translation,
                "rotation": native_rotation,
                "scale": scale,
            },
            "parentPointer": father.as_ref().map(pointer_json),
        }));
        order.push(key.path_id);
        let Some(father) = father.filter(|pointer| !pointer.is_null()) else {
            break;
        };
        let parent_key = env
            .resolve_pointer(&father)
            .map_err(|err| format!("could not resolve Transform parent: {err}"))?;
        let parent_asset = env
            .assets
            .get(parent_key.asset)
            .ok_or_else(|| format!("parent Transform asset {} is missing", parent_key.asset))?;
        let parent_info = parent_asset
            .objects
            .get(&parent_key.path_id)
            .ok_or_else(|| {
                format!(
                    "parent Transform {}#{} is missing",
                    parent_asset.name, parent_key.path_id
                )
            })?;
        if parent_asset.object_type_name(parent_info) != "Transform" {
            return Err(format!(
                "parent pointer resolves to {}, expected Transform",
                parent_asset.object_type_name(parent_info)
            ));
        }
        body = parent_asset.read_object(parent_key.asset, parent_info)?;
        key = parent_key;
    }
    let root_path_id = *order
        .last()
        .ok_or_else(|| "empty Transform root chain".to_string())?;
    let canonical_bytes =
        serde_json::to_vec(&nodes).map_err(|err| format!("could not hash root chain: {err}"))?;
    let chain_hash = hash_bytes(&canonical_bytes);
    Ok((
        json!({
            "order": "immediateParentToRoot",
            "includesOwnerTransform": false,
            "compositionOrderTransformPathIds": order,
            "nodes": nodes,
            "mapTileRootTransformPathId": root_path_id,
            "canonicalJsonBlake3": chain_hash,
        }),
        root_path_id,
        chain_hash,
    ))
}

pub(super) fn component_pointer(value: &UnityValue) -> Option<&Pointer> {
    match value {
        UnityValue::Pair(_, right) => right.as_pointer(),
        UnityValue::Array(items) if items.len() >= 2 => items[1].as_pointer(),
        _ => value.as_pointer(),
    }
}

pub(super) fn component_sidecar_stem(role: &str) -> Option<&'static str> {
    match role {
        "transform" => Some("transform"),
        "terrainRenderer" => Some("terrain_renderer"),
        "terrainCollider" => Some("terrain_collider"),
        "mapAttributeTable" => Some("map_attribute_table"),
        "terrainDetailManager" => Some("terrain_detail_manager"),
        _ => None,
    }
}

pub(super) fn register_case_folded_sidecar_stem(
    names: &mut BTreeMap<String, String>,
    stem: &str,
) -> Result<(), String> {
    let folded = stem.to_lowercase();
    if let Some(existing) = names.get(&folded) {
        return Err(format!(
            "case-insensitive component sidecar collision: {existing:?} and {stem:?}"
        ));
    }
    names.insert(folded, stem.to_string());
    Ok(())
}

pub(super) fn strict_vec3(value: Option<&UnityValue>, label: &str) -> Result<[f64; 3], String> {
    let object = value
        .and_then(UnityValue::as_object)
        .ok_or_else(|| format!("{label} is not an object"))?;
    Ok([
        strict_number(object.get("x"), &format!("{label}.x"))?,
        strict_number(object.get("y"), &format!("{label}.y"))?,
        strict_number(object.get("z"), &format!("{label}.z"))?,
    ])
}

pub(super) fn strict_quaternion(value: Option<&UnityValue>, label: &str) -> Result<[f64; 4], String> {
    let object = value
        .and_then(UnityValue::as_object)
        .ok_or_else(|| format!("{label} is not an object"))?;
    Ok([
        strict_number(object.get("x"), &format!("{label}.x"))?,
        strict_number(object.get("y"), &format!("{label}.y"))?,
        strict_number(object.get("z"), &format!("{label}.z"))?,
        strict_number(object.get("w"), &format!("{label}.w"))?,
    ])
}

pub(super) fn strict_number(value: Option<&UnityValue>, label: &str) -> Result<f64, String> {
    value
        .and_then(UnityValue::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{label} must be a finite number"))
}

pub(super) fn source_evidence(path: &Path) -> Result<SourceFileEvidence, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("could not read {}: {err}", path.display()))?;
    Ok(SourceFileEvidence {
        path: canonical_string(path)?,
        blake3: format!("blake3:{}", blake3::hash(&bytes).to_hex()),
    })
}

pub(super) fn build_publication_plan(
    manifest: &NativeTerrainBatchManifest,
    staging_root: &Path,
    source_output_root: &str,
) -> Result<JsonValue, String> {
    let mut entries = Vec::with_capacity(manifest.exported.len());
    for exported in &manifest.exported {
        let heightmap_path = format!("{}/heightmap.png", exported.terrain_root);
        let terrain_document_blake3 =
            hash_output_document(staging_root, &exported.terrain_document)?;
        let heightmap_blake3 = hash_output_document(staging_root, &heightmap_path)?;
        let (scene_instance_document, scene_instance_document_blake3) =
            match exported.scene_instance_document.as_deref() {
                Some(path) => {
                    let relative = path
                        .strip_prefix(&format!("{}/", exported.terrain_root))
                        .ok_or_else(|| {
                            format!(
                                "scene instance {path:?} is outside sourceRoot {:?}",
                                exported.terrain_root
                            )
                        })?;
                    (
                        Some(relative),
                        Some(hash_output_document(staging_root, path)?),
                    )
                }
                None => (None, None),
            };
        let (instance_id, destination_root) = match exported.scope {
            "worldMap" => (
                publication_instance_id("worldMap", &exported.tile_id)?,
                format!("assets/game/world/maps/map_{}/terrain", exported.tile_id),
            ),
            "tutorial" => (
                publication_instance_id("tutorial", &exported.tile_id)?,
                format!(
                    "assets/game/world/tutorial/terrain/tiles/tile_{}",
                    exported.tile_id
                ),
            ),
            other => return Err(format!("unknown publication scope {other:?}")),
        };
        let payloads = publication_payloads(
            &staging_root.join(path_from_forward_slashes(&exported.terrain_root)),
        )?;
        entries.push(json!({
            "scope": exported.scope,
            "instanceId": instance_id,
            "sourceRoot": exported.terrain_root,
            "destinationRoot": destination_root,
            "terrainDocument": "terrain.json",
            "terrainDocumentBlake3": terrain_document_blake3,
            "heightmapPath": "heightmap.png",
            "heightmapBlake3": heightmap_blake3,
            "sceneInstanceDocument": scene_instance_document,
            "sceneInstanceDocumentBlake3": scene_instance_document_blake3,
            "placementStatus": exported.placement_status,
            "sourceBundle": exported.resource_bundle.path,
            "payloads": payloads,
        }));
    }
    let blocked = manifest
        .blocked
        .iter()
        .map(|blocked| {
            let instance_id = match (blocked.scope, blocked.tile_id.as_deref()) {
                (Some("worldMap"), Some(tile_id)) => format!("map_{tile_id}"),
                (Some("tutorial"), Some(tile_id)) => format!("tile_{tile_id}"),
                (_, Some(tile_id)) => tile_id.to_string(),
                (_, None) => "batch".to_string(),
            };
            json!({
                "scope": blocked.scope.unwrap_or("batch"),
                "instanceId": instance_id,
                "stage": blocked.stage,
                "code": blocked.code,
                "message": blocked.message,
                "placementStatus": "blocked",
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema": PUBLICATION_PLAN_SCHEMA,
        "status": manifest.status,
        "sourceOutputRoot": source_output_root,
        "entries": entries,
        "blocked": blocked,
    }))
}

pub(super) fn publication_instance_id(scope: &str, tile_id: &str) -> Result<String, String> {
    match scope {
        "worldMap" => Ok(format!("map_{tile_id}")),
        "tutorial" => Ok(format!("tile_{tile_id}")),
        other => Err(format!("unknown publication scope {other:?}")),
    }
}
