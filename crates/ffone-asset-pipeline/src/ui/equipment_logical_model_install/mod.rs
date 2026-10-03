//! Transactional installation of the audited native player-equipment tree.
//!
//! The installer consumes only already-published GLB/PNG/JSON files and their
//! native GPU evidence. It never opens a Unity bundle at runtime or install
//! time. Standalone rendering acceptance is recorded independently from the
//! still-pending shared-player-skeleton and attachment parity gates.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ASSET_MANIFEST_FILE, EQUIPMENT_GPU_BATCH_SCHEMA, EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE,
    EQUIPMENT_LOGICAL_MODEL_BATCH_SCHEMA, EquipmentGpuBatchMode, EquipmentGpuBatchReport,
    EquipmentLogicalModelBatchReport, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, error::io_at,
};

pub const PLAYER_EQUIPMENT_CATALOG_SCHEMA: &str = "ffone.player-equipment-catalog.v1";
pub const PLAYER_EQUIPMENT_ROOT: &str = "characters/player/equipment";
pub const PLAYER_EQUIPMENT_CATALOG_PATH: &str = "characters/player/equipment/catalog.json";

static INSTALL_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerEquipmentInstallOptions {
    pub candidate_root: PathBuf,
    pub gpu_batch_path: PathBuf,
    pub asset_root: PathBuf,
    pub source_build: String,
}

