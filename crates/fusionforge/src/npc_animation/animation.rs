use super::*;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NpcAnimationError {
    #[error("animation clip '{0}' was not found in the NPC preview")]
    ClipNotFound(String),
    #[error("animation clip '{0}' has metadata but no sampleable TRS tracks")]
    MissingTrackData(String),
    #[error("NPC preview animation data is invalid: {0}")]
    InvalidPreview(String),
}

/// A sampled pose for animation inspection and native export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampledNpcPose {
    pub clip_name: String,
    pub time_seconds: f64,
    pub duration_seconds: f64,
    pub looped: bool,
    pub joints: Vec<SampledJointTransform>,
    pub meshes: Vec<SampledMeshPositions>,
    pub warnings: Vec<String>,
}

/// Parsed, reusable animation state for an NPC preview.
///
/// The accepted additive preview schema is intentionally small and JSON-only:
///
/// ```text
/// skeleton.joints[] = {
///   path, parent, translation|position, rotation, scale,
///   inverseBindMatrix? // row-major 4x4
/// }
/// animations[].animationData = {
///   translations|rotations|scales: [{ path, keys: [{ time, value }] }]
/// }
/// meshes[].skin = {
///   jointPaths, boneIndices|joints, weights,
///   inverseBindMatrices|bindPoses? // row-major 4x4 matrices
/// }
/// ```
///
/// Vector/quaternion values can be arrays or `{x,y,z[,w]}` objects. Weight and
/// bone-index streams can be flat (four values per vertex) or arrays of rows.
/// When bind matrices are omitted they are derived from the skeleton rest pose,
/// which guarantees that time zero does not move an otherwise unanimated mesh.
#[derive(Debug, Clone)]
pub struct NpcAnimationSampler {
    pub(super) clips: Vec<AnimationClip>,
    pub(super) joints: Vec<Joint>,
    pub(super) meshes: Vec<PreviewMesh>,
}

#[derive(Debug, Clone)]
pub(super) struct AnimationClip {
    pub(super) name: String,
    pub(super) path_id: Option<i64>,
    pub(super) duration: f64,
    pub(super) looped: bool,
    pub(super) tracks: BTreeMap<String, TransformTrack>,
}

impl NpcAnimationSampler {
    /// Parses the preview once. Call [`Self::sample`] for each animation frame.
    pub fn from_preview(preview: &JsonValue) -> Result<Self, NpcAnimationError> {
        let clips = parse_clips(preview)?;
        let mut joints = parse_skeleton(preview)?;
        add_missing_track_joints(&mut joints, &clips);
        resolve_joint_parents(&mut joints)?;

        let joint_lookup = JointLookup::new(&joints);
        let rest_locals = joints
            .iter()
            .map(|joint| {
                compose_matrix(
                    joint.rest_translation,
                    joint.rest_rotation,
                    joint.rest_scale,
                )
            })
            .collect::<Vec<_>>();
        let rest_globals = global_matrices(&joints, &rest_locals)?;
        let meshes = parse_meshes(preview, &joints, &joint_lookup, &rest_globals);

        Ok(Self {
            clips,
            joints,
            meshes,
        })
    }

    pub fn clip_names(&self) -> Vec<&str> {
        self.clips.iter().map(|clip| clip.name.as_str()).collect()
    }

    pub fn has_sampleable_clip(&self, name: &str) -> bool {
        self.find_clip(name)
            .is_some_and(|clip| !clip.tracks.is_empty())
    }

    pub fn clip_duration_seconds(&self, name: &str) -> Option<f64> {
        self.find_clip(name).map(|clip| clip.duration)
    }

    pub fn has_skinned_meshes(&self) -> bool {
        self.meshes.iter().any(|mesh| mesh.skin.is_some())
    }

    /// True only when every rendered mesh can follow the sampled skeleton.
    /// Mixed Unity/NIF previews often contain unskinned eyes or accessories;
    /// animating only the body tears those pieces away from the character.
    pub fn has_complete_skinning(&self) -> bool {
        !self.meshes.is_empty() && self.meshes.iter().all(|mesh| mesh.skin.is_some())
    }

