use super::*;

#[derive(Debug)]
pub(super) struct DecodedClip {
    pub(super) preview: JsonValue,
    pub(super) track_roots: BTreeSet<String>,
    pub(super) warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RawAnimationClip {
    pub(super) asset_name: String,
    pub(super) path_id: i64,
    pub(super) body: fusionforge::UnityValue,
}

/// Proves that an exact logical-model source did not silently discard any
/// serialized legacy TRS binding before a batch exporter stages the JSON.
/// Zero-key Unity bindings are metadata, while non-empty bindings must resolve
/// to one and only one node under the true model root.
pub(crate) fn validate_exact_animation_source(
    model_hierarchy: &JsonValue,
    animations: &[JsonValue],
) -> Result<(), String> {
    let hierarchy_paths = model_hierarchy
        .get("nodes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "exact animation validation has no modelHierarchy.nodes".to_string())?
        .iter()
        .map(|node| {
            node.get("path")
                .and_then(JsonValue::as_str)
                .filter(|path| !path.is_empty())
                .map(str::to_string)
                .ok_or_else(|| "exact model hierarchy contains a node without path".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;

    for animation in animations {
        let name = animation
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("<unnamed>");
        let asset = animation
            .get("asset")
            .and_then(JsonValue::as_str)
            .unwrap_or("<unknown asset>");
        let path_id = animation
            .get("pathId")
            .and_then(JsonValue::as_i64)
            .unwrap_or_default();
        let context = format!("AnimationClip {asset}#{path_id} ({name})");
        let duration = exact_required_duration(animation, "duration", &context)?;
        let declared_duration = exact_optional_duration(animation, "declaredDuration", &context)?;
        let source_keyed_duration = exact_optional_duration(animation, "keyedDuration", &context)?;
        let source_event_duration = exact_optional_duration(animation, "eventDuration", &context)?;

        let decode_warnings = animation
            .get("decodeWarnings")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(JsonValue::as_str)
            .collect::<Vec<_>>();
        if !decode_warnings.is_empty() {
            return Err(format!(
                "{context} has undecoded source curves: {}",
                decode_warnings.join("; ")
            ));
        }

        let rotation = exact_curve_count(animation, "rotation", &context)?;
        let compressed_rotation = exact_curve_count(animation, "compressedRotation", &context)?;
        let position = exact_curve_count(animation, "position", &context)?;
        let scale = exact_curve_count(animation, "scale", &context)?;
        let euler = exact_curve_count(animation, "euler", &context)?;
        let float = exact_curve_count(animation, "float", &context)?;
        let pptr = exact_curve_count(animation, "pptr", &context)?;
        let event_count = exact_curve_count(animation, "events", &context)?;
        if euler != 0 || pptr != 0 {
            return Err(format!(
                "{context} contains unsupported serialized curves (euler={euler}, pptr={pptr}); refusing a lossy source export"
            ));
        }
        if rotation > 0 && compressed_rotation > 0 {
            return Err(format!(
                "{context} mixes plain and compressed rotation curves without per-track provenance"
            ));
        }

        let data = animation.get("animationData");
        let translations = data
            .and_then(|value| value.get("translations"))
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let rotations = data
            .and_then(|value| value.get("rotations"))
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let scales = data
            .and_then(|value| value.get("scales"))
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let float_tracks = data
            .and_then(|value| value.get("floatCurves"))
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let empty_bindings = data
            .and_then(|value| value.get("emptyTrsBindings"))
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let duplicate_bindings = data
            .and_then(|value| value.get("duplicateTrsBindings"))
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let time_recoveries = data
            .and_then(|value| value.get("timeRecoveries"))
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let curve_recoveries = data
            .and_then(|value| value.get("curveRecoveries"))
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);

        let mut empty_translation = 0_usize;
        let mut empty_rotation = 0_usize;
        let mut empty_scale = 0_usize;
        let mut source_identities = BTreeSet::<(String, String, usize)>::new();
        let mut empty_targets = BTreeSet::new();
        let mut previous_order = None;
        for binding in empty_bindings {
            let kind = binding
                .get("kind")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| format!("{context} has emptyTrsBindings entry without kind"))?;
            let source_encoding = binding
                .get("sourceEncoding")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    format!("{context} has emptyTrsBindings entry without sourceEncoding")
                })?;
            let source_index_u64 = binding
                .get("sourceIndex")
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| {
                    format!("{context} has emptyTrsBindings entry without sourceIndex")
                })?;
            let source_index = usize::try_from(source_index_u64).map_err(|_| {
                format!("{context} emptyTrsBindings sourceIndex does not fit usize")
            })?;
            let path = binding
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| format!("{context} has emptyTrsBindings entry without path"))?;

            let (rank, source_count) = match raw_trs_source(
                kind,
                source_encoding,
                position,
                rotation,
                compressed_rotation,
                scale,
            ) {
                Some((rank, source_count)) => {
                    match kind {
                        "translation" => empty_translation += 1,
                        "rotation" => empty_rotation += 1,
                        "scale" => empty_scale += 1,
                        _ => unreachable!("raw_trs_source accepted invalid kind"),
                    }
                    (rank, source_count)
                }
                None => {
                    return Err(format!(
                        "{context} has invalid empty TRS binding kind/encoding {kind:?}/{source_encoding:?}"
                    ));
                }
            };
            if source_index >= source_count {
                return Err(format!(
                    "{context} empty {kind} {source_encoding} binding sourceIndex {source_index} is outside raw count {source_count}"
                ));
            }
            let order = (rank, source_index);
            if previous_order.is_some_and(|previous| previous >= order) {
                return Err(format!(
                    "{context} emptyTrsBindings are not in canonical source order"
                ));
            }
            previous_order = Some(order);
            if !source_identities.insert((
                kind.to_string(),
                source_encoding.to_string(),
                source_index,
            )) {
                return Err(format!(
                    "{context} repeats empty {kind} {source_encoding} sourceIndex {source_index}"
                ));
            }
            if path.contains('\\') || path.trim_matches('/') != path {
                return Err(format!("{context} has invalid animation path {path:?}"));
            }
            // A zero-key Unity binding is serialized metadata, not a sampled
            // transform contract. Dependency bundles can preserve such a
            // binding for a sibling rig (Dracula and Gunter both do this), so
            // retain and count its exact source identity without requiring the
            // target to exist under this model root. If it does resolve, keep
            // participating in the empty-vs-sampleable collision proof below.
            if let Ok(resolved) = resolve_exact_animation_path(&hierarchy_paths, path, &context) {
                empty_targets.insert((kind.to_string(), resolved));
            }
        }

