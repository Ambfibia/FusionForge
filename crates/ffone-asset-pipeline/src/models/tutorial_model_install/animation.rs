use super::*;

pub(super) fn verify_animation_evidence(
    evidence: &LogicalModelGpuEvidence,
    facts: &LogicalModelGpuFacts,
    selection: &str,
) -> Result<()> {
    let animation = &evidence.animation;
    if animation.standard_clips_loaded != facts.standard_animation_names.len() as u64 {
        return invalid(format!(
            "GPU animation count differs from GLB facts for {selection:?}"
        ));
    }
    if facts.standard_animation_names.is_empty() {
        if animation.selected_exact_name.is_some()
            || animation.sample_normalized_ppm.is_some()
            || animation.animation_players != 0
            || animation.sampled_players != 0
        {
            return invalid(format!(
                "static GLB has invented GPU animation evidence for {selection:?}"
            ));
        }
        return Ok(());
    }
    if animation.selected_exact_name.as_ref().is_none_or(|name| {
        !facts
            .standard_animation_names
            .iter()
            .any(|candidate| candidate == name)
    }) || animation.sample_normalized_ppm != Some(500_000)
        || animation.animation_players == 0
        || animation.sampled_players != animation.animation_players
    {
        return invalid(format!(
            "animated GLB lacks an exact fixed GPU sample for {selection:?}"
        ));
    }
    Ok(())
}
