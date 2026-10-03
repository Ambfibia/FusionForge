use super::*;

pub(super) fn source_identity(env: &UnityEnvironment, key: ObjectKey) -> Result<SourceIdentity, String> {
    Ok(SourceIdentity {
        asset: env
            .assets
            .get(key.asset)
            .ok_or_else(|| format!("source asset index {} is out of range", key.asset))?
            .name
            .clone(),
        path_id: key.path_id,
        object_type: object_type(env, key)?,
    })
}

pub(super) fn component_pointer(value: &UnityValue) -> Option<&super::super::unity::Pointer> {
    match value {
        UnityValue::Pair(_, right) => right.as_pointer(),
        UnityValue::Array(items) if items.len() >= 2 => items[1].as_pointer(),
        _ => value.as_pointer(),
    }
}

pub(super) fn enabled_component(value: &UnityValue) -> bool {
    value
        .get("m_Enabled")
        .and_then(UnityValue::as_i64)
        .map(|enabled| enabled != 0)
        .unwrap_or(true)
}

pub(super) fn authored_transform(value: &UnityValue) -> Result<JsonTransform, String> {
    fn require_components(
        value: Option<&UnityValue>,
        keys: &[&str],
        label: &str,
    ) -> Result<(), String> {
        let object = value
            .and_then(UnityValue::as_object)
            .ok_or_else(|| format!("Transform has no {label} object"))?;
        for key in keys {
            let number = object
                .get(*key)
                .and_then(UnityValue::as_f64)
                .ok_or_else(|| format!("Transform {label}.{key} is not numeric"))?;
            if !number.is_finite() {
                return Err(format!("Transform {label}.{key} is non-finite"));
            }
        }
        Ok(())
    }
    require_components(
        value.get("m_LocalPosition"),
        &["x", "y", "z"],
        "m_LocalPosition",
    )?;
    require_components(
        value.get("m_LocalRotation"),
        &["x", "y", "z", "w"],
        "m_LocalRotation",
    )?;
    require_components(value.get("m_LocalScale"), &["x", "y", "z"], "m_LocalScale")?;
    let position = converted_position(value.get("m_LocalPosition"));
    let rotation = converted_quaternion(value.get("m_LocalRotation"));
    let scale = converted_scale(value.get("m_LocalScale"));
    let result = JsonTransform {
        translation: [position.0, position.1, position.2],
        rotation: [rotation.0, rotation.1, rotation.2, rotation.3],
        scale: [scale.0, scale.1, scale.2],
    };
    if result
        .translation
        .iter()
        .chain(result.rotation.iter())
        .chain(result.scale.iter())
        .any(|number| !number.is_finite())
    {
        return Err("native authored Transform contains a non-finite number".to_string());
    }
    Ok(result)
}

pub(super) fn world_matrix_for_transform(
    key: ObjectKey,
    transforms: &HashMap<ObjectKey, TransformRecord>,
    cache: &mut HashMap<ObjectKey, Matrix4>,
    visiting: &mut HashSet<ObjectKey>,
) -> Result<Matrix4, String> {
    if let Some(matrix) = cache.get(&key) {
        return Ok(*matrix);
    }
    if !visiting.insert(key) {
        return Err(format!(
            "Transform graph contains a cycle at asset {} path {}",
            key.asset, key.path_id
        ));
    }
    let record = transforms.get(&key).ok_or_else(|| {
        format!(
            "Transform graph is missing asset {} path {}",
            key.asset, key.path_id
        )
    })?;
    let matrix = if let Some(parent) = record.parent {
        mat_mul(
            world_matrix_for_transform(parent, transforms, cache, visiting)?,
            record.local,
        )
    } else {
        record.local
    };
    visiting.remove(&key);
    ensure_finite_matrix(matrix, "scene Transform world matrix")?;
    cache.insert(key, matrix);
    Ok(matrix)
}

pub(super) fn effective_scene_active(
    key: ObjectKey,
    game_objects: &HashMap<ObjectKey, GameObjectRecord>,
    transforms: &HashMap<ObjectKey, TransformRecord>,
    transform_to_game_object: &HashMap<ObjectKey, ObjectKey>,
    cache: &mut HashMap<ObjectKey, bool>,
    visiting: &mut HashSet<ObjectKey>,
) -> Result<bool, String> {
    if let Some(active) = cache.get(&key) {
        return Ok(*active);
    }
    if !visiting.insert(key) {
        return Err(format!(
            "GameObject active hierarchy contains a cycle at asset {} path {}",
            key.asset, key.path_id
        ));
    }
    let game_object = game_objects.get(&key).ok_or_else(|| {
        format!(
            "active hierarchy is missing GameObject asset {} path {}",
            key.asset, key.path_id
        )
    })?;
    let transform = transforms.get(&game_object.transform).ok_or_else(|| {
        format!(
            "active hierarchy is missing Transform asset {} path {}",
            game_object.transform.asset, game_object.transform.path_id
        )
    })?;
    let active = if let Some(parent_transform) = transform.parent {
        let parent = transform_to_game_object
            .get(&parent_transform)
            .copied()
            .ok_or_else(|| {
                format!(
                    "parent Transform asset {} path {} has no GameObject",
                    parent_transform.asset, parent_transform.path_id
                )
            })?;
        game_object.active
            && effective_scene_active(
                parent,
                game_objects,
                transforms,
                transform_to_game_object,
                cache,
                visiting,
            )?
    } else {
        game_object.active
    };
    visiting.remove(&key);
    cache.insert(key, active);
    Ok(active)
}

pub(super) fn ensure_finite_matrix(matrix: Matrix4, context: &str) -> Result<(), String> {
    if matrix
        .iter()
        .flat_map(|row| row.iter())
        .all(|number| number.is_finite())
    {
        Ok(())
    } else {
        Err(format!("{context} contains a non-finite matrix value"))
    }
}

pub(super) fn required_json_string<'a>(
    value: &'a JsonValue,
    key: &str,
    context: &str,
) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{context} has no non-empty {key}"))
}

#[cfg(windows)]
pub(super) fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
pub(super) fn metadata_is_reparse_point(_: &fs::Metadata) -> bool {
    false
}
