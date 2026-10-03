use super::*;

pub(super) fn build_shift_restore_plan(
    asset_root: &Path,
    map_root: &Path,
    source_export_root: &Path,
) -> Result<(
    MigrationPlan,
    PublishedTerrainShiftRestoreCounts,
    Vec<PublishedTerrainShiftRestoreTile>,
    String,
)> {
    let source_manifest_bytes = read_file(
        &source_export_root.join("manifest.json"),
        "primary terrain export manifest",
    )?;
    let catalog_path = map_root.join("catalog.json");
    let catalog_bytes = read_file(&catalog_path, "map catalog")?;
    let source_catalog_blake3 = hash(&catalog_bytes);
    let mut catalog: JsonValue =
        serde_json::from_slice(&catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if catalog.get("schema").and_then(JsonValue::as_str) != Some("ffone.map-catalog.v1") {
        return invalid("map catalog has an unsupported schema");
    }
    let catalog_tiles = catalog
        .get("tiles")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("map catalog has no tiles array"))?
        .clone();
    let mut counts = PublishedTerrainShiftRestoreCounts {
        published_tiles: catalog_tiles.len() as u64,
        ..Default::default()
    };
    let mut replacements = BTreeMap::<String, Vec<u8>>::new();
    let mut manifest_artifacts = BTreeMap::<String, JsonValue>::new();
    let mut restored_tiles = Vec::new();

    for catalog_tile in &catalog_tiles {
        let tile_id = required_string(catalog_tile, "tileId", "catalog tile id")?.to_owned();
        let source_path = source_export_root
            .join("maps")
            .join(&tile_id)
            .join("terrain/terrain.json");
        if !source_path.is_file() {
            // The published catalog also contains six tutorial-only tiles.
            // They are not world-map TerrainData and have no `maps/` export.
            continue;
        }
        counts.primary_world_tiles += 1;
        let source_bytes = read_file(&source_path, "clean-primary terrain descriptor")?;
        let source: JsonValue =
            serde_json::from_slice(&source_bytes).map_err(|error| PipelineError::Json {
                path: source_path.display().to_string(),
                source: error,
            })?;
        let (mut manifest, mut scene, mut terrain, manifest_route, scene_route, terrain_route) =
            load_published_tile_core(asset_root, catalog_tile)?;
        validate_shift_restore_identity(&tile_id, &terrain, &source)?;

        let source_shifts = source
            .pointer("/heightmap/vertexShifts")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        let published_shifts = terrain
            .pointer("/heightmap/vertexShifts")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        if source_shifts.is_empty() {
            if !published_shifts.is_empty() {
                return invalid(format!(
                    "tile {tile_id} publishes vertex shifts absent from clean primary"
                ));
            }
            counts.primary_tiles_without_shifts += 1;
            continue;
        }
        if published_shifts == source_shifts {
            let canonical_encoding = canonical_shift_encoding(&source, &tile_id)?;
            if terrain.pointer("/nativeGeometry/vertexShiftEncoding") != Some(&canonical_encoding) {
                return invalid(format!(
                    "tile {tile_id} has correct shifts but a contradictory decoding contract"
                ));
            }
            counts.already_complete_tiles += 1;
            continue;
        }
        if !published_shifts.is_empty() {
            return invalid(format!(
                "tile {tile_id} has non-empty shifts that differ from clean primary"
            ));
        }

        let previous_terrain_blake3 = hash(&pretty_json(&terrain)?);
        terrain
            .pointer_mut("/heightmap")
            .and_then(JsonValue::as_object_mut)
            .ok_or_else(|| invalid_error(format!("tile {tile_id} has no heightmap object")))?
            .insert(
                "vertexShifts".to_owned(),
                JsonValue::Array(source_shifts.clone()),
            );
        let encoding = canonical_shift_encoding(&source, &tile_id)?;
        terrain
            .pointer_mut("/nativeGeometry")
            .and_then(JsonValue::as_object_mut)
            .ok_or_else(|| invalid_error(format!("tile {tile_id} has no nativeGeometry")))?
            .insert("vertexShiftEncoding".to_owned(), encoding);

        let terrain_bytes = pretty_json(&terrain)?;
        let terrain_artifact = artifact(&terrain_route, &terrain_bytes);
        let native_terrain = scene
            .get_mut("nativeTerrain")
            .and_then(JsonValue::as_object_mut)
            .ok_or_else(|| invalid_error(format!("tile {tile_id} scene has no nativeTerrain")))?;
        native_terrain.insert(
            "blake3".to_owned(),
            terrain_artifact
                .get("blake3")
                .cloned()
                .expect("artifact has hash"),
        );
        let scene_bytes = pretty_json(&scene)?;
        let scene_artifact = artifact(&scene_route, &scene_bytes);

        let manifest_object = manifest
            .as_object_mut()
            .ok_or_else(|| invalid_error(format!("tile {tile_id} manifest is not an object")))?;
        manifest_object.insert("terrain".to_owned(), terrain_artifact.clone());
        manifest_object.insert("scene".to_owned(), scene_artifact.clone());
        let files = manifest_object
            .get_mut("files")
            .and_then(JsonValue::as_array_mut)
            .ok_or_else(|| invalid_error(format!("tile {tile_id} manifest has no files")))?;
        replace_artifact_in_array(files, &terrain_route, &terrain_artifact, &tile_id)?;
        replace_artifact_in_array(files, &scene_route, &scene_artifact, &tile_id)?;
        let manifest_bytes = pretty_json(&manifest)?;
        let manifest_artifact = artifact(&manifest_route, &manifest_bytes);

        insert_bytes(
            &mut replacements,
            terrain_route,
            terrain_bytes.clone(),
            "restored terrain descriptor",
        )?;
        insert_bytes(
            &mut replacements,
            scene_route,
            scene_bytes,
            "restored terrain scene",
        )?;
        insert_bytes(
            &mut replacements,
            manifest_route,
            manifest_bytes,
            "restored tile manifest",
        )?;
        manifest_artifacts.insert(tile_id.clone(), manifest_artifact);
        counts.restored_tiles += 1;
        counts.restored_vertex_shifts += source_shifts.len() as u64;
        restored_tiles.push(PublishedTerrainShiftRestoreTile {
            tile_id,
            vertex_shifts: source_shifts.len() as u64,
            source_terrain_blake3: hash(&source_bytes),
            previous_terrain_blake3,
            restored_terrain_blake3: hash(&terrain_bytes),
        });
    }

    if counts.primary_world_tiles == 0 || counts.restored_tiles == 0 {
        return invalid("terrain shift restore found no clean-primary world tiles to repair");
    }
    let catalog_tiles_mut = catalog
        .get_mut("tiles")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("map catalog tiles disappeared"))?;
    for catalog_tile in catalog_tiles_mut {
        let tile_id = required_string(catalog_tile, "tileId", "catalog tile id")?;
        let Some(artifact) = manifest_artifacts.get(tile_id) else {
            continue;
        };
        catalog_tile
            .as_object_mut()
            .ok_or_else(|| invalid_error("catalog tile is not an object"))?
            .insert("manifest".to_owned(), artifact.clone());
    }
    restored_tiles.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    let result_catalog_bytes = pretty_json(&catalog)?;
    let result_catalog_blake3 = hash(&result_catalog_bytes);
    replacements.insert("map/catalog.json".to_owned(), result_catalog_bytes);
    let plan = MigrationPlan {
        source_catalog_blake3,
        result_catalog_blake3,
        replacements,
        additions: BTreeMap::new(),
        removals: BTreeSet::new(),
        counts: PublishedTerrainDedupCounts::default(),
        routes: Vec::new(),
    };
    Ok((plan, counts, restored_tiles, hash(&source_manifest_bytes)))
}

