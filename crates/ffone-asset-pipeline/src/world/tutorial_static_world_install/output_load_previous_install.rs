use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_install_report(
    source_build: &str,
    contracts: &[TileContract],
    model_count: u64,
    texture_count: u64,
    static_metadata_count: u64,
    installed_files: u64,
    installed_bytes: u64,
    manifest_files: u64,
    replaced_previous_install: bool,
    source_set_blake3: String,
) -> TutorialStaticWorldInstallReport {
    let scope = StaticWorldScope::of_contracts(contracts).unwrap_or(StaticWorldScope::Tutorial);
    TutorialStaticWorldInstallReport {
        schema: match scope {
            StaticWorldScope::Tutorial => TUTORIAL_STATIC_WORLD_INSTALL_REPORT_SCHEMA.to_owned(),
            StaticWorldScope::WorldMap => WORLD_MAP_STATIC_WORLD_INSTALL_REPORT_SCHEMA.to_owned(),
        },
        source_build: source_build.to_owned(),
        tile_count: contracts.len() as u64,
        model_count,
        texture_count,
        static_metadata_count,
        merged_scene_count: contracts.len() as u64,
        scene_node_count: contracts.iter().map(|tile| tile.scene_nodes).sum(),
        runtime_visual_count: contracts.iter().map(|tile| tile.runtime_visuals).sum(),
        runtime_collider_count: contracts.iter().map(|tile| tile.runtime_colliders).sum(),
        vertex_count: contracts.iter().map(|tile| tile.vertices).sum(),
        index_count: contracts.iter().map(|tile| tile.indices).sum(),
        installed_files,
        installed_bytes,
        manifest_files,
        replaced_previous_install,
        ownership_path: scope.ownership_path().to_owned(),
        source_set_blake3,
    }
}

