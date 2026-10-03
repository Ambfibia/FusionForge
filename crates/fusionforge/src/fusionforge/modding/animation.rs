use super::*;

#[derive(Debug, Clone)]
pub struct ImportedBoneWeight {
    pub weights: [f64; 4],
    pub bone_indices: [i64; 4],
}

#[derive(Debug, Clone)]
pub struct ImportedAnimationClip {
    pub name: String,
    pub sample_rate: f64,
    pub duration: f64,
    pub translations: Vec<ImportedVec3Curve>,
    pub rotations: Vec<ImportedQuatCurve>,
    pub scales: Vec<ImportedVec3Curve>,
}

#[derive(Debug, Clone, Default)]
pub struct ImportedSkeleton {
    /// Nodes that participate in a skin or are targeted by an animation,
    /// including their ancestors. Paths use the same convention as imported
    /// animation curves.
    pub joints: Vec<ImportedJoint>,
    /// First-skin palette in the exact order used by `JOINTS_0` and inverse
    /// bind matrices in [`ImportedMesh`].
    pub skin_joint_paths: Vec<String>,
}

pub(super) fn gltf_bind_pose_to_fusionfall(matrix: [[f32; 4]; 4]) -> ImportedMatrix4x4 {
    let mut gltf = [[0.0_f64; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            gltf[row][col] = f64::from(matrix[col][row]);
        }
    }

    let basis = [
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let converted = multiply_matrix4(multiply_matrix4(basis, gltf), basis);
    ImportedMatrix4x4 {
        values: [
            converted[0][0],
            converted[0][1],
            converted[0][2],
            converted[0][3],
            converted[1][0],
            converted[1][1],
            converted[1][2],
            converted[1][3],
            converted[2][0],
            converted[2][1],
            converted[2][2],
            converted[2][3],
            converted[3][0],
            converted[3][1],
            converted[3][2],
            converted[3][3],
        ],
    }
}

pub(super) fn normalized_bone_name(value: &str) -> String {
    let mut result = String::new();
    let mut previous_underscore = false;
    for ch in value.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '_' {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if mapped == '_' {
            if previous_underscore {
                continue;
            }
            previous_underscore = true;
        } else {
            previous_underscore = false;
        }
        result.push(mapped);
    }
    result.trim_matches('_').to_string()
}

pub(super) fn fallback_bone_name(value: &str) -> Option<String> {
    let without_ik = value
        .split_once("_iktarget")
        .map(|(base, _)| base)
        .unwrap_or(value);
    let without_numeric = without_ik
        .rsplit_once('_')
        .filter(|(_, suffix)| suffix.chars().all(|ch| ch.is_ascii_digit()))
        .map(|(base, _)| base)
        .unwrap_or(without_ik);
    if without_numeric != value {
        Some(without_numeric.to_string())
    } else {
        None
    }
}

impl ImportedSkeleton {
    pub fn from_gltf_path(path: &Path) -> Result<Self, String> {
        let (document, _, _) =
            gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
        Ok(Self::from_gltf_document(&document))
    }

    pub(super) fn from_gltf_document(document: &gltf::Document) -> Self {
        let node_paths = gltf_node_paths(document);
        let mut parent_by_child = BTreeMap::<usize, usize>::new();
        for node in document.nodes() {
            for child in node.children() {
                parent_by_child.insert(child.index(), node.index());
            }
        }

        let skin_joint_indices = document
            .skins()
            .next()
            .map(|skin| skin.joints().map(|joint| joint.index()).collect::<Vec<_>>())
            .unwrap_or_default();
        let mut relevant = skin_joint_indices.iter().copied().collect::<BTreeSet<_>>();
        for animation in document.animations() {
            for channel in animation.channels() {
                relevant.insert(channel.target().node().index());
            }
        }
        let mut pending = relevant.iter().copied().collect::<Vec<_>>();
        while let Some(node_index) = pending.pop() {
            let Some(parent_index) = parent_by_child.get(&node_index).copied() else {
                continue;
            };
            if relevant.insert(parent_index) {
                pending.push(parent_index);
            }
        }

        let joints = document
            .nodes()
            .filter(|node| relevant.contains(&node.index()))
            .map(|node| {
                let node_index = node.index();
                let (translation, rotation, scale) = node.transform().decomposed();
                ImportedJoint {
                    node_index,
                    path: node_paths
                        .get(&node_index)
                        .cloned()
                        .unwrap_or_else(|| format!("node_{node_index}")),
                    parent_path: parent_by_child
                        .get(&node_index)
                        .and_then(|parent| node_paths.get(parent))
                        .cloned(),
                    translation: gltf_vec3_to_fusionfall(
                        translation[0],
                        translation[1],
                        translation[2],
                    ),
                    rotation: gltf_quat_to_fusionfall(
                        rotation[0],
                        rotation[1],
                        rotation[2],
                        rotation[3],
                    ),
                    scale: gltf_scale_to_fusionfall(scale[0], scale[1], scale[2]),
                }
            })
            .collect();
        let skin_joint_paths = skin_joint_indices
            .into_iter()
            .filter_map(|index| node_paths.get(&index).cloned())
            .collect();
        Self {
            joints,
            skin_joint_paths,
        }
    }
}

impl ImportedAnimationClip {
    pub fn from_gltf_path(path: &Path, sample_rate: f64) -> Result<Vec<Self>, String> {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "glb" | "gltf" => {}
            _ => return Ok(Vec::new()),
        }
        let (document, buffers, _) =
            gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
        Self::from_gltf_document(path, sample_rate, &document, &buffers)
    }

    pub(super) fn from_gltf_document(
        path: &Path,
        sample_rate: f64,
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Vec<Self>, String> {
        let node_paths = gltf_node_paths(document);
        let mut clips = Vec::new();

        for (animation_index, animation) in document.animations().enumerate() {
            let mut clip = ImportedAnimationClip {
                name: animation
                    .name()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("animation_{animation_index}")),
                sample_rate,
                duration: 0.0,
                translations: Vec::new(),
                rotations: Vec::new(),
                scales: Vec::new(),
            };

            for channel in animation.channels() {
                let target = channel.target();
                let node_index = target.node().index();
                let node_path = node_paths.get(&node_index).cloned().unwrap_or_else(|| {
                    target
                        .node()
                        .name()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("node_{node_index}"))
                });
                if node_path.trim().is_empty() {
                    continue;
                }
                let reader = channel
                    .reader(|buffer| buffers.get(buffer.index()).map(|data| data.0.as_slice()));
                let times = reader
                    .read_inputs()
                    .ok_or_else(|| {
                        format!(
                            "{}: GLTF animation '{}' channel for '{}' has no input times",
                            path.display(),
                            clip.name,
                            node_path
                        )
                    })?
                    .map(f64::from)
                    .collect::<Vec<_>>();
                if times.is_empty() {
                    continue;
                }
                if let Some(last) = times.last().copied() {
                    clip.duration = clip.duration.max(last);
                }

                match reader.read_outputs().ok_or_else(|| {
                    format!(
                        "{}: GLTF animation '{}' channel for '{}' has no output values",
                        path.display(),
                        clip.name,
                        node_path
                    )
                })? {
                    gltf::animation::util::ReadOutputs::Translations(values) => {
                        let values = values.collect::<Vec<_>>();
                        if values.len() != times.len() {
                            return Err(format!(
                                "{}: GLTF animation '{}' translation channel length mismatch",
                                path.display(),
                                clip.name
                            ));
                        }
                        clip.translations.push(ImportedVec3Curve {
                            path: node_path,
                            keys: times
                                .iter()
                                .copied()
                                .zip(values)
                                .map(|(time, [x, y, z])| ImportedVec3Key {
                                    time,
                                    value: gltf_vec3_to_fusionfall(x, y, z),
                                })
                                .collect(),
                        });
                    }
                    gltf::animation::util::ReadOutputs::Rotations(values) => {
                        let values = values.into_f32().collect::<Vec<_>>();
                        if values.len() != times.len() {
                            return Err(format!(
                                "{}: GLTF animation '{}' rotation channel length mismatch",
                                path.display(),
                                clip.name
                            ));
                        }
                        clip.rotations.push(ImportedQuatCurve {
                            path: node_path,
                            keys: times
                                .iter()
                                .copied()
                                .zip(values)
                                .map(|(time, [x, y, z, w])| ImportedQuatKey {
                                    time,
                                    value: gltf_quat_to_fusionfall(x, y, z, w),
                                })
                                .collect(),
                        });
                    }
                    gltf::animation::util::ReadOutputs::Scales(values) => {
                        let values = values.collect::<Vec<_>>();
                        if values.len() != times.len() {
                            return Err(format!(
                                "{}: GLTF animation '{}' scale channel length mismatch",
                                path.display(),
                                clip.name
                            ));
                        }
                        clip.scales.push(ImportedVec3Curve {
                            path: node_path,
                            keys: times
                                .iter()
                                .copied()
                                .zip(values)
                                .map(|(time, [x, y, z])| ImportedVec3Key {
                                    time,
                                    value: gltf_scale_to_fusionfall(x, y, z),
                                })
                                .collect(),
                        });
                    }
                    gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => {}
                }
            }

            if !clip.translations.is_empty()
                || !clip.rotations.is_empty()
                || !clip.scales.is_empty()
            {
                clips.push(clip);
            }
        }

        Ok(clips)
    }
}

