use super::*;

pub(super) fn is_exact_larry_unused_end_event_pointer(
    asset: &str,
    path_id: i64,
    clip_name: &str,
    event_index: usize,
    event: &JsonValue,
) -> bool {
    if asset != "CustomAssetBundle-574ca8c2fed89492582a3fbd49965c12"
        || event_index != 0
        || event.get("functionName").and_then(JsonValue::as_str) != Some("end")
        || event.get("stringParameter").and_then(JsonValue::as_str) != Some("")
        || event.get("intParameter").and_then(JsonValue::as_i64) != Some(0)
    {
        return false;
    }
    let provenance = match event
        .get("objectParameterProvenance")
        .and_then(JsonValue::as_object)
    {
        Some(provenance) => provenance,
        None => return false,
    };
    if provenance
        .get("sourceAssetIndex")
        .and_then(JsonValue::as_u64)
        != Some(5)
        || provenance.get("fileId").and_then(JsonValue::as_i64) != Some(1)
    {
        return false;
    }
    let event_time = event.get("time").and_then(JsonValue::as_f64);
    let object_path_id = provenance.get("pathId").and_then(JsonValue::as_i64);
    let message_options = event.get("messageOptions").and_then(JsonValue::as_i64);
    matches!(
        (
            path_id,
            clip_name,
            event_time,
            object_path_id,
            message_options
        ),
        (
            152,
            "stand2",
            Some(4.730000019073486),
            Some(6),
            Some(13_156)
        ) | (153, "stand3", Some(2.5), Some(4), Some(0))
            | (154, "walk", Some(3.0), Some(2_816), Some(1_852_788_223))
    )
}

pub(super) fn is_exact_larry_walk_end_event_with_stale_float(
    asset_name: &str,
    path_id: i64,
    clip_name: &str,
    event_index: usize,
    event: &fusionforge::UnityValue,
    time: f64,
    function_name: &str,
    string_parameter: &str,
    float_parameter: f64,
) -> bool {
    asset_name == "CustomAssetBundle-574ca8c2fed89492582a3fbd49965c12"
        && path_id == 154
        && clip_name == "walk"
        && event_index == 0
        && time == 3.0
        && function_name == "end"
        && string_parameter.is_empty()
        && event_field(event, &["messageOptions", "m_MessageOptions"])
            .and_then(fusionforge::UnityValue::as_i64)
            == Some(1_852_788_223)
        && !float_parameter.is_finite()
        && (float_parameter as f32).to_bits() == 0xffff_ff00
}

pub(super) fn event_field<'a>(
    event: &'a fusionforge::UnityValue,
    names: &[&str],
) -> Option<&'a fusionforge::UnityValue> {
    names.iter().find_map(|name| event.get(name))
}

pub(super) fn optional_event_string(
    event: &fusionforge::UnityValue,
    names: &[&str],
    label: &str,
) -> Result<String, String> {
    match event_field(event, names) {
        None => Ok(String::new()),
        Some(value) => value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("has invalid {label}")),
    }
}

pub(super) fn optional_event_i64(
    event: &fusionforge::UnityValue,
    names: &[&str],
    label: &str,
) -> Result<i64, String> {
    match event_field(event, names) {
        None => Ok(0),
        Some(value) => value.as_i64().ok_or_else(|| format!("has invalid {label}")),
    }
}
