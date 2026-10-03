use super::*;

pub(super) fn collect_pointers(value: &JsonValue, path: &str, output: &mut Vec<JsonPointer>) {
    match value {
        JsonValue::Object(object) => {
            if let (Some(file_id), Some(path_id)) = (
                object.get("fileId").and_then(JsonValue::as_i64),
                object.get("pathId").and_then(JsonValue::as_i64),
            ) {
                output.push(JsonPointer {
                    file_id,
                    path_id,
                    json_path: path.to_owned(),
                });
                return;
            }
            for (key, child) in object {
                collect_pointers(child, &format!("{path}.{key}"), output);
            }
        }
        JsonValue::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                collect_pointers(child, &format!("{path}[{index}]"), output);
            }
        }
        _ => {}
    }
}

pub(super) fn parse_bullet_parameters(row: &JsonValue, bullet_type: i32) -> Result<TutorialBulletParameters> {
    Ok(TutorialBulletParameters {
        cancel_script: required_i32(row, "m_iCancelScript", bullet_type)?,
        fire_script: required_i32(row, "m_iFireScript", bullet_type)?,
        particle_script: required_i32(row, "m_iParticleScript", bullet_type)?,
        success_script: required_i32(row, "m_iSuccScript", bullet_type)?,
        cancel_model_scale: required_f64(row, "m_fCancelModelScale", bullet_type)?,
        curve_height: required_f64(row, "m_fCurveHeight", bullet_type)?,
        fire_model_scale: required_f64(row, "m_fFireModelScale", bullet_type)?,
        bullet_model_scale: required_f64(row, "m_fBulletModelScale", bullet_type)?,
        success_model_scale: required_f64(row, "m_fSuccModelScale", bullet_type)?,
        hide_time_seconds: required_f64(row, "m_fHideTime", bullet_type)?,
        maximum_time_seconds: required_f64(row, "m_fMaxTimer", bullet_type)?,
        fire_link: required_string(row, "m_strFireLink", bullet_type)?,
        success_link: required_string(row, "m_strSuccLink", bullet_type)?,
        success_sound: required_string(row, "m_strSuccSound", bullet_type)?,
    })
}

pub(super) fn find_unique_field<'a>(value: &'a JsonValue, field: &str) -> Result<&'a JsonValue> {
    let mut matches = Vec::new();
    collect_field(value, field, &mut matches);
    if matches.len() != 1 {
        return invalid(format!(
            "serialized Unity value contains {} occurrences of required field {field:?}",
            matches.len()
        ));
    }
    Ok(matches[0])
}

pub(super) fn collect_field<'a>(value: &'a JsonValue, field: &str, matches: &mut Vec<&'a JsonValue>) {
    match value {
        JsonValue::Object(object) => {
            if let Some(value) = object.get(field) {
                matches.push(value);
            }
            for value in object.values() {
                collect_field(value, field, matches);
            }
        }
        JsonValue::Array(array) => {
            for value in array {
                collect_field(value, field, matches);
            }
        }
        _ => {}
    }
}
