use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    ASSET_MANIFEST_FILE, PipelineError, ProjectAssetKind, ProjectAssetManifest, Result,
    error::io_at,
};

pub const MODEL_AUDIT_SCHEMA: &str = "ffone.model-audit.v1";
pub const GEOMETRY_DUMP_STATUS: &str = "geometry_dump_not_publishable";

#[derive(Clone, Debug)]
pub struct ModelAuditOptions {
    pub assets: PathBuf,
    pub cook_report: PathBuf,
    pub layout_report: PathBuf,
    pub table_set: PathBuf,
    pub output: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlbAuditStats {
    pub models: u64,
    pub hash_suffixed: u64,
    pub invalid: u64,
    pub multi_node: u64,
    pub multi_primitive: u64,
    pub with_materials: u64,
    pub with_skins: u64,
    pub with_animations: u64,
    pub with_joints: u64,
    pub with_weights: u64,
    pub nodes: u64,
    pub primitives: u64,
    pub materials: u64,
    pub skins: u64,
    pub animations: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceCoverage {
    pub cook_complete: bool,
    pub native_only: bool,
    pub emitted_meshes: u64,
    pub bundle_files_scanned: u64,
    pub bundle_objects_scanned: u64,
    pub supported_objects: u64,
    pub skipped_objects: u64,
    pub errors: u64,
    pub relevant_object_types: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameplayRoots {
    pub npc_referenced_slots: u64,
    pub npc_unique_names: u64,
    pub nano_referenced_slots: u64,
    pub nano_unique_names: u64,
    pub item_unique_names_by_family: BTreeMap<String, u64>,
    pub item_unique_names: u64,
    pub player_base_names: u64,
    pub known_unique_lower_bound: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingCoverage {
    pub output_container_routes: u64,
    pub source_container_routes: u64,
    pub semantic_routes: u64,
    pub semantic_routes_by_family: BTreeMap<String, u64>,
    pub route_conflicts: u64,
    pub tile_scoped_route_exceptions: u64,
    pub unclassified_roots: u64,
    pub exact_global_logical_model_roots: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelAuditReport {
    pub schema: String,
    pub status: String,
    pub audited_asset_tree: String,
    pub current_tree_preserved: bool,
    pub suggested_staging_root: String,
    pub glb: GlbAuditStats,
    pub source: SourceCoverage,
    pub gameplay_roots: GameplayRoots,
    pub routing: RoutingCoverage,
    pub blocking_reasons: Vec<String>,
}

pub fn audit_models(options: &ModelAuditOptions) -> Result<ModelAuditReport> {
    if options.output.starts_with(&options.assets) {
        return audit_error("audit output must be outside the immutable asset tree");
    }
    let manifest: ProjectAssetManifest = read_json(&options.assets.join(ASSET_MANIFEST_FILE))?;
    let cook: serde_json::Value = read_json(&options.cook_report)?;
    let layout: serde_json::Value = read_json(&options.layout_report)?;
    let tables: serde_json::Value = read_json(&options.table_set)?;
    let report = ModelAuditReport {
        schema: MODEL_AUDIT_SCHEMA.to_owned(),
        status: GEOMETRY_DUMP_STATUS.to_owned(),
        audited_asset_tree: options.assets.to_string_lossy().replace('\\', "/"),
        current_tree_preserved: true,
        suggested_staging_root: "assets/game-v2".to_owned(),
        glb: scan_glbs(&options.assets, &manifest)?,
        source: source_coverage(&cook)?,
        gameplay_roots: gameplay_roots(&tables)?,
        routing: routing_coverage(&layout)?,
        blocking_reasons: vec![
            "current GLBs are per-Mesh geometry dumps, not AssetBundle-route logical models".into(),
            "material, skin, hierarchy and animation preservation is not proven".into(),
            "legacy keyframe slopes/tangentMode, Euler curves, PPtr curves and events need lossless decoding".into(),
            "v1 reports omit enough route graph detail to count every world/effect logical root".into(),
            "publish to assets/game-v2 only after every model passes ffone.logical-model.v1".into(),
        ],
    };
    write_report(&options.output, &report)?;
    Ok(report)
}

fn audit_error<T>(reason: impl Into<String>) -> Result<T> {
    Err(PipelineError::ModelAudit(reason.into()))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
        path: path.to_string_lossy().into_owned(),
        source,
    })
}

fn write_report(path: &Path, report: &ModelAuditReport) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut bytes = serde_json::to_vec_pretty(report).map_err(|source| PipelineError::Json {
        path: path.to_string_lossy().into_owned(),
        source,
    })?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|error| io_at(path, error))
}

fn scan_glbs(root: &Path, manifest: &ProjectAssetManifest) -> Result<GlbAuditStats> {
    let mut files = manifest
        .files
        .iter()
        .filter(|file| file.kind == ProjectAssetKind::Model)
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let mut stats = GlbAuditStats::default();
    for file in files {
        stats.models += 1;
        if has_opaque_suffix(Path::new(&file.path)) {
            stats.hash_suffixed += 1;
        }
        let path = safe_asset_path(root, &file.path)?;
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        let Some(document) = parse_glb_json(&bytes) else {
            stats.invalid += 1;
            continue;
        };
        let nodes = array_len(&document, "nodes");
        let materials = array_len(&document, "materials");
        let skins = array_len(&document, "skins");
        let animations = array_len(&document, "animations");
        let mut primitives = 0_u64;
        let mut has_joints = false;
        let mut has_weights = false;
        for primitive in document
            .get("meshes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|mesh| mesh.get("primitives"))
            .filter_map(serde_json::Value::as_array)
            .flatten()
        {
            primitives += 1;
            let attributes = primitive.get("attributes");
            has_joints |= attributes.and_then(|value| value.get("JOINTS_0")).is_some();
            has_weights |= attributes
                .and_then(|value| value.get("WEIGHTS_0"))
                .is_some();
        }
        stats.nodes += nodes;
        stats.primitives += primitives;
        stats.materials += materials;
        stats.skins += skins;
        stats.animations += animations;
        stats.multi_node += u64::from(nodes > 1);
        stats.multi_primitive += u64::from(primitives > 1);
        stats.with_materials += u64::from(materials > 0);
        stats.with_skins += u64::from(skins > 0);
        stats.with_animations += u64::from(animations > 0);
        stats.with_joints += u64::from(has_joints);
        stats.with_weights += u64::from(has_weights);
    }
    Ok(stats)
}

fn safe_asset_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return audit_error(format!("asset manifest path escapes root: {relative:?}"));
    }
    Ok(root.join(path))
}

