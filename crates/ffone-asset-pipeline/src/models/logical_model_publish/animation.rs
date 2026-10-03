use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentPoseBindIdentityDeviation {
    pub metric: String,
    pub mesh_node_index: u32,
    pub mesh_node_path: String,
    pub mesh_name: String,
    pub skin_index: u32,
    pub skin_name: String,
    pub joint_palette_index: u32,
    pub joint_node_index: u32,
    pub joint_node_path: String,
    pub joint_positive_weight_sum: f64,
    pub max_abs_deviation: f64,
    pub max_element_row: u32,
    pub max_element_column: u32,
    pub max_element_actual: f64,
    pub max_element_identity: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceSkeleton {
    pub(super) source: String,
    pub(super) space: String,
    pub(super) joints: Vec<SourceJoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceAnimation {
    #[serde(default)]
    pub(super) asset: Option<String>,
    #[serde(default)]
    pub(super) path_id: Option<i64>,
    pub(super) name: String,
    pub(super) duration: f64,
    pub(super) declared_duration: SourceNullableDuration,
    pub(super) keyed_duration: SourceNullableDuration,
    pub(super) event_duration: SourceNullableDuration,
    pub(super) sample_rate: Option<f64>,
    pub(super) wrap_mode: Option<i32>,
    #[serde(rename = "loop")]
    pub(super) looped: bool,
    pub(super) curve_counts: SourceCurveCounts,
    #[serde(default)]
    pub(super) animation_data: SourceAnimationData,
    #[serde(default)]
    pub(super) events: Vec<SourceEvent>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceAnimationData {
    #[serde(default)]
    pub(super) translations: Vec<SourceVec3Track>,
    #[serde(default)]
    pub(super) rotations: Vec<SourceQuatTrack>,
    #[serde(default)]
    pub(super) scales: Vec<SourceVec3Track>,
    #[serde(rename = "floatCurves", default)]
    pub(super) float_curves: Vec<SourceFloatTrack>,
    #[serde(rename = "emptyTrsBindings")]
    pub(super) empty_trs_bindings: Vec<SourceEmptyTrsBinding>,
    #[serde(rename = "duplicateTrsBindings")]
    pub(super) duplicate_trs_bindings: Vec<SourceDuplicateTrsBinding>,
    #[serde(rename = "timeRecoveries")]
    pub(super) time_recoveries: Vec<SourceAnimationTimeRecovery>,
    #[serde(rename = "curveRecoveries")]
    pub(super) curve_recoveries: Vec<SourceAnimationCurveRecovery>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceAnimationTimeRecovery {
    pub(super) kind: EmptyTrsBindingKind,
    pub(super) path: String,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    pub(super) source_index: usize,
    pub(super) reason: AnimationTimeRecoveryReason,
    pub(super) original_times: Vec<f64>,
    pub(super) recovered_times: Vec<f64>,
    pub(super) source_sample_rate: Option<f64>,
    pub(super) reference: AnimationTimeRecoveryReference,
    pub(super) proof: AnimationTimeRecoveryProof,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceAnimationCurveRecovery {
    pub(super) kind: EmptyTrsBindingKind,
    pub(super) path: String,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    pub(super) reason: AnimationCurveRecoveryReason,
    pub(super) source_sample_rate: Option<f64>,
    pub(super) source: SourceAnimationCurveRecoverySource,
    pub(super) canonical: SourceAnimationCurveRecoveryCanonicalTrack,
    pub(super) rejected: Vec<SourceAnimationCurveRecoveryTrack>,
    pub(super) reference: SourceAnimationCurveRecoveryReference,
    pub(super) proof: AnimationCurveRecoveryProof,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceAnimationCurveRecoverySource {
    pub(super) field: String,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    pub(super) source_indices: Vec<usize>,
    pub(super) source_target_curve_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceAnimationCurveRecoveryCanonicalTrack {
    pub(super) path: String,
    pub(super) interpolation: String,
    pub(super) keys: SourceDuplicateTrsKeys,
    pub(super) duplicate_keys: Vec<SourceDuplicateAnimationKey>,
    pub(super) source_key_count: usize,
    pub(super) source_index: usize,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    pub(super) track_index: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceAnimationCurveRecoveryTrack {
    pub(super) path: String,
    pub(super) interpolation: String,
    pub(super) keys: SourceDuplicateTrsKeys,
    pub(super) duplicate_keys: Vec<SourceDuplicateAnimationKey>,
    pub(super) source_key_count: usize,
    pub(super) source_index: usize,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceAnimationCurveRecoveryReference {
    pub(super) asset: String,
    pub(super) path_id: i64,
    pub(super) clip_name: String,
    pub(super) field: String,
    pub(super) kind: EmptyTrsBindingKind,
    pub(super) path: String,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    pub(super) source_index: usize,
    pub(super) sample_rate: Option<f64>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceDuplicateAnimationKey {
    pub(super) source_key_index: usize,
    pub(super) canonical_key_index: usize,
    pub(super) relation: DuplicateKeyRelation,
    pub(super) key: SourceExactTrsKeyPayload,
}

pub(super) fn validate_skeleton(
    source: &SourceDocument,
    paths: &BTreeMap<String, u32>,
) -> Result<Option<u32>> {
    let skeleton = &source.skeleton;
    if skeleton.joints.is_empty() {
        return Ok(None);
    }
    if skeleton.source != "unity-transform-hierarchy" || skeleton.space.trim().is_empty() {
        return invalid("skeleton is not an identified Unity Transform hierarchy");
    }
    let mut joint_paths = BTreeSet::new();
    let mut mapped = BTreeSet::new();
    let mut roots = Vec::new();
    for joint in &skeleton.joints {
        if !joint_paths.insert(joint.path.clone()) {
            return invalid(format!("duplicate skeleton joint path {:?}", joint.path));
        }
        let node_index = suffix_node_path(paths, &joint.path, "skeleton joint")?;
        if !mapped.insert(node_index) {
            return invalid(format!(
                "two skeleton paths resolve to hierarchy node {:?}",
                source.model_hierarchy.nodes[node_index as usize].path
            ));
        }
        let node = &source.model_hierarchy.nodes[node_index as usize];
        if node.source_asset_index != joint.source_asset_index
            || node.transform_path_id != joint.transform_path_id
            || node.translation != joint.translation
            || node.rotation != joint.rotation
            || node.scale != joint.scale
        {
            return invalid(format!(
                "skeleton joint {:?} differs from its hierarchy Transform",
                joint.path
            ));
        }
        if let Some(parent_path) = &joint.parent {
            let parent_index = suffix_node_path(paths, parent_path, "skeleton parent")?;
            let actual_parent = node
                .parent
                .as_ref()
                .and_then(|path| paths.get(path))
                .copied();
            if actual_parent != Some(parent_index) {
                return invalid(format!(
                    "skeleton parent mismatch for joint {:?}",
                    joint.path
                ));
            }
        } else {
            roots.push(node_index);
        }
    }
    if roots.is_empty() {
        return invalid("skeleton has joints but no root joint");
    }
    let root_parents = roots
        .iter()
        .map(|root| {
            source.model_hierarchy.nodes[*root as usize]
                .parent
                .as_ref()
                .and_then(|path| paths.get(path))
                .copied()
        })
        .collect::<Vec<_>>();
    if root_parents
        .iter()
        .flatten()
        .any(|parent| mapped.contains(parent))
    {
        return invalid("skeleton root has a parent inside the skeleton joint set");
    }
    if roots.len() == 1 {
        return Ok(Some(roots[0]));
    }

    // Some clean Unity 2.x SkinnedMeshRenderers (notably Hippie Hop) use one
    // renderer palette made from several sibling bone chains.  Those chains
    // are disconnected only inside the palette: the authoritative Transform
    // hierarchy proves that every root is parented by the same logical-model
    // node. glTF permits that common node to be the skin's `skeleton` even
    // when it is not itself a joint, preserving all original joint indices and
    // inverse-bind matrices without inventing a synthetic bone.
    let common_parent = root_parents[0]
        .ok_or_else(|| invalid_error("multi-root skeleton has no common hierarchy parent"))?;
    if root_parents
        .iter()
        .any(|parent| *parent != Some(common_parent))
    {
        return invalid("multi-root skeleton joints do not share one hierarchy parent");
    }
    Ok(Some(common_parent))
}

#[must_use]
pub(super) fn legacy_npc_clip_is_additive(name: &str) -> bool {
    !name.contains("upper")
        && !name.contains("event")
        && (name.contains("melee") || name.contains("wound"))
}

pub(super) fn reconcile_animation_curve_counts(animation: &SourceAnimation) -> Result<bool> {
    let counts = &animation.curve_counts;
    if counts.events != animation.events.len() {
        return invalid(format!(
            "animation {:?} event curveCounts differ from decoded events",
            animation.name
        ));
    }

    let mut identities = BTreeSet::new();
    let mut previous_order = None;
    for identity in animation
        .animation_data
        .translations
        .iter()
        .map(|track| {
            validate_source_track_keys(
                &SourceCanonicalTrack::Vec3(track),
                &animation.name,
                "translation",
            )?;
            Ok::<_, PipelineError>((
                EmptyTrsBindingKind::Translation,
                track.source_encoding,
                track.source_index,
            ))
        })
        .chain(animation.animation_data.rotations.iter().map(|track| {
            validate_source_track_keys(
                &SourceCanonicalTrack::Quaternion(track),
                &animation.name,
                "rotation",
            )?;
            Ok::<_, PipelineError>((
                EmptyTrsBindingKind::Rotation,
                track.source_encoding,
                track.source_index,
            ))
        }))
        .chain(animation.animation_data.scales.iter().map(|track| {
            validate_source_track_keys(
                &SourceCanonicalTrack::Vec3(track),
                &animation.name,
                "scale",
            )?;
            Ok::<_, PipelineError>((
                EmptyTrsBindingKind::Scale,
                track.source_encoding,
                track.source_index,
            ))
        }))
    {
        let (kind, encoding, source_index) = identity?;
        let (rank, raw_count) = raw_trs_source(counts, kind, encoding).ok_or_else(|| {
            invalid_error(format!(
                "animation {:?} canonical {kind:?} curve uses invalid {encoding:?} encoding",
                animation.name
            ))
        })?;
        validate_source_identity(
            &animation.name,
            "canonical TRS track",
            kind,
            encoding,
            source_index,
            raw_count,
            &mut identities,
        )?;
        let current = (rank, source_index);
        if previous_order.is_some_and(|previous| previous >= current) {
            return invalid(format!(
                "animation {:?} canonical TRS tracks are not in source order",
                animation.name
            ));
        }
        previous_order = Some(current);
    }

    previous_order = None;
    for binding in &animation.animation_data.empty_trs_bindings {
        let (rank, raw_count) = raw_trs_source(counts, binding.kind, binding.source_encoding)
            .ok_or_else(|| {
                invalid_error(format!(
                    "animation {:?} empty {:?} curve uses invalid {:?} encoding",
                    animation.name, binding.kind, binding.source_encoding
                ))
            })?;
        validate_source_identity(
            &animation.name,
            "empty TRS binding",
            binding.kind,
            binding.source_encoding,
            binding.source_index,
            raw_count,
            &mut identities,
        )?;
        let current = (rank, binding.source_index);
        if previous_order.is_some_and(|previous| previous >= current) {
            return invalid(format!(
                "animation {:?} emptyTrsBindings are not in canonical source order",
                animation.name
            ));
        }
        previous_order = Some(current);
    }

    previous_order = None;
    for binding in &animation.animation_data.duplicate_trs_bindings {
        let (rank, raw_count) = raw_trs_source(counts, binding.kind, binding.source_encoding)
            .ok_or_else(|| {
                invalid_error(format!(
                    "animation {:?} duplicate {:?} curve uses invalid {:?} encoding",
                    animation.name, binding.kind, binding.source_encoding
                ))
            })?;
        validate_source_identity(
            &animation.name,
            "duplicate TRS binding",
            binding.kind,
            binding.source_encoding,
            binding.source_index,
            raw_count,
            &mut identities,
        )?;
        let current = (rank, binding.source_index);
        if previous_order.is_some_and(|previous| previous >= current) {
            return invalid(format!(
                "animation {:?} duplicateTrsBindings are not in canonical source order",
                animation.name
            ));
        }
        previous_order = Some(current);
        validate_duplicate_trs_source(animation, binding)?;
    }

    previous_order = None;
    for (recovery_index, recovery) in animation.animation_data.curve_recoveries.iter().enumerate() {
        let (rank, raw_count) = raw_trs_source(counts, recovery.kind, recovery.source_encoding)
            .ok_or_else(|| {
                invalid_error(format!(
                    "animation {:?} curve recovery {recovery_index} uses an invalid encoding",
                    animation.name
                ))
            })?;
        let first = recovery
            .source
            .source_indices
            .first()
            .copied()
            .ok_or_else(|| invalid_error("curve recovery sourceIndices is empty"))?;
        let order = (rank, first);
        if previous_order.is_some_and(|previous| previous >= order) {
            return invalid(format!(
                "animation {:?} curveRecoveries are not in canonical source order",
                animation.name
            ));
        }
        previous_order = Some(order);
        for rejected in &recovery.rejected {
            validate_source_identity(
                &animation.name,
                "curve recovery rejected TRS binding",
                recovery.kind,
                rejected.source_encoding,
                rejected.source_index,
                raw_count,
                &mut identities,
            )?;
        }
    }

    for (kind, encoding, raw_count) in [
        (
            EmptyTrsBindingKind::Translation,
            EmptyTrsSourceEncoding::Plain,
            counts.position,
        ),
        (
            EmptyTrsBindingKind::Rotation,
            EmptyTrsSourceEncoding::Plain,
            counts.rotation,
        ),
        (
            EmptyTrsBindingKind::Rotation,
            EmptyTrsSourceEncoding::Compressed,
            counts.compressed_rotation,
        ),
        (
            EmptyTrsBindingKind::Scale,
            EmptyTrsSourceEncoding::Plain,
            counts.scale,
        ),
    ] {
        if (0..raw_count).any(|source_index| !identities.contains(&(kind, encoding, source_index)))
        {
            return invalid(format!(
                "animation {:?} discarded a raw {kind:?}/{encoding:?} curve identity",
                animation.name
            ));
        }
    }
    Ok(animation
        .animation_data
        .rotations
        .iter()
        .any(|track| track.source_encoding == EmptyTrsSourceEncoding::Compressed))
}

pub(super) fn validate_animation_duration_provenance(
    animation: &SourceAnimation,
    keyed_duration: Option<f64>,
    event_duration: Option<f64>,
) -> Result<()> {
    for (label, value) in [
        ("declaredDuration", animation.declared_duration.0),
        ("keyedDuration", animation.keyed_duration.0),
        ("eventDuration", animation.event_duration.0),
        ("duration", Some(animation.duration)),
    ] {
        if value.is_some_and(|value| !value.is_finite() || value < 0.0) {
            return invalid(format!(
                "animation {:?} {label} is not finite/nonnegative",
                animation.name
            ));
        }
    }
    if !same_optional_duration(animation.keyed_duration.0, keyed_duration)
        || !same_optional_duration(animation.event_duration.0, event_duration)
    {
        return invalid(format!(
            "animation {:?} keyedDuration/eventDuration do not equal exact decoded maxima",
            animation.name
        ));
    }
    let effective = [
        animation.declared_duration.0,
        animation.keyed_duration.0,
        animation.event_duration.0,
    ]
    .into_iter()
    .flatten()
    .reduce(f64::max)
    .unwrap_or(0.0);
    if !same_duration(animation.duration, effective) {
        return invalid(format!(
            "animation {:?} effective duration is not max(declaredDuration,keyedDuration,eventDuration)",
            animation.name
        ));
    }
    Ok(())
}

pub(super) fn convert_duplicate_animation_keys(
    keys: &[SourceDuplicateAnimationKey],
) -> Result<Vec<DuplicateAnimationKey>> {
    keys.iter()
        .map(|key| {
            Ok(DuplicateAnimationKey {
                source_key_index: u32_index(
                    key.source_key_index,
                    "duplicate animation source key index",
                )?,
                canonical_key_index: u32_index(
                    key.canonical_key_index,
                    "duplicate animation canonical key index",
                )?,
                relation: key.relation,
                key: match &key.key {
                    SourceExactTrsKeyPayload::Vec3(key) => {
                        ExactTrsKeyPayload::Vec3(ExactVec3KeyPayload {
                            time: key.time,
                            value: key.value,
                            in_tangent: key.in_tangent,
                            out_tangent: key.out_tangent,
                            tangent_mode: key.tangent_mode,
                        })
                    }
                    SourceExactTrsKeyPayload::Quaternion(key) => {
                        ExactTrsKeyPayload::Quaternion(ExactQuaternionKeyPayload {
                            time: key.time,
                            value: key.value,
                            in_tangent: key.in_tangent,
                            out_tangent: key.out_tangent,
                            tangent_mode: key.tangent_mode,
                        })
                    }
                },
            })
        })
        .collect()
}

pub(super) fn animation_node_path(
    paths: &BTreeMap<String, u32>,
    root_path: &str,
    path: &str,
    context: &str,
) -> Result<u32> {
    if root_path.is_empty() || root_path.contains('\\') || root_path.trim_matches('/') != root_path
    {
        return invalid(format!("{context} root path {root_path:?} is invalid"));
    }
    if path.is_empty() {
        return exact_node_path(paths, root_path, context);
    }
    if path.contains('\\') || path.trim_matches('/') != path {
        return invalid(format!("{context} path {path:?} is invalid"));
    }

    // Unity AnimationClip paths are relative to the GameObject carrying the
    // Animation component. Prefer that exact relative path before a generic
    // hierarchy lookup. ES668 is the important collision: both its animation
    // root and its animated child are named `patrol_elena7`, while the
    // serialized binding `patrol_elena7` addresses the child.
    let relative = format!("{root_path}/{path}");
    if let Some(index) = paths.get(&relative) {
        return Ok(*index);
    }
    suffix_node_path(paths, path, context)
}

pub(super) fn validate_unbound_animation_node_path(
    paths: &BTreeMap<String, u32>,
    root_path: &str,
    path: &str,
    context: &str,
) -> Result<()> {
    match animation_node_path(paths, root_path, path, context) {
        Ok(_) => invalid(format!(
            "{context} path {path:?} is marked unbound but resolves under the true root"
        )),
        Err(error)
            if error
                .to_string()
                .contains("has no suffix match under the true root") =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
