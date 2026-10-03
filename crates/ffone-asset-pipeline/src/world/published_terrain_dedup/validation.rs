use super::*;

pub(super) fn validate_shift_restore_identity(
    tile_id: &str,
    published: &JsonValue,
    source: &JsonValue,
) -> Result<()> {
    if source.get("schema") != published.get("schema")
        || source.get("trueName") != published.get("trueName")
        || source.get("dimensions") != published.get("dimensions")
        || source.get("scale") != published.get("scale")
        || source.get("orientation") != published.get("orientation")
    {
        return invalid(format!(
            "tile {tile_id} clean-primary terrain identity/geometry differs from published terrain"
        ));
    }
    let mut published_heightmap = published
        .get("heightmap")
        .cloned()
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no published heightmap")))?;
    let mut source_heightmap = source
        .get("heightmap")
        .cloned()
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no source heightmap")))?;
    published_heightmap
        .as_object_mut()
        .ok_or_else(|| invalid_error("published heightmap is not an object"))?
        .remove("vertexShifts");
    source_heightmap
        .as_object_mut()
        .ok_or_else(|| invalid_error("source heightmap is not an object"))?
        .remove("vertexShifts");
    let mut published_geometry = published
        .get("nativeGeometry")
        .cloned()
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no nativeGeometry")))?;
    let mut source_geometry = source
        .get("nativeGeometry")
        .cloned()
        .ok_or_else(|| invalid_error(format!("tile {tile_id} source has no nativeGeometry")))?;
    published_geometry
        .as_object_mut()
        .ok_or_else(|| invalid_error("published nativeGeometry is not an object"))?
        .remove("vertexShiftEncoding");
    source_geometry
        .as_object_mut()
        .ok_or_else(|| invalid_error("source nativeGeometry is not an object"))?
        .remove("vertexShiftEncoding");
    if published_heightmap != source_heightmap || published_geometry != source_geometry {
        return invalid(format!(
            "tile {tile_id} heightmap or native geometry differs beyond vertex shifts"
        ));
    }
    Ok(())
}

