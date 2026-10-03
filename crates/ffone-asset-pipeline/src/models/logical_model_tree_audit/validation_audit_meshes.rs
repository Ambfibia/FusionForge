use super::*;

pub(super) fn audit_binary_layout(
    document: &Value,
    binary: &[u8],
    relative: &str,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> (Vec<Option<BufferViewInfo>>, Vec<Option<AccessorInfo>>) {
    let buffers = array_or_empty(document, "buffers");
    let declared_buffer_len = if buffers.len() == 1 {
        if buffers[0].get("uri").is_some() {
            push_violation(
                violations,
                "external_geometry_buffer",
                relative,
                "native logical-model GLB geometry must be self-contained",
            );
        }
        buffers[0]
            .get("byteLength")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or_else(|| {
                push_violation(
                    violations,
                    "invalid_buffer_length",
                    relative,
                    "buffer byteLength is missing or too large",
                );
                0
            })
    } else {
        push_violation(
            violations,
            "invalid_buffer_table",
            relative,
            "logical-model GLB must contain exactly one binary buffer",
        );
        0
    };
    if declared_buffer_len > binary.len() || binary.len().saturating_sub(declared_buffer_len) > 3 {
        push_violation(
            violations,
            "binary_buffer_length_mismatch",
            relative,
            format!(
                "declared buffer length {declared_buffer_len} does not match BIN chunk {}",
                binary.len()
            ),
        );
    }

    let raw_views = array_or_empty(document, "bufferViews");
    let mut views = Vec::with_capacity(raw_views.len());
    for (index, view) in raw_views.iter().enumerate() {
        let parsed = (|| {
            if view.get("buffer").and_then(Value::as_u64) != Some(0) {
                return None;
            }
            let offset = json_usize_default(view.get("byteOffset"), 0)?;
            let length = json_usize(view.get("byteLength"))?;
            let stride = match view.get("byteStride") {
                Some(value) => Some(json_usize(Some(value))?),
                None => None,
            };
            let end = offset.checked_add(length)?;
            if end > declared_buffer_len || end > binary.len() {
                return None;
            }
            if stride.is_some_and(|stride| !(4..=252).contains(&stride) || stride % 4 != 0) {
                return None;
            }
            Some(BufferViewInfo {
                offset,
                length,
                stride,
            })
        })();
        if parsed.is_none() {
            push_violation(
                violations,
                "invalid_buffer_view",
                relative,
                format!("bufferView {index} is out of bounds or malformed"),
            );
        }
        views.push(parsed);
    }

    let raw_accessors = array_or_empty(document, "accessors");
    let mut accessors = Vec::with_capacity(raw_accessors.len());
    for (index, accessor) in raw_accessors.iter().enumerate() {
        let parsed = (|| {
            if accessor.get("sparse").is_some() {
                return None;
            }
            let view_index = json_usize(accessor.get("bufferView"))?;
            let view = *views.get(view_index)?.as_ref()?;
            let accessor_offset = json_usize_default(accessor.get("byteOffset"), 0)?;
            let component_type = accessor
                .get("componentType")?
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())?;
            let component_count = accessor
                .get("type")?
                .as_str()
                .and_then(accessor_component_count)?;
            let count = json_usize(accessor.get("count"))?;
            if count == 0 {
                return None;
            }
            let normalized = accessor
                .get("normalized")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let info = AccessorInfo {
                view,
                accessor_offset,
                component_type,
                component_count,
                count,
                normalized,
            };
            let element_size = info.element_size()?;
            let stride = info.stride()?;
            if stride < element_size {
                return None;
            }
            let occupied = accessor_offset
                .checked_add((count - 1).checked_mul(stride)?)?
                .checked_add(element_size)?;
            if occupied > view.length {
                return None;
            }
            Some(info)
        })();
        if parsed.is_none() {
            push_violation(
                violations,
                "invalid_accessor",
                relative,
                format!("accessor {index} is out of bounds, sparse, empty or malformed"),
            );
        }
        accessors.push(parsed);
    }
    (views, accessors)
}

