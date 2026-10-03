//! Transactional replacement of one already-published runtime character GLB.
//!
//! This is deliberately narrower than the semantic character installer: it
//! preserves package ownership and permits only a byte replacement whose
//! logical identity and exact animation-name contract still match the runtime
//! registry. The GLB and generated registry are committed as one unit.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use ffone_skinned_model::{
    GPU_EVIDENCE_SCHEMA, GPU_RENDER_PROFILE, LogicalModelGpuEvidence, LogicalModelGpuFacts,
    gpu_model_facts_from_glb, semantic_digests_from_glb, validate_retrobution_fusion_eye_contract,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    PipelineError, Result, RuntimeCharacterCategory, RuntimeCharacterModel,
    SEMANTIC_CHARACTER_REGISTRY_PATH, SEMANTIC_CHARACTER_REGISTRY_SCHEMA,
    SemanticCharacterRegistry, error::io_at,
};

static TRANSACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCharacterModelReplaceOptions {
    pub asset_root: PathBuf,
    pub relative_glb: String,
    pub replacement_glb: PathBuf,
}

impl RuntimeCharacterModelReplaceOptions {
    pub fn new(
        asset_root: impl Into<PathBuf>,
        relative_glb: impl Into<String>,
        replacement_glb: impl Into<PathBuf>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            relative_glb: relative_glb.into(),
            replacement_glb: replacement_glb.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeCharacterModelReplaceReport {
    pub id: String,
    pub logical_name: String,
    pub glb: String,
    pub previous_blake3: String,
    pub installed_blake3: String,
    pub installed_bytes: u64,
    pub animations: Vec<String>,
    pub changed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCharacterModelInstallOptions {
    pub asset_root: PathBuf,
    pub id: String,
    pub candidate_glb: PathBuf,
    pub gpu_evidence: PathBuf,
}

impl RuntimeCharacterModelInstallOptions {
    pub fn new(
        asset_root: impl Into<PathBuf>,
        id: impl Into<String>,
        candidate_glb: impl Into<PathBuf>,
        gpu_evidence: impl Into<PathBuf>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            id: id.into(),
            candidate_glb: candidate_glb.into(),
            gpu_evidence: gpu_evidence.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeCharacterModelInstallReport {
    pub id: String,
    pub logical_name: String,
    pub legacy_aliases: Vec<String>,
    pub category: RuntimeCharacterCategory,
    pub glb: String,
    pub installed_blake3: String,
    pub installed_glb_bytes: u64,
    pub installed_texture_files: u64,
    pub installed_texture_bytes: u64,
    pub animations: Vec<String>,
    pub gpu_evidence_sha256: String,
    pub screenshot_sha256: String,
}

pub fn replace_runtime_character_model(
    options: &RuntimeCharacterModelReplaceOptions,
) -> Result<RuntimeCharacterModelReplaceReport> {
    let asset_root = canonical_plain_directory(&options.asset_root, "asset root")?;
    let relative_glb = normalize_relative_glb(&options.relative_glb)?;
    let replacement_path = canonical_plain_file(&options.replacement_glb, "replacement GLB")?;
    if replacement_path.starts_with(&asset_root) {
        return invalid("replacement GLB must be staged outside the runtime asset root");
    }

    let target_path = asset_root.join(native_path(&relative_glb));
    let registry_path = asset_root.join(native_path(SEMANTIC_CHARACTER_REGISTRY_PATH));
    let target_before = read_regular(&target_path)?;
    let registry_before = read_regular(&registry_path)?;
    let replacement = read_regular(&replacement_path)?;
    let facts = gpu_model_facts_from_glb(&replacement)
        .map_err(|error| invalid_error(format!("replacement GLB facts failed: {error}")))?;

    validate_retrobution_fusion_eye_contract(&replacement).map_err(|error| {
        invalid_error(format!("replacement Fusion Eyes contract failed: {error}"))
    })?;
    let mut registry: SemanticCharacterRegistry = serde_json::from_slice(&registry_before)
        .map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return invalid("semantic character registry has the wrong schema");
    }
    let matches = registry
        .models
        .iter()
        .enumerate()
        .filter(|(_, model)| model.glb == relative_glb)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [model_index] = matches.as_slice() else {
        return invalid(format!(
            "runtime registry must contain exactly one GLB entry for {relative_glb:?}"
        ));
    };
    let model = &registry.models[*model_index];
    if blake3::hash(&target_before).to_hex().as_str() != model.glb_blake3 {
        return invalid("installed GLB identity differs from its runtime registry entry");
    }
    if facts.true_name != model.logical_name {
        return invalid(format!(
            "replacement logical name {:?} differs from registry name {:?}",
            facts.true_name, model.logical_name
        ));
    }
    if facts.standard_animation_names != model.animations {
        return invalid("replacement animation names differ from the runtime registry contract");
    }

    let target_materials = semantic_digests_from_glb(&target_before)
        .map_err(|error| invalid_error(format!("installed material digest failed: {error}")))?
        .materials_sha256;
    let replacement_materials = semantic_digests_from_glb(&replacement)
        .map_err(|error| invalid_error(format!("replacement material digest failed: {error}")))?
        .materials_sha256;
    if target_materials != replacement_materials {
        return invalid(
            "replacement material/texture contract differs from the installed GLB; use the transactional semantic character installer so the complete texture closure is updated",
        );
    }

    let id = model.id.clone();
    let logical_name = model.logical_name.clone();
    let previous_blake3 = model.glb_blake3.clone();
    let installed_blake3 = blake3::hash(&replacement).to_hex().to_string();
    let changed = target_before != replacement || previous_blake3 != installed_blake3;
    registry.models[*model_index].glb_blake3 = installed_blake3.clone();
    let registry_after = pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH)?;
    if changed || registry_before != registry_after {
        commit_pair(
            &target_path,
            &target_before,
            &replacement,
            &registry_path,
            &registry_before,
            &registry_after,
        )?;
    }

    Ok(RuntimeCharacterModelReplaceReport {
        id,
        logical_name,
        glb: relative_glb,
        previous_blake3,
        installed_blake3,
        installed_bytes: replacement.len() as u64,
        animations: facts.standard_animation_names,
        changed,
    })
}

pub fn install_runtime_character_model(
    options: &RuntimeCharacterModelInstallOptions,
) -> Result<RuntimeCharacterModelInstallReport> {
    let asset_root = canonical_plain_directory(&options.asset_root, "asset root")?;
    let candidate_glb = canonical_plain_file(&options.candidate_glb, "candidate GLB")?;
    let gpu_evidence_path = canonical_plain_file(&options.gpu_evidence, "GPU evidence JSON")?;
    if candidate_glb.starts_with(&asset_root) || gpu_evidence_path.starts_with(&asset_root) {
        return install_invalid(
            "candidate and GPU evidence must be staged outside the runtime asset root",
        );
    }

    let (category, category_directory, route_name) = parse_runtime_character_id(&options.id)?;
    let candidate = read_regular(&candidate_glb)?;
    let facts = gpu_model_facts_from_glb(&candidate)
        .map_err(|error| install_invalid_error(format!("candidate GLB facts failed: {error}")))?;
    validate_retrobution_fusion_eye_contract(&candidate).map_err(|error| {
        install_invalid_error(format!("candidate Fusion Eyes contract failed: {error}"))
    })?;
    let candidate_stem = candidate_glb
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| install_invalid_error("candidate GLB has no UTF-8 stem"))?;
    if candidate_stem != facts.true_name {
        return install_invalid("candidate filename must equal its exact logical model name");
    }

    let gpu_evidence_bytes = read_regular(&gpu_evidence_path)?;
    let gpu_evidence: LogicalModelGpuEvidence = serde_json::from_slice(&gpu_evidence_bytes)
        .map_err(|source| PipelineError::Json {
            path: gpu_evidence_path.display().to_string(),
            source,
        })?;
    verify_install_gpu_evidence(&gpu_evidence_path, &gpu_evidence, &candidate, &facts)?;

    // A model whose materials bind no texture publishes no `<name>.textures`
    // directory at all, and that is a correct source shape rather than a broken
    // candidate: an NPC such as Paradox or Azmuth carries an explicitly null
    // `_MainTex` because the runtime dresses it from the NPC table's own
    // Texture1/Texture2. Demand the closure only where a binding needs it.
    let source_texture_directory =
        candidate_glb.with_file_name(format!("{}.textures", facts.true_name));
    let texture_files = if source_texture_directory.exists() {
        collect_plain_files(&source_texture_directory)?
    } else {
        Vec::new()
    };
    if facts.assigned_texture_bindings > 0 && texture_files.is_empty() {
        return install_invalid(
            "candidate GLB has texture bindings but no adjacent texture closure",
        );
    }

    let relative_directory = format!("characters/{category_directory}/{route_name}");
    let relative_glb = format!("{relative_directory}/{}.glb", facts.true_name);
    let target_directory = asset_root.join(native_path(&relative_directory));
    if target_directory.exists() {
        return install_invalid(format!(
            "runtime character target already exists: {relative_directory:?}"
        ));
    }
    let target_parent = target_directory
        .parent()
        .ok_or_else(|| install_invalid_error("runtime character target has no parent"))?;
    let target_parent = canonical_plain_directory(target_parent, "runtime category directory")?;

    let registry_path = asset_root.join(native_path(SEMANTIC_CHARACTER_REGISTRY_PATH));
    let registry_before = read_regular(&registry_path)?;
    let mut registry: SemanticCharacterRegistry = serde_json::from_slice(&registry_before)
        .map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return install_invalid("semantic character registry has the wrong schema");
    }
    if registry
        .models
        .iter()
        .any(|model| model.id == options.id || model.glb == relative_glb)
    {
        return install_invalid("runtime registry already contains the requested id or GLB path");
    }

    let legacy_aliases = (route_name != facts.true_name)
        .then(|| vec![route_name.to_string()])
        .unwrap_or_default();
    let installed_blake3 = blake3::hash(&candidate).to_hex().to_string();
    registry.models.push(RuntimeCharacterModel {
        id: options.id.clone(),
        logical_name: facts.true_name.clone(),
        legacy_aliases: legacy_aliases.clone(),
        category,
        glb: relative_glb.clone(),
        glb_blake3: installed_blake3.clone(),
        collision: None,
        animations: facts.standard_animation_names.clone(),
    });
    registry
        .models
        .sort_by(|left, right| left.id.cmp(&right.id));
    let registry_after = pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH)?;

    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let stage_directory = target_parent.join(format!(".{route_name}.runtime-install-{token}.next"));
    fs::create_dir(&stage_directory).map_err(|error| io_at(&stage_directory, error))?;
    let staged_glb = stage_directory.join(format!("{}.glb", facts.true_name));
    let install_result = (|| {
        write_new(&staged_glb, &candidate)?;
        let staged_textures = stage_directory.join(format!("{}.textures", facts.true_name));
        if !texture_files.is_empty() {
            fs::create_dir(&staged_textures).map_err(|error| io_at(&staged_textures, error))?;
            for (relative, bytes) in &texture_files {
                let destination = staged_textures.join(relative);
                let parent = destination
                    .parent()
                    .ok_or_else(|| install_invalid_error("texture target has no parent"))?;
                fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
                write_new(&destination, bytes)?;
            }
        }
        commit_new_directory_and_registry(
            &stage_directory,
            &target_directory,
            &registry_path,
            &registry_before,
            &registry_after,
            &token,
        )
    })();
    if install_result.is_err() {
        let _ = fs::remove_dir_all(&stage_directory);
    }
    install_result?;