pub(super) fn canonical_shift_encoding(source: &JsonValue, tile_id: &str) -> Result<JsonValue> {
    let mut encoding = source
        .pointer("/nativeGeometry/vertexShiftEncoding")
        .cloned()
        .ok_or_else(|| {
            invalid_error(format!("tile {tile_id} source has shifts without encoding"))
        })?;
    let object = encoding
        .as_object_mut()
        .ok_or_else(|| invalid_error(format!("tile {tile_id} shift encoding is not an object")))?;
    // The offline exporter records source UV coordinates where +Z increases
    // V. Published Bevy terrain flips V (`1 - row/(height-1)`), so the exact
    // runtime form must invert only this UV term. Geometry positions and the
    // source flags themselves remain verbatim clean-primary data.
    let source_formula = object
        .get("uvFormula")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error(format!("tile {tile_id} shift encoding has no UV formula")))?;
    if source_formula
        != "u += (positiveSourceX - negativeSourceX) / (2 * (width - 1)); v += (positiveSourceZ - negativeSourceZ) / (2 * (height - 1))"
    {
        return invalid(format!(
            "tile {tile_id} source shift UV formula is not the audited Unity export contract"
        ));
    }
    object.insert(
        "uvFormula".to_owned(),
        JsonValue::String(
            "u += (positiveSourceX - negativeSourceX) / (2 * (width - 1)); v -= (positiveSourceZ - negativeSourceZ) / (2 * (height - 1))"
                .to_owned(),
        ),
    );
    Ok(encoding)
}