pub(super) fn audit_hierarchy(
    document: &Value,
    nodes: &[Value],
    mesh_count: usize,
    skin_count: usize,
    logical_name: Option<&str>,
    relative: &str,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> Option<usize> {
    if nodes.is_empty() {
        push_violation(
            violations,
            "empty_node_hierarchy",
            relative,
            "logical model has no nodes",
        );
        return None;
    }
    let scenes = array_or_empty(document, "scenes");
    if scenes.len() != 1 || document.get("scene").and_then(Value::as_u64) != Some(0) {
        push_violation(
            violations,
            "scattered_scenes",
            relative,
            "logical model must contain exactly one selected scene",
        );
    }
    let roots = scenes
        .first()
        .and_then(|scene| scene.get("nodes"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if roots.len() != 1 {
        push_violation(
            violations,
            "multiple_logical_roots",
            relative,
            "logical model scene must contain exactly one root node",
        );
    }
    let root = roots
        .first()
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|index| *index < nodes.len());
    if roots.len() == 1 && root.is_none() {
        push_violation(
            violations,
            "invalid_scene_root",
            relative,
            "scene root node index is invalid",
        );
    }

    let mut parents = vec![None; nodes.len()];
    for (node_index, node) in nodes.iter().enumerate() {
        match node.get("name").and_then(Value::as_str) {
            Some(name) => validate_true_name(name, "node", relative, violations),
            None => push_violation(
                violations,
                "missing_node_name",
                relative,
                format!("node {node_index} has no exact name"),
            ),
        }
        if let Some(mesh) = node.get("mesh").and_then(Value::as_u64) {
            if usize::try_from(mesh)
                .ok()
                .is_none_or(|mesh| mesh >= mesh_count)
            {
                push_violation(
                    violations,
                    "invalid_node_mesh",
                    relative,
                    format!("node {node_index} references invalid mesh {mesh}"),
                );
            }
        }
        if let Some(skin) = node.get("skin").and_then(Value::as_u64) {
            if node.get("mesh").is_none()
                || usize::try_from(skin)
                    .ok()
                    .is_none_or(|skin| skin >= skin_count)
            {
                push_violation(
                    violations,
                    "invalid_node_skin",
                    relative,
                    format!("node {node_index} has a skin without a valid mesh/skin index"),
                );
            }
        }
        for child in node
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(child) = child
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|child| *child < nodes.len())
            else {
                push_violation(
                    violations,
                    "invalid_child_node",
                    relative,
                    format!("node {node_index} contains an invalid child index"),
                );
                continue;
            };
            if parents[child].replace(node_index).is_some() {
                push_violation(
                    violations,
                    "multiple_node_parents",
                    relative,
                    format!("node {child} is listed under more than one parent"),
                );
            }
        }
    }

    let parentless = parents
        .iter()
        .enumerate()
        .filter_map(|(index, parent)| parent.is_none().then_some(index))
        .collect::<Vec<_>>();
    if parentless.len() != 1 || root.is_some_and(|root| parentless != [root]) {
        push_violation(
            violations,
            "parentless_node_scatter",
            relative,
            format!(
                "node graph has {} parentless nodes instead of the sole scene root",
                parentless.len()
            ),
        );
    }
    if let Some(root) = root {
        if let Some(expected) = logical_name {
            let actual = nodes[root].get("name").and_then(Value::as_str);
            if actual != Some(expected) {
                push_violation(
                    violations,
                    "root_name_mismatch",
                    relative,
                    format!(
                        "scene root name {:?} differs from extras.logicalModelName {expected:?}",
                        actual
                    ),
                );
            }
        }
        let mut stack = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(index) = stack.pop() {
            if !reachable.insert(index) {
                push_violation(
                    violations,
                    "hierarchy_cycle_or_duplicate",
                    relative,
                    format!("node {index} is reached more than once"),
                );
                continue;
            }
            stack.extend(
                nodes[index]
                    .get("children")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_u64)
                    .filter_map(|value| usize::try_from(value).ok())
                    .filter(|child| *child < nodes.len()),
            );
        }
        if reachable.len() != nodes.len() {
            push_violation(
                violations,
                "unreachable_nodes",
                relative,
                format!(
                    "only {} of {} nodes are reachable from the logical root",
                    reachable.len(),
                    nodes.len()
                ),
            );
        }
    }
    root
}