impl PlayerEquipmentInstallOptions {
    pub fn new(
        candidate_root: impl Into<PathBuf>,
        gpu_batch_path: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            candidate_root: candidate_root.into(),
            gpu_batch_path: gpu_batch_path.into(),
            asset_root: asset_root.into(),
            source_build: source_build.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerEquipmentCatalog {
    pub schema: String,
    pub status: String,
    pub source_build: String,
    pub runtime_accepted: bool,
    pub standalone_gpu_passed: bool,
    pub player_attachment_parity_asserted: bool,
    pub player_assembly_parity_pending: bool,
    pub visual_parity_pending: bool,
    pub production_approved: bool,
    pub coordinate_contract: String,
    pub taxonomy: String,
    pub proofs: PlayerEquipmentCatalogProofs,
    pub counts: PlayerEquipmentCatalogCounts,
    pub models: Vec<PlayerEquipmentCatalogModel>,
    pub assembly_blockers: Vec<PlayerEquipmentAssemblyBlocker>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerEquipmentCatalogProofs {
    pub candidate_batch_report: String,
    pub candidate_batch_report_sha256: String,
    pub candidate_batch_schema: String,
    pub structural_audit_passed: bool,
    pub gpu_batch_report: String,
    pub gpu_batch_report_sha256: String,
    pub gpu_batch_schema: String,
    pub gpu_scope: String,
    pub deterministic_selection_policy: String,
    pub independent_structural_passed: bool,
    pub independent_full_evidence_passed: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerEquipmentCatalogCounts {
    pub models: u64,
    pub glbs: u64,
    pub textures: u64,
    pub publish_reports: u64,
    pub files: u64,
    pub assembly_blockers: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerEquipmentCatalogModel {
    pub category: String,
    pub exact_route: String,
    pub true_name: String,
    pub glb: String,
    pub publish_report: String,
    pub glb_blake3: String,
    pub coordinate_status: String,
    pub runtime_spawn_policy: String,
    pub material_status: String,
    pub standalone_gpu_passed: bool,
    pub skinned_mesh_parts: u64,
    pub skin_joint_references: u64,
    pub inverse_bind_matrices: u64,
    pub animation_names: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerEquipmentAssemblyBlocker {
    pub code: String,
    pub category: Option<String>,
    pub true_name: Option<String>,
    pub detail: String,
    pub disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerEquipmentInstallReport {
    pub schema: String,
    pub status: String,
    pub destination: String,
    pub catalog: String,
    pub manifest_files: u64,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub installed_models: u64,
    pub standalone_gpu_passed: bool,
    pub player_assembly_parity_pending: bool,
}

pub fn install_player_equipment(
    options: &PlayerEquipmentInstallOptions,
) -> Result<PlayerEquipmentInstallReport> {
    if options.source_build.trim().is_empty() {
        return invalid("source build must not be empty");
    }
    let candidate_root = canonical_directory(&options.candidate_root, "candidate root")?;
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    let gpu_batch_path = canonical_file(&options.gpu_batch_path, "GPU batch report")?;

    let batch_path = candidate_root.join(EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE);
    let batch_bytes = fs::read(&batch_path).map_err(|error| io_at(&batch_path, error))?;
    let batch: EquipmentLogicalModelBatchReport =
        serde_json::from_slice(&batch_bytes).map_err(|source| PipelineError::Json {
            path: batch_path.display().to_string(),
            source,
        })?;
    let gpu_bytes = fs::read(&gpu_batch_path).map_err(|error| io_at(&gpu_batch_path, error))?;
    let gpu: EquipmentGpuBatchReport =
        serde_json::from_slice(&gpu_bytes).map_err(|source| PipelineError::Json {
            path: gpu_batch_path.display().to_string(),
            source,
        })?;
    validate_reports(&candidate_root, &batch_bytes, &batch, &gpu)?;

    let source_root = candidate_root.join(PLAYER_EQUIPMENT_ROOT);
    let source_files = equipment_files(&source_root)?;
    let catalog = build_catalog(
        &candidate_root,
        &options.source_build,
        &batch_path,
        &batch_bytes,
        &batch,
        &gpu_batch_path,
        &gpu_bytes,
        &gpu,
        &source_files,
    )?;

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.files.iter().any(|entry| {
        entry.path == PLAYER_EQUIPMENT_ROOT
            || entry.path.starts_with(&format!("{PLAYER_EQUIPMENT_ROOT}/"))
    }) {
        return invalid("project manifest already owns a player-equipment tree");
    }

    let destination = asset_root.join(PLAYER_EQUIPMENT_ROOT);
    if fs::symlink_metadata(&destination).is_ok() {
        return invalid("player-equipment destination already exists");
    }
    let destination_parent = destination
        .parent()
        .ok_or_else(|| invalid_error("player-equipment destination has no parent"))?;
    fs::create_dir_all(destination_parent).map_err(|error| io_at(destination_parent, error))?;

    let stage = create_stage(&asset_root)?;
    let write_result = write_stage(&candidate_root, &stage, &source_files, &catalog, &manifest);
    let new_entries = match write_result {
        Ok(entries) => entries,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
    };
    fs::rename(&stage, &destination).map_err(|error| {
        let _ = fs::remove_dir_all(&stage);
        io_at(&destination, error)
    })?;

    manifest.files.extend(new_entries.iter().cloned());
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    if let Err(error) = replace_manifest(&asset_root, &manifest) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }

    Ok(PlayerEquipmentInstallReport {
        schema: "ffone.player-equipment-install.v1".to_owned(),
        status: "installed-standalone-gpu-passed-player-assembly-parity-pending".to_owned(),
        destination: normalize(&destination),
        catalog: PLAYER_EQUIPMENT_CATALOG_PATH.to_owned(),
        manifest_files: manifest.files.len() as u64,
        installed_files: new_entries.len() as u64,
        installed_bytes: new_entries.iter().map(|entry| entry.bytes).sum(),
        installed_models: catalog.counts.models,
        standalone_gpu_passed: true,
        player_assembly_parity_pending: true,
    })
}

fn validate_reports(
    candidate_root: &Path,
    batch_bytes: &[u8],
    batch: &EquipmentLogicalModelBatchReport,
    gpu: &EquipmentGpuBatchReport,
) -> Result<()> {
    if batch.schema != EQUIPMENT_LOGICAL_MODEL_BATCH_SCHEMA {
        return invalid(format!(
            "unsupported candidate batch schema {:?}",
            batch.schema
        ));
    }
    if !batch.structural_audit_passed
        || batch.production_assets_mutated
        || batch.counts.published_models != batch.models.len() as u64
        || batch.counts.published_models == 0
    {
        return invalid("candidate batch is not a complete, isolated structural pass");
    }
    if gpu.schema != EQUIPMENT_GPU_BATCH_SCHEMA
        || gpu.mode != EquipmentGpuBatchMode::Full
        || gpu.production_assets_mutated
        || gpu.player_attachment_parity_asserted
        || !gpu.visual_parity_pending
        || gpu.publishable
        || gpu.counts.execution_blockers != 0
        || gpu.counts.selected_models != batch.models.len() as u64
        || gpu.counts.standalone_gpu_passed_models != batch.models.len() as u64
        || gpu.models.len() != batch.models.len()
        || !gpu.coverage.independent_structural_passed
        || !gpu.coverage.independent_full_evidence_passed
    {
        return invalid("GPU batch does not prove the complete standalone candidate set");
    }
    if gpu.candidate_batch_report.byte_length != batch_bytes.len() as u64
        || gpu.candidate_batch_report.sha256 != sha256(batch_bytes)
    {
        return invalid("GPU batch candidate-report evidence does not match at rest");
    }
    let reported_root = canonical_directory(Path::new(&gpu.candidate_root), "GPU candidate root")?;
    if reported_root != candidate_root {
        return invalid("GPU batch was produced from a different candidate root");
    }

    let batch_paths = batch
        .models
        .iter()
        .map(|model| model.output_glb.clone())
        .collect::<BTreeSet<_>>();
    if batch_paths.len() != batch.models.len() {
        return invalid("candidate batch contains duplicate model paths");
    }
    let gpu_paths = gpu
        .models
        .iter()
        .map(|model| model.relative_glb.clone())
        .collect::<BTreeSet<_>>();
    if batch_paths != gpu_paths
        || gpu.models.iter().any(|model| {
            !matches!(
                model.disposition.as_str(),
                "resumed-valid-evidence" | "passed-new-evidence"
            )
        })
    {
        return invalid("standalone GPU evidence set differs from the candidate models");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_catalog(
    candidate_root: &Path,
    source_build: &str,
    batch_path: &Path,
    batch_bytes: &[u8],
    batch: &EquipmentLogicalModelBatchReport,
    gpu_path: &Path,
    gpu_bytes: &[u8],
    gpu: &EquipmentGpuBatchReport,
    source_files: &[(String, PathBuf)],
) -> Result<PlayerEquipmentCatalog> {
    let gpu_models = gpu
        .models
        .iter()
        .map(|model| (model.relative_glb.as_str(), model))
        .collect::<BTreeMap<_, _>>();
    let mut models = Vec::with_capacity(batch.models.len());
    for source in &batch.models {
        validate_relative(&source.output_glb)?;
        validate_relative(&source.output_publish_report)?;
        if !source
            .output_glb
            .starts_with(&format!("{PLAYER_EQUIPMENT_ROOT}/"))
            || !source
                .output_publish_report
                .starts_with(&format!("{PLAYER_EQUIPMENT_ROOT}/"))
        {
            return invalid("candidate model escaped the semantic player-equipment tree");
        }
        let glb_path = candidate_root.join(&source.output_glb);
        let glb = fs::read(&glb_path).map_err(|error| io_at(&glb_path, error))?;
        if blake3::hash(&glb).to_hex().as_str() != source.glb_blake3 {
            return invalid(format!(
                "candidate GLB no longer matches its report: {:?}",
                source.output_glb
            ));
        }
        let run = gpu_models
            .get(source.output_glb.as_str())
            .ok_or_else(|| invalid_error("missing GPU model evidence"))?;
        models.push(PlayerEquipmentCatalogModel {
            category: source.category.clone(),
            exact_route: source.exact_route.clone(),
            true_name: source.true_name.clone(),
            glb: source.output_glb.clone(),
            publish_report: source.output_publish_report.clone(),
            glb_blake3: source.glb_blake3.clone(),
            coordinate_status: source.coordinate_status.clone(),
            runtime_spawn_policy: source.runtime_spawn_policy.clone(),
            material_status: source.material_status.clone(),
            standalone_gpu_passed: true,
            skinned_mesh_parts: run.facts.skinned_mesh_parts,
            skin_joint_references: run.facts.skin_joint_references,
            inverse_bind_matrices: run.facts.inverse_bind_matrices,
            animation_names: run.facts.standard_animation_names.clone(),
        });
    }
    models.sort_by(|left, right| {
        (&left.category, &left.true_name, &left.exact_route).cmp(&(
            &right.category,
            &right.true_name,
            &right.exact_route,
        ))
    });
    let assembly_blockers = gpu
        .blockers
        .iter()
        .map(|blocker| PlayerEquipmentAssemblyBlocker {
            code: blocker.code.clone(),
            category: blocker.category.clone(),
            true_name: blocker.true_name.clone(),
            detail: blocker.detail.clone(),
            disposition: blocker.disposition.clone(),
        })
        .collect::<Vec<_>>();
    let glbs = source_files
        .iter()
        .filter(|(path, _)| path.ends_with(".glb"))
        .count() as u64;
    let textures = source_files
        .iter()
        .filter(|(path, _)| path.ends_with(".png"))
        .count() as u64;
    let publish_reports = source_files
        .iter()
        .filter(|(path, _)| path.ends_with(".publish.json"))
        .count() as u64;
    if glbs != batch.counts.glbs
        || textures != batch.counts.pngs
        || publish_reports != batch.counts.publish_reports
        || glbs != models.len() as u64
    {
        return invalid("candidate file counts differ from the audited batch");
    }
    Ok(PlayerEquipmentCatalog {
        schema: PLAYER_EQUIPMENT_CATALOG_SCHEMA.to_owned(),
        status: "runtime-readable-standalone-gpu-passed-player-assembly-parity-pending".to_owned(),
        source_build: source_build.to_owned(),
        runtime_accepted: true,
        standalone_gpu_passed: true,
        player_attachment_parity_asserted: false,
        player_assembly_parity_pending: true,
        visual_parity_pending: true,
        production_approved: false,
        coordinate_contract:
            "Unity-authored meters converted once by native GLB publication; Bevy spawn root is identity"
                .to_owned(),
        taxonomy: batch.taxonomy.clone(),
        proofs: PlayerEquipmentCatalogProofs {
            candidate_batch_report: normalize(batch_path),
            candidate_batch_report_sha256: sha256(batch_bytes),
            candidate_batch_schema: batch.schema.clone(),
            structural_audit_passed: true,
            gpu_batch_report: normalize(gpu_path),
            gpu_batch_report_sha256: sha256(gpu_bytes),
            gpu_batch_schema: gpu.schema.clone(),
            gpu_scope: gpu.scope.clone(),
            deterministic_selection_policy: gpu.deterministic_selection_policy.clone(),
            independent_structural_passed: gpu.coverage.independent_structural_passed,
            independent_full_evidence_passed: gpu.coverage.independent_full_evidence_passed,
        },
        counts: PlayerEquipmentCatalogCounts {
            models: models.len() as u64,
            glbs,
            textures,
            publish_reports,
            files: source_files.len() as u64 + 1,
            assembly_blockers: assembly_blockers.len() as u64,
        },
        models,
        assembly_blockers,
    })
}

fn equipment_files(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let root = canonical_directory(root, "candidate equipment root")?;
    let mut pending = vec![root.clone()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| io_at(entry.path(), error))?;
            if file_type.is_symlink() {
                return invalid(format!(
                    "candidate equipment tree contains a symlink: {:?}",
                    entry.path()
                ));
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                let relative = relative_path(&root, &entry.path())?;
                if !(relative.ends_with(".glb")
                    || relative.ends_with(".png")
                    || relative.ends_with(".publish.json"))
                {
                    return invalid(format!(
                        "unexpected file in candidate equipment tree: {relative:?}"
                    ));
                }
                files.push((relative, entry.path()));
            } else {
                return invalid("candidate equipment tree contains a non-regular file");
            }
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn write_stage(
    candidate_root: &Path,
    stage: &Path,
    source_files: &[(String, PathBuf)],
    catalog: &PlayerEquipmentCatalog,
    manifest: &ProjectAssetManifest,
) -> Result<Vec<ProjectAssetFile>> {
    let existing = manifest
        .files
        .iter()
        .map(|entry| entry.path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut planned = BTreeSet::new();
    let mut entries = Vec::with_capacity(source_files.len() + 1);
    for (relative, source) in source_files {
        let destination = format!("{PLAYER_EQUIPMENT_ROOT}/{relative}");
        reserve_path(&existing, &mut planned, &destination)?;
        let bytes = fs::read(source).map_err(|error| io_at(source, error))?;
        write_new_file(stage, relative, &bytes)?;
        entries.push(ProjectAssetFile {
            source_path: format!(
                "native-player-equipment/{}/{}",
                candidate_root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("candidate"),
                destination
            ),
            path: destination,
            kind: kind_for(relative)?,
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        });
    }
    reserve_path(&existing, &mut planned, PLAYER_EQUIPMENT_CATALOG_PATH)?;
    let mut catalog_bytes =
        serde_json::to_vec_pretty(catalog).map_err(|source| PipelineError::Json {
            path: PLAYER_EQUIPMENT_CATALOG_PATH.to_owned(),
            source,
        })?;
    catalog_bytes.push(b'\n');
    write_new_file(stage, "catalog.json", &catalog_bytes)?;
    entries.push(ProjectAssetFile {
        source_path: "native-player-equipment/generated/catalog.json".to_owned(),
        path: PLAYER_EQUIPMENT_CATALOG_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: catalog_bytes.len() as u64,
        blake3: blake3::hash(&catalog_bytes).to_hex().to_string(),
    });
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn create_stage(asset_root: &Path) -> Result<PathBuf> {
    for _ in 0..128 {
        let sequence = INSTALL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = asset_root.join(format!(
            ".player-equipment-install-stage-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_at(&path, error)),
        }
    }
    Err(PipelineError::StagingCollision(
        asset_root.join(PLAYER_EQUIPMENT_ROOT),
    ))
}

fn replace_manifest(asset_root: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let path = asset_root.join(ASSET_MANIFEST_FILE);
    let next = asset_root.join(".asset-manifest.player-equipment.next");
    let backup = asset_root.join(".asset-manifest.player-equipment.backup");
    if fs::symlink_metadata(&next).is_ok() || fs::symlink_metadata(&backup).is_ok() {
        return invalid("stale player-equipment manifest transaction files exist");
    }
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)
        .map_err(|error| io_at(&next, error))?;
    output
        .write_all(&bytes)
        .map_err(|error| io_at(&next, error))?;
    output.sync_all().map_err(|error| io_at(&next, error))?;
    fs::rename(&path, &backup).map_err(|error| io_at(&path, error))?;
    if let Err(error) = fs::rename(&next, &path) {
        let _ = fs::rename(&backup, &path);
        return Err(io_at(&path, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}

fn write_new_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    let path = join_relative(root, relative)?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("staged file has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| io_at(&path, error))?;
    output
        .write_all(bytes)
        .map_err(|error| io_at(&path, error))?;
    output.sync_all().map_err(|error| io_at(&path, error))
}

fn kind_for(path: &str) -> Result<ProjectAssetKind> {
    if path.ends_with(".glb") {
        Ok(ProjectAssetKind::Model)
    } else if path.ends_with(".png") {
        Ok(ProjectAssetKind::Texture)
    } else if path.ends_with(".json") {
        Ok(ProjectAssetKind::Data)
    } else {
        invalid(format!("unsupported player-equipment file: {path:?}"))
    }
}

fn reserve_path(
    existing: &BTreeSet<String>,
    planned: &mut BTreeSet<String>,
    path: &str,
) -> Result<()> {
    validate_relative(path)?;
    let portable = path.to_ascii_lowercase();
    if existing.contains(&portable) || !planned.insert(portable) {
        return invalid(format!("project-asset path collision: {path:?}"));
    }
    Ok(())
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|error| io_at(&canonical, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a regular directory"));
    }
    Ok(canonical)
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|error| io_at(&canonical, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a regular file"));
    }
    Ok(canonical)
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid_error("file escaped its candidate root"))?;
    let normalized = relative.to_string_lossy().replace('\\', "/");
    validate_relative(&normalized)?;
    Ok(normalized)
}

fn join_relative(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    let mut output = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            return invalid("native project path contains a non-normal component");
        };
        output.push(component);
    }
    Ok(output)
}

fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty() || path.contains('\\') {
        return invalid("native project paths must be non-empty forward-slash paths");
    }
    if Path::new(path)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe native project path: {path:?}"));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(message.into())
}

#[cfg(test)]
mod tests;
