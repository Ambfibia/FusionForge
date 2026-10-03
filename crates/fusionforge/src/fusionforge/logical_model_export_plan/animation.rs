use super::*;

pub(super) fn exact_animation_clip_paths(value: &super::super::UnityValue) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for field in [
        "m_CompressedRotationCurves",
        "m_PositionCurves",
        "m_RotationCurves",
        "m_ScaleCurves",
        "m_FloatCurves",
        "m_PPtrCurves",
        "m_EditorCurves",
    ] {
        for curve in super::super::value_array(value.get(field)) {
            if let Some(path) = curve
                .get("m_Path")
                .or_else(|| curve.get("path"))
                .and_then(super::super::UnityValue::as_str)
            {
                paths.insert(path.to_string());
            }
        }
    }
    paths
}
