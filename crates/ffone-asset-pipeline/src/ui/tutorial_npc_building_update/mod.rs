//! Fail-closed, transactional publication of the animated tutorial building.

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ffone_skinned_model::{
    AutomatedGpuStatus, GPU_EVIDENCE_SCHEMA, GPU_RENDER_PROFILE, LogicalModelGpuEvidence,
    LogicalModelGpuFacts, LogicalModelRuntimeSmoke, RUNTIME_SMOKE_SCHEMA, SourceOutlineMode,
    VisualParityClaim, gpu_evidence_relative_paths, gpu_model_facts_from_glb,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    ASSET_MANIFEST_FILE, LOGICAL_MODEL_BATCH_REPORT_FILE, LOGICAL_MODEL_BATCH_REPORT_SCHEMA,
    LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA, LogicalModelBatchPublishReport, ModelPublishContract,
    PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetKind, ProjectAssetManifest, Result,
    error::io_at,
};

pub const TUTORIAL_NPC_BUILDING_UPDATE_SCHEMA: &str =
    "ffone.tutorial-npc-building-update-report.v1";

const ASSET_ROOT_RELATIVE: &str = "assets/game";
const CANDIDATE_GLB: &str = "models/mob/npc_building/npc_building.glb";
const CANDIDATE_PUBLISH_REPORT: &str = "models/mob/npc_building/npc_building.publish.json";
const CANDIDATE_TEXTURE_ROOT: &str = "models/mob/npc_building/npc_building.textures";
const TARGET_ROOT: &str = "objects/collections/npc_building/models/npc_building";
const TARGET_GLB: &str = "objects/collections/npc_building/models/npc_building/npc_building.glb";
const TARGET_TEXTURE_ROOT: &str =
    "objects/collections/npc_building/models/npc_building/npc_building.textures";
const TRANSACTION_PREFIX: &str = ".tutorial-npc-building-update-";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TutorialNpcBuildingUpdateOptions {
    pub project_root: PathBuf,
    pub candidate_root: PathBuf,
    pub source_root: PathBuf,
    pub gpu_evidence_root: PathBuf,
    pub runtime_smoke_json: PathBuf,
    pub expected_installed_sha256: String,
    pub expected_manifest_sha256: String,
    pub apply: bool,
}