pub(super) fn validate_detail_identity(
    document: &JsonValue,
    record: &JsonValue,
    document_path: &str,
) -> Result<()> {
    if document.get("schema").and_then(JsonValue::as_str)
        != Some("ffone.native-terrain-detail-texture.v1")
        || document.get("trueTextureName") != record.get("trueTextureName")
        || document.pointer("/source/resolvedAssetName") != record.get("resolvedAssetName")
        || document.pointer("/source/resolvedPathId") != record.get("resolvedPathId")
        || document.pointer("/source/serializedObjectRawBlake3")
            != record.get("serializedObjectRawBlake3")
        || document.get("path") != record.get("path")
    {
        return invalid(format!(
            "detail document/record identity mismatch at {document_path:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_prototype_references(
    terrain: &JsonValue,
    documents: &BTreeSet<String>,
    tile_id: &str,
) -> Result<()> {
    let Some(prototypes) = terrain
        .pointer("/detailAndTrees/prototypes")
        .and_then(JsonValue::as_array)
    else {
        return Ok(());
    };
    for prototype in prototypes {
        let Some(reference) = prototype.get("prototypeTexture") else {
            continue;
        };
        let document = required_string(reference, "documentPath", "prototype detail document")?;
        if !documents.contains(document) {
            return invalid(format!(
                "tile {tile_id} prototype references absent detail document {document:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_all_scene_links(asset_root: &Path, map_root: &Path) -> Result<u64> {
    let catalog_path = map_root.join("catalog.json");
    let catalog_bytes = read_file(&catalog_path, "map catalog")?;
    let catalog: JsonValue =
        serde_json::from_slice(&catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    let tiles = catalog
        .get("tiles")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("map catalog has no tiles"))?;
    for tile in tiles {
        let tile_id = required_string(tile, "tileId", "catalog tile id")?;
        let manifest_artifact = tile
            .get("manifest")
            .ok_or_else(|| invalid_error("catalog tile has no manifest"))?;
        let manifest_bytes =
            verify_artifact_from_roots(asset_root, map_root, manifest_artifact, "tile manifest")?;
        let manifest: JsonValue =
            serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
                path: required_string(manifest_artifact, "path", "manifest path")
                    .unwrap_or("map tile manifest")
                    .to_owned(),
                source,
            })?;
        let terrain_artifact = manifest
            .get("terrain")
            .ok_or_else(|| invalid_error("tile manifest has no terrain"))?;
        let scene_artifact = manifest
            .get("scene")
            .ok_or_else(|| invalid_error("tile manifest has no scene"))?;
        let scene_bytes =
            verify_artifact_from_roots(asset_root, map_root, scene_artifact, "tile scene")?;
        let scene: JsonValue =
            serde_json::from_slice(&scene_bytes).map_err(|source| PipelineError::Json {
                path: required_string(scene_artifact, "path", "scene path")
                    .unwrap_or("map tile scene")
                    .to_owned(),
                source,
            })?;
        validate_scene_link(&scene, terrain_artifact, tile_id)?;
    }
    Ok(tiles.len() as u64)
}

pub(super) fn validate_migrated_map(
    asset_root: &Path,
    map_root: &Path,
    expected_catalog_hash: &str,
) -> Result<()> {
    let catalog_path = map_root.join("catalog.json");
    let catalog_bytes = read_file(&catalog_path, "staged map catalog")?;
    if hash(&catalog_bytes) != expected_catalog_hash {
        return invalid("staged map catalog differs from the migration plan");
    }
    let catalog: JsonValue =
        serde_json::from_slice(&catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    let mut expected_shared_details = BTreeSet::new();
    for shared in catalog
        .get("sharedFiles")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("staged catalog has no shared files"))?
    {
        let route = required_string(shared, "path", "shared file path")?;
        let root = if route.starts_with("map/") {
            map_root
        } else {
            asset_root
        };
        verify_artifact_from_roots(asset_root, map_root, shared, "staged shared file")?;
        let _ = root;
        if route.starts_with(DETAIL_SHARED_PREFIX) {
            expected_shared_details.insert(route.to_owned());
        }
    }
    let actual_shared_details = if map_root.join(DETAIL_SHARED_DIRECTORY).is_dir() {
        collect_files(&map_root.join(DETAIL_SHARED_DIRECTORY))?
            .into_iter()
            .map(|path| {
                Ok(format!(
                    "map/{}",
                    slash_path(
                        path.strip_prefix(map_root)
                            .map_err(|_| invalid_error("shared detail escaped map stage"))?
                    )
                ))
            })
            .collect::<Result<BTreeSet<_>>>()?
    } else {
        BTreeSet::new()
    };
    if expected_shared_details != actual_shared_details {
        return invalid("staged shared terrain-detail closure differs from catalog sharedFiles");
    }
    for catalog_tile in catalog
        .get("tiles")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("staged catalog has no tiles"))?
    {
        let tile_id = required_string(catalog_tile, "tileId", "catalog tile id")?;
        let manifest_artifact = catalog_tile
            .get("manifest")
            .ok_or_else(|| invalid_error("catalog tile has no manifest"))?;
        let manifest_bytes = verify_artifact_from_roots(
            asset_root,
            map_root,
            manifest_artifact,
            "staged tile manifest",
        )?;
        let manifest_path = required_string(manifest_artifact, "path", "manifest path")?.to_owned();
        let manifest: JsonValue =
            serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
                path: manifest_path,
                source,
            })?;
        let files = manifest
            .get("files")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| invalid_error(format!("staged tile {tile_id} has no files")))?;
        let mut expected = BTreeSet::new();
        for file in files {
            let route = required_string(file, "path", "staged tile file path")?;
            if !route.starts_with(&format!("map/tiles/{tile_id}/"))
                || !expected.insert(route.to_owned())
            {
                return invalid(format!("staged tile {tile_id} has invalid file {route}"));
            }
            verify_artifact_from_roots(asset_root, map_root, file, "staged tile file")?;
        }
        let manifest_route = required_string(manifest_artifact, "path", "manifest path")?;
        expected.insert(manifest_route.to_owned());
        let tile_root = checked_join(map_root, format!("tiles/{tile_id}"))?;
        let actual = collect_files(&tile_root)?
            .into_iter()
            .map(|path| {
                Ok(format!(
                    "map/{}",
                    slash_path(
                        path.strip_prefix(map_root)
                            .map_err(|_| invalid_error("staged tile file escaped map"))?
                    )
                ))
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if actual != expected {
            return invalid(format!(
                "staged tile {tile_id} has an incomplete physical closure"
            ));
        }
        let terrain_artifact = manifest
            .get("terrain")
            .ok_or_else(|| invalid_error("staged manifest has no terrain"))?;
        let scene_artifact = manifest
            .get("scene")
            .ok_or_else(|| invalid_error("staged manifest has no scene"))?;
        ensure_manifest_file(files, terrain_artifact, "terrain")?;
        ensure_manifest_file(files, scene_artifact, "scene")?;
        let terrain_bytes =
            verify_artifact_from_roots(asset_root, map_root, terrain_artifact, "staged terrain")?;
        let scene_bytes =
            verify_artifact_from_roots(asset_root, map_root, scene_artifact, "staged scene")?;
        let terrain_path = required_string(terrain_artifact, "path", "terrain path")?.to_owned();
        let terrain: JsonValue =
            serde_json::from_slice(&terrain_bytes).map_err(|source| PipelineError::Json {
                path: terrain_path,
                source,
            })?;
        let scene_path = required_string(scene_artifact, "path", "scene path")?.to_owned();
        let scene: JsonValue =
            serde_json::from_slice(&scene_bytes).map_err(|source| PipelineError::Json {
                path: scene_path,
                source,
            })?;
        validate_scene_link(&scene, terrain_artifact, tile_id)?;
        validate_migrated_terrain_references(asset_root, map_root, &terrain, tile_id)?;
    }
    Ok(())
}

pub(super) fn validate_scene_link(
    scene: &JsonValue,
    terrain_artifact: &JsonValue,
    tile_id: &str,
) -> Result<()> {
    let scene_path = scene
        .pointer("/nativeTerrain/path")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error(format!("tile {tile_id} scene has no terrain path")))?;
    let scene_hash = scene
        .pointer("/nativeTerrain/blake3")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error(format!("tile {tile_id} scene has no terrain hash")))?;
    if scene_path != required_string(terrain_artifact, "path", "terrain artifact path")?
        || scene_hash != required_string(terrain_artifact, "blake3", "terrain artifact hash")?
    {
        return invalid(format!(
            "tile {tile_id} scene/terrain acceptance link is stale"
        ));
    }
    Ok(())
}

pub(super) fn validate_relative(value: &str) -> Result<()> {
    validate_relative_path(Path::new(value))
}

pub(super) fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("path is not safe relative: {}", path.display()));
    }
    Ok(())
}

pub(super) fn generated_json_error(source: serde_json::Error) -> PipelineError {
    PipelineError::Json {
        path: "generated terrain-dedup JSON".to_owned(),
        source,
    }
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(message.into())
}
