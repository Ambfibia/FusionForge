use super::*;

pub(super) type Vec3 = [f64; 3];

pub(super) type Quat = [f64; 4];

pub(super) type Matrix4 = [[f64; 4]; 4];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampledJointTransform {
    pub path: String,
    pub parent_index: Option<usize>,
    pub local_matrix: Matrix4,
    pub global_matrix: Matrix4,
}

#[derive(Debug, Clone, Default)]
pub(super) struct TransformTrack {
    pub(super) translations: Vec<Vec3Key>,
    pub(super) rotations: Vec<QuatKey>,
    pub(super) scales: Vec<Vec3Key>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Vec3Key {
    pub(super) time: f64,
    pub(super) value: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct QuatKey {
    pub(super) time: f64,
    pub(super) value: Quat,
}

#[derive(Debug, Clone)]
pub(super) struct Joint {
    pub(super) path: String,
    pub(super) parent: Option<usize>,
    pub(super) rest_translation: Vec3,
    pub(super) rest_rotation: Quat,
    pub(super) rest_scale: Vec3,
    pub(super) inverse_bind: Option<Matrix4>,
}

#[derive(Clone, Copy)]
pub(super) enum CurveKind {
    Translation,
    Scale,
}

pub(super) enum ParentRef {
    Index(usize),
    Path(String),
}