impl TutorialNpcBuildingUpdateOptions {
    pub fn new(
        project_root: impl Into<PathBuf>,
        candidate_root: impl Into<PathBuf>,
        source_root: impl Into<PathBuf>,
        gpu_evidence_root: impl Into<PathBuf>,
        runtime_smoke_json: impl Into<PathBuf>,
        expected_installed_sha256: impl Into<String>,
        expected_manifest_sha256: impl Into<String>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            candidate_root: candidate_root.into(),
            source_root: source_root.into(),
            gpu_evidence_root: gpu_evidence_root.into(),
            runtime_smoke_json: runtime_smoke_json.into(),
            expected_installed_sha256: expected_installed_sha256.into(),
            expected_manifest_sha256: expected_manifest_sha256.into(),
            apply: false,
        }
    }

    #[must_use]
    pub fn with_apply(mut self, apply: bool) -> Self {
        self.apply = apply;
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialNpcBuildingUpdateMode {
    DryRun,
    Apply,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialNpcBuildingUpdateProof {
    pub source_document: String,
    pub source_sha256: String,
    pub batch_report_sha256: String,
    pub publish_report_sha256: String,
    pub candidate_glb_sha256: String,
    pub candidate_glb_blake3: String,
    pub installed_glb_sha256_before: String,
    pub gpu_evidence_json: String,
    pub gpu_evidence_sha256: String,
    pub gpu_screenshot_png: String,
    pub gpu_screenshot_sha256: String,
    pub runtime_smoke_sha256: String,
    pub exact_animation_names: Vec<String>,
    pub selected_exact_animation: String,
    pub animation_players: u64,
    pub animation_graph_handles: u64,
    pub animation_evaluation_frames: u64,
    pub texture_files_unchanged: u64,
    pub manifest_sha256_before: String,
    pub manifest_sha256_after: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialNpcBuildingUpdateReport {
    pub schema: String,
    pub mode: TutorialNpcBuildingUpdateMode,
    pub status: String,
    pub applied: bool,
    pub publishable: bool,
    pub runtime_smoke_verified: bool,
    pub only_glb_changed: bool,
    pub target_glb: String,
    pub proof: TutorialNpcBuildingUpdateProof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    bytes: u64,
    blake3: String,
    sha256: String,
}

#[derive(Clone, Debug)]
struct InputProof {
    path: PathBuf,
    sha256: String,
}

#[derive(Debug)]
struct UpdatePlan {
    project_root: PathBuf,
    manifest_path: PathBuf,
    installed_glb_path: PathBuf,
    candidate_glb_path: PathBuf,
    candidate_texture_root: PathBuf,
    installed_texture_root: PathBuf,
    manifest_before: Vec<u8>,
    manifest_after: Vec<u8>,
    installed_glb_before: Vec<u8>,
    candidate_glb: Vec<u8>,
    texture_tree: BTreeMap<String, FileIdentity>,
    inputs: Vec<InputProof>,
    report: TutorialNpcBuildingUpdateReport,
}

pub fn update_tutorial_npc_building(
    options: &TutorialNpcBuildingUpdateOptions,
) -> Result<TutorialNpcBuildingUpdateReport> {
    let plan = build_plan(options)?;
    if options.apply {
        apply_plan(&plan)?;
    }
    let mut report = plan.report;
    report.mode = if options.apply {
        TutorialNpcBuildingUpdateMode::Apply
    } else {
        TutorialNpcBuildingUpdateMode::DryRun
    };
    report.applied = options.apply;
    report.status = if options.apply {
        "runtime-attested-installed".to_owned()
    } else {
        "runtime-attested-transaction-ready".to_owned()
    };
    Ok(report)
}

fn build_plan(options: &TutorialNpcBuildingUpdateOptions) -> Result<UpdatePlan> {
    require_sha256(
        &options.expected_installed_sha256,
        "expected installed GLB SHA-256",
    )?;
    require_sha256(
        &options.expected_manifest_sha256,
        "expected manifest SHA-256",
    )?;
    let expected_installed = options.expected_installed_sha256.to_ascii_lowercase();
    let expected_manifest = options.expected_manifest_sha256.to_ascii_lowercase();

    let project_root = canonical_plain_directory(&options.project_root, "project root")?;
    let asset_root = canonical_plain_directory(
        &project_root.join(native_path(ASSET_ROOT_RELATIVE)),
        "asset root",
    )?;
    let candidate_root = canonical_plain_directory(&options.candidate_root, "candidate root")?;
    let source_root = canonical_plain_directory(&options.source_root, "source root")?;
    let evidence_root = canonical_plain_directory(&options.gpu_evidence_root, "GPU evidence root")?;
    reject_overlaps(&asset_root, &candidate_root, &source_root, &evidence_root)?;
    let runtime_smoke_path = canonical_plain_file(&options.runtime_smoke_json, "runtime smoke")?;
    if runtime_smoke_path.starts_with(&asset_root)
        || runtime_smoke_path.starts_with(&candidate_root)
    {
        return invalid("runtime smoke must be outside project assets and candidate root");
    }

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_before = read_regular(&manifest_path)?;
    if sha256(&manifest_before) != expected_manifest {
        return invalid("project manifest does not match --expected-manifest-sha256");
    }
    let installed_glb_path = asset_root.join(native_path(TARGET_GLB));
    let installed_glb_before = read_regular(&installed_glb_path)?;
    if sha256(&installed_glb_before) != expected_installed {
        return invalid("installed npc_building GLB does not match --expected-installed-sha256");
    }

    let candidate_glb_path = candidate_root.join(native_path(CANDIDATE_GLB));
    let candidate_glb = read_regular(&candidate_glb_path)?;
    if candidate_glb == installed_glb_before {
        return invalid("candidate GLB is identical to the installed baseline");
    }
    let facts = gpu_model_facts_from_glb(&candidate_glb)
        .map_err(|error| invalid_error(format!("candidate GLB facts failed: {error}")))?;
    validate_exact_candidate_facts(&facts)?;
    let installed_facts = gpu_model_facts_from_glb(&installed_glb_before)
        .map_err(|error| invalid_error(format!("installed GLB facts failed: {error}")))?;
    if installed_facts.true_name != "npc_building"
        || !installed_facts.standard_animation_names.is_empty()
    {
        return invalid("installed baseline is not the exact static npc_building predecessor");
    }

    let batch_path = candidate_root.join(LOGICAL_MODEL_BATCH_REPORT_FILE);
    let batch_bytes = read_regular(&batch_path)?;
    let batch: LogicalModelBatchPublishReport =
        serde_json::from_slice(&batch_bytes).map_err(|source| PipelineError::Json {
            path: batch_path.display().to_string(),
            source,
        })?;
    if batch.schema != LOGICAL_MODEL_BATCH_REPORT_SCHEMA
        || !batch.structural_audit_passed
        || batch.models.len() != 1
        || !batch.blockers.is_empty()
    {
        return invalid("candidate batch is not the exact one-model structurally passing batch");
    }
    let mapping = &batch.models[0];
    if mapping.family != "mob"
        || mapping.logical_name != "npc_building"
        || mapping.semantic_directories != ["npc_building"]
        || mapping.output_glb != CANDIDATE_GLB
    {
        return invalid("candidate batch mapping is not exact npc_building identity");
    }
    validate_relative(&mapping.source)?;
    require_sha256(&mapping.source_sha256, "source document SHA-256")?;
    let source_path = source_root.join(native_path(&mapping.source));
    let source_bytes = read_regular(&source_path)?;
    if sha256(&source_bytes) != mapping.source_sha256.to_ascii_lowercase() {
        return invalid("source document changed after deterministic batch publication");
    }

    let publish_path = candidate_root.join(native_path(CANDIDATE_PUBLISH_REPORT));
    let publish_bytes = read_regular(&publish_path)?;
    let publish: Value =
        serde_json::from_slice(&publish_bytes).map_err(|source| PipelineError::Json {
            path: publish_path.display().to_string(),
            source,
        })?;
    if publish.get("schema").and_then(Value::as_str) != Some(LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA)
        || publish
            .pointer("/semanticProof/matched")
            .and_then(Value::as_bool)
            != Some(true)
        || publish.get("reportPath").and_then(Value::as_str) != Some(CANDIDATE_PUBLISH_REPORT)
    {
        return invalid("candidate publish report lacks exact semantic roundtrip proof");
    }
    let contract: ModelPublishContract = serde_json::from_value(
        publish
            .get("contract")
            .cloned()
            .ok_or_else(|| invalid_error("candidate publish report has no contract"))?,
    )
    .map_err(|source| PipelineError::Json {
        path: format!("{}#contract", publish_path.display()),
        source,
    })?;
    if contract.family != mapping.family
        || contract.legacy_name != mapping.logical_name
        || contract.root_node != mapping.logical_name
        || contract.semantic_directories != mapping.semantic_directories
        || contract.output_glb != mapping.output_glb
    {
        return invalid("publish contract differs from exact batch mapping");
    }
    contract.validate(&candidate_glb).map_err(|error| {
        invalid_error(format!(
            "candidate publish contract validation failed: {error}"
        ))
    })?;
    validate_exact_feature_counts(&contract)?;

    let candidate_texture_root = candidate_root.join(native_path(CANDIDATE_TEXTURE_ROOT));
    let installed_texture_root = asset_root.join(native_path(TARGET_TEXTURE_ROOT));
    let candidate_textures = collect_tree(&candidate_texture_root)?;
    let installed_textures = collect_tree(&installed_texture_root)?;
    if candidate_textures != installed_textures {
        return invalid("candidate texture closure differs from installed production textures");
    }
    let installed_package = collect_tree(&asset_root.join(native_path(TARGET_ROOT)))?;
    let mut expected_package = BTreeMap::new();
    expected_package.insert(
        "npc_building.glb".to_owned(),
        identity(&installed_glb_before),
    );
    for (relative, identity) in &installed_textures {
        expected_package.insert(
            format!("npc_building.textures/{relative}"),
            identity.clone(),
        );
    }
    if installed_package != expected_package {
        return invalid(
            "installed npc_building package has files outside the exact GLB/PNG closure",
        );
    }

    let (evidence_json_relative, evidence_png_relative) =
        gpu_evidence_relative_paths(CANDIDATE_GLB)
            .map_err(|error| invalid_error(error.to_string()))?;
    let evidence_json_path = evidence_root.join(&evidence_json_relative);
    let evidence_png_path = evidence_root.join(&evidence_png_relative);
    let evidence_json_bytes = read_regular(&evidence_json_path)?;
    let evidence_png_bytes = read_regular(&evidence_png_path)?;
    let evidence: LogicalModelGpuEvidence =
        serde_json::from_slice(&evidence_json_bytes).map_err(|source| PipelineError::Json {
            path: evidence_json_path.display().to_string(),
            source,
        })?;
    verify_gpu_evidence(
        &evidence,
        &candidate_glb,
        &facts,
        &slash_path(&evidence_png_relative),
        &evidence_png_bytes,
    )?;

    let runtime_smoke_bytes = read_regular(&runtime_smoke_path)?;
    let runtime_smoke: LogicalModelRuntimeSmoke = serde_json::from_slice(&runtime_smoke_bytes)
        .map_err(|source| PipelineError::Json {
            path: runtime_smoke_path.display().to_string(),
            source,
        })?;
    verify_runtime_smoke(
        &runtime_smoke,
        &candidate_glb,
        &facts,
        &slash_path(&evidence_json_relative),
        &evidence_json_bytes,
        &slash_path(&evidence_png_relative),
        &evidence_png_bytes,
        &evidence,
    )?;

    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_before).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid("project asset manifest has the wrong schema");
    }
    let package_prefix = format!("{TARGET_ROOT}/");
    let mut package_entries = BTreeMap::new();
    for (index, entry) in manifest
        .files
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.path.starts_with(&package_prefix))
    {
        if package_entries.insert(entry.path.as_str(), index).is_some() {
            return invalid(format!("duplicate manifest package path {:?}", entry.path));
        }
    }
    if package_entries.len() != installed_package.len() {
        return invalid(format!(
            "manifest npc_building closure has {} entries; exact disk closure has {}",
            package_entries.len(),
            installed_package.len()
        ));
    }
    for (relative, texture_identity) in &installed_textures {
        let path = format!("{TARGET_TEXTURE_ROOT}/{relative}");
        let index = package_entries.get(path.as_str()).ok_or_else(|| {
            invalid_error(format!("manifest is missing exact texture entry {path:?}"))
        })?;
        let entry = &manifest.files[*index];
        if entry.kind != ProjectAssetKind::Texture
            || entry.bytes != texture_identity.bytes
            || entry.blake3 != texture_identity.blake3
        {
            return invalid(format!(
                "manifest texture identity differs from exact installed bytes at {path:?}"
            ));
        }
    }
    let matches = manifest
        .files
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.path == TARGET_GLB)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return invalid("manifest must contain exactly one npc_building GLB entry");
    }
    let entry = &mut manifest.files[matches[0]];
    let installed_identity = identity(&installed_glb_before);
    if entry.kind != ProjectAssetKind::Model
        || entry.bytes != installed_identity.bytes
        || entry.blake3 != installed_identity.blake3
    {
        return invalid("manifest npc_building entry does not match installed baseline bytes");
    }
    let candidate_identity = identity(&candidate_glb);
    entry.source_path = "native-tutorial-npc-building-runtime-attested-update/models/mob/npc_building/npc_building.glb".to_owned();
    entry.bytes = candidate_identity.bytes;
    entry.blake3 = candidate_identity.blake3.clone();
    let mut manifest_after =
        serde_json::to_vec_pretty(&manifest).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    manifest_after.push(b'\n');

    let proof = TutorialNpcBuildingUpdateProof {
        source_document: mapping.source.clone(),
        source_sha256: mapping.source_sha256.clone(),
        batch_report_sha256: sha256(&batch_bytes),
        publish_report_sha256: sha256(&publish_bytes),
        candidate_glb_sha256: candidate_identity.sha256.clone(),
        candidate_glb_blake3: candidate_identity.blake3,
        installed_glb_sha256_before: installed_identity.sha256,
        gpu_evidence_json: slash_path(&evidence_json_relative),
        gpu_evidence_sha256: sha256(&evidence_json_bytes),
        gpu_screenshot_png: slash_path(&evidence_png_relative),
        gpu_screenshot_sha256: sha256(&evidence_png_bytes),
        runtime_smoke_sha256: sha256(&runtime_smoke_bytes),
        exact_animation_names: runtime_smoke.exact_animation_names.clone(),
        selected_exact_animation: runtime_smoke
            .selected_exact_name
            .clone()
            .expect("verified animated runtime smoke has a selection"),
        animation_players: runtime_smoke.animation_players,
        animation_graph_handles: runtime_smoke.animation_graph_handles,
        animation_evaluation_frames: runtime_smoke.animation_evaluation_frames,
        texture_files_unchanged: candidate_textures.len() as u64,
        manifest_sha256_before: sha256(&manifest_before),
        manifest_sha256_after: sha256(&manifest_after),
    };
    let inputs = [
        (&batch_path, &batch_bytes),
        (&source_path, &source_bytes),
        (&publish_path, &publish_bytes),
        (&evidence_json_path, &evidence_json_bytes),
        (&evidence_png_path, &evidence_png_bytes),
        (&runtime_smoke_path, &runtime_smoke_bytes),
    ]
    .into_iter()
    .map(|(path, bytes)| InputProof {
        path: path.to_path_buf(),
        sha256: sha256(bytes),
    })
    .collect();
    Ok(UpdatePlan {
        project_root,
        manifest_path,
        installed_glb_path,
        candidate_glb_path,
        candidate_texture_root,
        installed_texture_root,
        manifest_before,
        manifest_after,
        installed_glb_before,
        candidate_glb,
        texture_tree: candidate_textures,
        inputs,
        report: TutorialNpcBuildingUpdateReport {
            schema: TUTORIAL_NPC_BUILDING_UPDATE_SCHEMA.to_owned(),
            mode: TutorialNpcBuildingUpdateMode::DryRun,
            status: "runtime-attested-transaction-ready".to_owned(),
            applied: false,
            publishable: true,
            runtime_smoke_verified: true,
            only_glb_changed: true,
            target_glb: TARGET_GLB.to_owned(),
            proof,
        },
    })
}

