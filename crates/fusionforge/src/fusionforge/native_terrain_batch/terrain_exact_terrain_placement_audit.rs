use super::*;

#[derive(Clone, Debug, Default)]
pub struct NativeTerrainBatchOptions {
    /// Empty means every exact DongResources tile in the build.
    pub tile_filter: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTerrainBatchManifest {
    pub schema: &'static str,
    pub status: &'static str,
    pub scope: BatchScope,
    pub source: BatchSource,
    pub output_root: String,
    pub counts: BatchCounts,
    pub exported: Vec<BatchExported>,
    pub blocked: Vec<BatchBlocked>,
    pub publication_plan_document: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTerrainEnrichmentSummary {
    pub schema: &'static str,
    pub status: &'static str,
    pub source_batch_root: String,
    pub output_root: String,
    pub source_manifest_blake3_before: String,
    pub source_manifest_blake3_after: String,
    pub source_publication_plan_blake3_before: String,
    pub source_publication_plan_blake3_after: String,
    pub scanned_scene_count: usize,
    pub environment_document_count: usize,
    pub exact_ambience_count: usize,
    pub exact_terrain_detail_count: usize,
    pub exact_placement_count: usize,
    pub source_resource_scene_mismatch_count: usize,
    pub blocker_count: usize,
    pub hard_linked_file_count: usize,
    pub hard_linked_byte_count: u64,
}

#[derive(Clone, Debug)]
pub(super) struct TerrainIdentity {
    pub(super) true_name: String,
    pub(super) asset_name: String,
    pub(super) path_id: i64,
    pub(super) key: ObjectKey,
    pub(super) source_container_route: String,
    pub(super) asset_route_tile_id: String,
    pub(super) provenance_findings: Vec<ProvenanceFinding>,
}

#[derive(Debug)]
pub(super) struct ResolvedTerrainSource {
    pub(super) identity: TerrainIdentity,
    pub(super) resource: SourceFileEvidence,
    pub(super) resource_path: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct PublishedTerrainIdentity {
    pub(super) true_name: String,
    pub(super) asset_name: String,
    pub(super) path_id: i64,
    pub(super) source_container_route: String,
    pub(super) asset_route_tile_id: String,
    pub(super) source_resource_bundle: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TerrainScope {
    WorldMap,
    Tutorial,
}

impl TerrainScope {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::WorldMap => "worldMap",
            Self::Tutorial => "tutorial",
        }
    }
}

pub(super) fn published_terrain_identities(
    source_batch_root: &Path,
    manifest: &JsonValue,
    candidates: &[TileSource],
) -> Result<BTreeMap<(String, String), PublishedTerrainIdentity>, String> {
    let candidates = candidates
        .iter()
        .map(|source| {
            (
                (source.scope.label().to_string(), source.tile_id.clone()),
                source,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let exported = manifest
        .get("exported")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "source batch manifest has no exported array".to_string())?;
    let mut identities = BTreeMap::new();
    for entry in exported {
        let scope = required_json_str(entry, "scope", "manifest exported entry")?;
        let tile_id = required_json_str(entry, "tileId", "manifest exported entry")?;
        let terrain_document =
            required_json_str(entry, "terrainDocument", "manifest exported entry")?;
        let source = candidates
            .get(&(scope.to_string(), tile_id.to_string()))
            .ok_or_else(|| format!("no build candidate for {scope} {tile_id}"))?;
        let path = source_batch_root.join(path_from_forward_slashes(terrain_document));
        let terrain: JsonValue = serde_json::from_slice(
            &fs::read(&path).map_err(|err| format!("could not read {}: {err}", path.display()))?,
        )
        .map_err(|err| format!("could not parse {}: {err}", path.display()))?;
        let source_object = terrain
            .get("source")
            .and_then(JsonValue::as_object)
            .ok_or_else(|| format!("{} has no source object", path.display()))?;
        let identity = PublishedTerrainIdentity {
            true_name: required_json_str(&terrain, "trueName", "terrain descriptor")?.to_string(),
            asset_name: source_object
                .get("assetName")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| format!("{} source has no assetName", path.display()))?
                .to_string(),
            path_id: source_object
                .get("terrainDataPathId")
                .and_then(JsonValue::as_i64)
                .ok_or_else(|| format!("{} source has no terrainDataPathId", path.display()))?,
            source_container_route: required_json_str(
                entry,
                "sourceContainerRoute",
                "manifest exported entry",
            )?
            .to_string(),
            asset_route_tile_id: required_json_str(
                entry,
                "assetRouteTileId",
                "manifest exported entry",
            )?
            .to_string(),
            source_resource_bundle: source_object
                .get("bundle")
                .and_then(JsonValue::as_object)
                .and_then(|bundle| bundle.get("path"))
                .and_then(JsonValue::as_str)
                .ok_or_else(|| format!("{} source bundle has no path", path.display()))?
                .to_string(),
        };
        let key = (source.scope.label().to_string(), source.tile_id.clone());
        if identities.insert(key.clone(), identity).is_some() {
            return Err(format!(
                "duplicate published TerrainData identity for {} {}",
                key.0, key.1
            ));
        }
    }
    Ok(identities)
}

pub(super) fn sibling_terrain_render_components(
    env: &UnityEnvironment,
    map_asset_names: &BTreeSet<String>,
    manager: &ExactSceneObject,
) -> Result<Vec<ExactSceneObject>, String> {
    let game_object_pointer = manager
        .body
        .get("m_GameObject")
        .and_then(UnityValue::as_pointer)
        .ok_or_else(|| "TerrainDetailManager has no m_GameObject pointer".to_string())?;
    let game_object_key = env
        .resolve_pointer(game_object_pointer)
        .map_err(|err| format!("could not resolve TerrainDetailManager GameObject: {err}"))?;
    let game_object = env.read_object(game_object_key).map_err(|err| {
        format!(
            "could not read TerrainDetailManager GameObject {}: {err}",
            game_object_key.path_id
        )
    })?;
    let mut matches = Vec::new();
    for component in value_array(game_object.get("m_Component")) {
        let Some(pointer) = component_pointer(component) else {
            continue;
        };
        let key = env
            .resolve_pointer(pointer)
            .map_err(|err| format!("could not resolve terrain owner component: {err}"))?;
        if key == manager.key {
            continue;
        }
        let asset = env
            .assets
            .get(key.asset)
            .ok_or_else(|| format!("terrain owner component asset {} missing", key.asset))?;
        if !map_asset_names.contains(&asset.name) {
            continue;
        }
        let info = asset.objects.get(&key.path_id).ok_or_else(|| {
            format!(
                "terrain owner component {}#{} missing",
                asset.name, key.path_id
            )
        })?;
        if asset.object_type_name(info) == "TerrainCollider" {
            continue;
        }
        let body = asset.read_object(key.asset, info).map_err(|err| {
            format!(
                "could not read terrain owner component {}#{}: {err}",
                asset.name, key.path_id
            )
        })?;
        if body
            .get("m_TerrainData")
            .and_then(UnityValue::as_pointer)
            .is_none()
            || body.get("m_HeightmapPixelError").is_none()
            || body.get("m_DetailObjectDistance").is_none()
        {
            continue;
        }
        let script_pointer = body
            .get("m_Script")
            .and_then(UnityValue::as_pointer)
            .cloned();
        let script_true_name = script_pointer
            .as_ref()
            .and_then(|value| resolved_object_name(env, value));
        matches.push(exact_scene_object_from_body(
            key.asset,
            asset,
            info,
            body,
            script_pointer,
            script_true_name,
        )?);
    }
    matches.sort_by(|left, right| {
        left.asset_name
            .cmp(&right.asset_name)
            .then_with(|| left.key.path_id.cmp(&right.key.path_id))
    });
    Ok(matches)
}

pub(super) fn exact_terrain_placement_audit(
    env: &UnityEnvironment,
    map_asset_names: &BTreeSet<String>,
    _source: &TileSource,
    terrain: &PublishedTerrainIdentity,
    sidecars: &mut Vec<SceneSidecar>,
) -> Result<JsonValue, String> {
    let mut colliders = Vec::<ExactSceneObject>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !map_asset_names.contains(&asset.name) {
            continue;
        }
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "TerrainCollider" {
                continue;
            }
            let body = asset.read_object(asset_index, info).map_err(|err| {
                format!(
                    "could not read TerrainCollider {}#{}: {err}",
                    asset.name, info.path_id
                )
            })?;
            colliders.push(exact_scene_object_from_body(
                asset_index,
                asset,
                info,
                body,
                None,
                None,
            )?);
        }
    }
    colliders.sort_by(|left, right| {
        left.asset_name
            .cmp(&right.asset_name)
            .then_with(|| left.key.path_id.cmp(&right.key.path_id))
    });
    let mut candidates = Vec::with_capacity(colliders.len());
    let mut resolved_targets = Vec::<Option<(String, i64, String)>>::new();
    for (index, collider) in colliders.iter().enumerate() {
        let source_object = exact_scene_object_document(
            env,
            collider,
            &format!("terrain-collider-candidate-{index:02}"),
            sidecars,
        )?;
        let pointer = collider
            .body
            .get("m_TerrainData")
            .and_then(UnityValue::as_pointer)
            .cloned();
        let (target, target_identity) = match pointer.as_ref() {
            Some(pointer) => match env.resolve_pointer(pointer) {
                Ok(key) => {
                    let asset = env.assets.get(key.asset).ok_or_else(|| {
                        format!("TerrainCollider target asset {} is missing", key.asset)
                    })?;
                    let body = env.read_object(key).map_err(|err| {
                        format!(
                            "could not read TerrainCollider target {}#{}: {err}",
                            asset.name, key.path_id
                        )
                    })?;
                    let true_name = object_name(&body);
                    (
                        json!({
                            "status": "resolved",
                            "assetName": asset.name,
                            "pathId": key.path_id,
                            "trueName": true_name,
                        }),
                        Some((asset.name.clone(), key.path_id, true_name)),
                    )
                }
                Err(err) => (
                    json!({
                        "status": "unresolved",
                        "error": err.to_string(),
                    }),
                    None,
                ),
            },
            None => (
                json!({
                    "status": "missingPointer",
                }),
                None,
            ),
        };
        resolved_targets.push(target_identity);
        candidates.push(json!({
            "sourceObject": source_object,
            "terrainDataPointer": pointer.as_ref().map(pointer_json),
            "resolvedTerrainData": target,
        }));
    }

    let source_identity = json!({
        "trueName": terrain.true_name,
        "assetName": terrain.asset_name,
        "pathId": terrain.path_id,
        "sourceContainerRoute": terrain.source_container_route,
        "assetRouteTileId": terrain.asset_route_tile_id,
        "sourceResourceBundle": terrain.source_resource_bundle,
    });
    if colliders.is_empty() {
        return Ok(json!({
            "status": "blocked",
            "classificationCode": "terrainColliderMissing",
            "sourceTerrainData": source_identity,
            "colliderCount": 0,
            "candidates": candidates,
            "message": "map scene contains no TerrainCollider",
        }));
    }
    if colliders.len() != 1 {
        let exact_count = resolved_targets
            .iter()
            .flatten()
            .filter(|(asset_name, path_id, _)| {
                asset_name == &terrain.asset_name && *path_id == terrain.path_id
            })
            .count();
        return Ok(json!({
            "status": "blocked",
            "classificationCode": "terrainColliderAmbiguous",
            "sourceTerrainData": source_identity,
            "colliderCount": colliders.len(),
            "exactTargetCount": exact_count,
            "candidates": candidates,
            "message": format!(
                "map scene has {} TerrainCollider objects ({} target the exported TerrainData)",
                colliders.len(),
                exact_count
            ),
        }));
    }
    let Some((target_asset_name, target_path_id, target_true_name)) =
        resolved_targets.into_iter().next().flatten()
    else {
        return Ok(json!({
            "status": "blocked",
            "classificationCode": "terrainColliderTargetUnresolved",
            "sourceTerrainData": source_identity,
            "colliderCount": 1,
            "candidates": candidates,
            "message": "the sole map TerrainCollider target could not be resolved exactly",
        }));
    };
    if target_asset_name == terrain.asset_name && target_path_id == terrain.path_id {
        return Ok(json!({
            "status": "linked",
            "classificationCode": "linkedExact",
            "sourceTerrainData": source_identity,
            "colliderCount": 1,
            "candidates": candidates,
            "message": format!(
                "the sole map TerrainCollider resolves exactly to {}#{}",
                terrain.asset_name, terrain.path_id
            ),
        }));
    }
    Ok(json!({
        "status": "blocked",
        "classificationCode": "sourceResourceSceneTerrainMismatch",
        "sourceTerrainData": source_identity,
        "colliderCount": 1,
        "candidates": candidates,
        "sceneTerrainData": {
            "trueName": target_true_name,
            "assetName": target_asset_name,
            "pathId": target_path_id,
        },
        "message": format!(
            "source resource TerrainData {}#{} ({}) differs from the sole scene TerrainCollider target {}#{} ({})",
            terrain.asset_name,
            terrain.path_id,
            terrain.true_name,
            target_asset_name,
            target_path_id,
            target_true_name
        ),
    }))
}

pub(super) fn terrain_detail_runtime_contract() -> JsonValue {
    json!({
        "source": "TerrainDetailManager.SetupRenderOptions",
        "target": "sibling Terrain component",
        "bindings": {
            "detailObjectDistance": "cnOption.graphicOption.GetDetailObjectCull()",
            "basemapDistance": "cnOption.graphicOption.GetBasemapDistance()",
            "heightmapPixelError": "cnOption.graphicOption.GetHeightMapPixelError()",
        },
        "legacySerializedFieldMapping": {
            "detailObjectDistance": "m_DetailObjectDistance",
            "basemapDistance": ["m_BasemapDistance", "m_SplatMapDistance"],
            "heightmapPixelError": "m_HeightmapPixelError",
        },
        "runtimePolicy": "serialized values are provenance baselines; live values follow the graphics-option bindings",
    })
}

pub(super) fn terrain_graph_closure_blockers(
    terrain_document_path: &Path,
) -> Result<Vec<(&'static str, String)>, String> {
    let bytes = fs::read(terrain_document_path)
        .map_err(|err| format!("could not read {}: {err}", terrain_document_path.display()))?;
    let document: JsonValue = serde_json::from_slice(&bytes)
        .map_err(|err| format!("could not parse {}: {err}", terrain_document_path.display()))?;
    Ok(terrain_graph_closure_blockers_from_document(&document))
}

pub(super) fn terrain_graph_closure_blockers_from_document(
    document: &JsonValue,
) -> Vec<(&'static str, String)> {
    let mut blockers = Vec::new();
    let detail = &document["detailAndTrees"];
    if let Some(prototypes) = detail.get("prototypes").and_then(JsonValue::as_array) {
        for (index, prototype) in prototypes.iter().enumerate() {
            let closure = &prototype["prototypeMeshRoot"];
            if closure["status"] == "rootObjectExportedGraphClosureBlocked" {
                blockers.push((
                    "detailPrototypeGraphClosure",
                    format!(
                        "detail prototype {index} root {}#{} requires full child/mesh/material/texture graph export",
                        closure["resolvedAssetName"].as_str().unwrap_or("<unknown>"),
                        closure["resolvedPathId"].as_i64().unwrap_or_default(),
                    ),
                ));
            }
        }
    }
    if let Some(prototypes) = detail["trees"]
        .get("prototypes")
        .and_then(JsonValue::as_array)
    {
        for (index, prototype) in prototypes.iter().enumerate() {
            let closure = &prototype["prefabRoot"];
            if closure["status"] == "rootObjectExportedGraphClosureBlocked" {
                blockers.push((
                    "treePrefabGraphClosure",
                    format!(
                        "tree prototype {index} root {}#{} requires full child/mesh/material/texture graph export",
                        closure["resolvedAssetName"].as_str().unwrap_or("<unknown>"),
                        closure["resolvedPathId"].as_i64().unwrap_or_default(),
                    ),
                ));
            }
        }
    }
    if let Some(entries) = detail["preloadTextureAtlasData"]
        .get("entries")
        .and_then(JsonValue::as_array)
    {
        for (index, entry) in entries.iter().enumerate() {
            if matches!(
                entry["status"].as_str(),
                Some("unmatchedPointerClosureBlocked" | "invalidSerializedEntryBlocked")
            ) {
                blockers.push((
                    "detailPreloadTextureClosure",
                    format!(
                        "detail preload atlas entry {index} is not covered by an exported prototype texture: {}",
                        entry["status"].as_str().unwrap_or("<unknown>")
                    ),
                ));
            }
        }
    }
    blockers
}

pub(super) fn exact_owned_terrain_identity(
    env: &UnityEnvironment,
    resource_asset_names: &BTreeSet<String>,
    source: &TileSource,
) -> Result<TerrainIdentity, String> {
    let mut identities = BTreeMap::<(String, String, i64), TerrainIdentity>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !resource_asset_names.contains(&asset.name) {
            continue;
        }
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let bundle = asset.read_object(asset_index, info).map_err(|err| {
                format!(
                    "could not read source AssetBundle {}#{}: {err}",
                    asset.name, info.path_id
                )
            })?;
            for entry in value_array(bundle.get("m_Container")) {
                let Some((route, metadata)) = pair_name_value(entry) else {
                    continue;
                };
                let Some(route_tile_id) = terrain_tile_id_from_container_route(route) else {
                    continue;
                };
                if source.scope == TerrainScope::Tutorial && route_tile_id != source.tile_id {
                    continue;
                }
                let pointer = metadata
                    .get("asset")
                    .and_then(UnityValue::as_pointer)
                    .ok_or_else(|| {
                        format!("terrain container route {route:?} has no asset pointer")
                    })?;
                let key = env.resolve_pointer(pointer).or_else(|_| {
                    (pointer.file_id == 0)
                        .then_some(ObjectKey {
                            asset: asset_index,
                            path_id: pointer.path_id,
                        })
                        .ok_or_else(|| {
                            format!("could not resolve terrain container route {route:?}")
                        })
                })?;
                let target_asset = env.assets.get(key.asset).ok_or_else(|| {
                    format!(
                        "terrain route {route:?} resolved to missing asset {}",
                        key.asset
                    )
                })?;
                if !resource_asset_names.contains(&target_asset.name) {
                    return Err(format!(
                        "terrain route {route:?} escaped exact source owner into {}",
                        target_asset.name
                    ));
                }
                let target_info = target_asset.objects.get(&key.path_id).ok_or_else(|| {
                    format!(
                        "terrain route {route:?} resolved to missing {}#{}",
                        target_asset.name, key.path_id
                    )
                })?;
                if target_asset.object_type_name(target_info) != "TerrainData" {
                    return Err(format!(
                        "terrain route {route:?} resolves to {}, expected TerrainData",
                        target_asset.object_type_name(target_info)
                    ));
                }
                let body = target_asset
                    .read_object(key.asset, target_info)
                    .map_err(|err| {
                        format!(
                            "could not read routed TerrainData {}#{}: {err}",
                            target_asset.name, key.path_id
                        )
                    })?;
                let true_name = object_name(&body);
                let expected_serialized_name = format!("TerrainData_{route_tile_id}");
                let mut findings = Vec::new();
                if true_name != expected_serialized_name {
                    findings.push(ProvenanceFinding {
                        code: "serializedNameDefect",
                        message: format!(
                            "raw m_Name is {true_name:?}; AssetBundle route proves {expected_serialized_name:?}"
                        ),
                    });
                }
                if route_tile_id != source.tile_id {
                    findings.push(ProvenanceFinding {
                        code: "outerContentMismatch",
                        message: format!(
                            "{} / Map_{}.unity3d owns route tile {}; placement remains keyed by the proven map scene instance",
                            source
                                .resource_path
                                .file_name()
                                .and_then(|value| value.to_str())
                                .unwrap_or("<source>"),
                            source.tile_id,
                            route_tile_id
                        ),
                    });
                }
                let identity = TerrainIdentity {
                    true_name,
                    asset_name: target_asset.name.clone(),
                    path_id: key.path_id,
                    key,
                    source_container_route: route.to_string(),
                    asset_route_tile_id: route_tile_id,
                    provenance_findings: findings,
                };
                identities.insert(
                    (
                        identity.source_container_route.clone(),
                        identity.asset_name.clone(),
                        identity.path_id,
                    ),
                    identity,
                );
            }
        }
    }
    if identities.len() != 1 {
        return Err(format!(
            "expected exactly one exact source-owned TerrainData container route for {} {}, found {}",
            source.scope.label(),
            source.tile_id,
            identities.len(),
        ));
    }
    Ok(identities.into_values().next().unwrap())
}