    let installed_texture_bytes = texture_files.iter().try_fold(0_u64, |total, (_, bytes)| {
        total
            .checked_add(
                u64::try_from(bytes.len())
                    .map_err(|_| install_invalid_error("texture byte length exceeds u64"))?,
            )
            .ok_or_else(|| install_invalid_error("installed texture byte total overflow"))
    })?;
    Ok(RuntimeCharacterModelInstallReport {
        id: options.id.clone(),
        logical_name: facts.true_name,
        legacy_aliases,
        category,
        glb: relative_glb,
        installed_blake3,
        installed_glb_bytes: u64::try_from(candidate.len())
            .map_err(|_| install_invalid_error("candidate GLB byte length exceeds u64"))?,
        installed_texture_files: u64::try_from(texture_files.len())
            .map_err(|_| install_invalid_error("texture file count exceeds u64"))?,
        installed_texture_bytes,
        animations: facts.standard_animation_names,
        gpu_evidence_sha256: sha256_hex(&gpu_evidence_bytes),
        screenshot_sha256: gpu_evidence.screenshot.sha256,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeCharacterModelLifecycleReport {
    pub schema: String,
    pub applied: bool,
    pub operation: String,
    pub id: String,
    pub new_id: Option<String>,
    pub logical_name: String,
    pub legacy_aliases: Vec<String>,
    pub glb: String,
    pub new_glb: Option<String>,
    pub glb_blake3: String,
}

pub const RUNTIME_CHARACTER_MODEL_LIFECYCLE_SCHEMA: &str =
    "ffone.runtime-character-model-lifecycle.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCharacterModelRenameOptions {
    pub asset_root: PathBuf,
    pub id: String,
    pub new_id: String,
    pub apply: bool,
}

impl RuntimeCharacterModelRenameOptions {
    pub fn new(
        asset_root: impl Into<PathBuf>,
        id: impl Into<String>,
        new_id: impl Into<String>,
        apply: bool,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            id: id.into(),
            new_id: new_id.into(),
            apply,
        }
    }
}

/// Rename an installed runtime character model's package route.
///
/// The route name is the package directory and the second half of the registry
/// id; `logicalName` is the GLB's own Unity root and is never touched, because
/// it is the model's identity rather than a label. The alias list is preserved
/// exactly: those entries are the table strings that actually resolve the model,
/// and dropping one would leave its NPCs with nothing to draw.
pub fn rename_runtime_character_model(
    options: &RuntimeCharacterModelRenameOptions,
) -> Result<RuntimeCharacterModelLifecycleReport> {
    let asset_root = canonical_plain_directory(&options.asset_root, "asset root")?;
    let (category, category_directory, route_name) = parse_runtime_character_id(&options.id)?;
    let (new_category, new_category_directory, new_route_name) =
        parse_runtime_character_id(&options.new_id)?;
    if category != new_category {
        return install_invalid("a rename may not move a model between categories");
    }
    if route_name == new_route_name {
        return install_invalid("the new id is the current id");
    }

    let registry_path = asset_root.join(native_path(SEMANTIC_CHARACTER_REGISTRY_PATH));
    let registry_before = read_regular(&registry_path)?;
    let mut registry: SemanticCharacterRegistry = serde_json::from_slice(&registry_before)
        .map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return install_invalid("semantic character registry has the wrong schema");
    }
    if registry
        .models
        .iter()
        .any(|model| model.id == options.new_id)
    {
        return install_invalid("the runtime registry already contains the requested new id");
    }