pub(super) fn load_previous_install(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    contracts: &[TileContract],
) -> Result<Option<TutorialStaticWorldOwnership>> {
    let scope = StaticWorldScope::of_contracts(contracts)?;
    let ownership_path = scope.ownership_path();
    let path = safe_join(asset_root, ownership_path)?;
    let manifest_has_entry = manifest
        .files
        .iter()
        .any(|entry| entry.path == ownership_path);
    let active_ownership = path.exists();
    let cleanup = if !active_ownership {
        if manifest_has_entry {
            return invalid("asset manifest owns a missing static-world install manifest");
        }
        if scope.supports_migrated_scene_recovery() {
            load_cleanup_archived_ownership(asset_root)?
        } else {
            None
        }
    } else {
        None
    };
    if active_ownership && !manifest_has_entry {
        return invalid("unowned static-world install manifest already exists");
    }
    let ownership: TutorialStaticWorldOwnership = if let Some(cleanup) = &cleanup {
        cleanup.ownership.clone()
    } else if active_ownership {
        let bytes = verify_manifest_file(
            asset_root,
            manifest,
            ownership_path,
            "static-world ownership manifest",
        )?;
        parse_json(&bytes, &path)?
    } else {
        return Ok(None);
    };
    let legacy_ownership = scope.supports_migrated_scene_recovery()
        && ownership.schema == LEGACY_TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA;
    let current_ownership = ownership.schema == scope.ownership_schema();
    // The first published world-map ownership recorded the shared tutorial
    // installer id. Accept it so that install can normalize the field, and
    // keep every other installer identity a hard failure.
    let first_release_world_map_installer_id =
        scope == StaticWorldScope::WorldMap && ownership.installer == INSTALLER_ID;
    if (!legacy_ownership && !current_ownership)
        || (ownership.installer != scope.installer_id() && !first_release_world_map_installer_id)
        || ownership.source_build != TUTORIAL_STATIC_WORLD_SOURCE_BUILD
    {
        return invalid("existing tutorial static-world ownership manifest has the wrong identity");
    }
    if (legacy_ownership && !ownership.references.is_empty())
        || (current_ownership && ownership.references.is_empty())
    {
        return invalid("existing tutorial static-world ownership has invalid reference proofs");
    }
    let expected_tiles = contracts
        .iter()
        .map(|contract| contract.id.as_ref())
        .collect::<BTreeSet<_>>();
    let actual_tiles = ownership
        .tiles
        .iter()
        .map(|tile| tile.tile_id.as_str())
        .collect::<BTreeSet<_>>();
    let scene_tiles = ownership
        .scenes
        .iter()
        .map(|scene| scene.tile_id.as_str())
        .collect::<BTreeSet<_>>();
    if actual_tiles != expected_tiles
        || scene_tiles != expected_tiles
        || ownership.tiles.len() != contracts.len()
        || ownership.scenes.len() != contracts.len()
    {
        return invalid("existing tutorial static-world ownership has incomplete tile coverage");
    }
    if source_set_blake3(&ownership.tiles) != ownership.source_set_blake3 {
        return invalid("existing tutorial static-world source-set proof is invalid");
    }

    let mut owned_paths = BTreeSet::new();
    let mut current_owned_paths = BTreeSet::new();
    for owned in &ownership.owned_files {
        validate_owned_path(&owned.path, contracts)?;
        if !owned_paths.insert(owned.path.clone()) {
            return invalid(format!(
                "duplicate owned static-world path {:?}",
                owned.path
            ));
        }
        let archived = cleanup
            .as_ref()
            .and_then(|cleanup| cleanup.files_by_source.get(&owned.path));
        if let Some(archived) = archived {
            if owned.kind != ProjectAssetKind::Data
                || archived.bytes != owned.bytes
                || archived.blake3 != owned.blake3
                || archived.reason != CLEANUP_STATIC_REASON
                || !archived.manifested
                || archived.manifest_source_path.as_deref() != Some(owned.source_path.as_str())
                || archived.manifest_kind != Some(owned.kind)
                || safe_join(asset_root, &owned.path)?.exists()
                || manifest.files.iter().any(|entry| entry.path == owned.path)
            {
                return invalid(format!(
                    "cleanup-archived static metadata proof differs at {:?}",
                    owned.path
                ));
            }
        } else {
            if cleanup.is_some() && owned.kind == ProjectAssetKind::Data {
                return invalid(format!(
                    "cleanup archive lacks owned static metadata {:?}",
                    owned.path
                ));
            }
            let bytes = verify_manifest_file(
                asset_root,
                manifest,
                &owned.path,
                "owned tutorial static-world file",
            )?;
            if bytes.len() as u64 != owned.bytes || hash_bytes(&bytes) != owned.blake3 {
                return invalid(format!(
                    "owned tutorial static-world file changed at {:?}",
                    owned.path
                ));
            }
            let project = project_entry(manifest, &owned.path)?;
            if project.source_path != owned.source_path || project.kind != owned.kind {
                return invalid(format!(
                    "project manifest ownership differs at {:?}",
                    owned.path
                ));
            }
            current_owned_paths.insert(owned.path.clone());
        }
    }
    let mut current_scene_proofs = Vec::with_capacity(ownership.scenes.len());
    let mut legacy_scene_drift = false;
    for scene in &ownership.scenes {
        if scene.path != tile_scene_path(&scene.tile_id) {
            return invalid(format!(
                "ownership scene path is invalid for {:?}",
                scene.tile_id
            ));
        }
        let bytes = verify_manifest_file(
            asset_root,
            manifest,
            &scene.path,
            "merged tutorial terrain scene",
        )?;
        let current_blake3 = hash_bytes(&bytes);
        let full_proof_matches =
            bytes.len() as u64 == scene.installed_bytes && current_blake3 == scene.installed_blake3;
        if !full_proof_matches && !legacy_ownership {
            return invalid(format!(
                "merged tutorial terrain scene changed at {:?}",
                scene.path
            ));
        }
        let value: JsonValue = parse_json(&bytes, &safe_join(asset_root, &scene.path)?)?;
        validate_installed_scene_identity(&value, scene)?;
        if static_fields_blake3(&value)? != scene.static_fields_blake3 {
            return invalid(format!(
                "merged tutorial terrain static fields changed at {:?}",
                scene.path
            ));
        }
        let mut current = scene.clone();
        current.installed_bytes = bytes.len() as u64;
        current.installed_blake3 = current_blake3;
        current_scene_proofs.push(current);
        legacy_scene_drift |= !full_proof_matches;
    }
    validate_owned_trees(
        asset_root,
        contracts,
        &current_owned_paths,
        cleanup.is_none(),
    )?;
    if current_ownership {
        verify_reference_closure(
            asset_root,
            manifest,
            &ownership.scenes,
            &ownership.references,
        )?;
    } else if legacy_scene_drift || cleanup.is_some() {
        verify_current_reference_documents(asset_root, manifest, &current_scene_proofs)?;
    }
    Ok(Some(ownership))
}

pub(super) fn required_export_file<'a>(
    files: &'a BTreeMap<String, &ExportManifestFile>,
    path: &str,
) -> Result<&'a ExportManifestFile> {
    files
        .get(path)
        .copied()
        .ok_or_else(|| invalid_error(format!("static export manifest lacks {path:?}")))
}

pub(super) fn export_kind(kind: &str) -> Result<ProjectAssetKind> {
    match kind {
        "model" => Ok(ProjectAssetKind::Model),
        "texture" => Ok(ProjectAssetKind::Texture),
        "data" => Ok(ProjectAssetKind::Data),
        other => invalid(format!("unsupported static export kind {other:?}")),
    }
}

pub(super) fn copy_new(source: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut input = fs::File::open(source).map_err(|error| io_at(source, error))?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut output = options.open(target).map_err(|error| io_at(target, error))?;
    std::io::copy(&mut input, &mut output).map_err(|error| io_at(target, error))?;
    output.sync_all().map_err(|error| io_at(target, error))
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options.open(path).map_err(|error| io_at(path, error))?;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}
