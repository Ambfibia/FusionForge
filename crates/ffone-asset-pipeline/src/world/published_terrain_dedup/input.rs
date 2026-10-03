use super::*;

pub(super) fn load_published_tile_core(
    asset_root: &Path,
    catalog_tile: &JsonValue,
) -> Result<(JsonValue, JsonValue, JsonValue, String, String, String)> {
    let tile_id = required_string(catalog_tile, "tileId", "catalog tile id")?;
    let manifest_artifact = catalog_tile
        .get("manifest")
        .ok_or_else(|| invalid_error(format!("catalog tile {tile_id} has no manifest")))?;
    let manifest_route =
        required_string(manifest_artifact, "path", "tile manifest path")?.to_owned();
    let manifest_bytes = verify_artifact(asset_root, manifest_artifact, "tile manifest")?;
    let manifest: JsonValue =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_route.clone(),
            source,
        })?;
    let terrain_artifact = manifest
        .get("terrain")
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no terrain artifact")))?;
    let scene_artifact = manifest
        .get("scene")
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no scene artifact")))?;
    let files = manifest
        .get("files")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no files array")))?;
    ensure_manifest_file(files, terrain_artifact, "terrain")?;
    ensure_manifest_file(files, scene_artifact, "scene")?;
    let terrain_route = required_string(terrain_artifact, "path", "terrain path")?.to_owned();
    let scene_route = required_string(scene_artifact, "path", "scene path")?.to_owned();
    let terrain_bytes = verify_artifact(asset_root, terrain_artifact, "terrain descriptor")?;
    let scene_bytes = verify_artifact(asset_root, scene_artifact, "tile scene")?;
    let terrain: JsonValue =
        serde_json::from_slice(&terrain_bytes).map_err(|source| PipelineError::Json {
            path: terrain_route.clone(),
            source,
        })?;
    let scene: JsonValue =
        serde_json::from_slice(&scene_bytes).map_err(|source| PipelineError::Json {
            path: scene_route.clone(),
            source,
        })?;
    validate_scene_link(&scene, terrain_artifact, tile_id)?;
    Ok((
        manifest,
        scene,
        terrain,
        manifest_route,
        scene_route,
        terrain_route,
    ))
}

