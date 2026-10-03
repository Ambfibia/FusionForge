//! Additive installation of GPU-accepted equipment models into the organized
//! `characters/player/items` layout.
//!
//! `install_player_equipment` is the first-time installer for the retired
//! pre-organization layout: it writes `characters/player/equipment/`, requires
//! the global `asset-manifest.json`, and refuses to run when its destination
//! already exists. `organize_player_item_sets` has since migrated that tree into
//! per-set packages plus `characters/player/items/catalog.json`, removed the
//! global manifest and made `verify_player_item_sets` reject the old directory,
//! so nothing can add a single model any more.
//!
//! This installer closes that gap. It reproduces the organizer's own identity
//! derivation exactly — a member id is `player-item-` plus the BLAKE3 of the
//! pre-organization rooted GLB route, and a single-member set id is
//! `player-item-set-` plus the same hash — so a model installed here is
//! indistinguishable from one the organizer would have produced. Recomputing
//! that derivation for the installed `back_alien` reproduces its published id,
//! which the unit tests below pin.
//!
//! Scope is deliberately narrow: it appends whole new sets, never edits or
//! replaces an existing one, and refuses any collision on set id, member id,
//! true name or destination directory.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{
    PLAYER_ITEM_SCHEMA, PLAYER_ITEM_SET_CATALOG_SCHEMA, PipelineError, PlayerItemCatalogModel,
    PlayerItemDefinition, PlayerItemSetCatalog, RESOURCE_SET_SCHEMA, ResourceSetArtifact,
    ResourceSetCatalogEntry, ResourceSetDocument, ResourceSetMember, Result, error::io_at,
};

static TRANSACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub const PLAYER_ITEM_MODEL_INSTALL_SCHEMA: &str = "ffone.player-item-model-install.v1";
const PLAYER_ITEM_CATALOG_PATH: &str = "characters/player/items/catalog.json";
const EQUIPMENT_BATCH_REPORT_FILE: &str = "equipment-logical-model-batch-report.json";
const EQUIPMENT_BATCH_SCHEMA: &str = "ffone.equipment-logical-model-batch-publish.v1";
const EQUIPMENT_GPU_BATCH_SCHEMA: &str = "ffone.equipment-gpu-batch.v1";
/// Pre-organization rooted route the organizer hashed for every member id.
const RETIRED_EQUIPMENT_ROOT: &str = "characters/player/equipment";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerItemModelInstallOptions {
    pub candidate_root: PathBuf,
    pub gpu_batch: PathBuf,
    pub asset_root: PathBuf,
    pub source_build: String,
    pub apply: bool,
}

