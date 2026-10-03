use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportAnimationClip {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) fields: JsonMap<String, JsonValue>,
}

pub(super) fn collect_animation_target_paths(clip: &JsonValue, output: &mut BTreeSet<String>) -> Result<()> {
    for field in ["channels", "floatCurves"] {
        for curve in clip
            .get(field)
            .and_then(JsonValue::as_array)
            .ok_or_else(|| invalid_error(format!("decoded AnimationClip has no {field} array")))?
        {
            let path = curve
                .get("targetPath")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| invalid_error("decoded animation curve has no targetPath"))?;
            output.insert(path.trim_matches('/').to_owned());
        }
    }
    Ok(())
}

/// Build the minimum exact hierarchy needed to evaluate one legacy Animation.
/// Static-world mesh vertices are world-baked, so runtime pivots keep their
/// authored world/local transforms and each directly owned model is attached
/// to its nearest pivot with the inverse authored world matrix.
pub(super) fn build_animation_targets(
    owner: &str,
    target_paths: &BTreeSet<String>,
    additional_pivot_paths: &BTreeSet<String>,
    expected_models: &[String],
    hierarchy: &HierarchyIndex,
) -> Result<Vec<JsonValue>> {
    if !hierarchy.nodes.contains_key(owner) {
        return invalid(format!(
            "animation owner node {owner:?} is absent from the hierarchy"
        ));
    }
    let mut node_by_path = BTreeMap::<String, String>::from([(String::new(), owner.to_owned())]);
    let mut path_by_node = BTreeMap::<String, String>::from([(owner.to_owned(), String::new())]);

    for target_path in target_paths.iter().chain(additional_pivot_paths.iter()) {
        let normalized = target_path.trim_matches('/');
        let mut current = owner.to_owned();
        let mut partial = String::new();
        if normalized.is_empty() {
            continue;
        }
        for segment in normalized.split('/') {
            let matches = hierarchy
                .children_by_node
                .get(&current)
                .into_iter()
                .flatten()
                .filter(|child| {
                    hierarchy
                        .nodes
                        .get(child.as_str())
                        .is_some_and(|node| node.name == segment)
                })
                .cloned()
                .collect::<Vec<_>>();
            let [child] = matches.as_slice() else {
                return invalid(format!(
                    "animation owner {owner:?} path {target_path:?} segment {segment:?} resolves to {} hierarchy nodes",
                    matches.len()
                ));
            };
            partial = if partial.is_empty() {
                segment.to_owned()
            } else {
                format!("{partial}/{segment}")
            };
            if let Some(previous) = node_by_path.insert(partial.clone(), child.clone()) {
                if previous != *child {
                    return invalid(format!(
                        "animation path {partial:?} resolves inconsistently"
                    ));
                }
            }
            path_by_node.insert(child.clone(), partial.clone());
            current = child.clone();
        }
    }

    let expected = expected_models.iter().cloned().collect::<BTreeSet<_>>();
    let mut models_by_path = BTreeMap::<String, Vec<String>>::new();
    let mut assigned = BTreeSet::new();
    for (node_id, models) in &hierarchy.direct_model_ids_by_node {
        let relevant = models
            .iter()
            .filter(|model| expected.contains(*model))
            .cloned()
            .collect::<Vec<_>>();
        if relevant.is_empty() {
            continue;
        }
        let mut current = Some(node_id.as_str());
        let mut visited = BTreeSet::new();
        let mut owner_path = None;
        while let Some(node_id) = current {
            if !visited.insert(node_id.to_owned()) {
                return invalid(format!(
                    "hierarchy cycle while assigning animation {owner:?}"
                ));
            }
            if let Some(path) = path_by_node.get(node_id) {
                owner_path = Some(path.clone());
                break;
            }
            current = hierarchy
                .nodes
                .get(node_id)
                .and_then(|node| node.parent.as_deref());
        }
        let path = owner_path.ok_or_else(|| {
            invalid_error(format!(
                "animation model {:?} is outside owner {owner:?}",
                relevant[0]
            ))
        })?;
        models_by_path
            .entry(path)
            .or_default()
            .extend(relevant.iter().cloned());
        assigned.extend(relevant);
    }
    if assigned != expected {
        let missing = expected
            .difference(&assigned)
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        return invalid(format!(
            "animation owner {owner:?} assigned {}/{} published models; missing {missing:?}",
            assigned.len(),
            expected.len()
        ));
    }

    let mut ordered_paths = node_by_path.keys().cloned().collect::<Vec<_>>();
    ordered_paths.sort_by(|left, right| {
        left.matches('/')
            .count()
            .cmp(&right.matches('/').count())
            .then_with(|| left.len().cmp(&right.len()))
            .then_with(|| left.cmp(right))
    });
    let mut targets = Vec::with_capacity(ordered_paths.len());
    for path in ordered_paths {
        let node_id = node_by_path
            .get(&path)
            .expect("ordered path comes from node map");
        let node = hierarchy.nodes.get(node_id).ok_or_else(|| {
            invalid_error(format!("animation target node {node_id:?} disappeared"))
        })?;
        let parent_path = if path.is_empty() {
            None
        } else {
            node.parent
                .as_deref()
                .and_then(|parent| path_by_node.get(parent))
                .cloned()
        };
        if !path.is_empty() && parent_path.is_none() {
            return invalid(format!(
                "animation target path {path:?} has no published pivot parent"
            ));
        }
        let mut models = models_by_path.remove(&path).unwrap_or_default();
        models.sort();
        models.dedup();
        let root_parent_world_matrix = path.is_empty().then(|| {
            node.parent
                .as_deref()
                .and_then(|parent| hierarchy.nodes.get(parent))
                .map(|parent| parent.world_matrix)
                .unwrap_or([
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ])
        });
        targets.push(serde_json::json!({
            "path": path,
            "node": node_id,
            "parentPath": parent_path,
            "animated": target_paths.contains(&path),
            "baseLocalTrs": {
                "translation": node.local_translation,
                "rotation": node.local_rotation,
                "scale": node.local_scale,
            },
            "baseWorldMatrix": node.world_matrix,
            "rootParentWorldMatrix": root_parent_world_matrix,
            "models": models,
        }));
    }
    Ok(targets)
}