pub(super) fn replace_artifact_in_array(
    files: &mut [JsonValue],
    route: &str,
    replacement: &JsonValue,
    tile_id: &str,
) -> Result<()> {
    let mut matches = 0_u32;
    for file in files {
        if file.get("path").and_then(JsonValue::as_str) == Some(route) {
            *file = replacement.clone();
            matches += 1;
        }
    }
    if matches != 1 {
        return invalid(format!(
            "tile {tile_id} expected one manifest artifact for {route:?}, found {matches}"
        ));
    }
    Ok(())
}

pub(super) fn build_plan(asset_root: &Path, map_root: &Path) -> Result<MigrationPlan> {
    let catalog_path = map_root.join("catalog.json");
    let source_catalog_bytes = read_file(&catalog_path, "map catalog")?;
    let source_catalog_blake3 = hash(&source_catalog_bytes);
    let mut catalog: JsonValue =
        serde_json::from_slice(&source_catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if catalog.get("schema").and_then(JsonValue::as_str) != Some("ffone.map-catalog.v1") {
        return invalid("map catalog has an unsupported schema");
    }
    let shared_detail_root = map_root.join(DETAIL_SHARED_DIRECTORY);
    if shared_detail_root.exists()
        || catalog
            .get("sharedFiles")
            .and_then(JsonValue::as_array)
            .is_some_and(|files| {
                files.iter().any(|file| {
                    file.get("path")
                        .and_then(JsonValue::as_str)
                        .is_some_and(|path| path.starts_with(DETAIL_SHARED_PREFIX))
                })
            })
    {
        return invalid(
            "published terrain details are already shared or partially migrated; refusing a second migration",
        );
    }

    let catalog_tiles = catalog
        .get("tiles")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("map catalog has no tiles array"))?
        .clone();
    let mut tiles = Vec::with_capacity(catalog_tiles.len());
    let mut detail_groups = BTreeMap::<(String, String), Vec<(usize, usize)>>::new();
    let mut counts = PublishedTerrainDedupCounts {
        tiles: catalog_tiles.len() as u64,
        ..Default::default()
    };
    for catalog_tile in &catalog_tiles {
        let tile = load_tile_source(asset_root, catalog_tile)?;
        let tile_index = tiles.len();
        if !tile.packages.is_empty() {
            counts.detail_tiles += 1;
        }
        counts.detail_source_packages += tile.packages.len() as u64;
        counts.repaired_detail_document_hashes += tile
            .packages
            .iter()
            .filter(|package| package.document_hash_was_stale)
            .count() as u64;
        counts.detail_references += tile.packages.len() as u64;
        counts.detail_prototype_references += tile
            .terrain
            .pointer("/detailAndTrees/prototypes")
            .and_then(JsonValue::as_array)
            .map_or(0, |values| {
                values
                    .iter()
                    .filter(|value| value.get("prototypeTexture").is_some())
                    .count() as u64
            });
        for (package_index, package) in tile.packages.iter().enumerate() {
            let slug = safe_slug(&package.true_texture_name, "detail", 56);
            detail_groups
                .entry((slug, package.closure_blake3.clone()))
                .or_default()
                .push((tile_index, package_index));
        }
        tiles.push(tile);
    }

    let package_roots = allocate_package_roots(&detail_groups);
    let mut shared_files = BTreeMap::<String, Vec<u8>>::new();
    let mut routes_by_package = BTreeMap::<(String, String), DetailRoutes>::new();
    let mut route_reports = Vec::with_capacity(detail_groups.len());
    for (key, uses) in &detail_groups {
        let &(tile_index, package_index) = uses
            .first()
            .ok_or_else(|| invalid_error("terrain detail group has no representative"))?;
        let package = &tiles[tile_index].packages[package_index];
        let package_root = package_roots
            .get(key)
            .ok_or_else(|| invalid_error("terrain detail group has no destination"))?;
        let (routes, output_bytes) =
            publish_shared_package(package, package_root, &mut shared_files)?;
        routes_by_package.insert(key.clone(), routes);
        route_reports.push(PublishedTerrainDetailRoute {
            true_texture_name: package.true_texture_name.clone(),
            closure_blake3: package.closure_blake3.clone(),
            uses: uses.len() as u64,
            source_bytes: package.files.iter().map(|file| file.bytes).sum(),
            shared_bytes: output_bytes,
            destination: package_root.clone(),
        });
    }
    route_reports.sort_by(|left, right| left.destination.cmp(&right.destination));
    counts.detail_shared_packages = detail_groups.len() as u64;
    counts.detail_redundant_packages = counts
        .detail_source_packages
        .saturating_sub(counts.detail_shared_packages);
    counts.shared_files = shared_files.len() as u64;
    counts.shared_bytes_added = shared_files.values().map(|bytes| bytes.len() as u64).sum();

    let mut replacements = BTreeMap::<String, Vec<u8>>::new();
    let mut removals = BTreeSet::<String>::new();
    let mut catalog_manifest_artifacts = BTreeMap::<String, JsonValue>::new();
    for tile in &tiles {
        let rewritten = rewrite_tile(
            asset_root,
            tile,
            &routes_by_package,
            &mut removals,
            &mut counts,
        )?;
        for (path, bytes) in rewritten.files {
            insert_bytes(&mut replacements, path, bytes, "tile replacement")?;
        }
        catalog_manifest_artifacts.insert(tile.tile_id.clone(), rewritten.manifest_artifact);
    }
    counts.removed_files = removals.len() as u64;
    counts.source_bytes_removed = removals
        .iter()
        .map(|route| {
            let path = checked_join(asset_root, route)?;
            Ok(read_metadata(&path, "removed duplicate terrain file")?.len())
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .sum();

    let tiles_mut = catalog
        .get_mut("tiles")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("map catalog tiles disappeared"))?;
    for tile in tiles_mut {
        let tile_id = required_string(tile, "tileId", "catalog tile id")?;
        let artifact = catalog_manifest_artifacts
            .get(tile_id)
            .ok_or_else(|| invalid_error(format!("catalog tile {tile_id:?} was not migrated")))?;
        let object = tile
            .as_object_mut()
            .ok_or_else(|| invalid_error("catalog tile is not an object"))?;
        object.insert("manifest".to_owned(), artifact.clone());
    }
    rebuild_shared_files(asset_root, map_root, &mut catalog, &shared_files)?;
    let result_catalog_bytes = pretty_json(&catalog)?;
    let result_catalog_blake3 = hash(&result_catalog_bytes);
    replacements.insert("map/catalog.json".to_owned(), result_catalog_bytes);

    let replacement_size_delta = replacements
        .iter()
        .map(|(route, bytes)| -> Result<i128> {
            let old = read_metadata(&checked_join(asset_root, route)?, "replaced map file")?.len();
            Ok(bytes.len() as i128 - old as i128)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .sum::<i128>();
    let saved = counts.source_bytes_removed as i128
        - counts.shared_bytes_added as i128
        - replacement_size_delta;
    counts.estimated_net_bytes_saved = u64::try_from(saved.max(0)).unwrap_or(u64::MAX);

    Ok(MigrationPlan {
        source_catalog_blake3,
        result_catalog_blake3,
        replacements,
        additions: shared_files,
        removals,
        counts,
        routes: route_reports,
    })
}

pub(super) fn allocate_package_roots(
    groups: &BTreeMap<(String, String), Vec<(usize, usize)>>,
) -> BTreeMap<(String, String), String> {
    let mut by_slug = BTreeMap::<String, Vec<((String, String), usize)>>::new();
    for (key, uses) in groups {
        by_slug
            .entry(key.0.clone())
            .or_default()
            .push((key.clone(), uses.len()));
    }
    let mut result = BTreeMap::new();
    for (slug, mut variants) in by_slug {
        variants.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.1.cmp(&right.0.1)));
        for (index, (key, _)) in variants.into_iter().enumerate() {
            let root = if index == 0 {
                format!("{DETAIL_SHARED_PREFIX}{slug}")
            } else {
                format!("{DETAIL_SHARED_PREFIX}{slug}_variant_{:02}", index + 1)
            };
            result.insert(key, root);
        }
    }
    result
}

pub(super) fn rewrite_tile(
    asset_root: &Path,
    tile: &TileSource,
    routes_by_package: &BTreeMap<(String, String), DetailRoutes>,
    removals: &mut BTreeSet<String>,
    counts: &mut PublishedTerrainDedupCounts,
) -> Result<RewrittenTile> {
    let mut terrain = tile.terrain.clone();
    let mut routes_by_document = BTreeMap::<String, DetailRoutes>::new();
    for package in &tile.packages {
        let key = (
            safe_slug(&package.true_texture_name, "detail", 56),
            package.closure_blake3.clone(),
        );
        let routes = routes_by_package
            .get(&key)
            .ok_or_else(|| invalid_error("detail package has no shared route"))?
            .clone();
        if let Some(existing) =
            routes_by_document.insert(package.document_path.clone(), routes.clone())
            && existing != routes
        {
            return invalid("detail document resolves to contradictory shared routes");
        }
        for file in &package.files {
            removals.insert(file.asset_path.clone());
        }
        if package.base_is_mip_zero && package.base_path != package.mip_zero_path {
            counts.collapsed_detail_base_mip_zero += 1;
        }
    }
    rewrite_detail_references(&mut terrain, &routes_by_document)?;
    let terrain_prefix = format!("{}/terrain/", tile.tile_route);
    if let Some(weights) = terrain
        .pointer_mut("/splat/weightMaps")
        .and_then(JsonValue::as_array_mut)
    {
        for weight in weights {
            if collapse_base_mip_zero(
                &tile.terrain_root,
                &terrain_prefix,
                weight,
                removals,
                "terrain weight map",
            )? {
                counts.collapsed_weight_base_mip_zero += 1;
            }
        }
    } else {
        return invalid(format!("tile {} has no splat weight maps", tile.tile_id));
    }
    if let Some(lightmap) = terrain.get_mut("lightmap")
        && lightmap.get("status").and_then(JsonValue::as_str) == Some("exported")
        && collapse_base_mip_zero(
            &tile.terrain_root,
            &terrain_prefix,
            lightmap,
            removals,
            "terrain lightmap",
        )?
    {
        counts.collapsed_lightmap_base_mip_zero += 1;
    }

    let terrain_route = format!("{}/terrain/terrain.json", tile.tile_route);
    let terrain_bytes = pretty_json(&terrain)?;
    let terrain_artifact = artifact(&terrain_route, &terrain_bytes);
    let mut scene = tile.scene.clone();
    let native_terrain = scene
        .get_mut("nativeTerrain")
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| {
            invalid_error(format!("tile {} scene has no nativeTerrain", tile.tile_id))
        })?;
    if native_terrain.get("path").and_then(JsonValue::as_str) != Some(terrain_route.as_str()) {
        return invalid(format!(
            "tile {} scene has a different terrain path",
            tile.tile_id
        ));
    }
    native_terrain.insert(
        "blake3".to_owned(),
        terrain_artifact
            .get("blake3")
            .cloned()
            .expect("artifact has hash"),
    );
    let scene_route = format!("{}/scene.json", tile.tile_route);
    let scene_bytes = pretty_json(&scene)?;
    let scene_artifact = artifact(&scene_route, &scene_bytes);

    let mut manifest = tile.manifest.clone();
    let manifest_object = manifest
        .as_object_mut()
        .ok_or_else(|| invalid_error("tile manifest is not an object"))?;
    manifest_object.insert("terrain".to_owned(), terrain_artifact.clone());
    manifest_object.insert("scene".to_owned(), scene_artifact.clone());
    let source_files = tile
        .manifest
        .get("files")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("tile manifest has no source files"))?;
    let mut files = BTreeMap::<String, JsonValue>::new();
    for file in source_files {
        let route = required_string(file, "path", "tile file path")?;
        if removals.contains(route) {
            continue;
        }
        let replacement = if route == terrain_route {
            terrain_artifact.clone()
        } else if route == scene_route {
            scene_artifact.clone()
        } else {
            file.clone()
        };
        if files.insert(route.to_owned(), replacement).is_some() {
            return invalid(format!("tile {} has duplicate file {route}", tile.tile_id));
        }
    }
    if files.get(&terrain_route) != Some(&terrain_artifact)
        || files.get(&scene_route) != Some(&scene_artifact)
    {
        return invalid(format!("tile {} lost terrain/scene closure", tile.tile_id));
    }
    manifest_object.insert(
        "files".to_owned(),
        JsonValue::Array(files.into_values().collect()),
    );
    let manifest_bytes = pretty_json(&manifest)?;
    let manifest_route = format!("{}/tile.json", tile.tile_route);
    let manifest_artifact = artifact(&manifest_route, &manifest_bytes);
    let mut rewritten = BTreeMap::new();
    rewritten.insert(terrain_route, terrain_bytes);
    rewritten.insert(scene_route, scene_bytes);
    rewritten.insert(manifest_route, manifest_bytes);
    let _ = asset_root;
    Ok(RewrittenTile {
        files: rewritten,
        manifest_artifact,
    })
}

