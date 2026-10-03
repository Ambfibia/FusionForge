use super::*;

pub(super) fn verify_runtime_evidence(
    evidence: &LogicalModelGpuEvidence,
    facts: &LogicalModelGpuFacts,
    selection: &str,
) -> Result<()> {
    let runtime = &evidence.runtime;
    let exact = runtime.scene_ready
        && runtime.outline_mode == SourceOutlineMode::Source
        && runtime.mesh_parts == facts.mesh_parts
        && runtime.skinned_mesh_parts == facts.skinned_mesh_parts
        && runtime.skin_joint_references == facts.skin_joint_references
        && runtime.resolved_skin_joint_references == facts.skin_joint_references
        && runtime.inverse_bind_matrices == facts.inverse_bind_matrices
        && runtime.materials_applied == facts.materials_applied
        && runtime.legacy_pass_companions == facts.legacy_pass_companions
        && runtime.outline_pass_companions == facts.outline_pass_companions
        && runtime.assigned_texture_bindings == facts.assigned_texture_bindings
        && runtime.exact_mip_markers == facts.exact_mip_markers
        && runtime.exact_mip_chains == facts.exact_mip_chains
        && runtime.exact_mip_levels == facts.exact_mip_levels
        && runtime.material_errors == 0
        && runtime.shader_errors == 0;
    if !exact {
        return invalid(format!(
            "GPU runtime facts/errors are not exact for {selection:?}"
        ));
    }
    Ok(())
}

pub(super) fn runtime_facts(facts: &LogicalModelGpuFacts) -> TutorialModelRuntimeFacts {
    TutorialModelRuntimeFacts {
        true_name: facts.true_name.clone(),
        standard_animation_names: facts.standard_animation_names.clone(),
        mesh_parts: facts.mesh_parts,
        skinned_mesh_parts: facts.skinned_mesh_parts,
        skin_joint_references: facts.skin_joint_references,
        inverse_bind_matrices: facts.inverse_bind_matrices,
        materials_applied: facts.materials_applied,
        legacy_pass_companions: facts.legacy_pass_companions,
        outline_pass_companions: facts.outline_pass_companions,
        assigned_texture_bindings: facts.assigned_texture_bindings,
        exact_mip_markers: facts.exact_mip_markers,
        exact_mip_chains: facts.exact_mip_chains,
        exact_mip_levels: facts.exact_mip_levels,
    }
}