pub fn gltf_animation_names(path: &Path) -> Result<Vec<String>, String> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "glb" | "gltf" => {}
        _ => return Ok(Vec::new()),
    }
    let (document, _, _) =
        gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(document
        .animations()
        .enumerate()
        .map(|(index, animation)| {
            animation
                .name()
                .map(str::to_string)
                .unwrap_or_else(|| format!("animation_{index}"))
        })
        .collect())
}

pub fn apply_animation_clip_import(
    target: &mut UnityValue,
    clip: ImportedAnimationClip,
) -> Result<(), String> {
    let object = target
        .as_object_mut()
        .ok_or_else(|| "target animation clip is not an object".to_string())?;
    object.insert("m_Name".to_string(), UnityValue::String(clip.name));
    object.insert(
        "m_SampleRate".to_string(),
        UnityValue::Float(clip.sample_rate),
    );
    // Write rotations as plain (uncompressed) quaternion curves. The legacy
    // compressed-rotation packing in this writer does not match the Unity 2.x
    // runtime bit layout (largest-component index lives in the LOW 2 bits and
    // the stored components fill the high bits), so compressed clips decode
    // into garbage rotations at runtime and visually destroy the skinned
    // mesh. Plain float curves have no packing convention to get wrong.
    object.insert("m_UseCompression".to_string(), UnityValue::Bool(false));
    object.insert(
        "m_CompressedRotationCurves".to_string(),
        UnityValue::Array(Vec::new()),
    );
    object.insert(
        "m_PositionCurves".to_string(),
        UnityValue::Array(
            clip.translations
                .iter()
                .map(vec3_curve_value)
                .collect::<Vec<_>>(),
        ),
    );
    object.insert(
        "m_RotationCurves".to_string(),
        UnityValue::Array(
            clip.rotations
                .iter()
                .map(quat_curve_value)
                .collect::<Vec<_>>(),
        ),
    );
    object.insert(
        "m_ScaleCurves".to_string(),
        UnityValue::Array(
            clip.scales
                .iter()
                .filter(|curve| should_keep_scale_curve(curve))
                .map(vec3_curve_value)
                .collect::<Vec<_>>(),
        ),
    );
    object.insert("m_FloatCurves".to_string(), UnityValue::Array(Vec::new()));
    object.insert("m_Events".to_string(), UnityValue::Array(Vec::new()));
    Ok(())
}