impl PlayerItemModelInstallOptions {
    pub fn new(
        candidate_root: impl Into<PathBuf>,
        gpu_batch: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
        apply: bool,
    ) -> Self {
        Self {
            candidate_root: candidate_root.into(),
            gpu_batch: gpu_batch.into(),
            asset_root: asset_root.into(),
            source_build: source_build.into(),
            apply,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledPlayerItemModel {
    pub category: String,
    pub true_name: String,
    pub set_id: String,
    pub item_id: String,
    /// Pre-organization rooted route whose BLAKE3 produced both ids.
    pub identity_route: String,
    pub source_route: String,
    pub set_relative: String,
    pub model: ResourceSetArtifact,
    pub textures: Vec<ResourceSetArtifact>,
    /// Candidate textures skipped because the catalog already publishes them as
    /// shared rendering textures with identical bytes.
    pub shared_rendering_textures: Vec<String>,
    pub gpu_disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerItemModelInstallReport {
    pub schema: String,
    pub source_build: String,
    pub applied: bool,
    pub candidate_root: String,
    pub gpu_batch: String,
    pub catalog: String,
    pub catalog_sets_before: u64,
    pub catalog_models_before: u64,
    pub catalog_sets_after: u64,
    pub catalog_models_after: u64,
    pub installed: Vec<InstalledPlayerItemModel>,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub catalog_blake3_before: String,
    pub catalog_blake3_after: String,
}

pub fn install_player_item_models(
    options: &PlayerItemModelInstallOptions,
) -> Result<PlayerItemModelInstallReport> {
    if options.source_build.trim().is_empty() {
        return invalid("source build must not be empty");
    }
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    let candidate_root = canonical_directory(&options.candidate_root, "candidate root")?;
    if candidate_root.starts_with(&asset_root) {
        return invalid("candidate root must be staged outside the runtime asset root");
    }

    let batch = read_json(&candidate_root.join(EQUIPMENT_BATCH_REPORT_FILE))?;
    require_schema(&batch, EQUIPMENT_BATCH_SCHEMA, "equipment batch report")?;
    if batch
        .get("productionAssetsMutated")
        .and_then(JsonValue::as_bool)
        != Some(false)
    {
        return invalid("equipment batch report does not prove production assets were untouched");
    }
    if !array(&batch, "blockers", "equipment batch report")?.is_empty() {
        return invalid("equipment batch report has publication blockers");
    }

    let gpu_path = canonical_file(&options.gpu_batch, "GPU batch report")?;
    let gpu = read_json(&gpu_path)?;
    require_schema(&gpu, EQUIPMENT_GPU_BATCH_SCHEMA, "equipment GPU batch")?;
    let gpu_counts = gpu
        .get("counts")
        .ok_or_else(|| invalid_error("equipment GPU batch has no counts"))?;
    if gpu_counts
        .get("executionBlockers")
        .and_then(JsonValue::as_u64)
        != Some(0)
    {
        return invalid("equipment GPU batch has execution blockers");
    }
    // Slot-coverage blockers describe the whole equipment surface, not this
    // batch: an additive batch legitimately carries one category. Only the
    // per-model standalone gate is authoritative here, and it is checked below.
    let gpu_dispositions = gpu_model_dispositions(&gpu)?;

    let catalog_path = asset_root.join(native_path(PLAYER_ITEM_CATALOG_PATH));
    let catalog_before = read_file(&catalog_path)?;
    let mut catalog: PlayerItemSetCatalog =
        serde_json::from_slice(&catalog_before).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if catalog.schema != PLAYER_ITEM_SET_CATALOG_SCHEMA {
        return invalid("player item-set catalog has the wrong schema");
    }
    let sets_before = catalog.sets.len() as u64;
    let models_before = catalog.models.len() as u64;
    let existing_set_ids = catalog
        .sets
        .iter()
        .map(|set| set.id.clone())
        .collect::<BTreeSet<_>>();
    let existing_true_names = catalog
        .models
        .iter()
        .map(|model| model.true_name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let rendering_by_hash = catalog
        .rendering_textures
        .iter()
        .map(|texture| (texture.blake3.clone(), texture.path.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut planned = Vec::new();
    let mut claimed_set_ids = BTreeSet::new();
    for model in array(&batch, "models", "equipment batch report")? {
        let category = string(model, "category", "equipment batch model")?;
        let true_name = string(model, "trueName", "equipment batch model")?;
        let output_glb = string(model, "outputGlb", "equipment batch model")?;
        let glb_blake3 = string(model, "glbBlake3", "equipment batch model")?;
        validate_name(&category)?;
        validate_name(&true_name)?;

        let disposition = gpu_dispositions.get(&output_glb).ok_or_else(|| {
            invalid_error(format!(
                "GPU batch has no standalone gate for {output_glb:?}; every installed model needs one"
            ))
        })?;
        if !disposition.starts_with("passed") {
            return invalid(format!(
                "model {true_name:?} did not pass its standalone GPU gate: {disposition}"
            ));
        }

        if existing_true_names.contains(&true_name.to_ascii_lowercase()) {
            return invalid(format!(
                "player item catalog already publishes a model named {true_name:?}"
            ));
        }
        // Exactly the organizer's derivation: hash the pre-organization route.
        let identity_route =
            format!("{RETIRED_EQUIPMENT_ROOT}/{category}/{true_name}/{true_name}.glb");
        let identity = blake3::hash(identity_route.as_bytes()).to_hex().to_string();
        let set_id = format!("player-item-set-{identity}");
        let item_id = format!("player-item-{identity}");
        if existing_set_ids.contains(&set_id) || !claimed_set_ids.insert(set_id.clone()) {
            return invalid(format!("resource set {set_id:?} already exists"));
        }

        let candidate_glb = candidate_root.join(native_path(&output_glb));
        let glb_bytes = read_file(&candidate_glb)?;
        let actual = blake3::hash(&glb_bytes).to_hex().to_string();
        if actual != glb_blake3 {
            return invalid(format!(
                "candidate GLB {output_glb:?} does not match its batch report hash"
            ));
        }

        let set_relative = format!("characters/player/items/{category}/{true_name}");
        if fs::symlink_metadata(asset_root.join(native_path(&set_relative))).is_ok() {
            return invalid(format!("destination {set_relative:?} already exists"));
        }

        let mut textures = Vec::new();
        let mut shared = Vec::new();
        for (name, bytes) in candidate_textures(&candidate_root, &category, &true_name)? {
            let hash = blake3::hash(&bytes).to_hex().to_string();
            if let Some(existing) = rendering_by_hash.get(&hash) {
                // Already published once as a shared rendering texture; the
                // organizer never duplicates those into a set.
                shared.push(existing.clone());
                continue;
            }
            textures.push((name, bytes, hash));
        }

        planned.push(PlannedSet {
            category,
            true_name,
            set_id,
            item_id,
            identity_route,
            set_relative,
            glb_bytes,
            glb_blake3,
            textures,
            shared,
            disposition: disposition.clone(),
        });
    }
    if planned.is_empty() {
        return invalid("equipment batch report contains no models");
    }

    let mut installed = Vec::new();
    let mut files = Vec::<(String, Vec<u8>)>::new();
    for plan in &planned {
        let model_relative = format!("{}/models/{}/model.glb", plan.set_relative, plan.true_name);
        let model_artifact = ResourceSetArtifact {
            path: model_relative.clone(),
            bytes: plan.glb_bytes.len() as u64,
            blake3: plan.glb_blake3.clone(),
        };
        let mut texture_artifacts = Vec::new();
        for (name, bytes, hash) in &plan.textures {
            let relative = format!("{}/textures/{name}", plan.set_relative);
            texture_artifacts.push(ResourceSetArtifact {
                path: relative.clone(),
                bytes: bytes.len() as u64,
                blake3: hash.clone(),
            });
            files.push((relative, bytes.clone()));
        }
        texture_artifacts.sort_by(|left, right| left.path.cmp(&right.path));

        let definition = PlayerItemDefinition {
            schema: PLAYER_ITEM_SCHEMA.to_owned(),
            id: plan.item_id.clone(),
            true_name: plan.true_name.clone(),
            category: plan.category.clone(),
            source_route: format!(
                "{}/{}/{}.glb",
                plan.category, plan.true_name, plan.true_name
            ),
            resource_set: plan.set_id.clone(),
            model: model_artifact.clone(),
        };
        let definition_relative =
            format!("{}/models/{}/item.json", plan.set_relative, plan.true_name);
        let definition_bytes = pretty_json(&definition)?;
        let definition_artifact = ResourceSetArtifact {
            path: definition_relative.clone(),
            bytes: definition_bytes.len() as u64,
            blake3: blake3::hash(&definition_bytes).to_hex().to_string(),
        };

        let mut member_files = texture_artifacts.clone();
        member_files.push(model_artifact.clone());
        member_files.sort_by(|left, right| left.path.cmp(&right.path));
        let document = ResourceSetDocument {
            schema: RESOURCE_SET_SCHEMA.to_owned(),
            id: plan.set_id.clone(),
            name: plan.true_name.clone(),
            domain: "player_item".to_owned(),
            category: plan.category.clone(),
            prefix: "PLAYER".to_owned(),
            family: plan.category.to_ascii_uppercase(),
            textures: texture_artifacts.clone(),
            members: vec![ResourceSetMember {
                id: plan.item_id.clone(),
                name: plan.true_name.clone(),
                definition: definition_artifact.clone(),
                files: vec![model_artifact.clone()],
            }],
        };
        let set_relative_json = format!("{}/set.json", plan.set_relative);
        let set_bytes = pretty_json(&document)?;

        files.push((model_relative, plan.glb_bytes.clone()));
        files.push((definition_relative, definition_bytes));
        files.push((set_relative_json.clone(), set_bytes.clone()));

        catalog.sets.push(ResourceSetCatalogEntry {
            id: plan.set_id.clone(),
            name: plan.true_name.clone(),
            category: plan.category.clone(),
            prefix: "PLAYER".to_owned(),
            family: plan.category.to_ascii_uppercase(),
            definition: ResourceSetArtifact {
                path: set_relative_json,
                bytes: set_bytes.len() as u64,
                blake3: blake3::hash(&set_bytes).to_hex().to_string(),
            },
            member_count: 1,
            texture_count: texture_artifacts.len() as u64,
        });
        catalog.models.push(PlayerItemCatalogModel {
            category: plan.category.clone(),
            true_name: plan.true_name.clone(),
            source_route: definition.source_route.clone(),
            resource_set: plan.set_id.clone(),
            model: model_artifact.clone(),
        });
        installed.push(InstalledPlayerItemModel {
            category: plan.category.clone(),
            true_name: plan.true_name.clone(),
            set_id: plan.set_id.clone(),
            item_id: plan.item_id.clone(),
            identity_route: plan.identity_route.clone(),
            source_route: definition.source_route,
            set_relative: plan.set_relative.clone(),
            model: model_artifact,
            textures: texture_artifacts,
            shared_rendering_textures: plan.shared.clone(),
            gpu_disposition: plan.disposition.clone(),
        });
    }

    catalog
        .sets
        .sort_by(|left, right| left.definition.path.cmp(&right.definition.path));
    catalog.models.sort_by(|left, right| {
        (&left.category, &left.true_name, &left.source_route).cmp(&(
            &right.category,
            &right.true_name,
            &right.source_route,
        ))
    });
    let catalog_after = pretty_json(&catalog)?;

    let mut report = PlayerItemModelInstallReport {
        schema: PLAYER_ITEM_MODEL_INSTALL_SCHEMA.to_owned(),
        source_build: options.source_build.clone(),
        applied: false,
        candidate_root: slash(&candidate_root),
        gpu_batch: slash(&gpu_path),
        catalog: PLAYER_ITEM_CATALOG_PATH.to_owned(),
        catalog_sets_before: sets_before,
        catalog_models_before: models_before,
        catalog_sets_after: catalog.sets.len() as u64,
        catalog_models_after: catalog.models.len() as u64,
        installed,
        installed_files: files.len() as u64,
        installed_bytes: files.iter().map(|(_, bytes)| bytes.len() as u64).sum(),
        catalog_blake3_before: blake3::hash(&catalog_before).to_hex().to_string(),
        catalog_blake3_after: blake3::hash(&catalog_after).to_hex().to_string(),
    };
    if !options.apply {
        return Ok(report);
    }

    commit(
        &asset_root,
        &planned,
        &files,
        &catalog_path,
        &catalog_before,
        &catalog_after,
    )?;
    report.applied = true;
    Ok(report)
}

struct PlannedSet {
    category: String,
    true_name: String,
    set_id: String,
    item_id: String,
    identity_route: String,
    set_relative: String,
    glb_bytes: Vec<u8>,
    glb_blake3: String,
    textures: Vec<(String, Vec<u8>, String)>,
    shared: Vec<String>,
    disposition: String,
}

/// Writes every planned set into a private stage, renames each into place, then
/// swaps the catalog. Any failure removes everything already moved.
fn commit(
    asset_root: &Path,
    planned: &[PlannedSet],
    files: &[(String, Vec<u8>)],
    catalog_path: &Path,
    catalog_before: &[u8],
    catalog_after: &[u8],
) -> Result<()> {
    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let stage = asset_root.join(format!(".player-item-models-stage-{token}"));
    if fs::symlink_metadata(&stage).is_ok() {
        return invalid("stale player-item-model transaction directory exists");
    }
    fs::create_dir(&stage).map_err(|error| io_at(&stage, error))?;

    let result = (|| -> Result<()> {
        for (relative, bytes) in files {
            let path = stage.join(native_path(relative));
            let parent = path
                .parent()
                .ok_or_else(|| invalid_error("staged file has no parent"))?;
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            write_new(&path, bytes)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let mut moved = Vec::new();
    let move_result = (|| -> Result<()> {
        for plan in planned {
            let from = stage.join(native_path(&plan.set_relative));
            let to = asset_root.join(native_path(&plan.set_relative));
            let parent = to
                .parent()
                .ok_or_else(|| invalid_error("destination set has no parent"))?;
            fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            if fs::symlink_metadata(&to).is_ok() {
                return invalid(format!(
                    "destination {:?} appeared while the install was staged",
                    plan.set_relative
                ));
            }
            fs::rename(&from, &to).map_err(|error| io_at(&to, error))?;
            moved.push(to);
        }
        if read_file(catalog_path)? != catalog_before {
            return invalid("player item catalog changed while the install was staged");
        }
        let next = sibling(catalog_path, &token, "next")?;
        let backup = sibling(catalog_path, &token, "backup")?;
        write_new(&next, catalog_after)?;
        fs::rename(catalog_path, &backup).map_err(|error| io_at(catalog_path, error))?;
        if let Err(error) = fs::rename(&next, catalog_path) {
            let _ = fs::rename(&backup, catalog_path);
            let _ = fs::remove_file(&next);
            return Err(io_at(&next, error));
        }
        let _ = fs::remove_file(&backup);
        Ok(())
    })();
    if move_result.is_err() {
        for path in moved.iter().rev() {
            let _ = fs::remove_dir_all(path);
        }
    }
    let _ = fs::remove_dir_all(&stage);
    move_result
}

fn gpu_model_dispositions(gpu: &JsonValue) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for model in array(gpu, "models", "equipment GPU batch")? {
        let relative = string(model, "relativeGlb", "equipment GPU batch model")?;
        let disposition = string(model, "disposition", "equipment GPU batch model")?;
        if out.insert(relative.clone(), disposition).is_some() {
            return invalid(format!("equipment GPU batch lists {relative:?} twice"));
        }
    }
    Ok(out)
}

/// Every PNG the candidate published beside one model, sorted by file name.
fn candidate_textures(
    candidate_root: &Path,
    category: &str,
    true_name: &str,
) -> Result<Vec<(String, Vec<u8>)>> {
    let directory = candidate_root
        .join(native_path(RETIRED_EQUIPMENT_ROOT))
        .join(category)
        .join(true_name)
        .join(format!("{true_name}.textures"));
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_at(&directory, error)),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| io_at(&directory, error))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return invalid(format!("candidate texture {:?} is not a plain file", path));
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| invalid_error("candidate texture name is not UTF-8"))?;
        if !name.ends_with(".png") {
            return invalid(format!("candidate texture {name:?} is not a PNG"));
        }
        validate_name(name)?;
        out.push((
            name.to_owned(),
            fs::read(&path).map_err(|error| io_at(&path, error))?,
        ));
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(out)
}

fn read_json(path: &Path) -> Result<JsonValue> {
    let bytes = read_file(path)?;
    serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })
}

fn require_schema(value: &JsonValue, expected: &str, label: &str) -> Result<()> {
    if value.get("schema").and_then(JsonValue::as_str) != Some(expected) {
        return invalid(format!("{label} must use schema {expected:?}"));
    }
    Ok(())
}

fn array<'a>(value: &'a JsonValue, field: &str, label: &str) -> Result<&'a Vec<JsonValue>> {
    value
        .get(field)
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{label} has no {field} array")))
}

fn string(value: &JsonValue, field: &str, label: &str) -> Result<String> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid_error(format!("{label} has no {field} string")))
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{} must be a plain regular file", path.display()));
    }
    fs::read(path).map_err(|error| io_at(path, error))
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a plain directory"));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a plain regular file"));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

fn pretty_json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: PLAYER_ITEM_CATALOG_PATH.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    output
        .write_all(bytes)
        .map_err(|error| io_at(path, error))?;
    output.sync_all().map_err(|error| io_at(path, error))
}

fn sibling(path: &Path, token: &str, suffix: &str) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("transaction path has no parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error("transaction filename is not UTF-8"))?;
    Ok(parent.join(format!(".{name}.player-item-models-{token}.{suffix}")))
}

fn native_path(relative: &str) -> PathBuf {
    relative.split('/').collect()
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn validate_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
        || Path::new(value)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe player-item path component {value:?}"));
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(format!(
        "player-item model install failed: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests;
