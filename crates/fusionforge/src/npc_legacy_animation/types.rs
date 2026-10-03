use super::*;

pub(super) type Vec3 = [f64; 3];

pub(super) type Quat = [f64; 4];

pub(super) type Matrix4 = [[f64; 4]; 4];

pub(super) type DecodedCurveTracks = Vec<DecodedCurveTrack>;

#[derive(Debug, Clone)]
pub(super) struct DecodedCurveTrack {
    pub(super) path: String,
    pub(super) keys: Vec<JsonValue>,
    pub(super) duplicate_keys: Vec<JsonValue>,
    pub(super) source_key_count: usize,
    pub(super) source_encoding: String,
    pub(super) source_index: usize,
}

#[derive(Debug, Clone)]
pub(super) struct DecodedCurveSamples {
    pub(super) keys: Vec<JsonValue>,
    pub(super) duplicate_keys: Vec<JsonValue>,
    pub(super) source_key_count: usize,
}

#[derive(Debug, Clone)]
pub(super) struct TransformNode {
    pub(super) key: ObjectKey,
    pub(super) game_object: Option<ObjectKey>,
    pub(super) name: String,
    pub(super) parent: Option<ObjectKey>,
    pub(super) translation: Vec3,
    pub(super) rotation: Quat,
    pub(super) scale: Vec3,
}

#[derive(Debug, Clone)]
pub(super) struct SkinnedRenderer {
    pub(super) key: ObjectKey,
    pub(super) mesh: ObjectKey,
    pub(super) transform: Option<ObjectKey>,
    pub(super) bones: Vec<ObjectKey>,
    pub(super) root_bone: Option<ObjectKey>,
    pub(super) body: fusionforge::UnityValue,
}

#[derive(Debug, Clone)]
pub(super) struct CurveTimeReference {
    pub(super) asset_name: String,
    pub(super) path_id: i64,
    pub(super) clip_name: String,
    pub(super) field: &'static str,
    pub(super) kind: &'static str,
    pub(super) source_index: usize,
    pub(super) path: String,
    pub(super) sample_rate: Option<f64>,
    pub(super) times: Vec<f64>,
    pub(super) payload_without_time: Vec<fusionforge::UnityValue>,
}

#[derive(Debug, Clone)]
pub(super) struct ConstantCurveReference {
    pub(super) asset_name: String,
    pub(super) path_id: i64,
    pub(super) clip_name: String,
    pub(super) field: &'static str,
    pub(super) kind: &'static str,
    pub(super) source_index: usize,
    pub(super) path: String,
    pub(super) sample_rate: Option<f64>,
    pub(super) constant_payload: JsonValue,
}

#[derive(Debug, Clone)]
pub(super) struct CurveRecoveryPlan {
    pub(super) field: &'static str,
    pub(super) kind: &'static str,
    pub(super) path: String,
    pub(super) source_indices: Vec<usize>,
    pub(super) canonical_source_index: usize,
    pub(super) canonical_samples: DecodedCurveSamples,
    pub(super) rejected: Vec<(usize, DecodedCurveSamples)>,
    pub(super) source_sample_rate: Option<f64>,
    pub(super) reference: ConstantCurveReference,
}

#[derive(Debug, Default)]
pub(super) struct CharacterRootCandidate {
    pub(super) meshes: BTreeSet<ObjectKey>,
    pub(super) searchable_names: BTreeSet<String>,
}

#[derive(Clone, Copy)]
pub(super) enum Vec3CurveKind {
    Translation,
    Scale,
}

impl Vec3CurveKind {
    pub(super) fn source_kind(self) -> &'static str {
        match self {
            Self::Translation => "translation",
            Self::Scale => "scale",
        }
    }
}