pub(super) fn compressed_bone_index_bits(mesh: &ImportedMesh) -> u32 {
    let max_index = mesh
        .skin
        .iter()
        .flat_map(|skin| skin.bone_indices)
        .max()
        .unwrap_or(0)
        .max(0) as u32;
    bit_size_for(
        max_index
            .max(mesh.bind_poses.len().saturating_sub(1) as u32)
            .max(1),
    )
}

pub(super) fn bone_weight_value(value: &ImportedBoneWeight) -> UnityValue {
    UnityValue::Object(BTreeMap::from([
        ("weight[0]".to_string(), UnityValue::Float(value.weights[0])),
        ("weight[1]".to_string(), UnityValue::Float(value.weights[1])),
        ("weight[2]".to_string(), UnityValue::Float(value.weights[2])),
        ("weight[3]".to_string(), UnityValue::Float(value.weights[3])),
        (
            "boneIndex[0]".to_string(),
            UnityValue::Int(value.bone_indices[0]),
        ),
        (
            "boneIndex[1]".to_string(),
            UnityValue::Int(value.bone_indices[1]),
        ),
        (
            "boneIndex[2]".to_string(),
            UnityValue::Int(value.bone_indices[2]),
        ),
        (
            "boneIndex[3]".to_string(),
            UnityValue::Int(value.bone_indices[3]),
        ),
    ]))
}