fn has_opaque_suffix(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.rsplit_once("--"))
        .is_some_and(|(_, suffix)| {
            (suffix.len() >= 8 && suffix.chars().all(|value| value.is_ascii_hexdigit()))
                || suffix
                    .strip_prefix("pathid-")
                    .is_some_and(|value| value.chars().all(|character| character.is_ascii_digit()))
        })
}

fn parse_glb_json(bytes: &[u8]) -> Option<serde_json::Value> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let declared_len = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
    let chunk_type = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if version != 2
        || declared_len != bytes.len()
        || chunk_type != 0x4e4f_534a
        || 20 + json_len > bytes.len()
    {
        return None;
    }
    serde_json::from_slice(&bytes[20..20 + json_len]).ok()
}

fn array_len(document: &serde_json::Value, key: &str) -> u64 {
    document
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map_or(0, |values| values.len() as u64)
}

fn source_coverage(cook: &serde_json::Value) -> Result<SourceCoverage> {
    let types = cook
        .pointer("/coverage/bundleTypes")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| PipelineError::ModelAudit("cook report has no bundleTypes".into()))?;
    let relevant = [
        "GameObject",
        "Transform",
        "MeshFilter",
        "MeshRenderer",
        "SkinnedMeshRenderer",
        "Mesh",
        "Material",
        "Animation",
        "AnimationClip",
        "MeshCollider",
    ];
    let relevant_object_types = relevant
        .into_iter()
        .map(|name| {
            let count = types
                .get(name)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            (name.to_owned(), count)
        })
        .collect();
    Ok(SourceCoverage {
        cook_complete: required_bool(cook, "/complete")?,
        native_only: required_bool(cook, "/nativeOnly")?,
        emitted_meshes: required_u64(cook, "/counts/mesh")?,
        bundle_files_scanned: required_u64(cook, "/coverage/bundleFilesScanned")?,
        bundle_objects_scanned: required_u64(cook, "/coverage/bundleObjectsScanned")?,
        supported_objects: required_u64(cook, "/coverage/bundleSupportedObjects")?,
        skipped_objects: required_u64(cook, "/coverage/bundleSkipped")?,
        errors: required_u64(cook, "/coverage/errors")?,
        relevant_object_types,
    })
}

