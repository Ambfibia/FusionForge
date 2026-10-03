//! Independent at-rest audit for immutable native Bevy GPU evidence.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

use ffone_skinned_model::{
    GPU_EVIDENCE_SCHEMA, GPU_RENDER_PROFILE, LogicalModelGpuEvidence, LogicalModelGpuFacts,
    SourceOutlineMode, VisualParityClaim, gpu_evidence_relative_paths, gpu_model_facts_from_glb,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    LogicalModelFileAudit, LogicalModelTreeViolation, PipelineError, Result,
    audit_logical_model_tree, error::io_at,
};

pub const GPU_EVIDENCE_AUDIT_SCHEMA: &str = "ffone.logical-model-gpu-evidence-audit.v1";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GpuEvidenceAuditCounts {
    pub candidate_glbs: u64,
    pub evidence_jsons: u64,
    pub evidence_pngs: u64,
    pub matched_models: u64,
    pub missing_models: u64,
    pub orphan_jsons: u64,
    pub orphan_pngs: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GpuEvidenceModelAudit {
    pub relative_glb: String,
    pub true_name: Option<String>,
    pub evidence_json: String,
    pub screenshot_png: String,
    pub passed: bool,
    pub violations: Vec<LogicalModelTreeViolation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelGpuEvidenceAuditReport {
    pub schema: String,
    pub candidate_root: String,
    pub evidence_root: String,
    pub structural_passed: bool,
    pub automated_gpu_passed: bool,
    /// Automated screenshots are content-addressed evidence, not visual approval.
    pub visual_parity_pending: bool,
    /// Catalog completeness and manual 1:1 review remain separate gates.
    pub publishable: bool,
    pub counts: GpuEvidenceAuditCounts,
    pub models: Vec<GpuEvidenceModelAudit>,
    pub violations: Vec<LogicalModelTreeViolation>,
}

struct EvidenceTree {
    jsons: BTreeMap<String, LogicalModelGpuEvidence>,
    invalid_jsons: BTreeSet<String>,
    pngs: BTreeSet<String>,
    violations: Vec<LogicalModelTreeViolation>,
}

/// Re-runs the structural audit and independently binds every candidate GLB to
/// exactly one evidence JSON and one screenshot. It never clears visual parity.
pub fn audit_logical_model_gpu_evidence(
    candidate_root: &Path,
    evidence_root: &Path,
) -> Result<LogicalModelGpuEvidenceAuditReport> {
    let candidate_root =
        fs::canonicalize(candidate_root).map_err(|error| io_at(candidate_root, error))?;
    let evidence_root =
        fs::canonicalize(evidence_root).map_err(|error| io_at(evidence_root, error))?;
    if !candidate_root.is_dir() || !evidence_root.is_dir() {
        return invalid("GPU evidence audit requires candidate and evidence directories");
    }

    let structural = audit_logical_model_tree(&candidate_root)?;
    let mut tree = read_evidence_tree(&evidence_root)?;
    if evidence_root.starts_with(&candidate_root) || candidate_root.starts_with(&evidence_root) {
        push(
            &mut tree.violations,
            "overlapping_evidence_root",
            "",
            "candidate and GPU evidence roots must be disjoint",
        );
    }
    for issue in &structural.violations {
        push(
            &mut tree.violations,
            &format!("structural_{}", issue.code),
            &issue.path,
            &issue.message,
        );
    }

    let mut expected_jsons = BTreeSet::new();
    let mut expected_pngs = BTreeSet::new();
    let mut models = Vec::with_capacity(structural.models.len());
    let mut counts = GpuEvidenceAuditCounts {
        candidate_glbs: structural.counts.glbs,
        evidence_jsons: tree.jsons.len().saturating_add(tree.invalid_jsons.len()) as u64,
        evidence_pngs: tree.pngs.len() as u64,
        ..GpuEvidenceAuditCounts::default()
    };

    for model in &structural.models {
        let (json_path, png_path) = gpu_evidence_relative_paths(&model.path)
            .map_err(|error| invalid_error(error.to_string()))?;
        let json_relative = slash_path(&json_path);
        let png_relative = slash_path(&png_path);
        expected_jsons.insert(json_relative.clone());
        expected_pngs.insert(png_relative.clone());
        let mut violations = Vec::new();

        if tree.invalid_jsons.contains(&json_relative) {
            push(
                &mut violations,
                "invalid_gpu_evidence_json",
                &json_relative,
                "GPU evidence JSON did not match the strict typed schema",
            );
        } else if let Some(evidence) = tree.jsons.get(&json_relative) {
            audit_one_model(
                &candidate_root,
                &evidence_root,
                model,
                &json_relative,
                &png_relative,
                evidence,
                &tree.pngs,
                &mut violations,
            )?;
        } else {
            counts.missing_models += 1;
            push(
                &mut violations,
                "missing_gpu_evidence",
                &model.path,
                format!("missing exact GPU evidence sidecar {json_relative:?}"),
            );
        }
        if !tree.pngs.contains(&png_relative) {
            push(
                &mut violations,
                "missing_gpu_screenshot",
                &model.path,
                format!("missing exact GPU screenshot {png_relative:?}"),
            );
        }
        let passed = violations.is_empty();
        counts.matched_models += u64::from(passed);
        models.push(GpuEvidenceModelAudit {
            relative_glb: model.path.clone(),
            true_name: model.logical_name.clone(),
            evidence_json: json_relative,
            screenshot_png: png_relative,
            passed,
            violations,
        });
    }

    for path in tree
        .jsons
        .keys()
        .chain(tree.invalid_jsons.iter())
        .filter(|path| !expected_jsons.contains(*path))
    {
        counts.orphan_jsons += 1;
        push(
            &mut tree.violations,
            "orphan_gpu_evidence",
            path,
            "GPU evidence JSON has no exact candidate GLB sidecar",
        );
    }
    for path in tree.pngs.difference(&expected_pngs) {
        counts.orphan_pngs += 1;
        push(
            &mut tree.violations,
            "orphan_gpu_screenshot",
            path,
            "GPU screenshot has no exact candidate GLB sidecar",
        );
    }

    let mut claims = BTreeMap::<&str, Vec<&str>>::new();
    for (path, evidence) in &tree.jsons {
        claims
            .entry(&evidence.model.relative_glb)
            .or_default()
            .push(path);
    }
    for (relative_glb, paths) in claims.into_iter().filter(|(_, paths)| paths.len() > 1) {
        push(
            &mut tree.violations,
            "duplicate_gpu_evidence_claim",
            relative_glb,
            format!(
                "multiple evidence JSON files claim this GLB: {}",
                paths.join(", ")
            ),
        );
    }

    for model in &models {
        tree.violations.extend(model.violations.iter().cloned());
    }
    tree.violations.sort_by(|left, right| {
        (&left.path, &left.code, &left.message).cmp(&(&right.path, &right.code, &right.message))
    });
    let automated_gpu_passed = structural.passed
        && tree.violations.is_empty()
        && counts.matched_models == counts.candidate_glbs
        && counts.missing_models == 0
        && counts.orphan_jsons == 0
        && counts.orphan_pngs == 0;
    Ok(LogicalModelGpuEvidenceAuditReport {
        schema: GPU_EVIDENCE_AUDIT_SCHEMA.to_owned(),
        candidate_root: slash_path(&candidate_root),
        evidence_root: slash_path(&evidence_root),
        structural_passed: structural.passed,
        automated_gpu_passed,
        visual_parity_pending: true,
        publishable: false,
        counts,
        models,
        violations: tree.violations,
    })
}

#[allow(clippy::too_many_arguments)]
fn audit_one_model(
    candidate_root: &Path,
    evidence_root: &Path,
    model: &LogicalModelFileAudit,
    json_relative: &str,
    png_relative: &str,
    evidence: &LogicalModelGpuEvidence,
    pngs: &BTreeSet<String>,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> Result<()> {
    if evidence.schema != GPU_EVIDENCE_SCHEMA
        || evidence.render_profile != GPU_RENDER_PROFILE
        || evidence.visual_parity != VisualParityClaim::NotAsserted
    {
        push(
            violations,
            "invalid_gpu_evidence_contract",
            json_relative,
            "schema/renderProfile must be v1 and visualParity must remain not-asserted",
        );
    }
    if evidence.model.relative_glb != model.path {
        push(
            violations,
            "gpu_evidence_glb_path_mismatch",
            json_relative,
            "evidence relativeGlb differs from its exact candidate sidecar",
        );
    }
    if evidence.model.true_name.as_str() != model.logical_name.as_deref().unwrap_or_default() {
        push(
            violations,
            "gpu_evidence_true_name_mismatch",
            json_relative,
            "evidence trueName differs from the sole audited root m_Name",
        );
    }

    let glb_path = candidate_root.join(Path::new(&model.path));
    let glb = fs::read(&glb_path).map_err(|error| io_at(&glb_path, error))?;
    let glb_length = glb.len() as u64;
    let glb_sha256 = sha256_hex(&glb);
    let facts = gpu_model_facts_from_glb(&glb).map_err(|error| invalid_error(error.to_string()))?;
    if evidence.model.glb_byte_length != glb_length || evidence.model.glb_sha256 != glb_sha256 {
        push(
            violations,
            "stale_gpu_evidence_glb",
            json_relative,
            "evidence GLB byte length/SHA-256 differs from the live candidate",
        );
    }
    if evidence.model.true_name != facts.true_name {
        push(
            violations,
            "gpu_evidence_decoded_name_mismatch",
            json_relative,
            "evidence trueName differs from the independently decoded GLB",
        );
    }
    audit_animation(evidence, &facts, json_relative, violations);
    audit_runtime(evidence, &facts, json_relative, violations);

    if evidence.screenshot.relative_png != png_relative {
        push(
            violations,
            "gpu_screenshot_path_mismatch",
            json_relative,
            "evidence screenshot path differs from the exact model sidecar",
        );
    }
    if !pngs.contains(png_relative) {
        return Ok(());
    }
    let png_path = evidence_root.join(Path::new(png_relative));
    let png = fs::read(&png_path).map_err(|error| io_at(&png_path, error))?;
    if evidence.screenshot.byte_length != png.len() as u64
        || evidence.screenshot.sha256 != sha256_hex(&png)
    {
        push(
            violations,
            "gpu_screenshot_hash_mismatch",
            png_relative,
            "screenshot byte length/SHA-256 differs from immutable evidence",
        );
    }
    let Some((width, height)) = png_dimensions(&png) else {
        push(
            violations,
            "invalid_gpu_screenshot_png",
            png_relative,
            "screenshot has no valid PNG signature/IHDR dimensions",
        );
        return Ok(());
    };
    if evidence.screenshot.width != width || evidence.screenshot.height != height {
        push(
            violations,
            "gpu_screenshot_dimension_mismatch",
            png_relative,
            "screenshot IHDR dimensions differ from immutable evidence",
        );
    }
    let pixels = u64::from(width) * u64::from(height);
    let minimum = (pixels / 2_000).max(128);
    let decoded = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map(|image| image.to_rgb8());
    let actual_foreground = decoded
        .as_ref()
        .ok()
        .and_then(|image| screenshot_foreground(image.as_raw(), image.width(), image.height()));
    if actual_foreground != Some(evidence.screenshot.foreground_pixels)
        || actual_foreground.is_none_or(|foreground| foreground < minimum || foreground > pixels)
    {
        push(
            violations,
            "invalid_gpu_screenshot_foreground",
            json_relative,
            "decoded foreground count differs from evidence or has no visible silhouette",
        );
    }
    Ok(())
}

fn audit_animation(
    evidence: &LogicalModelGpuEvidence,
    facts: &LogicalModelGpuFacts,
    path: &str,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let animation = &evidence.animation;
    if animation.standard_clips_loaded != facts.standard_animation_names.len() as u64 {
        push(
            violations,
            "gpu_animation_count_mismatch",
            path,
            "Bevy standard clip count differs from the GLB",
        );
    }
    if facts.standard_animation_names.is_empty() {
        if animation.selected_exact_name.is_some()
            || animation.sample_normalized_ppm.is_some()
            || animation.animation_players != 0
            || animation.sampled_players != 0
        {
            push(
                violations,
                "unexpected_gpu_animation_sample",
                path,
                "static GLB evidence must not invent an animation sample",
            );
        }
        return;
    }
    let exact = animation.selected_exact_name.as_ref().is_some_and(|name| {
        facts
            .standard_animation_names
            .iter()
            .any(|value| value == name)
    });
    if !exact
        || animation.sample_normalized_ppm != Some(500_000)
        || animation.animation_players == 0
        || animation.sampled_players != animation.animation_players
    {
        push(
            violations,
            "gpu_animation_sample_mismatch",
            path,
            "evidence must sample one exact named clip at fixed 50% on every AnimationPlayer",
        );
    }
}

fn audit_runtime(
    evidence: &LogicalModelGpuEvidence,
    facts: &LogicalModelGpuFacts,
    path: &str,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let actual = &evidence.runtime;
    let mismatches = [
        ("meshParts", actual.mesh_parts, facts.mesh_parts),
        (
            "skinnedMeshParts",
            actual.skinned_mesh_parts,
            facts.skinned_mesh_parts,
        ),
        (
            "skinJointReferences",
            actual.skin_joint_references,
            facts.skin_joint_references,
        ),
        (
            "resolvedSkinJointReferences",
            actual.resolved_skin_joint_references,
            facts.skin_joint_references,
        ),
        (
            "inverseBindMatrices",
            actual.inverse_bind_matrices,
            facts.inverse_bind_matrices,
        ),
        (
            "materialsApplied",
            actual.materials_applied,
            facts.materials_applied,
        ),
        (
            "legacyPassCompanions",
            actual.legacy_pass_companions,
            facts.legacy_pass_companions,
        ),
        (
            "outlinePassCompanions",
            actual.outline_pass_companions,
            facts.outline_pass_companions,
        ),
        (
            "assignedTextureBindings",
            actual.assigned_texture_bindings,
            facts.assigned_texture_bindings,
        ),
        (
            "exactMipMarkers",
            actual.exact_mip_markers,
            facts.exact_mip_markers,
        ),
        (
            "exactMipChains",
            actual.exact_mip_chains,
            facts.exact_mip_chains,
        ),
        (
            "exactMipLevels",
            actual.exact_mip_levels,
            facts.exact_mip_levels,
        ),
    ]
    .into_iter()
    .filter_map(|(name, actual, expected)| {
        (actual != expected).then_some(format!("{name}: {actual} != {expected}"))
    })
    .collect::<Vec<_>>();
    if !actual.scene_ready
        || actual.outline_mode != SourceOutlineMode::Source
        || actual.material_errors != 0
        || actual.shader_errors != 0
        || !mismatches.is_empty()
    {
        push(
            violations,
            "gpu_runtime_stats_mismatch",
            path,
            format!(
                "runtime evidence is not the exact source-outline profile: {}",
                mismatches.join(", ")
            ),
        );
    }
}

fn read_evidence_tree(root: &Path) -> Result<EvidenceTree> {
    let mut pending = vec![root.to_path_buf()];
    let mut jsons = BTreeMap::new();
    let mut invalid_jsons = BTreeSet::new();
    let mut pngs = BTreeSet::new();
    let mut violations = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| io_at(&directory, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(&directory, error))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let relative = relative_string(root, &path)?;
            let kind = entry.file_type().map_err(|error| io_at(&path, error))?;
            if kind.is_symlink() {
                push(
                    &mut violations,
                    "gpu_evidence_symlink_forbidden",
                    &relative,
                    "GPU evidence may contain only owned regular files",
                );
            } else if kind.is_dir() {
                pending.push(path);
            } else if !kind.is_file() {
                push(
                    &mut violations,
                    "gpu_evidence_non_regular_file",
                    &relative,
                    "GPU evidence may contain only owned regular files",
                );
            } else if relative.ends_with(".gpu.json") {
                let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
                match serde_json::from_slice::<LogicalModelGpuEvidence>(&bytes) {
                    Ok(evidence) => {
                        jsons.insert(relative, evidence);
                    }
                    Err(error) => {
                        invalid_jsons.insert(relative.clone());
                        push(
                            &mut violations,
                            "invalid_gpu_evidence_json",
                            &relative,
                            error.to_string(),
                        );
                    }
                }
            } else if relative.ends_with(".gpu.png") {
                pngs.insert(relative);
            } else {
                push(
                    &mut violations,
                    "unexpected_gpu_evidence_file",
                    &relative,
                    "evidence root may contain only exact .gpu.json/.gpu.png sidecars",
                );
            }
        }
    }
    Ok(EvidenceTree {
        jsons,
        invalid_jsons,
        pngs,
        violations,
    })
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24
        || bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n")
        || bytes.get(12..16) != Some(b"IHDR")
    {
        return None;
    }
    let width = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

fn screenshot_foreground(rgb: &[u8], width: u32, height: u32) -> Option<u64> {
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    if width < 2 || height < 2 || rgb.len() != usize::try_from(pixels).ok()?.checked_mul(3)? {
        return None;
    }
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let corners = [
        0,
        (width - 1) * 3,
        (height - 1) * width * 3,
        (height * width - 1) * 3,
    ];
    let mut background = [0_u32; 3];
    for offset in corners {
        for channel in 0..3 {
            background[channel] += u32::from(rgb[offset + channel]);
        }
    }
    let background = background.map(|value| (value / 4) as i16);
    Some(
        rgb.chunks_exact(3)
            .filter(|pixel| {
                (0..3).any(|channel| (i16::from(pixel[channel]) - background[channel]).abs() > 8)
            })
            .count() as u64,
    )
}

fn relative_string(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| PipelineError::ModelAudit("GPU evidence path escaped its root".to_owned()))?;
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return invalid("GPU evidence has an unsafe relative path");
    }
    Ok(slash_path(relative))
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn push(
    violations: &mut Vec<LogicalModelTreeViolation>,
    code: &str,
    path: &str,
    message: impl Into<String>,
) {
    violations.push(LogicalModelTreeViolation {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.into(),
    });
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::ModelAudit(format!("GPU evidence audit: {}", message.into()))
}

#[cfg(test)]
mod tests;
