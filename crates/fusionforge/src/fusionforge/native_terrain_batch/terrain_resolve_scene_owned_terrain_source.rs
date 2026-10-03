use super::*;

pub(super) fn resolve_scene_owned_terrain_source(
    loaded: &LoadedTile,
    source: &TileSource,
    archive_index: &HashMap<String, PathBuf>,
    nominal_identity: TerrainIdentity,
) -> Result<ResolvedTerrainSource, String> {
    let mut collider_targets = Vec::<(String, i64, Pointer, ObjectKey)>::new();
    for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
        if !loaded.map_asset_names.contains(&asset.name) {
            continue;
        }
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "TerrainCollider" {
                continue;
            }
            let body = asset.read_object(asset_index, info).map_err(|err| {
                format!(
                    "could not read map TerrainCollider {}#{}: {err}",
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
            let Ok(target) = loaded.env.resolve_pointer(&pointer) else {
                continue;
            };
            let Some(target_asset) = loaded.env.assets.get(target.asset) else {
                continue;
            };
            let Some(target_info) = target_asset.objects.get(&target.path_id) else {
                continue;
            };
            if target_asset.object_type_name(target_info) != "TerrainData" {
                continue;
            }
            collider_targets.push((asset.name.clone(), *path_id, pointer, target));
        }
    }
    collider_targets.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    if collider_targets.len() != 1 {
        return Ok(ResolvedTerrainSource {
            identity: nominal_identity,
            resource: source.resource.clone(),
            resource_path: source.resource_path.clone(),
        });
    }

    let (collider_asset_name, collider_path_id, terrain_pointer, target_key) =
        collider_targets.into_iter().next().unwrap();
    if !scene_dependency_alias_required(nominal_identity.key, target_key) {
        return Ok(ResolvedTerrainSource {
            identity: nominal_identity,
            resource: source.resource.clone(),
            resource_path: source.resource_path.clone(),
        });
    }

    let target_asset = loaded.env.assets.get(target_key.asset).ok_or_else(|| {
        format!(
            "scene TerrainData asset index {} is missing",
            target_key.asset
        )
    })?;
    let dependency_key = normalize_bundle_name(&target_asset.name);
    let dependency_path = archive_index.get(&dependency_key).ok_or_else(|| {
        format!(
            "scene TerrainCollider {}#{} points to {}#{}, but no exact build archive owns that CAB",
            collider_asset_name, collider_path_id, target_asset.name, target_key.path_id
        )
    })?;
    let dependency_path = fs::canonicalize(dependency_path)
        .map_err(|err| format!("{}: {err}", dependency_path.display()))?;
    let dependency_resource = source_evidence(&dependency_path)?;
    let dependency_asset_names = BTreeSet::from([target_asset.name.clone()]);
    let mut identity =
        exact_owned_terrain_identity(&loaded.env, &dependency_asset_names, source).map_err(
            |err| {
                format!(
                    "scene TerrainCollider {}#{} target {}#{} has no exact source-owned TerrainData route in {}: {err}",
                    collider_asset_name,
                    collider_path_id,
                    target_asset.name,
                    target_key.path_id,
                    dependency_path.display()
                )
            },
        )?;
    if identity.key != target_key {
        return Err(format!(
            "scene TerrainCollider {}#{} target {}#{} differs from exact dependency route {}#{}",
            collider_asset_name,
            collider_path_id,
            target_asset.name,
            target_key.path_id,
            identity.asset_name,
            identity.path_id
        ));
    }

    let nominal_raw_blake3 = raw_object_blake3(&loaded.env, nominal_identity.key)?;
    let collider_key = loaded
        .env
        .assets
        .iter()
        .position(|asset| asset.name == collider_asset_name)
        .map(|asset| ObjectKey {
            asset,
            path_id: collider_path_id,
        })
        .ok_or_else(|| format!("map TerrainCollider asset {collider_asset_name} disappeared"))?;
    let collider_raw_blake3 = raw_object_blake3(&loaded.env, collider_key)?;
    let target_raw_blake3 = raw_object_blake3(&loaded.env, target_key)?;
    identity.provenance_findings.push(ProvenanceFinding {
        code: "sceneDependencyAlias",
        message: format!(
            "nominal {} route {}#{} raw {} is not referenced by the map scene; {}#{} m_TerrainData fileId={} pathId={} raw {} resolves exactly to {}#{} raw {} in {} ({})",
            source
                .resource_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("<source>"),
            nominal_identity.asset_name,
            nominal_identity.path_id,
            nominal_raw_blake3,
            collider_asset_name,
            collider_path_id,
            terrain_pointer.file_id,
            terrain_pointer.path_id,
            collider_raw_blake3,
            identity.asset_name,
            identity.path_id,
            target_raw_blake3,
            dependency_path.display(),
            dependency_resource.blake3,
        ),
    });
    Ok(ResolvedTerrainSource {
        identity,
        resource: dependency_resource,
        resource_path: dependency_path,
    })
}

pub(super) fn terrain_tile_id_from_container_route(route: &str) -> Option<String> {
    let tile_id = route
        .strip_prefix("generateddongs/terraindata_")?
        .strip_suffix(".asset")?;
    let (x, y) = tile_id.split_once('_')?;
    if y.contains('_') || !valid_tile_component(x) || !valid_tile_component(y) {
        return None;
    }
    Some(format!("{x}_{y}"))
}

pub(super) fn terrain_dimensions(
    env: &UnityEnvironment,
    terrain_key: ObjectKey,
) -> Result<(u32, u32), String> {
    let body = env
        .read_object(terrain_key)
        .map_err(|err| format!("could not read TerrainData dimensions: {err}"))?;
    let heightmap = body
        .get("m_Heightmap")
        .and_then(UnityValue::as_object)
        .ok_or_else(|| "TerrainData has no m_Heightmap".to_string())?;
    let width = heightmap
        .get("m_Width")
        .and_then(UnityValue::as_i64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| "TerrainData m_Heightmap.m_Width is invalid".to_string())?;
    let height = heightmap
        .get("m_Height")
        .and_then(UnityValue::as_i64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| "TerrainData m_Heightmap.m_Height is invalid".to_string())?;
    Ok((width, height))
}

pub(super) fn terrain_render_contract(body: &UnityValue) -> Result<JsonValue, String> {
    let required = [
        "m_CastShadows",
        "m_HeightmapMaximumLOD",
        "m_HeightmapPixelError",
        "m_RenderMode",
        "m_SplatMapDistance",
        "m_DetailObjectDistance",
        "m_TreeBillboardDistance",
        "m_TreeCrossFadeLength",
        "m_TreeDistance",
        "m_TreeMaximumFullLODCount",
        "m_UseLightmap",
    ];
    let mut fields = serde_json::Map::new();
    for field in required {
        let value = body
            .get(field)
            .ok_or_else(|| format!("Terrain component is missing required field {field}"))?;
        fields.insert(field.to_string(), unity_to_json(value));
    }
    for optional in ["m_BasemapDistance", "m_DebugDrawMainCamera", "m_Enabled"] {
        if let Some(value) = body.get(optional) {
            fields.insert(optional.to_string(), unity_to_json(value));
        }
    }
    Ok(json!({
        "selection": "owner component whose m_TerrainData resolves to the exact routed TerrainData",
        "serializedFields": fields,
    }))
}

pub(super) fn terrain_collider_contract(body: &UnityValue) -> Result<JsonValue, String> {
    let terrain_data = body
        .get("m_TerrainData")
        .and_then(UnityValue::as_pointer)
        .ok_or_else(|| "TerrainCollider has no m_TerrainData pointer".to_string())?;
    let create_tree_colliders = body
        .get("m_CreateTreeColliders")
        .ok_or_else(|| "TerrainCollider has no m_CreateTreeColliders".to_string())?;
    let is_trigger = body
        .get("m_IsTrigger")
        .ok_or_else(|| "TerrainCollider has no m_IsTrigger".to_string())?;
    let material = body
        .get("m_Material")
        .and_then(any_pointer)
        .ok_or_else(|| "TerrainCollider has no m_Material pointer".to_string())?;
    Ok(json!({
        "terrainDataPointer": pointer_json(terrain_data),
        "createTreeColliders": unity_to_json(create_tree_colliders),
        "isTrigger": unity_to_json(is_trigger),
        "materialPointer": pointer_json(material),
        "enabled": body.get("m_Enabled").map(unity_to_json),
    }))
}