    pub fn sample(
        &self,
        clip_name: &str,
        time_seconds: f64,
    ) -> Result<SampledNpcPose, NpcAnimationError> {
        let clip = self
            .find_clip(clip_name)
            .ok_or_else(|| NpcAnimationError::ClipNotFound(clip_name.to_string()))?;
        if clip.tracks.is_empty() {
            return Err(NpcAnimationError::MissingTrackData(clip.name.clone()));
        }

        let sampled_time = normalized_sample_time(time_seconds, clip.duration, clip.looped);
        let mut local_matrices = Vec::with_capacity(self.joints.len());
        for joint in &self.joints {
            let track = find_track(&clip.tracks, &joint.path);
            let translation = track
                .and_then(|track| sample_vec3(&track.translations, sampled_time))
                .unwrap_or(joint.rest_translation);
            let rotation = track
                .and_then(|track| sample_quat(&track.rotations, sampled_time))
                .unwrap_or(joint.rest_rotation);
            let scale = track
                .and_then(|track| sample_vec3(&track.scales, sampled_time))
                .unwrap_or(joint.rest_scale);
            local_matrices.push(compose_matrix(translation, rotation, scale));
        }
        let globals = global_matrices(&self.joints, &local_matrices)?;

        let mut warnings = Vec::new();
        let meshes = self
            .meshes
            .iter()
            .map(|mesh| {
                if let Some(warning) = &mesh.skin_warning {
                    warnings.push(format!("{}: {warning}", mesh.id));
                }
                let (positions, skinned) = match &mesh.skin {
                    Some(skin) => (skin_positions(mesh, skin, &globals), true),
                    None => (
                        mesh.positions
                            .iter()
                            .map(|value| [value[0] as f32, value[1] as f32, value[2] as f32])
                            .collect(),
                        false,
                    ),
                };
                SampledMeshPositions {
                    id: mesh.id.clone(),
                    positions,
                    skinned,
                }
            })
            .collect();

        Ok(SampledNpcPose {
            clip_name: clip.name.clone(),
            time_seconds: sampled_time,
            duration_seconds: clip.duration,
            looped: clip.looped,
            joints: self
                .joints
                .iter()
                .zip(local_matrices)
                .zip(globals)
                .map(
                    |((joint, local_matrix), global_matrix)| SampledJointTransform {
                        path: joint.path.clone(),
                        parent_index: joint.parent,
                        local_matrix,
                        global_matrix,
                    },
                )
                .collect(),
            meshes,
            warnings,
        })
    }

    pub(super) fn find_clip(&self, name: &str) -> Option<&AnimationClip> {
        self.clips
            .iter()
            .find(|clip| clip.name == name)
            .or_else(|| {
                self.clips
                    .iter()
                    .find(|clip| clip.name.eq_ignore_ascii_case(name))
            })
            .or_else(|| {
                name.parse::<i64>().ok().and_then(|path_id| {
                    self.clips.iter().find(|clip| clip.path_id == Some(path_id))
                })
            })
    }
}

/// Convenience API for one-off callers. Interactive playback should cache a
/// [`NpcAnimationSampler`] instead of reparsing the preview every frame.
pub fn sample_preview_animation(
    preview: &JsonValue,
    clip_name: &str,
    time_seconds: f64,
) -> Result<SampledNpcPose, NpcAnimationError> {
    NpcAnimationSampler::from_preview(preview)?.sample(clip_name, time_seconds)
}

