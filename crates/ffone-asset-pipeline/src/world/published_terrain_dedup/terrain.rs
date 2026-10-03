use super::*;

pub const PUBLISHED_TERRAIN_DEDUP_SCHEMA: &str = "ffone.published-terrain-dedup.v1";

pub const PUBLISHED_TERRAIN_SHIFT_RESTORE_SCHEMA: &str = "ffone.published-terrain-shift-restore.v1";

#[derive(Clone, Debug)]
pub struct PublishedTerrainDedupOptions {
    pub project_root: PathBuf,
    pub report_path: PathBuf,
    pub apply: bool,
}

impl PublishedTerrainDedupOptions {
    #[must_use]
    pub fn new(
        project_root: impl Into<PathBuf>,
        report_path: impl Into<PathBuf>,
        apply: bool,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            report_path: report_path.into(),
            apply,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedTerrainDedupCounts {
    pub tiles: u64,
    pub detail_tiles: u64,
    pub detail_references: u64,
    pub detail_prototype_references: u64,
    pub detail_source_packages: u64,
    pub detail_shared_packages: u64,
    pub detail_redundant_packages: u64,
    pub repaired_detail_document_hashes: u64,
    pub collapsed_detail_base_mip_zero: u64,
    pub collapsed_weight_base_mip_zero: u64,
    pub collapsed_lightmap_base_mip_zero: u64,
    pub removed_files: u64,
    pub shared_files: u64,
    pub source_bytes_removed: u64,
    pub shared_bytes_added: u64,
    pub estimated_net_bytes_saved: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedTerrainDetailRoute {
    pub true_texture_name: String,
    pub closure_blake3: String,
    pub uses: u64,
    pub source_bytes: u64,
    pub shared_bytes: u64,
    pub destination: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedTerrainDedupReport {
    pub schema: String,
    pub mode: String,
    pub source_alias: String,
    pub source_build: String,
    pub policy: String,
    pub counts: PublishedTerrainDedupCounts,
    pub source_catalog_blake3: String,
    pub result_catalog_blake3: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<PublishedTerrainDedupVerification>,
    pub routes: Vec<PublishedTerrainDetailRoute>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedTerrainDedupVerification {
    pub catalog_blake3: String,
    pub files: u64,
    pub bytes: u64,
    pub result_set_blake3: String,
    pub scene_links: u64,
}

#[derive(Clone, Debug)]
pub struct PublishedTerrainShiftRestoreOptions {
    pub project_root: PathBuf,
    pub source_export_root: PathBuf,
    pub report_path: PathBuf,
    pub apply: bool,
}

impl PublishedTerrainShiftRestoreOptions {
    #[must_use]
    pub fn new(
        project_root: impl Into<PathBuf>,
        source_export_root: impl Into<PathBuf>,
        report_path: impl Into<PathBuf>,
        apply: bool,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            source_export_root: source_export_root.into(),
            report_path: report_path.into(),
            apply,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedTerrainShiftRestoreCounts {
    pub published_tiles: u64,
    pub primary_world_tiles: u64,
    pub restored_tiles: u64,
    pub already_complete_tiles: u64,
    pub primary_tiles_without_shifts: u64,
    pub restored_vertex_shifts: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedTerrainShiftRestoreTile {
    pub tile_id: String,
    pub vertex_shifts: u64,
    pub source_terrain_blake3: String,
    pub previous_terrain_blake3: String,
    pub restored_terrain_blake3: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedTerrainShiftRestoreReport {
    pub schema: String,
    pub mode: String,
    pub source_alias: String,
    pub source_build: String,
    pub source_export_manifest_blake3: String,
    pub policy: String,
    pub counts: PublishedTerrainShiftRestoreCounts,
    pub source_catalog_blake3: String,
    pub result_catalog_blake3: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<PublishedTerrainDedupVerification>,
    pub tiles: Vec<PublishedTerrainShiftRestoreTile>,
}

/// Plans or atomically applies exact physical terrain deduplication.
pub fn dedupe_published_terrain(
    options: &PublishedTerrainDedupOptions,
) -> Result<PublishedTerrainDedupReport> {
    let project_root = canonical_directory(&options.project_root, "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let map_root = canonical_directory(&asset_root.join("map"), "map root")?;
    let report_path = checked_report_path(
        &project_root,
        &asset_root,
        &options.report_path,
        "terrain dedup report",
    )?;

    // The migration is only safe from a complete, currently accepted physical
    // release closure. Keep this deliberately independent from evolving GLB
    // semantic checks: a converter upgrade may reject old-yet-accepted mesh
    // layout while every byte/hash and ownership edge remains valid.
    verify_physical_library(&asset_root, &map_root, true)?;
    let plan = build_plan(&asset_root, &map_root)?;
    let mut verification = None;
    if options.apply {
        verification = Some(apply_plan(&project_root, &asset_root, &map_root, &plan)?);
    }

    let report = PublishedTerrainDedupReport {
        schema: PUBLISHED_TERRAIN_DEDUP_SCHEMA.to_owned(),
        mode: if options.apply { "apply" } else { "plan" }.to_owned(),
        source_alias: "primary".to_owned(),
        source_build: "retrobution-20260613".to_owned(),
        policy: "exact complete terrain-detail closures are stored once under map/shared/terrain/details; logical Unity identity remains in every tile record/prototype; only byte-verified duplicate base/mip-zero files are collapsed; scene, tile and catalog acceptance hashes are refreshed transactionally"
            .to_owned(),
        counts: plan.counts,
        source_catalog_blake3: plan.source_catalog_blake3,
        result_catalog_blake3: plan.result_catalog_blake3,
        verification,
        routes: plan.routes,
    };
    write_replace(&report_path, &pretty_json(&report)?)?;
    Ok(report)
}

/// Restores Unity's terrain vertex-shift topology from a fresh clean-primary
/// native-terrain export and atomically refreshes the published hash chain.
///
/// Height samples alone cannot represent Unity's authored vertical walls.
/// Legacy `m_Heightmap.m_Shifts` moves selected copies of shared grid vertices
/// by one sample before triangulation. Dropping it turns pool walls, cliffs and
/// other right-angle relief into smooth ramps even though the heightmap PNG is
/// byte-perfect.
pub fn restore_published_terrain_shifts(
    options: &PublishedTerrainShiftRestoreOptions,
) -> Result<PublishedTerrainShiftRestoreReport> {
    let project_root = canonical_directory(&options.project_root, "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let map_root = canonical_directory(&asset_root.join("map"), "map root")?;
    let source_export_root =
        canonical_directory(&options.source_export_root, "primary terrain export root")?;
    let report_path = checked_report_path(
        &project_root,
        &asset_root,
        &options.report_path,
        "terrain shift restore report",
    )?;

    verify_primary_terrain_export(&project_root, &source_export_root)?;
    verify_physical_library(&asset_root, &map_root, true)?;
    let (plan, counts, tiles, source_export_manifest_blake3) =
        build_shift_restore_plan(&asset_root, &map_root, &source_export_root)?;
    let verification = if options.apply {
        Some(apply_plan(&project_root, &asset_root, &map_root, &plan)?)
    } else {
        None
    };
    let report = PublishedTerrainShiftRestoreReport {
        schema: PUBLISHED_TERRAIN_SHIFT_RESTORE_SCHEMA.to_owned(),
        mode: if options.apply { "apply" } else { "plan" }.to_owned(),
        source_alias: "primary".to_owned(),
        source_build: "retrobution-20260613".to_owned(),
        source_export_manifest_blake3,
        policy: "restore only exact clean-primary m_Heightmap.m_Shifts and its nativeGeometry decoding contract after proving the published and primary heightmap, dimensions, scale, orientation and remaining geometry contract are identical; refresh terrain, scene, tile and catalog acceptance hashes in one rollback-safe transaction"
            .to_owned(),
        counts,
        source_catalog_blake3: plan.source_catalog_blake3,
        result_catalog_blake3: plan.result_catalog_blake3,
        verification,
        tiles,
    };
    write_replace(&report_path, &pretty_json(&report)?)?;
    Ok(report)
}

pub(super) fn verify_primary_terrain_export(project_root: &Path, source_export_root: &Path) -> Result<()> {
    let manifest_path = source_export_root.join("manifest.json");
    let manifest_bytes = read_file(&manifest_path, "primary terrain export manifest")?;
    let manifest: JsonValue =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.get("schema").and_then(JsonValue::as_str) != Some("ffone.native-terrain-batch.v1")
        || manifest
            .pointer("/source/inputKind")
            .and_then(JsonValue::as_str)
            != Some("effective-build-root")
    {
        return invalid("terrain shift source is not a native-terrain effective-build export");
    }
    let declared = manifest
        .pointer("/source/effectiveBuildRoot")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error("terrain export has no effective build root"))?;
    let declared = canonical_directory(Path::new(declared), "terrain export source build")?;
    let expected = canonical_directory(
        &project_root
            .parent()
            .ok_or_else(|| invalid_error("project root has no parent"))?
            .join("builds/retrobution-20260613"),
        "clean primary build",
    )?;
    if declared != expected {
        return invalid(format!(
            "terrain shift source is not clean primary: declared={}, expected={}",
            declared.display(),
            expected.display()
        ));
    }
    Ok(())
}

pub(super) fn validate_migrated_terrain_references(
    asset_root: &Path,
    map_root: &Path,
    terrain: &JsonValue,
    tile_id: &str,
) -> Result<()> {
    let Some(textures) = terrain
        .pointer("/detailAndTrees/textures")
        .and_then(JsonValue::as_array)
    else {
        return Ok(());
    };
    let mut routes = BTreeMap::new();
    for texture in textures {
        let document_path = required_string(texture, "documentPath", "shared detail document")?;
        if !document_path.starts_with(DETAIL_SHARED_PREFIX) {
            return invalid(format!("tile {tile_id} retained a local detail document"));
        }
        let bytes = read_asset_from_roots(asset_root, map_root, document_path, "shared detail")?;
        if format!("blake3:{}", hash(&bytes))
            != required_string(texture, "documentBlake3", "shared detail document hash")?
        {
            return invalid(format!("tile {tile_id} has a stale shared detail hash"));
        }
        let texture_path = required_string(texture, "path", "shared detail texture path")?;
        read_asset_from_roots(asset_root, map_root, texture_path, "shared detail texture")?;
        routes.insert(document_path.to_owned(), (texture_path.to_owned(), bytes));
    }
    if let Some(prototypes) = terrain
        .pointer("/detailAndTrees/prototypes")
        .and_then(JsonValue::as_array)
    {
        for prototype in prototypes {
            let Some(reference) = prototype.get("prototypeTexture") else {
                continue;
            };
            let document = required_string(reference, "documentPath", "prototype document")?;
            let (path, bytes) = routes.get(document).ok_or_else(|| {
                invalid_error(format!(
                    "tile {tile_id} prototype has no shared texture record"
                ))
            })?;
            if required_string(reference, "path", "prototype texture path")? != path
                || required_string(reference, "documentBlake3", "prototype document hash")?
                    != format!("blake3:{}", hash(bytes))
            {
                return invalid(format!("tile {tile_id} prototype shared route is stale"));
            }
        }
    }
    Ok(())
}