    let index = registry
        .models
        .iter()
        .position(|model| model.id == options.id)
        .ok_or_else(|| install_invalid_error("the runtime registry has no such id"))?;
    let logical_name = registry.models[index].logical_name.clone();
    let old_glb = registry.models[index].glb.clone();
    let expected_blake3 = registry.models[index].glb_blake3.clone();

    let old_directory = asset_root.join(native_path(&format!(
        "characters/{category_directory}/{route_name}"
    )));
    let new_relative_directory = format!("characters/{new_category_directory}/{new_route_name}");
    let new_glb = normalize_relative_glb(&format!("{new_relative_directory}/{logical_name}.glb"))?;
    let new_directory = asset_root.join(native_path(&new_relative_directory));
    let old_directory = canonical_plain_directory(&old_directory, "installed package directory")?;
    if new_directory.exists() {
        return install_invalid("the new package directory already exists");
    }

    let installed = read_regular(&old_directory.join(format!("{logical_name}.glb")))?;
    if blake3::hash(&installed).to_hex().to_string() != expected_blake3 {
        return install_invalid("installed GLB no longer matches the registry blake3");
    }

    registry.models[index].id = options.new_id.clone();
    registry.models[index].glb = new_glb.clone();
    // `install_runtime_character_model` records the package route as an alias
    // whenever it differs from the Unity root, and `network_world_runtime`
    // requires every installed model to stay reachable by that route. A rename
    // moves the route, so the new one has to be added -- while every alias the
    // model already carries is kept, because those are the table strings its
    // NPCs actually select.
    if new_route_name != registry.models[index].logical_name
        && !registry.models[index]
            .legacy_aliases
            .iter()
            .any(|alias| alias == new_route_name)
    {
        registry.models[index]
            .legacy_aliases
            .push(new_route_name.to_owned());
        registry.models[index].legacy_aliases.sort();
    }
    let legacy_aliases = registry.models[index].legacy_aliases.clone();
    registry
        .models
        .sort_by(|left, right| left.id.cmp(&right.id));
    let registry_after = pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH)?;

    let mut report = RuntimeCharacterModelLifecycleReport {
        schema: RUNTIME_CHARACTER_MODEL_LIFECYCLE_SCHEMA.to_owned(),
        applied: false,
        operation: "rename".to_owned(),
        id: options.id.clone(),
        new_id: Some(options.new_id.clone()),
        logical_name,
        legacy_aliases,
        glb: old_glb,
        new_glb: Some(new_glb),
        glb_blake3: expected_blake3,
    };
    if !options.apply {
        return Ok(report);
    }

    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    fs::rename(&old_directory, &new_directory).map_err(|error| io_at(&new_directory, error))?;
    let registry_next = sibling(&registry_path, &token, "next")?;
    let registry_backup = sibling(&registry_path, &token, "backup")?;
    let result = (|| {
        write_new(&registry_next, &registry_after)?;
        fs::rename(&registry_path, &registry_backup)
            .map_err(|error| io_at(&registry_path, error))?;
        if let Err(error) = fs::rename(&registry_next, &registry_path) {
            let _ = fs::rename(&registry_backup, &registry_path);
            return Err(io_at(&registry_next, error));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::rename(&new_directory, &old_directory);
        remove_file_if_exists(&registry_next);
        result?;
    }
    remove_file_if_exists(&registry_backup);
    report.applied = true;
    Ok(report)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCharacterModelRemoveOptions {
    pub asset_root: PathBuf,
    pub id: String,
    pub apply: bool,
}

impl RuntimeCharacterModelRemoveOptions {
    pub fn new(asset_root: impl Into<PathBuf>, id: impl Into<String>, apply: bool) -> Self {
        Self {
            asset_root: asset_root.into(),
            id: id.into(),
            apply,
        }
    }
}

/// Remove an installed runtime character model and its package directory.
///
/// This exists so that a wrong install is undone by the same tracked, atomic
/// path that made it, instead of by hand-deleting a directory and editing the
/// generated registry.
pub fn remove_runtime_character_model(
    options: &RuntimeCharacterModelRemoveOptions,
) -> Result<RuntimeCharacterModelLifecycleReport> {
    let asset_root = canonical_plain_directory(&options.asset_root, "asset root")?;
    let (_, category_directory, route_name) = parse_runtime_character_id(&options.id)?;

    let registry_path = asset_root.join(native_path(SEMANTIC_CHARACTER_REGISTRY_PATH));
    let registry_before = read_regular(&registry_path)?;
    let mut registry: SemanticCharacterRegistry = serde_json::from_slice(&registry_before)
        .map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return install_invalid("semantic character registry has the wrong schema");
    }
    let index = registry
        .models
        .iter()
        .position(|model| model.id == options.id)
        .ok_or_else(|| install_invalid_error("the runtime registry has no such id"))?;
    let removed = registry.models.remove(index);

    let directory = asset_root.join(native_path(&format!(
        "characters/{category_directory}/{route_name}"
    )));
    let directory = canonical_plain_directory(&directory, "installed package directory")?;
    let registry_after = pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH)?;

    let mut report = RuntimeCharacterModelLifecycleReport {
        schema: RUNTIME_CHARACTER_MODEL_LIFECYCLE_SCHEMA.to_owned(),
        applied: false,
        operation: "remove".to_owned(),
        id: options.id.clone(),
        new_id: None,
        logical_name: removed.logical_name,
        legacy_aliases: removed.legacy_aliases,
        glb: removed.glb,
        new_glb: None,
        glb_blake3: removed.glb_blake3,
    };
    if !options.apply {
        return Ok(report);
    }

    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let parent = directory
        .parent()
        .ok_or_else(|| install_invalid_error("package directory has no parent"))?;
    let retired = parent.join(format!(".{route_name}.runtime-remove-{token}.backup"));
    fs::rename(&directory, &retired).map_err(|error| io_at(&directory, error))?;
    let registry_next = sibling(&registry_path, &token, "next")?;
    let registry_backup = sibling(&registry_path, &token, "backup")?;
    let result = (|| {
        write_new(&registry_next, &registry_after)?;
        fs::rename(&registry_path, &registry_backup)
            .map_err(|error| io_at(&registry_path, error))?;
        if let Err(error) = fs::rename(&registry_next, &registry_path) {
            let _ = fs::rename(&registry_backup, &registry_path);
            return Err(io_at(&registry_next, error));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::rename(&retired, &directory);
        remove_file_if_exists(&registry_next);
        result?;
    }
    remove_file_if_exists(&registry_backup);
    let _ = fs::remove_dir_all(&retired);
    report.applied = true;
    Ok(report)
}

fn verify_install_gpu_evidence(
    evidence_path: &Path,
    evidence: &LogicalModelGpuEvidence,
    candidate: &[u8],
    facts: &LogicalModelGpuFacts,
) -> Result<()> {
    if evidence.schema != GPU_EVIDENCE_SCHEMA || evidence.render_profile != GPU_RENDER_PROFILE {
        return install_invalid("GPU evidence has the wrong schema or render profile");
    }
    if evidence.model.true_name != facts.true_name
        || evidence.model.glb_byte_length != candidate.len() as u64
        || evidence.model.glb_sha256 != sha256_hex(candidate)
    {
        return install_invalid("GPU evidence model identity differs from the candidate GLB");
    }
    let expected_model_name = format!("{}.glb", facts.true_name);
    let evidence_model_name = Path::new(&evidence.model.relative_glb)
        .file_name()
        .and_then(|value| value.to_str());
    if evidence_model_name != Some(expected_model_name.as_str()) {
        return install_invalid("GPU evidence relative model filename differs from the candidate");
    }
    if evidence.animation.standard_clips_loaded != facts.standard_animation_names.len() as u64
        || evidence.animation.animation_players == 0
        || evidence.animation.sampled_players == 0
        || evidence
            .animation
            .selected_exact_name
            .as_ref()
            .is_none_or(|name| !facts.standard_animation_names.contains(name))
    {
        return install_invalid("GPU evidence did not evaluate an exact candidate animation");
    }
    let runtime = &evidence.runtime;
    if !runtime.scene_ready
        || runtime.material_errors != 0
        || runtime.shader_errors != 0
        || runtime.mesh_parts != facts.mesh_parts
        || runtime.skinned_mesh_parts != facts.skinned_mesh_parts
        || runtime.skin_joint_references != facts.skin_joint_references
        || runtime.resolved_skin_joint_references != facts.skin_joint_references
        || runtime.inverse_bind_matrices != facts.inverse_bind_matrices
        || runtime.materials_applied != facts.materials_applied
        || runtime.legacy_pass_companions != facts.legacy_pass_companions
        || runtime.outline_pass_companions != facts.outline_pass_companions
        || runtime.assigned_texture_bindings != facts.assigned_texture_bindings
        || runtime.exact_mip_markers != facts.exact_mip_markers
        || runtime.exact_mip_chains != facts.exact_mip_chains
        || runtime.exact_mip_levels != facts.exact_mip_levels
    {
        return install_invalid("GPU evidence runtime facts differ from the candidate GLB");
    }
    if evidence.screenshot.width == 0
        || evidence.screenshot.height == 0
        || evidence.screenshot.foreground_pixels == 0
    {
        return install_invalid("GPU evidence screenshot is empty");
    }
    let expected_screenshot_name = format!("{}.gpu.png", facts.true_name);
    if Path::new(&evidence.screenshot.relative_png)
        .file_name()
        .and_then(|value| value.to_str())
        != Some(expected_screenshot_name.as_str())
    {
        return install_invalid("GPU evidence screenshot filename differs from the candidate");
    }
    let screenshot_path = evidence_path.with_file_name(expected_screenshot_name);
    let screenshot = read_regular(&screenshot_path)?;
    if evidence.screenshot.byte_length != screenshot.len() as u64
        || evidence.screenshot.sha256 != sha256_hex(&screenshot)
    {
        return install_invalid("GPU evidence screenshot bytes do not match their attestation");
    }
    Ok(())
}

fn parse_runtime_character_id(id: &str) -> Result<(RuntimeCharacterCategory, &'static str, &str)> {
    let Some((category, route_name)) = id.split_once('/') else {
        return install_invalid("runtime character id must be category/name");
    };
    if route_name.is_empty()
        || route_name.contains('/')
        || route_name.contains('\\')
        || route_name != route_name.to_ascii_lowercase()
        || route_name
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || character == '_'))
    {
        return install_invalid("runtime character route name must be safe lowercase ASCII");
    }
    let (category, directory) = match category {
        "npc" => (RuntimeCharacterCategory::Npc, "npcs"),
        "mob" => (RuntimeCharacterCategory::Mob, "mobs"),
        "fusion" => (RuntimeCharacterCategory::Fusion, "fusions"),
        "nano" => (RuntimeCharacterCategory::Nano, "nanos"),
        "shared" => (RuntimeCharacterCategory::Shared, "shared"),
        _ => return install_invalid("runtime character id has an unknown category"),
    };
    Ok((category, directory, route_name))
}

