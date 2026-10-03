use super::*;

pub(super) fn stage_manifestless_publication(stage: &Path, publications: &[Publication]) -> Result<()> {
    for publication in publications {
        let target = safe_join(stage, &publication.entry.path)?;
        match &publication.content {
            PublicationContent::Source(source) => copy_new(source, &target)?,
            PublicationContent::Generated(bytes) => write_new(&target, bytes)?,
        }
        let (bytes, blake3) = hash_regular_file(&target, "staged static-world domain file")?;
        if bytes != publication.entry.bytes || blake3 != publication.entry.blake3 {
            return invalid(format!(
                "staged static-world domain proof mismatch at {:?}",
                publication.entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn commit_manifestless_publication(
    asset_root: &Path,
    stage: &Path,
    backup: &Path,
    contracts: &[TileContract],
    scope: StaticWorldScope,
    scene_hashes: &BTreeMap<String, String>,
) -> Result<()> {
    fs::create_dir(backup).map_err(|error| io_at(backup, error))?;
    let mut tree_paths = contracts
        .iter()
        .map(|contract| model_tile_root(&contract.id))
        .collect::<Vec<_>>();
    tree_paths.push(scope.static_root().to_owned());
    let file_paths = scene_hashes
        .keys()
        .cloned()
        .chain(std::iter::once(RUNTIME_WORLD_REGISTRY_PATH.to_owned()))
        .collect::<Vec<_>>();
    let mut backed_up = Vec::<String>::new();
    let mut published_trees = Vec::<String>::new();
    let mut published_files = Vec::<String>::new();

    let transaction = (|| {
        for relative in &tree_paths {
            let current = safe_join(asset_root, relative)?;
            if !current.is_dir() {
                return invalid(format!(
                    "owned static-world tree disappeared during transaction: {}",
                    current.display()
                ));
            }
            reject_symlink(&current, "owned static-world domain tree")?;
            move_path(asset_root, backup, relative)?;
            backed_up.push(relative.clone());
        }
        for relative in &file_paths {
            move_path(asset_root, backup, relative)?;
            backed_up.push(relative.clone());
        }
        for relative in &tree_paths {
            publish_path(stage, asset_root, relative)?;
            published_trees.push(relative.clone());
        }
        for relative in &file_paths {
            publish_path(stage, asset_root, relative)?;
            published_files.push(relative.clone());
        }
        Ok(())
    })();
    if let Err(error) = transaction {
        rollback_publication(
            asset_root,
            backup,
            &published_trees,
            &published_files,
            &backed_up,
        );
        return Err(error);
    }
    fs::remove_dir_all(backup).map_err(|error| io_at(backup, error))?;
    fs::remove_dir_all(stage).map_err(|error| io_at(stage, error))
}

pub(super) fn scene_non_static_identity_matches(current: &JsonValue, source: &JsonValue) -> Result<bool> {
    let current = current
        .as_object()
        .ok_or_else(|| invalid_error("current tutorial scene is not an object"))?;
    let source = source
        .as_object()
        .ok_or_else(|| invalid_error("source tutorial scene is not an object"))?;
    Ok([
        "coordinateContract",
        "name",
        "nativeTerrain",
        "provenance",
        "root",
        "schema",
        "scope",
        "tile",
    ]
    .into_iter()
    .all(|key| current.get(key) == source.get(key)))
}

pub(super) fn merge_scene(
    current: &JsonValue,
    source: &JsonValue,
    previous: Option<&TutorialStaticWorldSceneProof>,
    tile_id: &str,
) -> Result<JsonValue> {
    let current_object = current
        .as_object()
        .ok_or_else(|| invalid_error(format!("current scene for {tile_id} is not an object")))?;
    let source_object = source
        .as_object()
        .ok_or_else(|| invalid_error(format!("source scene for {tile_id} is not an object")))?;
    for key in [
        "coordinateContract",
        "name",
        "nativeTerrain",
        "provenance",
        "root",
        "schema",
        "scope",
        "tile",
    ] {
        if current_object.get(key) != source_object.get(key) {
            return invalid(format!(
                "native terrain field {key:?} differs between current and static source scene for {tile_id}"
            ));
        }
    }

    let current_static = static_fields_blake3(current)?;
    if let Some(previous) = previous {
        if previous.tile_id != tile_id || previous.static_fields_blake3 != current_static {
            return invalid(format!(
                "static scene fields changed outside this installer for {tile_id}"
            ));
        }
    } else if current_object.get("coverage").and_then(JsonValue::as_str) != Some(BASE_COVERAGE)
        || !required_array(current, "models", tile_id)?.is_empty()
        || !required_array(current, "visuals", tile_id)?.is_empty()
        || !required_array(current, "colliders", tile_id)?.is_empty()
    {
        return invalid(format!(
            "first static-world install requires an unmodified native-heightmap scene for {tile_id}"
        ));
    }

    let mut merged = current_object.clone();
    for key in ["coverage", "models", "visuals", "colliders"] {
        let value = source_object.get(key).ok_or_else(|| {
            invalid_error(format!("source static scene lacks {key:?} for {tile_id}"))
        })?;
        merged.insert(key.to_owned(), value.clone());
    }
    Ok(JsonValue::Object(merged))
}

pub(super) fn canonicalize_migrated_tutorial_scene(
    proof: &RuntimeMigrationProof,
    tile: &AuditedTile,
    current_bytes: &[u8],
    current_blake3: &str,
    previous: &TutorialStaticWorldSceneProof,
    legacy_ownership: bool,
) -> Result<Vec<u8>> {
    let scene_path = tile_scene_path(&tile.proof.tile_id);
    let registry = proof.registry_by_scene.get(&scene_path).ok_or_else(|| {
        invalid_error(format!(
            "completed runtime registry lacks tutorial scene {scene_path:?}"
        ))
    })?;
    if registry.scope != WorldReferenceScope::Tutorial
        || registry.id != tile.proof.tile_id
        || registry.tile != tile_coordinates(&tile.proof.tile_id)?
        || registry.scene.blake3 != current_blake3
    {
        return invalid(format!(
            "completed runtime registry identity differs for {scene_path:?}"
        ));
    }
    let environment = registry.environment.as_ref().ok_or_else(|| {
        invalid_error(format!(
            "completed runtime registry lacks an environment for {scene_path:?}"
        ))
    })?;
    let rewritten = proof
        .report
        .rewritten
        .iter()
        .find(|entry| entry.path == scene_path)
        .ok_or_else(|| {
            invalid_error(format!(
                "completed runtime migration report lacks {scene_path:?}"
            ))
        })?;
    if rewritten.original_bytes != tile.scene_bytes.len() as u64
        || rewritten.original_blake3 != tile.scene_blake3
        || rewritten.runtime_bytes != current_bytes.len() as u64
        || rewritten.runtime_blake3 != current_blake3
        || previous.source_scene_blake3 != tile.scene_blake3
    {
        return invalid(format!(
            "completed runtime migration hashes do not prove the exact static source and current scene for {scene_path:?}"
        ));
    }

    let archived_original_path = safe_join(&proof.archive_root, &rewritten.original_archive_path)?;
    let archived_original =
        read_regular_file(&archived_original_path, "archived original tutorial scene")?;
    if archived_original != tile.scene_bytes {
        return invalid(format!(
            "archived original tutorial scene differs from the exact export at {scene_path:?}"
        ));
    }

    let previous_is_current = previous.installed_bytes == current_bytes.len() as u64
        && previous.installed_blake3 == current_blake3;
    if !previous_is_current
        && (!legacy_ownership
            || previous.installed_bytes != rewritten.original_bytes
            || previous.installed_blake3 != rewritten.original_blake3)
    {
        return invalid(format!(
            "tutorial ownership is neither the exact archived original nor current runtime scene at {scene_path:?}"
        ));
    }

    let (canonical_bytes, removed_evidence_fields) = sanitize_scene(
        &scene_path,
        &tile.scene_bytes,
        &registry.terrain.path,
        &registry.terrain.blake3,
        &environment.path,
        &environment.blake3,
    )?;
    if removed_evidence_fields != rewritten.removed_evidence_fields
        || canonical_bytes != current_bytes
        || hash_bytes(&canonical_bytes) != current_blake3
    {
        return invalid(format!(
            "canonical runtime migration reproduction differs from the current scene at {scene_path:?}"
        ));
    }
    Ok(canonical_bytes)
}

pub(super) fn static_fields_blake3(scene: &JsonValue) -> Result<String> {
    let object = scene
        .as_object()
        .ok_or_else(|| invalid_error("scene static fields require an object"))?;
    let mut fields = JsonMap::new();
    for key in ["coverage", "models", "visuals", "colliders"] {
        fields.insert(
            key.to_owned(),
            object
                .get(key)
                .ok_or_else(|| invalid_error(format!("scene lacks {key:?}")))?
                .clone(),
        );
    }
    let bytes = serde_json::to_vec(&canonical_json(&JsonValue::Object(fields)))
        .map_err(generated_json_error)?;
    Ok(hash_bytes(&bytes))
}

pub(super) fn build_reference_publications(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    scenes: &[TutorialStaticWorldSceneProof],
    current_scene_blake3: &BTreeMap<String, String>,
    allow_legacy_stale_hashes: bool,
) -> Result<(Vec<Publication>, Vec<TutorialStaticWorldReferenceProof>)> {
    let expected = expected_scene_references(scenes)?;
    let documents = present_reference_documents(asset_root, manifest)?;

    let mut publications = Vec::with_capacity(documents.len());
    let mut proofs = Vec::with_capacity(documents.len());
    for (relative, bytes) in documents {
        let path = safe_join(asset_root, &relative)?;
        let (next_bytes, scene_count) = rewrite_reference_document(
            &relative,
            &bytes,
            &path,
            &expected,
            current_scene_blake3,
            allow_legacy_stale_hashes,
        )?;
        let blake3 = hash_bytes(&next_bytes);
        let entry = ProjectAssetFile {
            source_path: format!("{INSTALLER_ID}/{relative}"),
            path: relative.clone(),
            kind: ProjectAssetKind::Data,
            bytes: next_bytes.len() as u64,
            blake3: blake3.clone(),
        };
        proofs.push(TutorialStaticWorldReferenceProof {
            path: relative,
            bytes: entry.bytes,
            blake3,
            scene_count,
        });
        publications.push(Publication {
            entry,
            content: PublicationContent::Generated(next_bytes),
        });
    }
    publications.sort_by(|left, right| left.entry.path.cmp(&right.entry.path));
    proofs.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((publications, proofs))
}

pub(super) fn present_reference_documents(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<Vec<(String, Vec<u8>)>> {
    let mut documents = Vec::new();
    for (relative, label) in [
        (WORLD_CATALOG_PATH, "native world catalog"),
        (RUNTIME_WORLD_REGISTRY_PATH, "runtime world registry"),
    ] {
        let path = safe_join(asset_root, relative)?;
        let in_manifest = manifest.files.iter().any(|entry| entry.path == relative);
        if path.exists() != in_manifest {
            return invalid(format!(
                "{label} must either be absent or be a regular project-manifest file"
            ));
        }
        if in_manifest {
            documents.push((
                relative.to_owned(),
                verify_manifest_file(asset_root, manifest, relative, label)?,
            ));
        }
    }
    if documents.is_empty() {
        return invalid(
            "tutorial static-world install requires a world catalog or runtime world registry",
        );
    }
    Ok(documents)
}

pub(super) fn verify_current_reference_documents(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    scenes: &[TutorialStaticWorldSceneProof],
) -> Result<()> {
    let expected = expected_scene_references(scenes)?;
    let current_scene_blake3 = scenes
        .iter()
        .map(|scene| (scene.path.clone(), scene.installed_blake3.clone()))
        .collect::<BTreeMap<_, _>>();
    for (relative, bytes) in present_reference_documents(asset_root, manifest)? {
        let path = safe_join(asset_root, &relative)?;
        let (_, scene_count) = rewrite_reference_document(
            &relative,
            &bytes,
            &path,
            &expected,
            &current_scene_blake3,
            false,
        )?;
        if scene_count != scenes.len() as u64 {
            return invalid(format!(
                "current tutorial reference coverage differs in {relative:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn expected_scene_references(
    scenes: &[TutorialStaticWorldSceneProof],
) -> Result<BTreeMap<String, ExpectedSceneReference>> {
    let mut expected = BTreeMap::new();
    for scene in scenes {
        validate_relative(&scene.path)?;
        validate_blake3(&scene.installed_blake3, "installed tutorial scene")?;
        if scene.path != tile_scene_path(&scene.tile_id) {
            return invalid(format!(
                "tutorial scene proof path is invalid for {:?}",
                scene.tile_id
            ));
        }
        let reference = ExpectedSceneReference {
            tile_id: scene.tile_id.clone(),
            tile: tile_coordinates(&scene.tile_id)?,
            blake3: scene.installed_blake3.clone(),
        };
        if expected.insert(scene.path.clone(), reference).is_some() {
            return invalid(format!(
                "duplicate tutorial scene reference proof at {:?}",
                scene.path
            ));
        }
    }
    if expected.is_empty() {
        return invalid("tutorial reference closure contains no scenes");
    }
    Ok(expected)
}

pub(super) fn tile_coordinates(tile_id: &str) -> Result<[i32; 2]> {
    let prefix = match StaticWorldScope::of_tile(tile_id)? {
        StaticWorldScope::Tutorial => "tile_",
        StaticWorldScope::WorldMap => "map_",
    };
    let suffix = tile_id
        .strip_prefix(prefix)
        .ok_or_else(|| invalid_error(format!("invalid static-world tile id {tile_id:?}")))?;
    let mut components = suffix.split('_');
    let x = components
        .next()
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| invalid_error(format!("invalid static-world tile id {tile_id:?}")))?;
    let y = components
        .next()
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| invalid_error(format!("invalid static-world tile id {tile_id:?}")))?;
    if components.next().is_some() || format!("{prefix}{x:02}_{y:02}") != tile_id {
        return invalid(format!("invalid static-world tile id {tile_id:?}"));
    }
    Ok([x, y])
}

pub(super) fn rewrite_reference_document(
    relative: &str,
    bytes: &[u8],
    path: &Path,
    expected: &BTreeMap<String, ExpectedSceneReference>,
    current_scene_blake3: &BTreeMap<String, String>,
    allow_legacy_stale_hashes: bool,
) -> Result<(Vec<u8>, u64)> {
    match relative {
        WORLD_CATALOG_PATH => rewrite_world_catalog(
            bytes,
            path,
            expected,
            current_scene_blake3,
            allow_legacy_stale_hashes,
        ),
        RUNTIME_WORLD_REGISTRY_PATH => rewrite_runtime_world_registry(
            bytes,
            path,
            expected,
            current_scene_blake3,
            allow_legacy_stale_hashes,
        ),
        _ => invalid(format!(
            "unsupported tutorial world reference document {relative:?}"
        )),
    }
}

pub(super) fn verify_reference_closure(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    scenes: &[TutorialStaticWorldSceneProof],
    references: &[TutorialStaticWorldReferenceProof],
) -> Result<()> {
    let expected = expected_scene_references(scenes)?;
    let current_scene_blake3 = expected
        .iter()
        .map(|(path, reference)| (path.clone(), reference.blake3.clone()))
        .collect::<BTreeMap<_, _>>();
    for scene in scenes {
        let bytes = verify_manifest_file(
            asset_root,
            manifest,
            &scene.path,
            "reference-closed tutorial scene",
        )?;
        if bytes.len() as u64 != scene.installed_bytes
            || hash_bytes(&bytes) != scene.installed_blake3
        {
            return invalid(format!(
                "reference-closed tutorial scene proof differs at {:?}",
                scene.path
            ));
        }
    }

    let expected_paths = present_reference_documents(asset_root, manifest)?
        .into_iter()
        .map(|(path, _)| path)
        .collect::<BTreeSet<_>>();
    let actual_paths = references
        .iter()
        .map(|reference| reference.path.clone())
        .collect::<BTreeSet<_>>();
    if actual_paths != expected_paths || actual_paths.len() != references.len() {
        return invalid("tutorial static-world reference proofs have incomplete coverage");
    }

    for reference in references {
        validate_blake3(&reference.blake3, "tutorial world reference document")?;
        let bytes = verify_manifest_file(
            asset_root,
            manifest,
            &reference.path,
            "tutorial world reference document",
        )?;
        if bytes.len() as u64 != reference.bytes || hash_bytes(&bytes) != reference.blake3 {
            return invalid(format!(
                "tutorial world reference proof differs at {:?}",
                reference.path
            ));
        }
        let (_, scene_count) = rewrite_reference_document(
            &reference.path,
            &bytes,
            &safe_join(asset_root, &reference.path)?,
            &expected,
            &current_scene_blake3,
            false,
        )?;
        if scene_count != reference.scene_count || scene_count != scenes.len() as u64 {
            return invalid(format!(
                "tutorial world reference scene count differs at {:?}",
                reference.path
            ));
        }
    }
    Ok(())
}

pub(super) fn previous_publication_paths(
    previous: Option<&TutorialStaticWorldOwnership>,
    contracts: &[TileContract],
    references: &[TutorialStaticWorldReferenceProof],
) -> BTreeSet<String> {
    let mut paths = contracts
        .iter()
        .map(|contract| tile_scene_path(&contract.id))
        .collect::<BTreeSet<_>>();
    if let Some(previous) = previous {
        paths.extend(previous.owned_files.iter().map(|file| file.path.clone()));
        if let Ok(scope) = StaticWorldScope::of_contracts(contracts) {
            paths.insert(scope.ownership_path().to_owned());
        }
    }
    paths.extend(references.iter().map(|reference| reference.path.clone()));
    paths
}

pub(super) fn ensure_publication_paths_are_available(
    manifest: &ProjectAssetManifest,
    publications: &[Publication],
) -> Result<()> {
    let existing = manifest
        .files
        .iter()
        .map(|entry| entry.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut next = BTreeSet::new();
    for publication in publications {
        validate_relative(&publication.entry.path)?;
        let folded = publication.entry.path.to_ascii_lowercase();
        if existing.contains(&folded) || !next.insert(folded) {
            return invalid(format!(
                "tutorial static-world publication collides at {:?}",
                publication.entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn stage_publication(
    stage: &Path,
    contracts: &[TileContract],
    publications: &[Publication],
    manifest_bytes: &[u8],
    scenes: &[TutorialStaticWorldSceneProof],
    references: &[TutorialStaticWorldReferenceProof],
) -> Result<()> {
    for contract in contracts {
        let directory = safe_join(stage, &model_tile_root(&contract.id))?;
        fs::create_dir_all(&directory).map_err(|error| io_at(&directory, error))?;
    }
    let static_root = stage.join("world").join("tutorial").join("static");
    fs::create_dir_all(&static_root).map_err(|error| io_at(&static_root, error))?;
    for publication in publications {
        let target = safe_join(stage, &publication.entry.path)?;
        match &publication.content {
            PublicationContent::Source(source) => copy_new(source, &target)?,
            PublicationContent::Generated(bytes) => write_new(&target, bytes)?,
        }
        let (bytes, blake3) = hash_regular_file(&target, "staged tutorial static-world file")?;
        if bytes != publication.entry.bytes || blake3 != publication.entry.blake3 {
            return invalid(format!(
                "staged tutorial static-world proof mismatch at {:?}",
                publication.entry.path
            ));
        }
    }
    let manifest_path = stage.join(ASSET_MANIFEST_FILE);
    write_new(&manifest_path, manifest_bytes)?;
    let manifest: ProjectAssetManifest = parse_json(manifest_bytes, &manifest_path)?;
    validate_project_manifest(&manifest)?;
    verify_reference_closure(stage, &manifest, scenes, references)
}

pub(super) fn commit_publication(
    asset_root: &Path,
    stage: &Path,
    backup: &Path,
    contracts: &[TileContract],
    references: &[TutorialStaticWorldReferenceProof],
    replacing: bool,
    restoring_cleanup_archived_metadata: bool,
) -> Result<()> {
    fs::create_dir(backup).map_err(|error| io_at(backup, error))?;
    let scope = StaticWorldScope::of_contracts(contracts)?;
    let mut tree_paths = contracts
        .iter()
        .map(|contract| model_tile_root(&contract.id))
        .collect::<Vec<_>>();
    tree_paths.push(scope.static_root().to_owned());
    let file_paths = contracts
        .iter()
        .map(|contract| tile_scene_path(&contract.id))
        .chain(references.iter().map(|reference| reference.path.clone()))
        .collect::<BTreeSet<_>>();
    let mut backed_up = Vec::<String>::new();
    let mut published_trees = Vec::<String>::new();
    let mut published_files = Vec::<String>::new();

    let transaction = (|| {
        for relative in &tree_paths {
            let current = safe_join(asset_root, relative)?;
            if current.exists() {
                if !replacing {
                    return invalid(format!(
                        "tutorial static-world destination appeared during transaction: {}",
                        current.display()
                    ));
                }
                reject_symlink(&current, "owned tutorial static-world tree")?;
                move_path(asset_root, backup, relative)?;
                backed_up.push(relative.clone());
            } else if replacing
                && !(restoring_cleanup_archived_metadata && relative == "world/tutorial/static")
            {
                return invalid(format!(
                    "owned tutorial static-world tree disappeared during transaction: {}",
                    current.display()
                ));
            }
        }
        for relative in &file_paths {
            move_path(asset_root, backup, relative)?;
            backed_up.push(relative.clone());
        }
        move_path(asset_root, backup, ASSET_MANIFEST_FILE)?;
        backed_up.push(ASSET_MANIFEST_FILE.to_owned());

        for relative in &tree_paths {
            publish_path(stage, asset_root, relative)?;
            published_trees.push(relative.clone());
        }
        for relative in &file_paths {
            publish_path(stage, asset_root, relative)?;
            published_files.push(relative.clone());
        }
        publish_path(stage, asset_root, ASSET_MANIFEST_FILE)?;
        published_files.push(ASSET_MANIFEST_FILE.to_owned());
        Ok(())
    })();

    if let Err(error) = transaction {
        rollback_publication(
            asset_root,
            backup,
            &published_trees,
            &published_files,
            &backed_up,
        );
        return Err(error);
    }

    fs::remove_dir_all(backup).map_err(|error| io_at(backup, error))?;
    fs::remove_dir_all(stage).map_err(|error| io_at(stage, error))
}

pub(super) fn rollback_publication(
    asset_root: &Path,
    backup: &Path,
    published_trees: &[String],
    published_files: &[String],
    backed_up: &[String],
) {
    for relative in published_files.iter().rev() {
        if let Ok(path) = safe_join(asset_root, relative) {
            let _ = fs::remove_file(path);
        }
    }
    for relative in published_trees.iter().rev() {
        if let Ok(path) = safe_join(asset_root, relative) {
            let _ = fs::remove_dir_all(path);
        }
    }
    for relative in backed_up.iter().rev() {
        let Ok(source) = safe_join(backup, relative) else {
            continue;
        };
        let Ok(destination) = safe_join(asset_root, relative) else {
            continue;
        };
        if source.exists() && !destination.exists() {
            if let Some(parent) = destination.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::rename(source, destination);
        }
    }
}

pub(super) fn source_set_blake3(tiles: &[TutorialStaticWorldTileProof]) -> String {
    let mut sorted = tiles.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    let mut hasher = blake3::Hasher::new();
    for tile in sorted {
        hasher.update(tile.tile_id.as_bytes());
        hasher.update(&[0]);
        hasher.update(tile.export_manifest_blake3.as_bytes());
        hasher.update(&[0]);
        hasher.update(tile.source_archive_blake3.as_bytes());
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().to_string()
}

pub(super) fn required_array<'a>(value: &'a JsonValue, field: &str, label: &str) -> Result<&'a [JsonValue]> {
    value
        .get(field)
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid_error(format!("{label} field {field:?} is not an array")))
}

pub(super) fn required_string<'a>(value: &'a JsonValue, field: &str, label: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error(format!("{label} field {field:?} is not a string")))
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    reject_symlink(path, label)?;
    let metadata = fs::metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.is_dir() {
        return invalid(format!("{label} must be a directory: {}", path.display()));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}
