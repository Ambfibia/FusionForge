use super::*;

pub(super) fn extract_actor_nodes(
    catalog: &DumpCatalog,
    root_path_id: i64,
    spec: GenderSpec,
) -> Result<(Vec<PlayerRigNode>, i64, i64)> {
    let root = catalog.value(root_path_id)?;
    let components = component_path_ids(&root)?;
    let mut combiners = Vec::new();
    let mut animations = Vec::new();
    for component in components {
        let object = catalog.get(component)?;
        let value = catalog.value(component)?;
        if object.object_type == "MonoBehaviour" && value.get("actorBones").is_some() {
            combiners.push(component);
        } else if object.object_type == "Animation" {
            animations.push(component);
        }
    }
    let [combiner_path_id] = combiners.as_slice() else {
        return rig_error(format!(
            "{} expected one ActorSkinCombiner, found {}",
            spec.route,
            combiners.len()
        ));
    };
    let [animation_component_path_id] = animations.as_slice() else {
        return rig_error(format!(
            "{} expected one Animation component, found {}",
            spec.route,
            animations.len()
        ));
    };
    let combiner = catalog.value(*combiner_path_id)?;
    let actor_bones = array_field(&combiner, "actorBones")?
        .iter()
        .map(pointer_path_id)
        .collect::<Result<Vec<_>>>()?;
    if actor_bones.len() != spec.expected_actor_bones
        || actor_bones.iter().any(|path_id| *path_id == 0)
    {
        return rig_error(format!(
            "{} actorBones expected {}, found {}",
            spec.route,
            spec.expected_actor_bones,
            actor_bones.len()
        ));
    }
    let indices = actor_bones
        .iter()
        .enumerate()
        .map(|(index, path_id)| (*path_id, index as u32))
        .collect::<HashMap<_, _>>();
    struct Pending {
        transform: i64,
        game_object: i64,
        name: String,
        parent: i64,
        translation: [f64; 3],
        rotation: [f64; 4],
        scale: [f64; 3],
    }
    let mut pending = Vec::with_capacity(actor_bones.len());
    for transform in &actor_bones {
        let object = catalog.get(*transform)?;
        if object.object_type != "Transform" {
            return rig_error(format!(
                "{} actorBones contains {}#{}",
                spec.route, object.object_type, transform
            ));
        }
        let body = catalog.value(*transform)?;
        let game_object = pointer_path_id(
            body.get("m_GameObject")
                .ok_or_else(|| rig_message("Transform has no m_GameObject"))?,
        )?;
        let game_object_value = catalog.get(game_object)?;
        let parent = pointer_path_id(
            body.get("m_Father")
                .ok_or_else(|| rig_message("Transform has no m_Father"))?,
        )?;
        if parent != 0 && !indices.contains_key(&parent) {
            return rig_error(format!(
                "{} actor Transform#{} parent #{} is outside actorBones",
                spec.route, transform, parent
            ));
        }
        pending.push(Pending {
            transform: *transform,
            game_object,
            name: game_object_value.name.clone(),
            parent,
            translation: native_translation(vec3(body.get("m_LocalPosition"), "m_LocalPosition")?),
            rotation: native_rotation(quat(body.get("m_LocalRotation"), "m_LocalRotation")?)?,
            scale: vec3(body.get("m_LocalScale"), "m_LocalScale")?,
        });
    }

    fn path_for(
        index: usize,
        pending: &[Pending],
        indices: &HashMap<i64, u32>,
        cache: &mut [Option<String>],
        visiting: &mut BTreeSet<usize>,
    ) -> Result<String> {
        if let Some(path) = &cache[index] {
            return Ok(path.clone());
        }
        if !visiting.insert(index) {
            return rig_error("actor hierarchy contains a cycle");
        }
        let node = &pending[index];
        let path = if node.parent == 0 {
            node.name.clone()
        } else {
            let parent = *indices
                .get(&node.parent)
                .ok_or_else(|| rig_message("actor parent index disappeared"))?
                as usize;
            format!(
                "{}/{}",
                path_for(parent, pending, indices, cache, visiting)?,
                node.name
            )
        };
        visiting.remove(&index);
        cache[index] = Some(path.clone());
        Ok(path)
    }

    let mut cache = vec![None; pending.len()];
    let mut nodes = Vec::with_capacity(pending.len());
    for (index, node) in pending.iter().enumerate() {
        let full_path = path_for(index, &pending, &indices, &mut cache, &mut BTreeSet::new())?;
        nodes.push(PlayerRigNode {
            actor_bone_index: index as u32,
            source_transform_path_id: node.transform,
            source_game_object_path_id: node.game_object,
            true_name: node.name.clone(),
            full_path,
            parent_actor_bone_index: (node.parent != 0).then(|| indices[&node.parent]),
            translation: node.translation,
            rotation: node.rotation,
            scale: node.scale,
        });
    }
    if nodes.first().is_none_or(|node| {
        node.true_name != spec.root_name || node.parent_actor_bone_index.is_some()
    }) {
        return rig_error(format!(
            "{} actorBones[0] is not the parentless root {}",
            spec.route, spec.root_name
        ));
    }
    Ok((nodes, *combiner_path_id, *animation_component_path_id))
}