pub(super) fn parse_clip(value: &JsonValue, index: usize) -> Result<AnimationClip, NpcAnimationError> {
    let name = value
        .get("name")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .get("pathId")
                .and_then(JsonValue::as_i64)
                .map(|id| id.to_string())
        })
        .unwrap_or_else(|| format!("animation_{index}"));
    let data = value.get("animationData").unwrap_or(value);
    let mut tracks = BTreeMap::<String, TransformTrack>::new();

    parse_split_vec3_curves(data, "translations", &mut tracks, CurveKind::Translation)?;
    parse_split_quat_curves(data, "rotations", &mut tracks)?;
    parse_split_vec3_curves(data, "scales", &mut tracks, CurveKind::Scale)?;

    if let Some(unified_tracks) = data.get("tracks").and_then(JsonValue::as_array) {
        for track in unified_tracks {
            let Some(path) = track.get("path").and_then(JsonValue::as_str) else {
                continue;
            };
            let path = normalized_path(path);
            if path.is_empty() {
                continue;
            }
            let target = tracks.entry(path).or_default();
            if let Some(keys) = first_array(
                track,
                &[
                    "translationKeys",
                    "translation",
                    "translations",
                    "positionKeys",
                ],
            ) {
                target.translations = parse_vec3_keys(keys)?;
            }
            if let Some(keys) = first_array(track, &["rotationKeys", "rotation", "rotations"]) {
                target.rotations = parse_quat_keys(keys)?;
            }
            if let Some(keys) = first_array(track, &["scaleKeys", "scale", "scales"]) {
                target.scales = parse_vec3_keys(keys)?;
            }
        }
    }

    tracks.retain(|_, track| {
        !(track.translations.is_empty() && track.rotations.is_empty() && track.scales.is_empty())
    });
    let keyed_duration = tracks
        .values()
        .flat_map(|track| {
            [
                track.translations.last().map(|key| key.time),
                track.rotations.last().map(|key| key.time),
                track.scales.last().map(|key| key.time),
            ]
        })
        .flatten()
        .fold(0.0_f64, f64::max);
    let duration = value
        .get("duration")
        .and_then(JsonValue::as_f64)
        .filter(|duration| duration.is_finite() && *duration >= 0.0)
        .unwrap_or(keyed_duration)
        .max(keyed_duration);
    let looped = value
        .get("loop")
        .and_then(JsonValue::as_bool)
        .or_else(|| value.get("looped").and_then(JsonValue::as_bool))
        .or_else(|| {
            value
                .get("wrapMode")
                .and_then(JsonValue::as_i64)
                .map(|mode| mode == 2 || mode == 4)
        })
        .unwrap_or(true);

    Ok(AnimationClip {
        name,
        path_id: value.get("pathId").and_then(JsonValue::as_i64),
        duration,
        looped,
        tracks,
    })
}

pub(super) fn parse_skeleton(preview: &JsonValue) -> Result<Vec<Joint>, NpcAnimationError> {
    let values = preview
        .pointer("/skeleton/joints")
        .or_else(|| preview.get("joints"))
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let mut joints = Vec::new();
    let mut pending_parents = Vec::<Option<ParentRef>>::new();
    let mut seen = BTreeSet::new();

    for (index, value) in values.iter().enumerate() {
        let path = value
            .get("path")
            .or_else(|| value.get("name"))
            .and_then(JsonValue::as_str)
            .map(normalized_path)
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| format!("joint_{index}"));
        let key = path.to_ascii_lowercase();
        if !seen.insert(key) {
            return Err(NpcAnimationError::InvalidPreview(format!(
                "duplicate skeleton joint path '{path}'"
            )));
        }
        let rest = value.get("rest").unwrap_or(value);
        let rest_translation = rest
            .get("translation")
            .or_else(|| rest.get("position"))
            .and_then(parse_vec3)
            .unwrap_or([0.0, 0.0, 0.0]);
        let rest_rotation = rest
            .get("rotation")
            .and_then(parse_quat)
            .map(normalize_quat)
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let rest_scale = rest
            .get("scale")
            .and_then(parse_vec3)
            .unwrap_or([1.0, 1.0, 1.0]);
        let inverse_bind = value
            .get("inverseBindMatrix")
            .or_else(|| value.get("bindPose"))
            .and_then(parse_matrix4);
        pending_parents.push(parse_parent_ref(value.get("parent")));
        joints.push(Joint {
            path,
            parent: None,
            rest_translation,
            rest_rotation,
            rest_scale,
            inverse_bind,
        });
    }

    let lookup = joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.path.to_ascii_lowercase(), index))
        .collect::<HashMap<_, _>>();
    for (index, parent_ref) in pending_parents.into_iter().enumerate() {
        joints[index].parent = match parent_ref {
            Some(ParentRef::Index(parent)) => Some(parent),
            Some(ParentRef::Path(path)) => lookup.get(&path.to_ascii_lowercase()).copied(),
            None => None,
        };
        if joints[index]
            .parent
            .is_some_and(|parent| parent >= joints.len())
        {
            return Err(NpcAnimationError::InvalidPreview(format!(
                "joint '{}' references an out-of-range parent",
                joints[index].path
            )));
        }
    }
    Ok(joints)
}