pub(super) fn rewrite_detail_references(
    terrain: &mut JsonValue,
    routes_by_document: &BTreeMap<String, DetailRoutes>,
) -> Result<()> {
    let Some(detail) = terrain
        .get_mut("detailAndTrees")
        .and_then(JsonValue::as_object_mut)
    else {
        if routes_by_document.is_empty() {
            return Ok(());
        }
        return invalid("terrain lost detailAndTrees");
    };
    let textures = detail
        .get_mut("textures")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("terrain detail textures are not an array"))?;
    for texture in textures {
        rewrite_detail_reference(texture, routes_by_document)?;
    }
    if let Some(prototypes) = detail
        .get_mut("prototypes")
        .and_then(JsonValue::as_array_mut)
    {
        for prototype in prototypes {
            if let Some(reference) = prototype.get_mut("prototypeTexture") {
                rewrite_detail_reference(reference, routes_by_document)?;
            }
        }
    }
    Ok(())
}

pub(super) fn rewrite_detail_reference(
    reference: &mut JsonValue,
    routes_by_document: &BTreeMap<String, DetailRoutes>,
) -> Result<()> {
    let object = reference
        .as_object_mut()
        .ok_or_else(|| invalid_error("detail reference is not an object"))?;
    let original = object
        .get("documentPath")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error("detail reference document is absent"))?;
    let routes = routes_by_document.get(original).ok_or_else(|| {
        invalid_error(format!("detail reference {original:?} has no shared route"))
    })?;
    object.insert(
        "documentPath".to_owned(),
        JsonValue::String(routes.document_path.clone()),
    );
    object.insert(
        "documentBlake3".to_owned(),
        JsonValue::String(routes.document_blake3.clone()),
    );
    object.insert(
        "path".to_owned(),
        JsonValue::String(routes.texture_path.clone()),
    );
    Ok(())
}