pub(super) fn load_tile_source(asset_root: &Path, catalog_tile: &JsonValue) -> Result<TileSource> {
    let tile_id = required_string(catalog_tile, "tileId", "catalog tile id")?.to_owned();
    let tile_route = format!("map/tiles/{tile_id}");
    let tile_root = canonical_directory(
        &checked_join(asset_root, &tile_route)?,
        "published map tile root",
    )?;
    let manifest_artifact = catalog_tile
        .get("manifest")
        .ok_or_else(|| invalid_error(format!("catalog tile {tile_id} has no manifest")))?;
    let manifest_route = required_string(manifest_artifact, "path", "tile manifest path")?;
    if manifest_route != format!("{tile_route}/tile.json") {
        return invalid(format!(
            "catalog tile {tile_id} has unexpected manifest route {manifest_route:?}"
        ));
    }
    let manifest_bytes = verify_artifact(asset_root, manifest_artifact, "tile manifest")?;
    let manifest: JsonValue =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_route.to_owned(),
            source,
        })?;
    if manifest.get("schema").and_then(JsonValue::as_str) != Some("ffone.map-tile.v1")
        || manifest.get("id").and_then(JsonValue::as_str) != Some(tile_id.as_str())
    {
        return invalid(format!("tile manifest identity mismatch for {tile_id}"));
    }
    let files = manifest
        .get("files")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no files array")))?;
    let mut manifest_paths = BTreeSet::new();
    for artifact in files {
        let route = required_string(artifact, "path", "tile file path")?;
        if !route.starts_with(&format!("{tile_route}/")) || !manifest_paths.insert(route.to_owned())
        {
            return invalid(format!(
                "tile {tile_id} has invalid/duplicate file {route:?}"
            ));
        }
        verify_artifact(asset_root, artifact, "tile owned file")?;
    }
    let actual_paths = collect_files(&tile_root)?
        .into_iter()
        .map(|path| asset_relative(asset_root, &path))
        .collect::<Result<BTreeSet<_>>>()?;
    let expected_paths = manifest_paths
        .iter()
        .cloned()
        .chain([manifest_route.to_owned()])
        .collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        return invalid(format!(
            "tile {tile_id} physical closure differs from its manifest: missing={:?}, orphan={:?}",
            expected_paths
                .difference(&actual_paths)
                .take(3)
                .collect::<Vec<_>>(),
            actual_paths
                .difference(&expected_paths)
                .take(3)
                .collect::<Vec<_>>()
        ));
    }
    let terrain_artifact = manifest
        .get("terrain")
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no terrain artifact")))?;
    let scene_artifact = manifest
        .get("scene")
        .ok_or_else(|| invalid_error(format!("tile {tile_id} has no scene artifact")))?;
    ensure_manifest_file(files, terrain_artifact, "terrain")?;
    ensure_manifest_file(files, scene_artifact, "scene")?;
    let terrain_bytes = verify_artifact(asset_root, terrain_artifact, "tile terrain descriptor")?;
    let scene_bytes = verify_artifact(asset_root, scene_artifact, "tile scene")?;
    let terrain_route = required_string(terrain_artifact, "path", "terrain artifact path")?;
    let scene_route = required_string(scene_artifact, "path", "scene artifact path")?;
    let terrain: JsonValue =
        serde_json::from_slice(&terrain_bytes).map_err(|source| PipelineError::Json {
            path: terrain_route.to_owned(),
            source,
        })?;
    let scene: JsonValue =
        serde_json::from_slice(&scene_bytes).map_err(|source| PipelineError::Json {
            path: scene_route.to_owned(),
            source,
        })?;
    validate_scene_link(&scene, terrain_artifact, &tile_id)?;
    if terrain
        .pointer("/detailAndTrees/textures")
        .and_then(JsonValue::as_array)
        .is_some_and(|textures| {
            textures.iter().any(|texture| {
                texture
                    .get("documentPath")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|path| path.starts_with("map/shared/"))
            })
        })
    {
        return invalid(format!(
            "tile {tile_id} is already partially terrain-deduplicated"
        ));
    }
    let terrain_root = canonical_directory(
        checked_join(asset_root, terrain_route)?
            .parent()
            .ok_or_else(|| invalid_error("terrain descriptor has no parent"))?,
        "tile terrain root",
    )?;
    let packages = load_detail_packages(asset_root, &terrain_root, &terrain, &tile_id)?;
    Ok(TileSource {
        tile_id,
        tile_route,
        terrain_root,
        terrain,
        scene,
        manifest,
        packages,
    })
}