pub(super) fn quat_conjugate(value: [f64; 4]) -> [f64; 4] {
    [-value[0], -value[1], -value[2], value[3]]
}

pub(super) fn quat_multiply(left: [f64; 4], right: [f64; 4]) -> [f64; 4] {
    let [lx, ly, lz, lw] = left;
    let [rx, ry, rz, rw] = right;
    [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]
}

pub(super) fn unique_target(paths: &BTreeMap<String, u32>, path: &str) -> Result<u32> {
    paths
        .get(path.trim_matches('/'))
        .copied()
        .ok_or_else(|| rig_message(format!("animation path {path:?} has no exact actor bone")))
}

pub(super) fn unpack_legacy_compressed_quaternion(packed: u32) -> [f64; 4] {
    let flags = packed & 0x7;
    let omitted = (flags & 0x3) as usize;
    let negative = flags & 0x4 != 0;
    let mut cursor = 3_u32;
    let mut quaternion = [0.0_f64; 4];
    let mut squared_sum = 0.0_f64;
    for (component, slot) in quaternion.iter_mut().enumerate() {
        if component == omitted {
            continue;
        }
        let bits = if component == (omitted + 1) % 4 {
            9
        } else {
            10
        };
        let mask = (1_u32 << bits) - 1;
        let raw = (packed >> cursor) & mask;
        cursor += bits;
        *slot = f64::from(raw) / (0.5 * f64::from(mask)) - 1.0;
        squared_sum += *slot * *slot;
    }
    quaternion[omitted] = (1.0 - squared_sum).max(0.0).sqrt();
    if negative {
        quaternion[omitted] = -quaternion[omitted];
    }
    normalize_quaternion(quaternion)
}

pub(super) fn normalize_quaternion(value: [f64; 4]) -> [f64; 4] {
    let norm = value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= f64::EPSILON {
        return [0.0, 0.0, 0.0, 1.0];
    }
    value.map(|component| component / norm)
}

pub(super) fn native_rotation(value: [f64; 4]) -> Result<[f64; 4]> {
    let norm = value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || (norm - 1.0).abs() > 1.0e-4 {
        return rig_error(format!("source quaternion is not unit length: {norm}"));
    }
    Ok([value[0], -value[1], -value[2], value[3]])
}

pub(super) fn native_quat_tangent(value: [f64; 4]) -> [f64; 4] {
    [value[0], -value[1], -value[2], value[3]]
}

pub(super) fn vec3(value: Option<&Value>, label: &str) -> Result<[f64; 3]> {
    let value = value.ok_or_else(|| rig_message(format!("{label} is absent")))?;
    Ok([
        finite(value.get("x"), label)?,
        finite(value.get("y"), label)?,
        finite(value.get("z"), label)?,
    ])
}

pub(super) fn quat(value: Option<&Value>, label: &str) -> Result<[f64; 4]> {
    let value = value.ok_or_else(|| rig_message(format!("{label} is absent")))?;
    Ok([
        finite(value.get("x"), label)?,
        finite(value.get("y"), label)?,
        finite(value.get("z"), label)?,
        finite(value.get("w"), label)?,
    ])
}

pub(super) fn finite(value: Option<&Value>, label: &str) -> Result<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| rig_message(format!("{label} is not finite")))
}

pub(super) fn usize_number(value: &Value, label: &str) -> Result<usize> {
    let value = value
        .as_u64()
        .ok_or_else(|| rig_message(format!("{label} is not unsigned")))?;
    usize::try_from(value).map_err(|_| rig_message(format!("{label} exceeds usize")))
}

pub(super) fn array_field<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| rig_message(format!("{field} is missing or not an array")))
}

pub(super) fn optional_array<'a>(value: &'a Value, field: &str) -> &'a [Value] {
    value
        .get(field)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}
