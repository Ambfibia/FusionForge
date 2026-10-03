use super::*;

pub(super) fn decode_constant_sibling_fixture(
    raw_clips: &[RawAnimationClip],
) -> (Vec<JsonValue>, Vec<CurveRecoveryPlan>) {
    let catalog = build_constant_curve_recovery_catalog(raw_clips);
    let plans = recover_conflicting_constant_curves(&raw_clips[0], &catalog);
    let target = decode_animation_clip_with_curve_recoveries(
        &raw_clips[0].asset_name,
        raw_clips[0].path_id,
        &raw_clips[0].body,
        Vec::new(),
        &plans,
    )
    .preview;
    let sibling = decode_animation_clip(
        &raw_clips[1].asset_name,
        raw_clips[1].path_id,
        &raw_clips[1].body,
        Vec::new(),
    )
    .preview;
    (vec![target, sibling], plans)
}

#[test]
fn curve_recovery_validator_recomputes_reference_payload() {
    let (raw_clips, hierarchy) = exact_constant_sibling_fixture();
    let (mut animations, _) = decode_constant_sibling_fixture(&raw_clips);
    animations[1]["animationData"]["translations"][0]["keys"][0]["value"][1] = json!(-999.0);

    let error = validate_exact_animation_source(&hierarchy, &animations).unwrap_err();
    assert!(
        error.contains("referenceTrack") || error.contains("single exact constant authority"),
        "{error}"
    );
}

pub(super) fn decode_compressed_skin_values(
    packed_weights: &[u32],
    packed_indices: &[u32],
    vertex_count: usize,
    palette_len: usize,
) -> Result<(Vec<[usize; 4]>, Vec<[f64; 4]>, Option<String>), String> {
    decode_compressed_skin_streams(packed_weights, packed_indices, vertex_count, palette_len)
}
