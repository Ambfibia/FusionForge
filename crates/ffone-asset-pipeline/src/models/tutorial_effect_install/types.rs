use super::*;

#[derive(Clone, Debug)]
pub struct TutorialEffectDependencyInput {
    pub source_bundle: PathBuf,
    pub object_dump: PathBuf,
}

impl TutorialEffectDependencyInput {
    pub fn new(source_bundle: impl Into<PathBuf>, object_dump: impl Into<PathBuf>) -> Self {
        Self {
            source_bundle: source_bundle.into(),
            object_dump: object_dump.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialSourceFileProof {
    pub logical_name: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialBulletRowFile {
    pub schema: String,
    pub bullet_type: i32,
    pub source_route: String,
    pub source_root_path_id: i64,
    pub serialized_row_blake3: String,
    pub parameters: TutorialBulletParameters,
    pub serialized_row: JsonValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialBulletParameters {
    pub cancel_script: i32,
    pub fire_script: i32,
    pub particle_script: i32,
    pub success_script: i32,
    pub cancel_model_scale: f64,
    pub curve_height: f64,
    pub fire_model_scale: f64,
    pub bullet_model_scale: f64,
    pub success_model_scale: f64,
    pub hide_time_seconds: f64,
    pub maximum_time_seconds: f64,
    pub fire_link: String,
    pub success_link: String,
    pub success_sound: String,
}

impl Eq for TutorialBulletParameters {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialEffectClosureFile {
    pub schema: String,
    pub effect_id: Option<i32>,
    pub container_route: String,
    pub root_asset: String,
    pub root_path_id: i64,
    pub source_bundle_blake3: String,
    pub source_dump_blake3: String,
    pub source_assets: Vec<TutorialSourceAssetProof>,
    pub objects: Vec<TutorialUnityObjectProof>,
}

#[derive(Clone, Debug)]
pub(super) struct PreparedFile {
    pub(super) relative: String,
    pub(super) source: String,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnedTree {
    Effects,
    Projectiles,
}

#[derive(Clone, Debug)]
pub(super) struct JsonPointer {
    pub(super) file_id: i64,
    pub(super) path_id: i64,
    pub(super) json_path: String,
}