fn validate_exact_candidate_facts(facts: &LogicalModelGpuFacts) -> Result<()> {
    if facts.true_name != "npc_building"
        || facts.standard_animation_names != ["fall", "stand1"]
        || facts.mesh_parts != 25
        || facts.skinned_mesh_parts != 0
        || facts.skin_joint_references != 0
        || facts.inverse_bind_matrices != 0
    {
        return invalid("candidate is not the exact rigid npc_building fall/stand1 artifact");
    }
    Ok(())
}

fn validate_exact_feature_counts(contract: &ModelPublishContract) -> Result<()> {
    let counts = &contract.source;
    if counts.nodes != 83
        || counts.mesh_parts != 25
        || counts.material_slots != 25
        || counts.skinned_meshes != 0
        || counts.joints != 0
        || counts.inverse_bind_matrices != 0
        || counts.weighted_vertices != 0
        || counts.animation_clips != 2
        || counts.animation_channels != 44
        || counts.animation_keyframes != 1628
        || counts.cubic_spline_keyframes != 1574
        || counts.animation_events != 8
        || counts.colliders != 0
        || counts.lod_levels != 0
    {
        return invalid("candidate feature counts differ from the proven Unity source artifact");
    }
    Ok(())
}

fn verify_gpu_evidence(
    evidence: &LogicalModelGpuEvidence,
    glb: &[u8],
    facts: &LogicalModelGpuFacts,
    screenshot_relative: &str,
    screenshot: &[u8],
) -> Result<()> {
    if evidence.schema != GPU_EVIDENCE_SCHEMA
        || evidence.status != AutomatedGpuStatus::Passed
        || evidence.render_profile != GPU_RENDER_PROFILE
        || evidence.visual_parity != VisualParityClaim::NotAsserted
        || evidence.model.relative_glb != CANDIDATE_GLB
        || evidence.model.true_name != facts.true_name
        || evidence.model.glb_byte_length != glb.len() as u64
        || evidence.model.glb_sha256 != sha256(glb)
    {
        return invalid("GPU evidence identity does not match the exact candidate GLB");
    }
    let animation = &evidence.animation;
    if animation.standard_clips_loaded != 2
        || animation.selected_exact_name.as_deref() != Some("fall")
        || animation.sample_normalized_ppm != Some(500_000)
        || animation.animation_players == 0
        || animation.sampled_players != animation.animation_players
    {
        return invalid("GPU evidence did not sample exact fall on every AnimationPlayer");
    }
    let runtime = &evidence.runtime;
    if !runtime.scene_ready
        || runtime.outline_mode != SourceOutlineMode::Source
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
        || runtime.material_errors != 0
        || runtime.shader_errors != 0
    {
        return invalid("GPU evidence runtime facts/errors are not exact");
    }
    if evidence.screenshot.relative_png != screenshot_relative
        || evidence.screenshot.byte_length != screenshot.len() as u64
        || evidence.screenshot.sha256 != sha256(screenshot)
        || evidence.screenshot.width == 0
        || evidence.screenshot.height == 0
        || evidence.screenshot.foreground_pixels == 0
    {
        return invalid("GPU screenshot identity/foreground proof failed");
    }
    let image = image::load_from_memory_with_format(screenshot, image::ImageFormat::Png)
        .map_err(|error| invalid_error(format!("GPU screenshot is invalid PNG: {error}")))?;
    if image.width() != evidence.screenshot.width || image.height() != evidence.screenshot.height {
        return invalid("GPU screenshot dimensions differ from evidence");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify_runtime_smoke(
    smoke: &LogicalModelRuntimeSmoke,
    glb: &[u8],
    facts: &LogicalModelGpuFacts,
    evidence_relative: &str,
    evidence_bytes: &[u8],
    screenshot_relative: &str,
    screenshot_bytes: &[u8],
    evidence: &LogicalModelGpuEvidence,
) -> Result<()> {
    if smoke.schema != RUNTIME_SMOKE_SCHEMA
        || smoke.status != AutomatedGpuStatus::Passed
        || smoke.render_profile != GPU_RENDER_PROFILE
        || smoke.model.relative_glb != CANDIDATE_GLB
        || smoke.model.true_name != facts.true_name
        || smoke.model.glb_byte_length != glb.len() as u64
        || smoke.model.glb_sha256 != sha256(glb)
        || !smoke.gltf_loaded_with_dependencies
        || smoke.exact_animation_names != ["fall", "stand1"]
        || smoke.selected_exact_name.as_deref() != Some("fall")
        || !smoke.scene_ready
        || smoke.animation_players == 0
        || smoke.animation_graph_handles != smoke.animation_players
        || smoke.sampled_players != smoke.animation_players
        || smoke.animation_evaluation_frames == 0
        || smoke.frames < smoke.animation_evaluation_frames
        || smoke.material_errors != 0
        || smoke.shader_errors != 0
        || smoke.gpu_evidence_relative_json != evidence_relative
        || smoke.gpu_evidence_sha256 != sha256(evidence_bytes)
        || smoke.screenshot_relative_png != screenshot_relative
        || smoke.screenshot_sha256 != sha256(screenshot_bytes)
        || smoke.animation_players != evidence.animation.animation_players
        || smoke.sampled_players != evidence.animation.sampled_players
    {
        return invalid("runtime smoke is not exact LoadedWithDependencies animation proof");
    }
    Ok(())
}

fn apply_plan(plan: &UpdatePlan) -> Result<()> {
    validate_plan_unchanged(plan)?;
    reject_stale_transactions(&plan.project_root)?;
    let stage = plan.project_root.join(format!(
        "{TRANSACTION_PREFIX}{}-{}",
        std::process::id(),
        unique_stamp()
    ));
    fs::create_dir(&stage).map_err(|source| io_at(&stage, source))?;
    let staged_glb = stage.join("npc_building.glb.next");
    let staged_manifest = stage.join("asset-manifest.json.next");
    let backup_glb = stage.join("npc_building.glb.backup");
    let backup_manifest = stage.join("asset-manifest.json.backup");
    let prepare = (|| -> Result<()> {
        fs::write(&staged_glb, &plan.candidate_glb).map_err(|source| io_at(&staged_glb, source))?;
        fs::write(&staged_manifest, &plan.manifest_after)
            .map_err(|source| io_at(&staged_manifest, source))?;
        if read_regular(&staged_glb)? != plan.candidate_glb
            || read_regular(&staged_manifest)? != plan.manifest_after
        {
            return invalid("staged update bytes changed before commit");
        }
        Ok(())
    })();
    if let Err(error) = prepare {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    if let Err(error) = validate_plan_unchanged(plan) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    let result = commit_pair(
        &plan.installed_glb_path,
        &plan.manifest_path,
        &staged_glb,
        &staged_manifest,
        &backup_glb,
        &backup_manifest,
        &plan.candidate_glb,
        &plan.manifest_after,
        false,
    );
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    fs::remove_dir_all(&stage).map_err(|source| {
        invalid_error(format!(
            "update committed but recovery staging could not be removed: {source}"
        ))
    })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_pair(
    installed_glb: &Path,
    manifest: &Path,
    staged_glb: &Path,
    staged_manifest: &Path,
    backup_glb: &Path,
    backup_manifest: &Path,
    expected_glb: &[u8],
    expected_manifest: &[u8],
    inject_failure_after_glb: bool,
) -> Result<()> {
    let mut glb_backed_up = false;
    let mut glb_installed = false;
    let mut manifest_backed_up = false;
    let mut manifest_installed = false;
    let commit = (|| -> Result<()> {
        fs::rename(installed_glb, backup_glb).map_err(|source| io_at(installed_glb, source))?;
        glb_backed_up = true;
        fs::rename(staged_glb, installed_glb).map_err(|source| io_at(installed_glb, source))?;
        glb_installed = true;
        if inject_failure_after_glb {
            return invalid("injected failure after GLB installation");
        }
        fs::rename(manifest, backup_manifest).map_err(|source| io_at(manifest, source))?;
        manifest_backed_up = true;
        fs::rename(staged_manifest, manifest).map_err(|source| io_at(manifest, source))?;
        manifest_installed = true;
        if read_regular(installed_glb)? != expected_glb
            || read_regular(manifest)? != expected_manifest
        {
            return invalid("committed GLB/manifest pair differs from staged identity");
        }
        Ok(())
    })();
    if let Err(error) = commit {
        if manifest_installed {
            let _ = fs::remove_file(manifest);
        }
        if manifest_backed_up {
            let _ = fs::rename(backup_manifest, manifest);
        }
        if glb_installed {
            let _ = fs::remove_file(installed_glb);
        }
        if glb_backed_up {
            let _ = fs::rename(backup_glb, installed_glb);
        }
        return Err(error);
    }
    Ok(())
}

fn validate_plan_unchanged(plan: &UpdatePlan) -> Result<()> {
    if read_regular(&plan.manifest_path)? != plan.manifest_before
        || read_regular(&plan.installed_glb_path)? != plan.installed_glb_before
        || read_regular(&plan.candidate_glb_path)? != plan.candidate_glb
    {
        return invalid("manifest or GLB identity changed after update planning");
    }
    if collect_tree(&plan.candidate_texture_root)? != plan.texture_tree
        || collect_tree(&plan.installed_texture_root)? != plan.texture_tree
    {
        return invalid("texture closure changed after update planning");
    }
    for input in &plan.inputs {
        if sha256(&read_regular(&input.path)?) != input.sha256 {
            return invalid(format!(
                "proof input changed after update planning: {}",
                input.path.display()
            ));
        }
    }
    Ok(())
}

fn reject_stale_transactions(project_root: &Path) -> Result<()> {
    for entry in fs::read_dir(project_root).map_err(|source| io_at(project_root, source))? {
        let entry = entry.map_err(|source| io_at(project_root, source))?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(TRANSACTION_PREFIX)
        {
            return invalid(format!(
                "stale npc_building recovery staging must be inspected: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn collect_tree(root: &Path) -> Result<BTreeMap<String, FileIdentity>> {
    let metadata = fs::symlink_metadata(root).map_err(|source| io_at(root, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return invalid(format!(
            "tree root is not a regular directory: {}",
            root.display()
        ));
    }
    let mut output = BTreeMap::new();
    collect_tree_at(root, root, &mut output)?;
    if output.is_empty() {
        return invalid(format!("tree is empty: {}", root.display()));
    }
    Ok(output)
}

fn collect_tree_at(
    root: &Path,
    current: &Path,
    output: &mut BTreeMap<String, FileIdentity>,
) -> Result<()> {
    let mut entries = fs::read_dir(current)
        .map_err(|source| io_at(current, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| io_at(current, source))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| io_at(&path, source))?;
        if metadata.file_type().is_symlink() {
            return invalid(format!("symlink in exact closure: {}", path.display()));
        }
        if metadata.is_dir() {
            collect_tree_at(root, &path, output)?;
            continue;
        }
        if !metadata.is_file() {
            return invalid(format!("unsupported closure entry: {}", path.display()));
        }
        let relative = slash_path(path.strip_prefix(root).map_err(|_| {
            invalid_error(format!("closure file escaped root: {}", path.display()))
        })?);
        validate_relative(&relative)?;
        if output
            .insert(relative.clone(), identity(&read_regular(&path)?))
            .is_some()
        {
            return invalid(format!("duplicate closure path {relative:?}"));
        }
    }
    Ok(())
}

fn identity(bytes: &[u8]) -> FileIdentity {
    FileIdentity {
        bytes: bytes.len() as u64,
        blake3: blake3::hash(bytes).to_hex().to_string(),
        sha256: sha256(bytes),
    }
}

fn canonical_plain_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|source| io_at(path, source))?;
    let metadata = fs::symlink_metadata(path).map_err(|source| io_at(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return invalid(format!("{label} must be a regular directory"));
    }
    Ok(canonical)
}

fn canonical_plain_file(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|source| io_at(path, source))?;
    let metadata = fs::symlink_metadata(path).map_err(|source| io_at(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return invalid(format!("{label} must be a regular file"));
    }
    Ok(canonical)
}

fn reject_overlaps(asset: &Path, candidate: &Path, source: &Path, evidence: &Path) -> Result<()> {
    let roots = [asset, candidate, source, evidence];
    for (index, left) in roots.iter().enumerate() {
        for right in roots.iter().skip(index + 1) {
            if left.starts_with(right) || right.starts_with(left) {
                return invalid(format!(
                    "update input roots must not overlap: {} and {}",
                    left.display(),
                    right.display()
                ));
            }
        }
    }
    Ok(())
}

fn read_regular(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_at(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return invalid(format!("expected regular file: {}", path.display()));
    }
    fs::read(path).map_err(|source| io_at(path, source))
}

fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe relative path {path:?}"));
    }
    Ok(())
}

fn require_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(format!("{label} is not a SHA-256 digest"));
    }
    Ok(())
}

fn native_path(path: &str) -> PathBuf {
    path.split('/').collect()
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn unique_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(format!(
        "tutorial npc_building update failed: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests;