fn collect_plain_files(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let metadata = fs::symlink_metadata(root).map_err(|error| io_at(root, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return install_invalid("candidate texture closure must be a plain directory");
    }
    let root = fs::canonicalize(root).map_err(|error| io_at(root, error))?;
    let mut pending = vec![root.clone()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
            if metadata.file_type().is_symlink() {
                return install_invalid("candidate texture closure contains a symlink");
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file()
                || path.extension().and_then(|value| value.to_str()) != Some("png")
            {
                return install_invalid("candidate texture closure contains a non-PNG file");
            }
            let relative = path
                .strip_prefix(&root)
                .map(Path::to_path_buf)
                .map_err(|_| install_invalid_error("texture escaped its closure root"))?;
            if relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                return install_invalid("candidate texture closure has an unsafe path");
            }
            files.push((relative, read_regular(&path)?));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn commit_new_directory_and_registry(
    stage_directory: &Path,
    target_directory: &Path,
    registry: &Path,
    registry_before: &[u8],
    registry_after: &[u8],
    token: &str,
) -> Result<()> {
    let registry_next = sibling(registry, token, "next")?;
    let registry_backup = sibling(registry, token, "backup")?;
    write_new(&registry_next, registry_after)?;
    if target_directory.exists() || read_regular(registry)? != registry_before {
        remove_file_if_exists(&registry_next);
        return install_invalid("runtime target or registry changed while installation was staged");
    }
    fs::rename(stage_directory, target_directory)
        .map_err(|error| io_at(target_directory, error))?;
    if let Err(error) = fs::rename(registry, &registry_backup) {
        let _ = fs::remove_dir_all(target_directory);
        remove_file_if_exists(&registry_next);
        return Err(io_at(registry, error));
    }
    if let Err(error) = fs::rename(&registry_next, registry) {
        let _ = fs::rename(&registry_backup, registry);
        let _ = fs::remove_dir_all(target_directory);
        return Err(io_at(&registry_next, error));
    }
    remove_file_if_exists(&registry_backup);
    Ok(())
}

fn commit_pair(
    target: &Path,
    target_before: &[u8],
    target_after: &[u8],
    registry: &Path,
    registry_before: &[u8],
    registry_after: &[u8],
) -> Result<()> {
    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let target_next = sibling(target, &token, "next")?;
    let target_backup = sibling(target, &token, "backup")?;
    let registry_next = sibling(registry, &token, "next")?;
    let registry_backup = sibling(registry, &token, "backup")?;
    write_new(&target_next, target_after)?;
    if let Err(error) = write_new(&registry_next, registry_after) {
        remove_file_if_exists(&target_next);
        return Err(error);
    }
    if read_regular(target)? != target_before || read_regular(registry)? != registry_before {
        remove_file_if_exists(&target_next);
        remove_file_if_exists(&registry_next);
        return invalid("runtime GLB or registry changed while replacement was staged");
    }

    let result = (|| {
        fs::rename(target, &target_backup).map_err(|error| io_at(target, error))?;
        if let Err(error) = fs::rename(&target_next, target) {
            let _ = fs::rename(&target_backup, target);
            return Err(io_at(&target_next, error));
        }
        if let Err(error) = fs::rename(registry, &registry_backup) {
            let _ = fs::remove_file(target);
            let _ = fs::rename(&target_backup, target);
            return Err(io_at(registry, error));
        }
        if let Err(error) = fs::rename(&registry_next, registry) {
            let _ = fs::rename(&registry_backup, registry);
            let _ = fs::remove_file(target);
            let _ = fs::rename(&target_backup, target);
            return Err(io_at(&registry_next, error));
        }
        Ok(())
    })();
    if result.is_ok() {
        remove_file_if_exists(&target_backup);
        remove_file_if_exists(&registry_backup);
    }
    remove_file_if_exists(&target_next);
    remove_file_if_exists(&registry_next);
    result
}

fn normalize_relative_glb(value: &str) -> Result<String> {
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    if normalized.is_empty()
        || path.is_absolute()
        || path.extension().and_then(|value| value.to_str()) != Some("glb")
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid("runtime character path must be a safe relative lowercase .glb path");
    }
    Ok(normalized)
}

fn native_path(relative: &str) -> PathBuf {
    relative.split('/').collect()
}

fn canonical_plain_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a plain directory"));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

fn canonical_plain_file(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a plain regular file"));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

fn read_regular(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("expected a plain regular file at {path:?}"));
    }
    fs::read(path).map_err(|error| io_at(path, error))
}

fn pretty_json<T: Serialize>(value: &T, label: &str) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: label.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sibling(path: &Path, token: &str, suffix: &str) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("transaction path has no parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error("transaction filename is not UTF-8"))?;
    Ok(parent.join(format!(".{name}.runtime-replace-{token}.{suffix}")))
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

fn remove_file_if_exists(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(format!(
        "runtime character model replacement failed: {}",
        message.into()
    ))
}

fn install_invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(install_invalid_error(message))
}

fn install_invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(format!(
        "runtime character model installation failed: {}",
        message.into()
    ))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests;
