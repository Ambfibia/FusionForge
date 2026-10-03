use super::*;
use ffone_skinned_model::{
    AutomatedGpuStatus, GpuAnimationEvidence, GpuModelIdentity, GpuRuntimeEvidence,
    GpuScreenshotEvidence,
};

fn facts() -> LogicalModelGpuFacts {
    LogicalModelGpuFacts {
        true_name: "npc_test".to_owned(),
        standard_animation_names: vec!["run".to_owned()],
        mesh_parts: 2,
        skinned_mesh_parts: 1,
        skin_joint_references: 20,
        inverse_bind_matrices: 20,
        materials_applied: 2,
        legacy_pass_companions: 1,
        outline_pass_companions: 1,
        assigned_texture_bindings: 3,
        exact_mip_markers: 2,
        exact_mip_chains: 1,
        exact_mip_levels: 6,
    }
}

fn evidence() -> LogicalModelGpuEvidence {
    let facts = facts();
    LogicalModelGpuEvidence {
        schema: GPU_EVIDENCE_SCHEMA.to_owned(),
        status: AutomatedGpuStatus::Passed,
        render_profile: GPU_RENDER_PROFILE.to_owned(),
        visual_parity: VisualParityClaim::NotAsserted,
        model: GpuModelIdentity {
            relative_glb: "models/npc/npc_test.glb".to_owned(),
            true_name: facts.true_name.clone(),
            glb_byte_length: 1,
            glb_sha256: "0".repeat(64),
        },
        animation: GpuAnimationEvidence {
            standard_clips_loaded: 1,
            selected_exact_name: Some("run".to_owned()),
            sample_normalized_ppm: Some(500_000),
            animation_players: 1,
            sampled_players: 1,
        },
        runtime: GpuRuntimeEvidence {
            scene_ready: true,
            outline_mode: SourceOutlineMode::Source,
            mesh_parts: facts.mesh_parts,
            skinned_mesh_parts: facts.skinned_mesh_parts,
            skin_joint_references: facts.skin_joint_references,
            resolved_skin_joint_references: facts.skin_joint_references,
            inverse_bind_matrices: facts.inverse_bind_matrices,
            materials_applied: facts.materials_applied,
            legacy_pass_companions: facts.legacy_pass_companions,
            outline_pass_companions: facts.outline_pass_companions,
            assigned_texture_bindings: facts.assigned_texture_bindings,
            exact_mip_markers: facts.exact_mip_markers,
            exact_mip_chains: facts.exact_mip_chains,
            exact_mip_levels: facts.exact_mip_levels,
            material_errors: 0,
            shader_errors: 0,
        },
        screenshot: GpuScreenshotEvidence {
            relative_png: "models/npc/npc_test.gpu.png".to_owned(),
            byte_length: 1,
            sha256: "0".repeat(64),
            width: 960,
            height: 960,
            foreground_pixels: 1_000,
        },
    }
}

#[test]
fn exact_runtime_and_named_fixed_sample_pass_contract_checks() {
    let evidence = evidence();
    let facts = facts();
    let mut issues = Vec::new();
    audit_animation(&evidence, &facts, "test", &mut issues);
    audit_runtime(&evidence, &facts, "test", &mut issues);
    assert!(issues.is_empty(), "{issues:#?}");
}

#[test]
fn rejects_hidden_outline_errors_stale_skin_and_index_label() {
    let mut evidence = evidence();
    evidence.animation.selected_exact_name = Some("index:0".to_owned());
    evidence.runtime.resolved_skin_joint_references -= 1;
    evidence.runtime.material_errors = 1;
    let facts = facts();
    let mut issues = Vec::new();
    audit_animation(&evidence, &facts, "test", &mut issues);
    audit_runtime(&evidence, &facts, "test", &mut issues);
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "gpu_animation_sample_mismatch")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "gpu_runtime_stats_mismatch")
    );
}

#[test]
fn rigid_static_facts_accept_zero_skin_and_animation() {
    let mut facts = facts();
    facts.standard_animation_names.clear();
    facts.skinned_mesh_parts = 0;
    facts.skin_joint_references = 0;
    facts.inverse_bind_matrices = 0;
    let mut evidence = evidence();
    evidence.animation = GpuAnimationEvidence {
        standard_clips_loaded: 0,
        selected_exact_name: None,
        sample_normalized_ppm: None,
        animation_players: 0,
        sampled_players: 0,
    };
    evidence.runtime.skinned_mesh_parts = 0;
    evidence.runtime.skin_joint_references = 0;
    evidence.runtime.resolved_skin_joint_references = 0;
    evidence.runtime.inverse_bind_matrices = 0;
    let mut issues = Vec::new();
    audit_animation(&evidence, &facts, "test", &mut issues);
    audit_runtime(&evidence, &facts, "test", &mut issues);
    assert!(issues.is_empty(), "{issues:#?}");
}

#[test]
fn typed_evidence_cannot_claim_visual_parity() {
    let mut value = serde_json::to_value(evidence()).unwrap();
    value["visualParity"] = serde_json::json!("passed");
    assert!(serde_json::from_value::<LogicalModelGpuEvidence>(value).is_err());
}
