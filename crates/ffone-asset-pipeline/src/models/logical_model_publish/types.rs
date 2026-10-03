use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceGeometryFilterReport {
    pub policy: String,
    pub reason: String,
    pub excluded_rigid_mesh_bindings: u64,
    pub excluded_mesh_parts: u64,
    pub excluded_source_mesh_ids: Vec<String>,
    pub preserved_transform_nodes: u64,
    pub preserved_skinned_mesh_bindings: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinateBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceDocument {
    pub(super) schema: String,
    pub(super) selection_mode: String,
    pub(super) status: String,
    pub(super) logical_name: String,
    pub(super) exact_container_route: String,
    pub(super) container_paths: Vec<String>,
    pub(super) matched_paths: Vec<String>,
    pub(super) native_coordinate_contract: SourceNativeCoordinateContract,
    pub(super) model_hierarchy: SourceHierarchy,
    pub(super) skeleton: SourceSkeleton,
    pub(super) meshes: Vec<SourceMesh>,
    #[serde(default)]
    pub(super) materials: BTreeMap<String, SourceMaterial>,
    #[serde(default)]
    pub(super) renderer_material_bindings: Vec<SourceRendererMaterialBinding>,
    #[serde(default)]
    pub(super) textures: BTreeMap<String, SourceTexture>,
    pub(super) animations: Vec<SourceAnimation>,
    #[serde(default)]
    pub(super) kfm: Vec<SourceKfmMetadata>,
    #[serde(default)]
    pub(super) warnings: Vec<String>,
    #[serde(default)]
    pub(super) exact_mesh_selection_proof: Option<SourceExactMeshSelectionProof>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceNativeCoordinateContract {
    pub(super) schema: String,
    pub(super) published_space: String,
    pub(super) position: String,
    pub(super) normal: String,
    pub(super) translation: String,
    pub(super) rotation: String,
    pub(super) scale: String,
    pub(super) uv: String,
    pub(super) inverse_bind_matrix: String,
    pub(super) source_triangle_winding: String,
    pub(super) published_triangle_winding: String,
    pub(super) winding_conversion_owner: String,
    pub(super) unit_scale: String,
    pub(super) origin_policy: String,
    pub(super) auto_centered: bool,
    pub(super) auto_scaled: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceHierarchy {
    pub(super) source: String,
    pub(super) roots: Vec<SourceRoot>,
    pub(super) nodes: Vec<SourceNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceRoot {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) source_asset_index: usize,
    pub(super) transform_path_id: i64,
    pub(super) game_object_path_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceNode {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) parent: Option<String>,
    pub(super) source_asset_index: usize,
    pub(super) transform_path_id: i64,
    pub(super) game_object_path_id: Option<i64>,
    pub(super) translation: [f64; 3],
    pub(super) rotation: [f64; 4],
    pub(super) scale: [f64; 3],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceJoint {
    pub(super) path: String,
    pub(super) parent: Option<String>,
    pub(super) source_asset_index: usize,
    pub(super) transform_path_id: i64,
    pub(super) translation: [f64; 3],
    pub(super) rotation: [f64; 4],
    pub(super) scale: [f64; 3],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceGroup {
    pub(super) start: usize,
    pub(super) count: usize,
    pub(super) material_index: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceBinding {
    pub(super) component_asset_index: Option<usize>,
    pub(super) component_path_id: Option<i64>,
    pub(super) renderer_asset_index: Option<usize>,
    pub(super) renderer_path_id: Option<i64>,
    pub(super) mesh_filter_asset_index: Option<usize>,
    pub(super) mesh_filter_path_id: Option<i64>,
    pub(super) component_type: String,
    pub(super) root_transform_name: String,
    pub(super) root_transform_path_id: i64,
    pub(super) transform_asset_index: usize,
    pub(super) transform_path_id: i64,
    pub(super) transform_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceSkin {
    pub(super) source: String,
    #[serde(default)]
    pub(super) joint_paths: Vec<String>,
    #[serde(default)]
    pub(super) inverse_bind_matrices: Vec<[[f64; 4]; 4]>,
    #[serde(default)]
    pub(super) bone_indices: Vec<u16>,
    #[serde(default)]
    pub(super) weights: Vec<f64>,
    pub(super) renderer_asset_index: Option<usize>,
    pub(super) renderer_path_id: Option<i64>,
    pub(super) renderer_transform_path_id: Option<i64>,
    pub(super) mesh_filter_asset_index: Option<usize>,
    pub(super) mesh_filter_path_id: Option<i64>,
    pub(super) attachment_transform_path_id: Option<i64>,
    #[serde(default)]
    pub(super) decode_warning: Option<String>,
    #[serde(default)]
    pub(super) rest_vertex_transform: Option<[[f64; 4]; 4]>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceSavedProperties {
    #[serde(default)]
    pub(super) colors: Vec<SourceColorProperty>,
    #[serde(default)]
    pub(super) floats: Vec<SourceFloatProperty>,
    #[serde(default)]
    pub(super) texture_envs: Vec<SourceTextureEnvironment>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceColorProperty {
    pub(super) slot: usize,
    pub(super) name: String,
    pub(super) value: SourceColor,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceFloatProperty {
    pub(super) slot: usize,
    pub(super) name: String,
    pub(super) value: f64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub(super) struct SourceColor {
    pub(super) r: f64,
    pub(super) g: f64,
    pub(super) b: f64,
    pub(super) a: f64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub(super) struct SourceVec2 {
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourcePointer {
    pub(super) source_asset_index: usize,
    pub(super) file_id: i64,
    pub(super) path_id: i64,
    pub(super) is_null: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceSampler {
    pub(super) filter_mode: SourceSetting<i32>,
    pub(super) wrap_mode: SourceSetting<i32>,
    pub(super) aniso: SourceSetting<u32>,
    pub(super) mip_bias: SourceSetting<f64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceSetting<T> {
    pub(super) value: T,
    pub(super) source: String,
    pub(super) serialized: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceKfmMetadata {
    pub(super) source: String,
    pub(super) path: String,
    #[serde(default)]
    pub(super) clips: Vec<Value>,
    #[serde(default)]
    pub(super) references: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub(super) struct SourceNullableDuration(pub(super) Option<f64>);

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceCurveCounts {
    #[serde(default)]
    pub(super) compressed_rotation: usize,
    #[serde(default)]
    pub(super) rotation: usize,
    #[serde(default)]
    pub(super) position: usize,
    #[serde(default)]
    pub(super) scale: usize,
    #[serde(default)]
    pub(super) euler: usize,
    #[serde(default)]
    pub(super) float: usize,
    #[serde(default)]
    pub(super) pptr: usize,
    #[serde(default)]
    pub(super) events: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceFloatTrack {
    pub(super) path: String,
    pub(super) property: String,
    pub(super) class_id: i32,
    pub(super) script: AnimationBindingPointer,
    pub(super) pre_infinity: i32,
    pub(super) post_infinity: i32,
    pub(super) interpolation: String,
    pub(super) keys: Vec<SourceFloatKey>,
    pub(super) source_index: usize,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    pub(super) source_key_count: usize,
    pub(super) duplicate_keys: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceFloatKey {
    pub(super) source_key_index: usize,
    pub(super) time: f64,
    pub(super) value: f64,
    pub(super) in_tangent: Option<f64>,
    pub(super) out_tangent: Option<f64>,
    pub(super) tangent_mode: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceEmptyTrsBinding {
    pub(super) kind: EmptyTrsBindingKind,
    pub(super) path: String,
    pub(super) source_index: usize,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    #[serde(default)]
    pub(super) unbound_model_target: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceVec3Track {
    pub(super) path: String,
    pub(super) interpolation: String,
    pub(super) keys: Vec<SourceVec3Key>,
    #[serde(rename = "sourceIndex")]
    pub(super) source_index: usize,
    #[serde(rename = "sourceEncoding")]
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    #[serde(rename = "sourceKeyCount")]
    pub(super) source_key_count: usize,
    #[serde(rename = "duplicateKeys")]
    pub(super) duplicate_keys: Vec<SourceDuplicateAnimationKey>,
    /// Unity legacy Animation ignores a keyed curve when its relative target
    /// does not exist below the component GameObject. Shared source clips can
    /// retain curves for a second rig; exact extraction preserves them while
    /// the native runtime deliberately emits no channel for them.
    #[serde(rename = "unboundModelTarget", default)]
    pub(super) unbound_model_target: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceQuatTrack {
    pub(super) path: String,
    pub(super) interpolation: String,
    pub(super) keys: Vec<SourceQuatKey>,
    #[serde(rename = "sourceIndex")]
    pub(super) source_index: usize,
    #[serde(rename = "sourceEncoding")]
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    #[serde(rename = "sourceKeyCount")]
    pub(super) source_key_count: usize,
    #[serde(rename = "duplicateKeys")]
    pub(super) duplicate_keys: Vec<SourceDuplicateAnimationKey>,
    #[serde(rename = "unboundModelTarget", default)]
    pub(super) unbound_model_target: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceVec3Key {
    pub(super) source_key_index: usize,
    pub(super) time: f64,
    pub(super) value: [f64; 3],
    pub(super) in_tangent: Option<[f64; 3]>,
    pub(super) out_tangent: Option<[f64; 3]>,
    pub(super) tangent_mode: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceQuatKey {
    pub(super) source_key_index: usize,
    pub(super) time: f64,
    pub(super) value: [f64; 4],
    pub(super) in_tangent: Option<[f64; 4]>,
    pub(super) out_tangent: Option<[f64; 4]>,
    pub(super) tangent_mode: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceDuplicateTrsBinding {
    pub(super) kind: EmptyTrsBindingKind,
    pub(super) path: String,
    pub(super) source_index: usize,
    pub(super) source_encoding: EmptyTrsSourceEncoding,
    pub(super) relation: DuplicateTrsRelation,
    pub(super) canonical_track_index: usize,
    pub(super) canonical_source_index: usize,
    pub(super) canonical_source_encoding: EmptyTrsSourceEncoding,
    pub(super) source_key_count: usize,
    pub(super) duplicate_keys: Vec<SourceDuplicateAnimationKey>,
    pub(super) keys: SourceDuplicateTrsKeys,
    #[serde(default)]
    pub(super) resolution_proof: Option<SerializedCurveOverwriteProof>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub(super) enum SourceDuplicateTrsKeys {
    Vec3(Vec<SourceVec3Key>),
    Quaternion(Vec<SourceQuatKey>),
}

pub(super) struct PreparedPublicationFile {
    pub(super) relative_path: PathBuf,
    pub(super) bytes: Vec<u8>,
}

pub(super) type BuiltTextures = (
    Vec<NativeTexture>,
    Vec<NativeSampler>,
    Vec<TexturePublication>,
    Vec<PublishedTextureReport>,
    BTreeMap<String, u32>,
);

#[derive(Clone, Copy)]
pub(super) enum SourceRecoveryTrackRef<'a> {
    Vec3(&'a SourceVec3Track),
    Quaternion(&'a SourceQuatTrack),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum TrackKind {
    Translation,
    Rotation,
    Scale,
}

pub(super) enum SourceCanonicalTrack<'a> {
    Vec3(&'a SourceVec3Track),
    Quaternion(&'a SourceQuatTrack),
}

pub(super) trait Vec3TrackValues {
    fn from_vec3(kind: TrackKind, values: Vec<[f64; 3]>) -> Result<TrackValues>;
}

impl Vec3TrackValues for TrackValues {
    fn from_vec3(kind: TrackKind, values: Vec<[f64; 3]>) -> Result<TrackValues> {
        match kind {
            TrackKind::Translation => Ok(TrackValues::Translation(values)),
            TrackKind::Scale => Ok(TrackValues::Scale(values)),
            TrackKind::Rotation => invalid("rotation cannot be represented by a vec3 track"),
        }
    }
}

pub(super) struct MatrixIdentityDeviation {
    pub(super) max_abs: f64,
    pub(super) row: usize,
    pub(super) column: usize,
    pub(super) actual: f64,
    pub(super) identity: f64,
}

#[derive(Default)]
pub(super) struct BoundsAccumulator {
    pub(super) min: Option<Vec3>,
    pub(super) max: Option<Vec3>,
}

impl BoundsAccumulator {
    pub(super) fn include(&mut self, point: Vec3) -> Result<()> {
        if !point.is_finite() {
            return invalid("coordinate bounds contain a non-finite point");
        }
        self.min = Some(self.min.map_or(point, |value| value.min(point)));
        self.max = Some(self.max.map_or(point, |value| value.max(point)));
        Ok(())
    }

    pub(super) fn finish(self, label: &str) -> Result<CoordinateBounds> {
        let min = self
            .min
            .ok_or_else(|| invalid_error(format!("coordinate {label} bounds are empty")))?;
        let max = self
            .max
            .ok_or_else(|| invalid_error(format!("coordinate {label} bounds are empty")))?;
        Ok(CoordinateBounds {
            min: [f64::from(min.x), f64::from(min.y), f64::from(min.z)],
            max: [f64::from(max.x), f64::from(max.y), f64::from(max.z)],
        })
    }
}