fn routing_coverage(layout: &serde_json::Value) -> Result<RoutingCoverage> {
    let output_container_routes = sum_route_counts(
        layout
            .get("parts")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| PipelineError::ModelAudit("layout report has no parts".into()))?,
        "containerRouteCount",
    );
    let source_container_routes = sum_route_counts(
        layout
            .get("sourceBundles")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                PipelineError::ModelAudit("layout report has no sourceBundles".into())
            })?,
        "containerRoutes",
    );
    let semantic_routes_by_family = layout
        .pointer("/semanticIndex/routesByFamily")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| PipelineError::ModelAudit("layout report has no routesByFamily".into()))?
        .iter()
        .map(|(name, count)| (name.clone(), count.as_u64().unwrap_or(0)))
        .collect();
    Ok(RoutingCoverage {
        output_container_routes,
        source_container_routes,
        semantic_routes: required_u64(layout, "/semanticIndex/routes")?,
        semantic_routes_by_family,
        route_conflicts: required_array_len(layout, "/result/routeConflicts")?,
        tile_scoped_route_exceptions: required_array_len(
            layout,
            "/result/tileScopedRouteExceptions",
        )?,
        unclassified_roots: required_array_len(layout, "/result/unclassifiedRoots")?,
        // v1 retains counts, not the normalized route extension/graph identities needed here.
        exact_global_logical_model_roots: None,
    })
}

fn sum_route_counts(values: &[serde_json::Value], field: &str) -> u64 {
    values
        .iter()
        .filter_map(|value| value.get(field))
        .filter_map(serde_json::Value::as_u64)
        .sum()
}

fn required_u64(document: &serde_json::Value, pointer: &str) -> Result<u64> {
    document
        .pointer(pointer)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| PipelineError::ModelAudit(format!("missing integer {pointer}")))
}

fn required_bool(document: &serde_json::Value, pointer: &str) -> Result<bool> {
    document
        .pointer(pointer)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| PipelineError::ModelAudit(format!("missing boolean {pointer}")))
}

fn required_array_len(document: &serde_json::Value, pointer: &str) -> Result<u64> {
    document
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
        .map(|values| values.len() as u64)
        .ok_or_else(|| PipelineError::ModelAudit(format!("missing array {pointer}")))
}