pub(super) fn load_detail_packages(
    asset_root: &Path,
    terrain_root: &Path,
    terrain: &JsonValue,
    tile_id: &str,
) -> Result<Vec<DetailPackage>> {
    let Some(records) = terrain
        .pointer("/detailAndTrees/textures")
        .and_then(JsonValue::as_array)
    else {
        return Ok(Vec::new());
    };
    let mut packages = Vec::with_capacity(records.len());
    let mut documents = BTreeSet::new();
    for record in records {
        let document_path = required_string(record, "documentPath", "detail document path")?;
        if !documents.insert(document_path.to_owned()) {
            return invalid(format!(
                "tile {tile_id} contains duplicate detail document {document_path:?}"
            ));
        }
        validate_relative(document_path)?;
        let document_relative = Path::new(document_path);
        if document_relative
            .file_name()
            .and_then(|value| value.to_str())
            != Some("texture.json")
        {
            return invalid(format!(
                "non-canonical detail document path {document_path:?}"
            ));
        }
        let package_relative_path = document_relative
            .parent()
            .ok_or_else(|| invalid_error("detail document has no package"))?;
        if !package_relative_path.starts_with("details/textures") {
            return invalid(format!(
                "detail package escaped details/textures: {document_path}"
            ));
        }
        let package_relative = slash_path(package_relative_path);
        let package_root = canonical_directory(
            &checked_join(terrain_root, package_relative_path)?,
            "detail package",
        )?;
        if !package_root.starts_with(terrain_root) {
            return invalid("detail package escaped terrain root");
        }
        let mut files = Vec::new();
        for absolute in collect_files(&package_root)? {
            let member = slash_path(
                absolute
                    .strip_prefix(&package_root)
                    .map_err(|_| invalid_error("detail member escaped package"))?,
            );
            let bytes = read_file(&absolute, "detail package member")?;
            files.push(PackageFile {
                member,
                asset_path: asset_relative(asset_root, &absolute)?,
                absolute,
                bytes: bytes.len() as u64,
                blake3: hash(&bytes),
            });
        }
        if files.is_empty() || !files.iter().any(|file| file.member == "texture.json") {
            return invalid(format!(
                "detail package {document_path:?} has no complete closure"
            ));
        }
        files.sort_by(|left, right| left.member.cmp(&right.member));
        let mut closure = blake3::Hasher::new();
        for file in &files {
            append_set_hash(&mut closure, &file.member, &file.blake3);
        }
        let closure_blake3 = closure.finalize().to_hex().to_string();
        let document_file = files
            .iter()
            .find(|file| file.member == "texture.json")
            .expect("checked above");
        let document_bytes = read_file(&document_file.absolute, "detail texture document")?;
        // Runtime-metadata cleanup historically rewrote texture.json without
        // refreshing this nested terrain reference. The tile manifest was
        // verified above against every physical package member and is the
        // authoritative published-byte proof. Record the drift and repair the
        // reference instead of trusting either side without that proof.
        let expected_document = normalized_hash(required_string(
            record,
            "documentBlake3",
            "detail document BLAKE3",
        )?)?;
        let document_hash_was_stale = hash(&document_bytes) != expected_document;
        let document: JsonValue =
            serde_json::from_slice(&document_bytes).map_err(|source| PipelineError::Json {
                path: document_file.absolute.display().to_string(),
                source,
            })?;
        validate_detail_identity(&document, record, document_path)?;
        let base_path = required_string(&document, "path", "detail base path")?.to_owned();
        let base_hash = required_string(&document, "pngBlake3", "detail base hash")?;
        let mip_zero = document
            .get("mips")
            .and_then(JsonValue::as_array)
            .and_then(|mips| {
                mips.iter()
                    .find(|mip| mip.get("level").and_then(JsonValue::as_u64) == Some(0))
            })
            .ok_or_else(|| invalid_error("detail texture has no mip zero"))?;
        let mip_zero_path = required_string(mip_zero, "path", "detail mip-zero path")?.to_owned();
        let mip_zero_hash = required_string(mip_zero, "pngBlake3", "detail mip-zero hash")?;
        let base_is_mip_zero = exact_hashed_files_match(
            terrain_root,
            &base_path,
            base_hash,
            &mip_zero_path,
            mip_zero_hash,
            "detail base/mip zero",
        )?;
        packages.push(DetailPackage {
            true_texture_name: required_string(record, "trueTextureName", "detail true name")?
                .to_owned(),
            document_path: document_path.to_owned(),
            package_relative,
            closure_blake3,
            files,
            document,
            base_path,
            mip_zero_path,
            base_is_mip_zero,
            document_hash_was_stale,
        });
    }
    validate_prototype_references(terrain, &documents, tile_id)?;
    Ok(packages)
}

pub(super) fn discover_physical_artifacts(
    value: &JsonValue,
    expected: &mut BTreeMap<String, PhysicalArtifact>,
    pending: &mut Vec<String>,
) -> Result<()> {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                discover_physical_artifacts(value, expected, pending)?;
            }
        }
        JsonValue::Object(values) => {
            if let (Some(route), Some(bytes), Some(blake3)) = (
                values.get("path").and_then(JsonValue::as_str),
                values.get("bytes").and_then(JsonValue::as_u64),
                values.get("blake3").and_then(JsonValue::as_str),
            ) && (route.starts_with("map/") || route.starts_with("objects/"))
            {
                validate_relative(route)?;
                let proof = PhysicalArtifact {
                    bytes,
                    blake3: normalized_hash(blake3)?.to_owned(),
                };
                match expected.get(route) {
                    Some(existing) if existing != &proof => {
                        return invalid(format!(
                            "published artifact {route:?} has contradictory acceptance proofs"
                        ));
                    }
                    Some(_) => {}
                    None => {
                        expected.insert(route.to_owned(), proof);
                        pending.push(route.to_owned());
                    }
                }
            }
            for child in values.values() {
                discover_physical_artifacts(child, expected, pending)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| io_at(&path, error))?;
            if kind.is_symlink() {
                return invalid(format!(
                    "asset closure contains symlink: {}",
                    path.display()
                ));
            }
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn read_metadata(path: &Path, label: &str) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} is not a regular file: {}", path.display()));
    }
    Ok(metadata)
}

pub(super) fn read_file(path: &Path, label: &str) -> Result<Vec<u8>> {
    read_metadata(path, label)?;
    fs::read(path).map_err(|error| io_at(path, error))
}