        let mut duplicate_translation = 0_usize;
        let mut duplicate_rotation = 0_usize;
        let mut duplicate_scale = 0_usize;
        let mut previous_duplicate_order = None;
        for (duplicate_index, binding) in duplicate_bindings.iter().enumerate() {
            let kind = binding
                .get("kind")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    format!("{context} duplicateTrsBindings[{duplicate_index}] has no kind")
                })?;
            let path = binding
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    format!("{context} duplicateTrsBindings[{duplicate_index}] has no path")
                })?;
            let source_encoding = binding
                .get("sourceEncoding")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    format!(
                        "{context} duplicateTrsBindings[{duplicate_index}] has no sourceEncoding"
                    )
                })?;
            let source_index = binding
                .get("sourceIndex")
                .and_then(JsonValue::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    format!(
                        "{context} duplicateTrsBindings[{duplicate_index}] has invalid sourceIndex"
                    )
                })?;
            let relation = binding
                .get("relation")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    format!("{context} duplicateTrsBindings[{duplicate_index}] has no relation")
                })?;
            if relation != "identical" && relation != "serializedLastWriteWins" {
                return Err(format!(
                    "{context} duplicateTrsBindings[{duplicate_index}] relation {relation:?} is unsupported"
                ));
            }
            let (rank, source_count) = raw_trs_source(
                kind,
                source_encoding,
                position,
                rotation,
                compressed_rotation,
                scale,
            )
            .ok_or_else(|| {
                format!(
                    "{context} duplicateTrsBindings[{duplicate_index}] has invalid kind/encoding {kind:?}/{source_encoding:?}"
                )
            })?;
            if source_index >= source_count {
                return Err(format!(
                    "{context} duplicateTrsBindings[{duplicate_index}] sourceIndex {source_index} is outside raw count {source_count}"
                ));
            }
            let order = (rank, source_index);
            if previous_duplicate_order.is_some_and(|previous| previous >= order) {
                return Err(format!(
                    "{context} duplicateTrsBindings are not in canonical source order"
                ));
            }
            previous_duplicate_order = Some(order);
            if !source_identities.insert((
                kind.to_string(),
                source_encoding.to_string(),
                source_index,
            )) {
                return Err(format!(
                    "{context} repeats {kind} {source_encoding} sourceIndex {source_index}"
                ));
            }

            let canonical_track_index = binding
                .get("canonicalTrackIndex")
                .and_then(JsonValue::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| format!("{context} duplicateTrsBindings[{duplicate_index}] has invalid canonicalTrackIndex"))?;
            let canonical_tracks = match kind {
                "translation" => {
                    duplicate_translation += 1;
                    translations
                }
                "rotation" => {
                    duplicate_rotation += 1;
                    rotations
                }
                "scale" => {
                    duplicate_scale += 1;
                    scales
                }
                _ => unreachable!("raw_trs_source accepted invalid kind"),
            };
            let canonical = canonical_tracks.get(canonical_track_index).ok_or_else(|| {
                format!(
                    "{context} duplicateTrsBindings[{duplicate_index}] canonicalTrackIndex {canonical_track_index} is out of range"
                )
            })?;
            let canonical_path = canonical.get("path").and_then(JsonValue::as_str);
            let canonical_source_index = canonical
                .get("sourceIndex")
                .and_then(JsonValue::as_u64)
                .and_then(|value| usize::try_from(value).ok());
            let canonical_source_encoding =
                canonical.get("sourceEncoding").and_then(JsonValue::as_str);
            if canonical_path != Some(path)
                || binding
                    .get("canonicalSourceIndex")
                    .and_then(JsonValue::as_u64)
                    != canonical_source_index.and_then(|value| u64::try_from(value).ok())
                || binding
                    .get("canonicalSourceEncoding")
                    .and_then(JsonValue::as_str)
                    != canonical_source_encoding
            {
                return Err(format!(
                    "{context} duplicateTrsBindings[{duplicate_index}] does not target its declared canonical track"
                ));
            }
            if relation == "identical"
                && (binding.get("keys") != canonical.get("keys")
                    || binding.get("duplicateKeys") != canonical.get("duplicateKeys")
                    || binding.get("sourceKeyCount") != canonical.get("sourceKeyCount"))
            {
                return Err(format!(
                    "{context} duplicateTrsBindings[{duplicate_index}] does not exactly match its canonical track"
                ));
            }
            if relation == "serializedLastWriteWins" {
                let proof = binding
                    .get("resolutionProof")
                    .ok_or_else(|| format!(
                        "{context} duplicateTrsBindings[{duplicate_index}] has no serialized overwrite proof"
                    ))?;
                if proof.get("asset").and_then(JsonValue::as_str) != Some(asset)
                    || proof.get("pathId").and_then(JsonValue::as_i64) != Some(path_id)
                    || proof.get("clipName").and_then(JsonValue::as_str) != Some(name)
                    || proof.get("rule").and_then(JsonValue::as_str)
                        != Some("later-serialized-binding-overwrites-earlier")
                    || proof.get("exactTargetPath").and_then(JsonValue::as_bool) != Some(true)
                    || proof
                        .get("exactSourceArrayOrder")
                        .and_then(JsonValue::as_bool)
                        != Some(true)
                    || canonical_source_encoding != Some(source_encoding)
                    || canonical_source_index.is_none_or(|canonical| canonical <= source_index)
                {
                    return Err(format!(
                        "{context} duplicateTrsBindings[{duplicate_index}] has an invalid serialized overwrite proof"
                    ));
                }
            }
            if canonical_source_index == Some(source_index)
                && canonical_source_encoding == Some(source_encoding)
            {
                return Err(format!(
                    "{context} duplicateTrsBindings[{duplicate_index}] repeats its canonical source identity"
                ));
            }
            resolve_exact_animation_path(&hierarchy_paths, path, &context)?;
        }

        let recovered_translation = curve_recovery_rejected_count(curve_recoveries, "translation");
        let recovered_rotation = curve_recovery_rejected_count(curve_recoveries, "rotation");
        let recovered_scale = curve_recovery_rejected_count(curve_recoveries, "scale");
        if position
            != translations.len()
                + empty_translation
                + duplicate_translation
                + recovered_translation
        {
            return Err(format!(
                "{context} position curve count {position} != {} sampleable + {empty_translation} empty + {duplicate_translation} preserved duplicate + {recovered_translation} sibling-rejected bindings",
                translations.len()
            ));
        }
        if rotation + compressed_rotation
            != rotations.len() + empty_rotation + duplicate_rotation + recovered_rotation
        {
            return Err(format!(
                "{context} rotation curve count {} != {} sampleable + {empty_rotation} empty + {duplicate_rotation} preserved duplicate + {recovered_rotation} sibling-rejected bindings",
                rotation + compressed_rotation,
                rotations.len()
            ));
        }
        if scale != scales.len() + empty_scale + duplicate_scale + recovered_scale {
            return Err(format!(
                "{context} scale curve count {scale} != {} sampleable + {empty_scale} empty + {duplicate_scale} preserved duplicate + {recovered_scale} sibling-rejected bindings",
                scales.len()
            ));
        }
        if float != float_tracks.len() {
            return Err(format!(
                "{context} float curve count {float} != {} exact decoded bindings",
                float_tracks.len()
            ));
        }
        let events = animation
            .get("events")
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if event_count != events.len() {
            return Err(format!(
                "{context} event count {event_count} != {} decoded events",
                events.len()
            ));
        }

        let mut sampleable_targets = BTreeSet::new();
        let translation_duration = validate_sampleable_tracks(
            translations,
            "translation",
            &[("plain", position)],
            &hierarchy_paths,
            &context,
            duration,
            &mut sampleable_targets,
            &mut source_identities,
        )?;
        let rotation_duration = validate_sampleable_tracks(
            rotations,
            "rotation",
            &[("plain", rotation), ("compressed", compressed_rotation)],
            &hierarchy_paths,
            &context,
            duration,
            &mut sampleable_targets,
            &mut source_identities,
        )?;
        let scale_duration = validate_sampleable_tracks(
            scales,
            "scale",
            &[("plain", scale)],
            &hierarchy_paths,
            &context,
            duration,
            &mut sampleable_targets,
            &mut source_identities,
        )?;
        let float_duration = validate_sampleable_float_tracks(
            float_tracks,
            float,
            &hierarchy_paths,
            &context,
            duration,
        )?;
        validate_time_recoveries(
            animation,
            time_recoveries,
            translations,
            rotations,
            scales,
            animations,
            &hierarchy_paths,
            &context,
        )?;
        validate_curve_recoveries(
            animation,
            curve_recoveries,
            translations,
            rotations,
            scales,
            animations,
            &hierarchy_paths,
            &context,
            &mut source_identities,
        )?;
        for (kind, encoding, count) in [
            ("translation", "plain", position),
            ("rotation", "plain", rotation),
            ("rotation", "compressed", compressed_rotation),
            ("scale", "plain", scale),
        ] {
            for source_index in 0..count {
                if !source_identities.contains(&(
                    kind.to_string(),
                    encoding.to_string(),
                    source_index,
                )) {
                    return Err(format!(
                        "{context} discarded raw {kind} {encoding} curve[{source_index}]"
                    ));
                }
            }
        }
        if let Some((kind, path)) = empty_targets.intersection(&sampleable_targets).next() {
            return Err(format!(
                "{context} has both empty and sampleable {kind} bindings for target {path:?}"
            ));
        }
        let recovered_curve_duration = curve_recovery_rejected_keyed_duration(curve_recoveries);
        let keyed_duration = [
            translation_duration,
            rotation_duration,
            scale_duration,
            float_duration,
            recovered_curve_duration,
        ]
        .into_iter()
        .flatten()
        .fold(None::<f64>, |maximum, value| {
            Some(maximum.map_or(value, |current| current.max(value)))
        });
        let event_duration = events
            .iter()
            .enumerate()
            .map(|(event_index, event)| {
                validate_exact_event_object_parameter(event, event_index, &context)?;
                validate_exact_event_float_parameter(event, event_index, &context)?;
                let time = event
                    .get("time")
                    .and_then(JsonValue::as_f64)
                    .filter(|time| {
                        time.is_finite()
                            && (*time as f32).is_finite()
                            && *time >= 0.0
                            && *time <= duration + 1.0e-6
                    })
                    .ok_or_else(|| format!("{context} event[{event_index}] has invalid time"))?;
                Ok(time)
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .fold(None::<f64>, |maximum, value| {
                Some(maximum.map_or(value, |current| current.max(value)))
            });
        exact_duration_matches(
            source_keyed_duration,
            keyed_duration,
            "keyedDuration",
            &context,
        )?;
        exact_duration_matches(
            source_event_duration,
            event_duration,
            "eventDuration",
            &context,
        )?;
        let effective_duration = [declared_duration, keyed_duration, event_duration]
            .into_iter()
            .flatten()
            .fold(0.0_f64, f64::max);
        if !durations_equal(duration, effective_duration) {
            return Err(format!(
                "{context} effective duration {duration} != max(declared/keyed/event) {effective_duration}"
            ));
        }
    }
    Ok(())
}

