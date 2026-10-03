use super::*;

pub(super) fn full_transform_path(
    transforms: &BTreeMap<ObjectKey, TransformNode>,
    key: ObjectKey,
) -> Option<String> {
    let mut names = Vec::new();
    let mut current = Some(key);
    let mut visited = BTreeSet::new();
    while let Some(candidate) = current {
        if !visited.insert(candidate) {
            return None;
        }
        let node = transforms.get(&candidate)?;
        names.push(node.name.clone());
        current = node.parent;
    }
    names.reverse();
    Some(normalized_path(&names.join("/")))
}

pub(super) fn preferred_path_parts(path: &str) -> (String, BTreeSet<String>) {
    let normalized = path.trim().replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let compact = compact_character_name(stem);
    let ignored = ["mob", "npc", "character", "model", "mesh"];
    let tokens = stem
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::trim)
        .filter(|token| token.len() >= 3)
        .map(str::to_ascii_lowercase)
        .filter(|token| !ignored.contains(&token.as_str()))
        .collect::<BTreeSet<_>>();
    (compact, tokens)
}

pub(super) fn build_curve_time_recovery_catalog(clips: &[RawAnimationClip]) -> Vec<CurveTimeReference> {
    let mut result = Vec::new();
    for clip in clips {
        let clip_name = fusionforge::object_name(&clip.body);
        let sample_rate = clip_sample_rate(&clip.body);
        for (field, kind) in [
            ("m_PositionCurves", "translation"),
            ("m_RotationCurves", "rotation"),
            ("m_ScaleCurves", "scale"),
        ] {
            for (source_index, curve) in fusionforge::value_array(clip.body.get(field))
                .iter()
                .enumerate()
            {
                let Some((path, times, payload_without_time)) = raw_plain_curve_parts(curve) else {
                    continue;
                };
                if times.is_empty() || !strictly_increasing_finite_times(&times) {
                    continue;
                }
                result.push(CurveTimeReference {
                    asset_name: clip.asset_name.clone(),
                    path_id: clip.path_id,
                    clip_name: clip_name.clone(),
                    field,
                    kind,
                    source_index,
                    path,
                    sample_rate,
                    times,
                    payload_without_time,
                });
            }
        }
    }
    result
}

pub(super) fn build_constant_curve_recovery_catalog(
    clips: &[RawAnimationClip],
) -> Vec<ConstantCurveReference> {
    let mut result = Vec::new();
    for clip in clips {
        let clip_name = fusionforge::object_name(&clip.body);
        let sample_rate = clip_sample_rate(&clip.body);
        for field in ["m_PositionCurves", "m_RotationCurves", "m_ScaleCurves"] {
            let Some(kind) = plain_trs_kind(field) else {
                continue;
            };
            let curves = fusionforge::value_array(clip.body.get(field));
            let mut path_counts = BTreeMap::<String, usize>::new();
            for curve in curves {
                if let Some(path) = plain_curve_path(curve) {
                    *path_counts.entry(path).or_default() += 1;
                }
            }
            for (source_index, curve) in curves.iter().enumerate() {
                let Ok((path, samples)) = decode_plain_trs_curve(field, curve) else {
                    continue;
                };
                if path_counts.get(&path) != Some(&1) {
                    continue;
                }
                let Some(constant_payload) = constant_curve_payload(&samples) else {
                    continue;
                };
                result.push(ConstantCurveReference {
                    asset_name: clip.asset_name.clone(),
                    path_id: clip.path_id,
                    clip_name: clip_name.clone(),
                    field,
                    kind,
                    source_index,
                    path,
                    sample_rate,
                    constant_payload,
                });
            }
        }
    }
    result
}

pub(super) fn plain_curve_path(curve: &fusionforge::UnityValue) -> Option<String> {
    curve
        .get("path")
        .or_else(|| curve.get("m_Path"))
        .and_then(fusionforge::UnityValue::as_str)
        .map(normalized_path)
        .filter(|path| !path.is_empty())
}

pub(super) fn normalized_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}
