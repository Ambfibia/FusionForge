use super::*;

pub const TUTORIAL_STATIC_WORLD_INSTALL_REPORT_SCHEMA: &str =
    "ffone.tutorial-static-world-install-report.v1";

pub const WORLD_MAP_STATIC_WORLD_INSTALL_REPORT_SCHEMA: &str =
    "ffone.world-map-static-world-install-report.v1";

pub(super) const EXPORT_REPORT_SCHEMA: &str = "ffone.native-static-world-export.v1";

pub(super) const EXPORT_STATUS: &str = "complete-hash-verified-no-preview-budget";

pub(super) const EXPORT_REPORT_FILE: &str = "export-report.json";

#[derive(Clone, Debug)]
pub struct TutorialStaticWorldInstallOptions {
    pub export_root: PathBuf,
    pub asset_root: PathBuf,
    pub source_build: String,
}

impl TutorialStaticWorldInstallOptions {
    pub fn new(export_root: impl Into<PathBuf>, asset_root: impl Into<PathBuf>) -> Self {
        Self {
            export_root: export_root.into(),
            asset_root: asset_root.into(),
            source_build: TUTORIAL_STATIC_WORLD_SOURCE_BUILD.to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorldMapStaticWorldInstallOptions {
    pub export_root: PathBuf,
    pub asset_root: PathBuf,
    pub contract_path: PathBuf,
    pub source_build: String,
}

impl WorldMapStaticWorldInstallOptions {
    pub fn new(
        export_root: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
        contract_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            export_root: export_root.into(),
            asset_root: asset_root.into(),
            contract_path: contract_path.into(),
            source_build: TUTORIAL_STATIC_WORLD_SOURCE_BUILD.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialStaticWorldInstallReport {
    pub schema: String,
    pub source_build: String,
    pub tile_count: u64,
    pub model_count: u64,
    pub texture_count: u64,
    pub static_metadata_count: u64,
    pub merged_scene_count: u64,
    pub scene_node_count: u64,
    pub runtime_visual_count: u64,
    pub runtime_collider_count: u64,
    pub vertex_count: u64,
    pub index_count: u64,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub manifest_files: u64,
    pub replaced_previous_install: bool,
    pub ownership_path: String,
    pub source_set_blake3: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportReport {
    pub(super) schema: String,
    pub(super) status: String,
    pub(super) source_build: String,
    pub(super) tile_id: String,
    pub(super) source_archive_blake3: String,
    pub(super) scene_path: String,
    pub(super) scene_blake3: String,
    pub(super) hierarchy_path: String,
    pub(super) hierarchy_blake3: String,
    pub(super) material_path: String,
    pub(super) material_blake3: String,
    pub(super) catalog_path: String,
    pub(super) catalog_blake3: String,
    pub(super) manifest_path: String,
    pub(super) counts: ExportReportCounts,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportReportCounts {
    pub(super) scene_nodes: u64,
    pub(super) exported_visual_payloads: u64,
    pub(super) runtime_visuals: u64,
    pub(super) exported_collider_payloads: u64,
    pub(super) runtime_colliders: u64,
    pub(super) exported_models: u64,
    pub(super) vertices: u64,
    pub(super) indices: u64,
    pub(super) output_files: u64,
    pub(super) output_bytes: u64,
}

pub fn install_tutorial_static_world(
    options: &TutorialStaticWorldInstallOptions,
) -> Result<TutorialStaticWorldInstallReport> {
    install_with_contract(options, &EXACT_TILES)
}

/// Install the exact static scene of every world-map tile declared by a
/// reviewed `ffone.world-map-static-world-contract.v1` document.
///
/// The 161 world-map tiles are too many to pin as a literal in this file, so
/// the reviewed import contract is passed explicitly.
/// Everything else - export closure, hash proofs, scene merge and the atomic
/// transaction - is exactly the audited tutorial path.
pub fn install_world_map_static_world(
    options: &WorldMapStaticWorldInstallOptions,
) -> Result<TutorialStaticWorldInstallReport> {
    let contracts = load_world_map_contract(&options.contract_path, &options.source_build)?;
    if StaticWorldScope::of_contracts(&contracts)? != StaticWorldScope::WorldMap {
        return invalid("world-map static world contract declares non world-map tiles");
    }
    install_with_contract(
        &TutorialStaticWorldInstallOptions {
            export_root: options.export_root.clone(),
            asset_root: options.asset_root.clone(),
            source_build: options.source_build.clone(),
        },
        &contracts,
    )
}

pub(super) fn install_with_contract(
    options: &TutorialStaticWorldInstallOptions,
    contracts: &[TileContract],
) -> Result<TutorialStaticWorldInstallReport> {
    if options.source_build != TUTORIAL_STATIC_WORLD_SOURCE_BUILD {
        return invalid(format!(
            "tutorial static world is audited only for source build {:?}, got {:?}",
            TUTORIAL_STATIC_WORLD_SOURCE_BUILD, options.source_build
        ));
    }
    if contracts.is_empty() {
        return invalid("static world contract contains no tiles");
    }
    let scope = StaticWorldScope::of_contracts(contracts)?;

    let export_root = canonical_directory(&options.export_root, "static world export root")?;
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    if export_root == asset_root
        || export_root.starts_with(&asset_root)
        || asset_root.starts_with(&export_root)
    {
        return invalid("static world export root and asset root must be disjoint");
    }
    validate_export_root_entries(&export_root, contracts)?;

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    if !manifest_path.is_file() {
        if scope != StaticWorldScope::WorldMap {
            return invalid(
                "manifestless static-world installation is currently defined only for worldMap",
            );
        }
        return install_manifestless_with_contract(
            options,
            contracts,
            scope,
            &export_root,
            &asset_root,
        );
    }
    let manifest_bytes = read_regular_file(&manifest_path, "project asset manifest")?;
    let mut manifest: ProjectAssetManifest = parse_json(&manifest_bytes, &manifest_path)?;
    validate_project_manifest(&manifest)?;

    let ownership_was_active = safe_join(&asset_root, scope.ownership_path())?.exists();
    let previous = load_previous_install(&asset_root, &manifest, contracts)?;
    let cleanup_archived_previous = previous.is_some() && !ownership_was_active;
    validate_first_install_destinations(&asset_root, &manifest, contracts, previous.as_ref())?;
    let allow_legacy_stale_hashes = previous
        .as_ref()
        .is_some_and(|ownership| ownership.schema == LEGACY_TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA);

    let mut audited = Vec::with_capacity(contracts.len());
    for contract in contracts {
        audited.push(audit_tile_export(
            &export_root,
            contract,
            &options.source_build,
        )?);
    }

    let previous_scenes = previous
        .as_ref()
        .map(|ownership| {
            ownership
                .scenes
                .iter()
                .map(|scene| (scene.tile_id.as_str(), scene))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    let mut publications = Vec::<Publication>::new();
    let mut owned_files = Vec::<TutorialStaticWorldOwnedFile>::new();
    let mut scene_proofs = Vec::<TutorialStaticWorldSceneProof>::new();
    let mut tile_proofs = Vec::<TutorialStaticWorldTileProof>::new();
    let mut current_scene_blake3 = BTreeMap::<String, String>::new();
    let mut model_count = 0_u64;
    let mut texture_count = 0_u64;
    let mut static_metadata_count = 0_u64;
    let mut runtime_migration_proof = None::<RuntimeMigrationProof>;

    for tile in &audited {
        tile_proofs.push(tile.proof.clone());
        for source in &tile.selected {
            match source.kind {
                ProjectAssetKind::Model => model_count = model_count.saturating_add(1),
                ProjectAssetKind::Texture => texture_count = texture_count.saturating_add(1),
                ProjectAssetKind::Data => {
                    static_metadata_count = static_metadata_count.saturating_add(1)
                }
                _ => {
                    return invalid(format!(
                        "unsupported static-world publication kind {:?} at {:?}",
                        source.kind, source.path
                    ));
                }
            }
            let entry = ProjectAssetFile {
                source_path: source.source_path.clone(),
                path: source.path.clone(),
                kind: source.kind,
                bytes: source.bytes,
                blake3: source.blake3.clone(),
            };
            owned_files.push(TutorialStaticWorldOwnedFile {
                source_path: entry.source_path.clone(),
                path: entry.path.clone(),
                kind: entry.kind,
                bytes: entry.bytes,
                blake3: entry.blake3.clone(),
            });
            publications.push(Publication {
                entry,
                content: PublicationContent::Source(source.source.clone()),
            });
        }

        let scene_path = tile_scene_path(&tile.proof.tile_id);
        let current_bytes = verify_manifest_file(
            &asset_root,
            &manifest,
            &scene_path,
            "native tutorial terrain scene",
        )?;
        let current_blake3 = hash_bytes(&current_bytes);
        if current_scene_blake3
            .insert(scene_path.clone(), current_blake3.clone())
            .is_some()
        {
            return invalid(format!(
                "duplicate tutorial scene contract at {scene_path:?}"
            ));
        }
        let current: JsonValue = parse_json(&current_bytes, &safe_join(&asset_root, &scene_path)?)?;
        let previous_scene = previous_scenes.get(tile.proof.tile_id.as_str()).copied();
        let (merged, merged_bytes) = if scene_non_static_identity_matches(&current, &tile.scene)? {
            let merged = merge_scene(&current, &tile.scene, previous_scene, &tile.proof.tile_id)?;
            let bytes = pretty_json(&merged)?;
            (merged, bytes)
        } else {
            if !scope.supports_migrated_scene_recovery() {
                return invalid(format!(
                    "installed scene {:?} does not match its exact static export identity",
                    tile.proof.tile_id
                ));
            }
            let previous_scene = previous_scene.ok_or_else(|| {
                invalid_error(format!(
                    "runtime-migrated tutorial scene {:?} has no installer ownership proof",
                    tile.proof.tile_id
                ))
            })?;
            if runtime_migration_proof.is_none() {
                runtime_migration_proof = Some(load_runtime_migration_proof(
                    &asset_root,
                    &manifest,
                    &options.source_build,
                )?);
            }
            let bytes = canonicalize_migrated_tutorial_scene(
                runtime_migration_proof
                    .as_ref()
                    .expect("runtime migration proof initialized above"),
                tile,
                &current_bytes,
                &current_blake3,
                previous_scene,
                allow_legacy_stale_hashes,
            )?;
            let merged: JsonValue = parse_json(&bytes, &safe_join(&asset_root, &scene_path)?)?;
            (merged, bytes)
        };
        let merged_blake3 = hash_bytes(&merged_bytes);
        let static_blake3 = static_fields_blake3(&merged)?;
        let scene_entry = ProjectAssetFile {
            source_path: format!(
                "native-terrain-export+{}/{}/{}",
                scope.source_path_prefix(),
                tile.proof.tile_id,
                scene_path
            ),
            path: scene_path.clone(),
            kind: ProjectAssetKind::Data,
            bytes: merged_bytes.len() as u64,
            blake3: merged_blake3.clone(),
        };
        scene_proofs.push(TutorialStaticWorldSceneProof {
            tile_id: tile.proof.tile_id.clone(),
            path: scene_path,
            source_scene_blake3: tile.scene_blake3.clone(),
            static_fields_blake3: static_blake3,
            installed_bytes: scene_entry.bytes,
            installed_blake3: merged_blake3,
            model_count: tile.model_count,
            visual_count: tile.visual_count,
            collider_count: tile.collider_count,
        });
        publications.push(Publication {
            entry: scene_entry,
            content: PublicationContent::Generated(merged_bytes),
        });
    }

    owned_files.sort_by(|left, right| left.path.cmp(&right.path));
    scene_proofs.sort_by(|left, right| left.path.cmp(&right.path));
    tile_proofs.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    let source_set_blake3 = source_set_blake3(&tile_proofs);
    if cleanup_archived_previous
        && validate_cleanup_noop_source(
            previous
                .as_ref()
                .expect("cleanup-archived install was loaded above"),
            &tile_proofs,
            &owned_files,
            &scene_proofs,
        )
        .is_ok()
    {
        verify_current_reference_documents(&asset_root, &manifest, &scene_proofs)?;
        return Ok(build_install_report(
            &options.source_build,
            contracts,
            model_count,
            texture_count,
            static_metadata_count,
            0,
            0,
            manifest.files.len() as u64,
            true,
            source_set_blake3,
        ));
    }
    // A cleanup-archived install may still receive a genuinely new, fully
    // audited source set. In that case continue through the normal atomic
    // publication path, temporarily restoring its conversion metadata and
    // active ownership proof. `clean-runtime-metadata --apply` archives that
    // new revision again after installation.
    let (reference_publications, reference_proofs) = build_reference_publications(
        &asset_root,
        &manifest,
        &scene_proofs,
        &current_scene_blake3,
        allow_legacy_stale_hashes,
    )?;
    publications.extend(reference_publications);
    let ownership = TutorialStaticWorldOwnership {
        schema: scope.ownership_schema().to_owned(),
        installer: scope.installer_id().to_owned(),
        source_build: options.source_build.clone(),
        source_set_blake3: source_set_blake3.clone(),
        winding_repair: None,
        tiles: tile_proofs,
        owned_files,
        scenes: scene_proofs,
        references: reference_proofs,
    };
    let ownership_bytes = pretty_json(&ownership)?;
    let ownership_entry = ProjectAssetFile {
        source_path: scope.installer_id().to_owned(),
        path: scope.ownership_path().to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: ownership_bytes.len() as u64,
        blake3: hash_bytes(&ownership_bytes),
    };
    publications.push(Publication {
        entry: ownership_entry,
        content: PublicationContent::Generated(ownership_bytes),
    });

    let remove_paths =
        previous_publication_paths(previous.as_ref(), contracts, &ownership.references);
    manifest
        .files
        .retain(|entry| !remove_paths.contains(&entry.path));
    ensure_publication_paths_are_available(&manifest, &publications)?;
    manifest.files.extend(
        publications
            .iter()
            .map(|publication| publication.entry.clone()),
    );
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    validate_project_manifest(&manifest)?;
    let next_manifest_bytes = pretty_json(&manifest)?;

    let token = unique_token();
    let stage = asset_root.join(format!(".tutorial-static-world-stage-{token}"));
    let backup = asset_root.join(format!(".tutorial-static-world-backup-{token}"));
    if stage.exists() || backup.exists() {
        return invalid("tutorial static world transaction path collision");
    }
    fs::create_dir(&stage).map_err(|error| io_at(&stage, error))?;
    let stage_result = stage_publication(
        &stage,
        contracts,
        &publications,
        &next_manifest_bytes,
        &ownership.scenes,
        &ownership.references,
    );
    if let Err(error) = stage_result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let replaced_previous_install = previous.is_some();
    if let Err(error) = commit_publication(
        &asset_root,
        &stage,
        &backup,
        contracts,
        &ownership.references,
        replaced_previous_install,
        cleanup_archived_previous,
    ) {
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_dir_all(&backup);
        return Err(error);
    }

    let installed_bytes = publications
        .iter()
        .map(|publication| publication.entry.bytes)
        .sum::<u64>();
    let installed_files = publications.len() as u64;
    Ok(build_install_report(
        &options.source_build,
        contracts,
        model_count,
        texture_count,
        static_metadata_count,
        installed_files,
        installed_bytes,
        manifest.files.len() as u64,
        replaced_previous_install,
        source_set_blake3,
    ))
}

pub(super) fn install_manifestless_with_contract(
    options: &TutorialStaticWorldInstallOptions,
    contracts: &[TileContract],
    scope: StaticWorldScope,
    export_root: &Path,
    asset_root: &Path,
) -> Result<TutorialStaticWorldInstallReport> {
    let ownership_path = safe_join(asset_root, scope.ownership_path())?;
    let ownership_bytes = read_regular_file(&ownership_path, "static-world domain ownership")?;
    let previous: TutorialStaticWorldOwnership = parse_json(&ownership_bytes, &ownership_path)?;
    if previous.schema != scope.ownership_schema()
        || previous.installer != scope.installer_id()
        || previous.source_build != options.source_build
        || previous.tiles.len() != contracts.len()
        || previous.scenes.len() != contracts.len()
        || source_set_blake3(&previous.tiles) != previous.source_set_blake3
    {
        return invalid("existing manifestless static-world ownership has the wrong identity");
    }
    let expected_tiles = contracts
        .iter()
        .map(|contract| contract.id.as_ref())
        .collect::<BTreeSet<_>>();
    let owned_tiles = previous
        .tiles
        .iter()
        .map(|tile| tile.tile_id.as_str())
        .collect::<BTreeSet<_>>();
    let scene_tiles = previous
        .scenes
        .iter()
        .map(|scene| scene.tile_id.as_str())
        .collect::<BTreeSet<_>>();
    if expected_tiles != owned_tiles || expected_tiles != scene_tiles {
        return invalid(
            "existing manifestless static-world ownership has incomplete tile coverage",
        );
    }
    for contract in contracts {
        let model_root = safe_join(asset_root, &model_tile_root(&contract.id))?;
        if !model_root.is_dir() {
            return invalid(format!(
                "owned static-world model tree is missing: {}",
                model_root.display()
            ));
        }
    }
    if !safe_join(asset_root, scope.static_root())?.is_dir() {
        return invalid("owned static-world metadata tree is missing");
    }

    let mut audited = Vec::with_capacity(contracts.len());
    for contract in contracts {
        audited.push(audit_tile_export(
            export_root,
            contract,
            &options.source_build,
        )?);
    }
    let previous_scenes = previous
        .scenes
        .iter()
        .map(|scene| (scene.tile_id.as_str(), scene))
        .collect::<BTreeMap<_, _>>();

    let mut publications = Vec::<Publication>::new();
    let mut owned_files = Vec::<TutorialStaticWorldOwnedFile>::new();
    let mut scene_proofs = Vec::<TutorialStaticWorldSceneProof>::new();
    let mut tile_proofs = Vec::<TutorialStaticWorldTileProof>::new();
    let mut model_count = 0_u64;
    let mut texture_count = 0_u64;
    let mut static_metadata_count = 0_u64;
    let mut scene_hashes = BTreeMap::<String, String>::new();

    for tile in &audited {
        tile_proofs.push(tile.proof.clone());
        for source in &tile.selected {
            match source.kind {
                ProjectAssetKind::Model => model_count = model_count.saturating_add(1),
                ProjectAssetKind::Texture => texture_count = texture_count.saturating_add(1),
                ProjectAssetKind::Data => {
                    static_metadata_count = static_metadata_count.saturating_add(1)
                }
                _ => {
                    return invalid(format!(
                        "unsupported static-world publication kind {:?} at {:?}",
                        source.kind, source.path
                    ));
                }
            }
            let entry = ProjectAssetFile {
                source_path: source.source_path.clone(),
                path: source.path.clone(),
                kind: source.kind,
                bytes: source.bytes,
                blake3: source.blake3.clone(),
            };
            owned_files.push(TutorialStaticWorldOwnedFile {
                source_path: entry.source_path.clone(),
                path: entry.path.clone(),
                kind: entry.kind,
                bytes: entry.bytes,
                blake3: entry.blake3.clone(),
            });
            publications.push(Publication {
                entry,
                content: PublicationContent::Source(source.source.clone()),
            });
        }

        let scene_path = tile_scene_path(&tile.proof.tile_id);
        let current_path = safe_join(asset_root, &scene_path)?;
        let current_bytes = read_regular_file(&current_path, "installed native terrain scene")?;
        let current: JsonValue = parse_json(&current_bytes, &current_path)?;
        let previous_scene = previous_scenes
            .get(tile.proof.tile_id.as_str())
            .copied()
            .ok_or_else(|| {
                invalid_error(format!(
                    "manifestless ownership lacks scene proof for {:?}",
                    tile.proof.tile_id
                ))
            })?;
        validate_installed_scene_identity(&current, previous_scene)?;
        if static_fields_blake3(&current)? != previous_scene.static_fields_blake3 {
            return invalid(format!(
                "installed static scene fields changed outside their domain at {:?}",
                scene_path
            ));
        }
        if !scene_non_static_identity_matches(&current, &tile.scene)? {
            return invalid(format!(
                "installed terrain identity differs from fresh exact export at {:?}",
                scene_path
            ));
        }
        let merged = merge_scene(
            &current,
            &tile.scene,
            Some(previous_scene),
            &tile.proof.tile_id,
        )?;
        let merged_bytes = pretty_json(&merged)?;
        let merged_blake3 = hash_bytes(&merged_bytes);
        scene_hashes.insert(scene_path.clone(), merged_blake3.clone());
        let scene_entry = ProjectAssetFile {
            source_path: format!(
                "native-terrain-export+{}/{}/{}",
                scope.source_path_prefix(),
                tile.proof.tile_id,
                scene_path
            ),
            path: scene_path.clone(),
            kind: ProjectAssetKind::Data,
            bytes: merged_bytes.len() as u64,
            blake3: merged_blake3.clone(),
        };
        scene_proofs.push(TutorialStaticWorldSceneProof {
            tile_id: tile.proof.tile_id.clone(),
            path: scene_path,
            source_scene_blake3: tile.scene_blake3.clone(),
            static_fields_blake3: static_fields_blake3(&merged)?,
            installed_bytes: scene_entry.bytes,
            installed_blake3: merged_blake3,
            model_count: tile.model_count,
            visual_count: tile.visual_count,
            collider_count: tile.collider_count,
        });
        publications.push(Publication {
            entry: scene_entry,
            content: PublicationContent::Generated(merged_bytes),
        });
    }

    owned_files.sort_by(|left, right| left.path.cmp(&right.path));
    scene_proofs.sort_by(|left, right| left.path.cmp(&right.path));
    tile_proofs.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    let source_set_blake3 = source_set_blake3(&tile_proofs);

    let registry_path = safe_join(asset_root, RUNTIME_WORLD_REGISTRY_PATH)?;
    let registry_bytes = read_regular_file(&registry_path, "runtime world registry")?;
    let mut registry: JsonValue = parse_json(&registry_bytes, &registry_path)?;
    if registry.get("schema").and_then(JsonValue::as_str) != Some(RUNTIME_WORLD_REGISTRY_SCHEMA) {
        return invalid("runtime world registry has the wrong schema");
    }
    let entries = registry
        .get_mut("entries")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("runtime world registry has no entries"))?;
    let mut updated_scenes = BTreeSet::new();
    for entry in entries {
        let Some(scene) = entry.get_mut("scene").and_then(JsonValue::as_object_mut) else {
            continue;
        };
        let Some(path) = scene
            .get("path")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let Some(blake3) = scene_hashes.get(&path) else {
            continue;
        };
        scene.insert("blake3".to_owned(), JsonValue::String(blake3.clone()));
        updated_scenes.insert(path);
    }
    if updated_scenes.len() != scene_hashes.len() {
        return invalid(format!(
            "runtime world registry updated {} of {} exact static scenes",
            updated_scenes.len(),
            scene_hashes.len()
        ));
    }
    let next_registry_bytes = pretty_json(&registry)?;
    let registry_proof = TutorialStaticWorldReferenceProof {
        path: RUNTIME_WORLD_REGISTRY_PATH.to_owned(),
        bytes: next_registry_bytes.len() as u64,
        blake3: hash_bytes(&next_registry_bytes),
        scene_count: scene_hashes.len() as u64,
    };
    publications.push(Publication {
        entry: ProjectAssetFile {
            source_path: scope.installer_id().to_owned(),
            path: RUNTIME_WORLD_REGISTRY_PATH.to_owned(),
            kind: ProjectAssetKind::Data,
            bytes: registry_proof.bytes,
            blake3: registry_proof.blake3.clone(),
        },
        content: PublicationContent::Generated(next_registry_bytes),
    });

    let ownership = TutorialStaticWorldOwnership {
        schema: scope.ownership_schema().to_owned(),
        installer: scope.installer_id().to_owned(),
        source_build: options.source_build.clone(),
        source_set_blake3: source_set_blake3.clone(),
        winding_repair: None,
        tiles: tile_proofs,
        owned_files,
        scenes: scene_proofs,
        references: vec![registry_proof],
    };
    let next_ownership_bytes = pretty_json(&ownership)?;
    publications.push(Publication {
        entry: ProjectAssetFile {
            source_path: scope.installer_id().to_owned(),
            path: scope.ownership_path().to_owned(),
            kind: ProjectAssetKind::Data,
            bytes: next_ownership_bytes.len() as u64,
            blake3: hash_bytes(&next_ownership_bytes),
        },
        content: PublicationContent::Generated(next_ownership_bytes),
    });

    let token = unique_token();
    let stage = asset_root.join(format!(".static-world-domain-stage-{token}"));
    let backup = asset_root.join(format!(".static-world-domain-backup-{token}"));
    if stage.exists() || backup.exists() {
        return invalid("static-world domain transaction path collision");
    }
    fs::create_dir(&stage).map_err(|error| io_at(&stage, error))?;
    if let Err(error) = stage_manifestless_publication(&stage, &publications) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    if let Err(error) = commit_manifestless_publication(
        asset_root,
        &stage,
        &backup,
        contracts,
        scope,
        &scene_hashes,
    ) {
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_dir_all(&backup);
        return Err(error);
    }

    let installed_bytes = publications
        .iter()
        .map(|publication| publication.entry.bytes)
        .sum::<u64>();
    Ok(build_install_report(
        &options.source_build,
        contracts,
        model_count,
        texture_count,
        static_metadata_count,
        publications.len() as u64,
        installed_bytes,
        0,
        true,
        source_set_blake3,
    ))
}