/// Marks keyed Unity bindings which the selected model hierarchy cannot
/// consume. Legacy `Animation` silently ignores curves whose relative paths do
/// not exist below its GameObject; shared clips can therefore contain a second
/// rig alongside the selected character. Keep those curves in source order and
/// with their full key payload, but make their non-runtime disposition explicit
/// for the native publisher.
pub(crate) fn mark_unbound_animation_bindings(
    model_hierarchy: &JsonValue,
    animations: &mut [JsonValue],
) -> Result<(), String> {
    let hierarchy_paths = model_hierarchy
        .get("nodes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "unbound animation classification has no modelHierarchy.nodes".to_string())?
        .iter()
        .map(|node| {
            node.get("path")
                .and_then(JsonValue::as_str)
                .filter(|path| !path.is_empty())
                .map(str::to_string)
                .ok_or_else(|| "exact model hierarchy contains a node without path".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;

    for animation in animations {
        let name = animation
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("<unnamed>");
        let context = format!("AnimationClip {name}");
        let Some(data) = animation
            .get_mut("animationData")
            .and_then(JsonValue::as_object_mut)
        else {
            // Unity preserves zero-curve metadata-only clips (most commonly
            // `nif-default`) without an animationData payload. There are no
            // keyed model targets to classify in that case. The exact source
            // validator that runs immediately afterwards still compares all
            // serialized curve counts, so a non-empty clip cannot use this
            // branch to hide missing TRS data.
            continue;
        };
        for field in ["translations", "rotations", "scales"] {
            let tracks = data
                .get_mut(field)
                .and_then(JsonValue::as_array_mut)
                .ok_or_else(|| format!("{context} has no {field} array"))?;
            for (index, track) in tracks.iter_mut().enumerate() {
                let path = track
                    .get("path")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| format!("{context} {field}[{index}] has no path"))?;
                match resolve_exact_animation_path(&hierarchy_paths, path, &context) {
                    Ok(_) => {}
                    Err(error) if error.contains("has no suffix match under the true root") => {
                        track
                            .as_object_mut()
                            .ok_or_else(|| format!("{context} {field}[{index}] is not an object"))?
                            .insert("unboundModelTarget".to_string(), JsonValue::Bool(true));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        let empty_bindings = data
            .get_mut("emptyTrsBindings")
            .and_then(JsonValue::as_array_mut)
            .ok_or_else(|| format!("{context} has no emptyTrsBindings array"))?;
        for (index, binding) in empty_bindings.iter_mut().enumerate() {
            let path = binding
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| format!("{context} emptyTrsBindings[{index}] has no path"))?;
            match resolve_exact_animation_path(&hierarchy_paths, path, &context) {
                Ok(_) => {}
                Err(error) if error.contains("has no suffix match under the true root") => {
                    binding
                        .as_object_mut()
                        .ok_or_else(|| {
                            format!("{context} emptyTrsBindings[{index}] is not an object")
                        })?
                        .insert("unboundModelTarget".to_string(), JsonValue::Bool(true));
                }
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}

pub(super) fn animation_tracks<'a>(animation: &'a JsonValue, kind: &str) -> &'a [JsonValue] {
    let field = match kind {
        "translation" => "translations",
        "rotation" => "rotations",
        "scale" => "scales",
        _ => return &[],
    };
    animation
        .get("animationData")
        .and_then(|data| data.get(field))
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(super) fn resolve_exact_animation_path(
    hierarchy_paths: &[String],
    path: &str,
    context: &str,
) -> Result<String, String> {
    if path.contains('\\') || path.trim_matches('/') != path {
        return Err(format!("{context} has invalid animation path {path:?}"));
    }
    if path.is_empty() {
        let roots = hierarchy_paths
            .iter()
            .filter(|candidate| !candidate.contains('/'))
            .collect::<Vec<_>>();
        return match roots.as_slice() {
            [root] => Ok((*root).clone()),
            [] => Err(format!(
                "{context} empty animation path has no hierarchy root"
            )),
            _ => Err(format!(
                "{context} empty animation path is ambiguous across hierarchy roots"
            )),
        };
    }
    let suffix = format!("/{path}");
    let exact_matches = hierarchy_paths
        .iter()
        .filter(|candidate| candidate.as_str() == path)
        .collect::<Vec<_>>();
    let descendant_matches = hierarchy_paths
        .iter()
        .filter(|candidate| candidate.as_str() != path && candidate.ends_with(&suffix))
        .collect::<Vec<_>>();
    // Unity legacy-animation paths are relative to the GameObject that owns
    // the Animation component. A non-empty path therefore names a descendant,
    // even when the true model root and its direct child share the same
    // m_Name (for example f_pants_gothgirl/f_pants_gothgirl). The old suffix
    // resolver counted the root itself as a second candidate and rejected an
    // otherwise exact serialized closure.
    match descendant_matches.as_slice() {
        [resolved] => Ok((*resolved).clone()),
        [] => match exact_matches.as_slice() {
            [resolved] => Ok((*resolved).clone()),
            [] => Err(format!(
                "{context} animation path {path:?} has no suffix match under the true root"
            )),
            _ => Err(format!(
                "{context} animation path {path:?} is ambiguous under the true root"
            )),
        },
        _ => Err(format!(
            "{context} animation path {path:?} is ambiguous under the true root"
        )),
    }
}

pub(super) fn rest_global_matrix_in_skeleton_space(
    key: ObjectKey,
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    joint_paths: &BTreeMap<ObjectKey, String>,
    cache: &mut BTreeMap<ObjectKey, Matrix4>,
    visiting: &mut BTreeSet<ObjectKey>,
) -> Option<Matrix4> {
    if let Some(matrix) = cache.get(&key) {
        return Some(*matrix);
    }
    if !joint_paths.contains_key(&key) || !visiting.insert(key) {
        return None;
    }
    let node = transforms.get(&key)?;
    let local = preview_local_matrix(node);
    let global = match node
        .parent
        .filter(|parent| joint_paths.contains_key(parent))
    {
        Some(parent) => mat_mul(
            rest_global_matrix_in_skeleton_space(parent, transforms, joint_paths, cache, visiting)?,
            local,
        ),
        None => local,
    };
    visiting.remove(&key);
    cache.insert(key, global);
    Some(global)
}