fn gameplay_roots(table_set: &serde_json::Value) -> Result<GameplayRoots> {
    let value = table_set
        .get("tables")
        .and_then(serde_json::Value::as_array)
        .and_then(|tables| {
            tables.iter().find(|table| {
                table.get("name").and_then(serde_json::Value::as_str)
                    == Some("npc_imports_consolidated")
            })
        })
        .and_then(|table| table.get("value"))
        .ok_or_else(|| PipelineError::ModelAudit("native table set lacks npc imports".into()))?;

    let npc_table = required_object(value, "m_pNpcTable")?;
    let npc_meshes = required_array(npc_table, "m_pNpcMeshData")?;
    let npc_refs = required_array(npc_table, "m_pNpcData")?
        .iter()
        .filter(|row| row.get("m_iNpcNumber").and_then(serde_json::Value::as_i64) > Some(0))
        .filter(|row| row.get("m_iHNpc").and_then(serde_json::Value::as_i64) == Some(0))
        .filter_map(|row| row.get("m_iMesh").and_then(serde_json::Value::as_u64))
        .collect::<BTreeSet<_>>();
    let (npc_referenced_slots, npc_names) =
        names_for_references(npc_meshes, &npc_refs, &["m_pstrMMeshModelString"]);

    let nano_table = required_object(value, "m_pNanoTable")?;
    let nano_meshes = required_array(nano_table, "m_pNanoMeshData")?;
    let nano_refs = required_array(nano_table, "m_pNanoData")?
        .iter()
        .filter_map(|row| row.get("m_iMesh").and_then(serde_json::Value::as_u64))
        .collect::<BTreeSet<_>>();
    let (nano_referenced_slots, nano_names) =
        names_for_references(nano_meshes, &nano_refs, &["m_pstrMMeshModelString"]);

    let item_tables = [
        ("back", "m_pBackItemTable"),
        ("face", "m_pFaceItemTable"),
        ("glass", "m_pGlassItemTable"),
        ("hat", "m_pHatItemTable"),
        ("head", "m_pHeadItemTable"),
        ("pants", "m_pPantsItemTable"),
        ("shirts", "m_pShirtsItemTable"),
        ("shoes", "m_pShoesItemTable"),
        ("vehicle", "m_pVehicleItemTable"),
        ("weapon", "m_pWeaponItemTable"),
    ];
    let mut item_unique_names_by_family = BTreeMap::new();
    for (family, table_name) in item_tables {
        let table = required_object(value, table_name)?;
        let meshes = required_array(table, "m_pItemMeshData")?;
        let references = required_array(table, "m_pItemData")?
            .iter()
            .filter_map(|row| row.get("m_iMesh").and_then(serde_json::Value::as_u64))
            .collect::<BTreeSet<_>>();
        let (_, names) = names_for_references(
            meshes,
            &references,
            &["m_pstrFMeshModelString", "m_pstrMMeshModelString"],
        );
        item_unique_names_by_family.insert(family.to_owned(), names.len() as u64);
    }
    let item_unique_names = item_unique_names_by_family.values().sum::<u64>();
    let player_base_names = 2;
    Ok(GameplayRoots {
        npc_referenced_slots,
        npc_unique_names: npc_names.len() as u64,
        nano_referenced_slots,
        nano_unique_names: nano_names.len() as u64,
        item_unique_names_by_family,
        item_unique_names,
        player_base_names,
        known_unique_lower_bound: npc_names.len() as u64
            + nano_names.len() as u64
            + item_unique_names
            + player_base_names,
    })
}

fn names_for_references(
    meshes: &[serde_json::Value],
    references: &BTreeSet<u64>,
    fields: &[&str],
) -> (u64, BTreeSet<String>) {
    let mut slots = 0;
    let mut names = BTreeSet::new();
    for index in references {
        let Some(row) = usize::try_from(*index)
            .ok()
            .and_then(|index| meshes.get(index))
        else {
            continue;
        };
        let row_names = fields
            .iter()
            .filter_map(|field| row.get(field).and_then(serde_json::Value::as_str))
            .filter(|name| is_real_model_name(name))
            .collect::<Vec<_>>();
        if !row_names.is_empty() {
            slots += 1;
            names.extend(row_names.into_iter().map(str::to_owned));
        }
    }
    (slots, names)
}

fn is_real_model_name(name: &&str) -> bool {
    !name.trim().is_empty() && !name.trim().eq_ignore_ascii_case("null")
}

fn required_object<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a serde_json::Value> {
    value
        .get(key)
        .filter(|value| value.is_object())
        .ok_or_else(|| PipelineError::ModelAudit(format!("native table lacks object {key}")))
}

fn required_array<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a [serde_json::Value]> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| PipelineError::ModelAudit(format!("native table lacks array {key}")))
}