pub(super) fn audit_skins(
    skins: &[Value],
    node_count: usize,
    accessors: &[Option<AccessorInfo>],
    relative: &str,
    features: &mut LogicalModelFeatureCounts,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> Vec<Option<usize>> {
    let mut palettes = Vec::with_capacity(skins.len());
    for (skin_index, skin) in skins.iter().enumerate() {
        match skin.get("name").and_then(Value::as_str) {
            Some(name) => validate_true_name(name, "skin", relative, violations),
            None => push_violation(
                violations,
                "missing_skin_name",
                relative,
                format!("skin {skin_index} has no exact name"),
            ),
        }
        let joints = skin
            .get("joints")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut unique = BTreeSet::new();
        let valid_joints = !joints.is_empty()
            && joints.iter().all(|joint| {
                joint
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .is_some_and(|joint| joint < node_count && unique.insert(joint))
            });
        if !valid_joints {
            push_violation(
                violations,
                "invalid_skin_joints",
                relative,
                format!("skin {skin_index} has empty, duplicate or invalid joints"),
            );
        }
        if skin
            .get("skeleton")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .is_none_or(|root| root >= node_count)
        {
            push_violation(
                violations,
                "invalid_skeleton_root",
                relative,
                format!("skin {skin_index} has no valid skeleton root"),
            );
        }
        let inverse = skin
            .get("inverseBindMatrices")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|index| accessors.get(index))
            .and_then(Option::as_ref);
        match inverse {
            Some(accessor)
                if accessor.component_type == 5_126
                    && accessor.component_count == 16
                    && accessor.count == joints.len() => {}
            _ => push_violation(
                violations,
                "invalid_inverse_bind_matrices",
                relative,
                format!("skin {skin_index} must have one FLOAT MAT4 inverse bind per joint"),
            ),
        }
        features.joints += to_u64(joints.len());
        features.inverse_bind_matrices += inverse.map_or(0, |value| to_u64(value.count));
        palettes.push(valid_joints.then_some(joints.len()));
    }
    palettes
}

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_meshes(
    meshes: &[Value],
    nodes: &[Value],
    material_count: usize,
    skin_palettes: &[Option<usize>],
    accessors: &[Option<AccessorInfo>],
    binary: &[u8],
    relative: &str,
    features: &mut LogicalModelFeatureCounts,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let mut owners = vec![Vec::<usize>::new(); meshes.len()];
    for (node_index, node) in nodes.iter().enumerate() {
        if let Some(mesh) = node
            .get("mesh")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|mesh| *mesh < meshes.len())
        {
            owners[mesh].push(node_index);
            if node
                .get("skin")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .is_some_and(|skin| skin < skin_palettes.len())
            {
                features.skinned_meshes += 1;
            }
        }
    }

    let mut used_materials = BTreeSet::new();
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        if owners[mesh_index].is_empty() {
            push_violation(
                violations,
                "orphan_mesh",
                relative,
                format!("mesh {mesh_index} is not attached to the logical hierarchy"),
            );
        }
        match mesh.get("name").and_then(Value::as_str) {
            Some(name) => validate_true_name(name, "mesh", relative, violations),
            None => push_violation(
                violations,
                "missing_mesh_name",
                relative,
                format!("mesh {mesh_index} has no exact name"),
            ),
        }
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if primitives.is_empty() {
            push_violation(
                violations,
                "empty_mesh",
                relative,
                format!("mesh {mesh_index} has no primitives"),
            );
        }
        features.mesh_parts += to_u64(primitives.len());
        let mut weighted_counts = Vec::new();
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let context = format!("mesh {mesh_index} primitive {primitive_index}");
            if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                push_violation(
                    violations,
                    "non_triangle_primitive",
                    relative,
                    format!("{context} is not TRIANGLES"),
                );
            }
            match primitive.get("material").and_then(Value::as_u64) {
                Some(material)
                    if usize::try_from(material)
                        .ok()
                        .is_some_and(|m| m < material_count) =>
                {
                    used_materials.insert(material as usize);
                }
                Some(material) => push_violation(
                    violations,
                    "invalid_primitive_material",
                    relative,
                    format!("{context} references invalid material {material}"),
                ),
                None => {}
            }
            if primitive
                .pointer("/extras/materialSlot")
                .is_some_and(Value::is_string)
            {
                features.material_slots += 1;
            }
            let attributes = primitive.get("attributes").and_then(Value::as_object);
            let position = attributes
                .and_then(|values| values.get("POSITION"))
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| accessors.get(index))
                .and_then(Option::as_ref);
            let vertex_count = match position {
                Some(accessor)
                    if accessor.component_type == 5_126 && accessor.component_count == 3 =>
                {
                    accessor.count
                }
                _ => {
                    push_violation(
                        violations,
                        "invalid_positions",
                        relative,
                        format!("{context} has no valid FLOAT VEC3 POSITION accessor"),
                    );
                    0
                }
            };
            let indices = primitive
                .get("indices")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| accessors.get(index))
                .and_then(Option::as_ref);
            match indices {
                Some(accessor)
                    if accessor.component_count == 1
                        && matches!(accessor.component_type, 5_121 | 5_123 | 5_125)
                        && accessor.count % 3 == 0 =>
                {
                    if (0..accessor.count).any(|element| {
                        read_unsigned(*accessor, binary, element, 0)
                            .is_none_or(|index| index as usize >= vertex_count)
                    }) {
                        push_violation(
                            violations,
                            "index_out_of_bounds",
                            relative,
                            format!("{context} has an invalid vertex index"),
                        );
                    }
                }
                _ => push_violation(
                    violations,
                    "invalid_indices",
                    relative,
                    format!("{context} has no valid triangle index accessor"),
                ),
            }

            let joints = attribute_accessor(attributes, "JOINTS_0", accessors);
            let weights = attribute_accessor(attributes, "WEIGHTS_0", accessors);
            match (joints, weights) {
                (None, None) => {
                    if owners[mesh_index]
                        .iter()
                        .any(|node| nodes[*node].get("skin").is_some())
                    {
                        push_violation(
                            violations,
                            "missing_skin_attributes",
                            relative,
                            format!(
                                "{context} is owned by a skin node but has no JOINTS_0/WEIGHTS_0"
                            ),
                        );
                    }
                }
                (Some(joints), Some(weights)) => {
                    let valid_types = joints.component_count == 4
                        && matches!(joints.component_type, 5_121 | 5_123)
                        && !joints.normalized
                        && weights.component_count == 4
                        && (weights.component_type == 5_126
                            || (matches!(weights.component_type, 5_121 | 5_123)
                                && weights.normalized));
                    if !valid_types || joints.count != vertex_count || weights.count != vertex_count
                    {
                        push_violation(
                            violations,
                            "invalid_skin_attributes",
                            relative,
                            format!("{context} has incoherent JOINTS_0/WEIGHTS_0 accessors"),
                        );
                    } else {
                        let palettes = owners[mesh_index]
                            .iter()
                            .filter_map(|node| {
                                nodes[*node]
                                    .get("skin")
                                    .and_then(Value::as_u64)
                                    .and_then(|value| usize::try_from(value).ok())
                                    .and_then(|skin| skin_palettes.get(skin))
                                    .and_then(|palette| *palette)
                            })
                            .collect::<Vec<_>>();
                        if palettes.len() != owners[mesh_index].len() {
                            push_violation(
                                violations,
                                "weighted_rigid_attachment",
                                relative,
                                format!(
                                    "{context} has skin attributes without a valid owning skin"
                                ),
                            );
                        }
                        for vertex in 0..vertex_count {
                            let mut sum = 0.0;
                            let mut valid = true;
                            for component in 0..4 {
                                let joint = read_unsigned(*joints, binary, vertex, component);
                                let weight = read_weight(*weights, binary, vertex, component);
                                match (joint, weight) {
                                    (Some(joint), Some(weight))
                                        if weight.is_finite()
                                            && weight >= 0.0
                                            && palettes
                                                .iter()
                                                .all(|palette| (joint as usize) < *palette) =>
                                    {
                                        sum += weight;
                                    }
                                    _ => valid = false,
                                }
                            }
                            if !valid || (sum - 1.0).abs() > 1.0e-4 {
                                push_violation(
                                    violations,
                                    "invalid_vertex_weights",
                                    relative,
                                    format!("{context} vertex {vertex} has invalid joints/weights"),
                                );
                                break;
                            }
                        }
                    }
                    weighted_counts.push(joints.count);
                }
                _ => push_violation(
                    violations,
                    "partial_skin_attributes",
                    relative,
                    format!("{context} must have both JOINTS_0 and WEIGHTS_0 or neither"),
                ),
            }
        }
        if owners[mesh_index]
            .iter()
            .any(|node| nodes[*node].get("skin").is_some())
        {
            if weighted_counts.is_empty()
                || weighted_counts
                    .iter()
                    .any(|count| *count != weighted_counts[0])
            {
                push_violation(
                    violations,
                    "skinned_submesh_cardinality_mismatch",
                    relative,
                    format!(
                        "skinned mesh {mesh_index} primitives disagree on source vertex cardinality"
                    ),
                );
            } else {
                let skinned_owners = owners[mesh_index]
                    .iter()
                    .filter(|node| nodes[**node].get("skin").is_some())
                    .count();
                features.weighted_vertices +=
                    to_u64(weighted_counts[0].saturating_mul(skinned_owners));
            }
        }
    }
    if used_materials.len() != material_count {
        push_violation(
            violations,
            "orphan_material",
            relative,
            format!(
                "{} of {material_count} materials are referenced by primitives",
                used_materials.len()
            ),
        );
    }
}