pub(super) fn rebuild_shared_files(
    asset_root: &Path,
    map_root: &Path,
    catalog: &mut JsonValue,
    additions: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let files = catalog
        .get_mut("sharedFiles")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("map catalog has no sharedFiles array"))?;
    let mut rebuilt = BTreeMap::<String, JsonValue>::new();
    for existing in files.drain(..) {
        let route = required_string(&existing, "path", "shared file path")?.to_owned();
        if route.starts_with(DETAIL_SHARED_PREFIX) {
            return invalid("catalog unexpectedly contains a preexisting shared detail file");
        }
        verify_artifact(asset_root, &existing, "existing shared map file")?;
        if rebuilt.insert(route.clone(), existing).is_some() {
            return invalid(format!("duplicate shared file {route:?}"));
        }
    }
    let shared_root = canonical_directory(&map_root.join("shared"), "shared map root")?;
    for path in collect_files(&shared_root)? {
        let relative = path
            .strip_prefix(map_root)
            .map_err(|_| invalid_error("shared file escaped map root"))?;
        let route = format!("map/{}", slash_path(relative));
        let bytes = read_file(&path, "shared map file")?;
        let proof = artifact(&route, &bytes);
        if let Some(existing) = rebuilt.insert(route.clone(), proof.clone())
            && existing != proof
        {
            return invalid(format!(
                "shared map artifact {route:?} differs from its catalog proof"
            ));
        }
    }
    for (route, bytes) in additions {
        if rebuilt
            .insert(route.clone(), artifact(route, bytes))
            .is_some()
        {
            return invalid(format!("shared detail route collides at {route:?}"));
        }
    }
    *files = rebuilt.into_values().collect();
    Ok(())
}