pub(super) fn animation_curve_path(binding: &JsonValue, clip_id: &str, label: &str) -> Result<String> {
    binding
        .get("path")
        .or_else(|| binding.get("m_Path"))
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid_error(format!("AnimationClip {clip_id} {label} curve has no path")))
}

pub(super) fn native_animation_quaternion(
    value: [f64; 4],
    require_unit: bool,
    clip_id: &str,
) -> Result<[f64; 4]> {
    if require_unit {
        let norm = value
            .iter()
            .map(|component| component * component)
            .sum::<f64>()
            .sqrt();
        if !norm.is_finite() || (norm - 1.0).abs() > 1.0e-4 {
            return invalid(format!(
                "AnimationClip {clip_id} quaternion is not unit length: {norm}"
            ));
        }
    }
    Ok([value[0], -value[1], -value[2], value[3]])
}

pub(super) fn read_packed_animation_bits(value: &JsonValue, clip_id: &str) -> Result<Vec<u32>> {
    let count = usize_json(value.get("m_NumItems"), "packed bit count")?;
    let bit_size = u32::try_from(usize_json(value.get("m_BitSize"), "packed bit size")?)
        .map_err(|_| invalid_error("packed bit size exceeds u32"))?;
    if count == 0 || bit_size == 0 || bit_size > 32 {
        return invalid(format!(
            "AnimationClip {clip_id} packed bit vector is invalid"
        ));
    }
    let data = animation_byte_payload(value.get("m_Data"), clip_id)?;
    let mut reader = AnimationBitReader::new(&data, bit_size);
    Ok((0..count).map(|_| reader.read()).collect())
}

pub(super) fn read_packed_animation_floats(value: &JsonValue, clip_id: &str) -> Result<Vec<f64>> {
    let bit_size = u32::try_from(usize_json(value.get("m_BitSize"), "packed float bit size")?)
        .map_err(|_| invalid_error("packed float bit size exceeds u32"))?;
    let values = read_packed_animation_bits(value, clip_id)?;
    let maximum = if bit_size == 32 {
        u32::MAX as f64
    } else {
        ((1_u64 << bit_size) - 1) as f64
    };
    let range = finite_json(value.get("m_Range"), "packed float range")? / maximum.max(1.0);
    let start = finite_json(value.get("m_Start"), "packed float start")?;
    Ok(values
        .into_iter()
        .map(|value| f64::from(value) * range + start)
        .collect())
}

pub(super) struct AnimationBitReader<'a> {
    pub(super) data: &'a [u8],
    pub(super) bit_size: u32,
    pub(super) bit_count: u32,
    pub(super) index: usize,
    pub(super) byte: u64,
}

impl<'a> AnimationBitReader<'a> {
    pub(super) fn new(data: &'a [u8], bit_size: u32) -> Self {
        let mut reader = Self {
            data,
            bit_size,
            bit_count: 8,
            index: 0,
            byte: 0,
        };
        reader.byte = u64::from(reader.next_byte());
        reader
    }

    pub(super) fn next_byte(&mut self) -> u8 {
        let value = self.data.get(self.index).copied().unwrap_or(0);
        self.index += usize::from(self.index < self.data.len());
        value
    }

    pub(super) fn read(&mut self) -> u32 {
        while self.bit_count < self.bit_size {
            self.byte |= u64::from(self.next_byte()) << self.bit_count;
            self.bit_count += 8;
        }
        let mask = if self.bit_size == 32 {
            u64::from(u32::MAX)
        } else {
            (1_u64 << self.bit_size) - 1
        };
        let value = self.byte & mask;
        self.byte >>= self.bit_size;
        self.bit_count -= self.bit_size;
        value as u32
    }
}

pub(super) fn unpack_legacy_animation_quaternion(packed: u32) -> [f64; 4] {
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
    let norm = quaternion
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= f64::EPSILON {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        quaternion.map(|component| component / norm)
    }
}
