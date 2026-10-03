use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmbiguousDependencyArchive {
    pub archive: String,
    pub candidate_bundle_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootOrigin {
    pub asset: String,
    pub transform_path_id: i64,
    pub game_object: String,
    pub native_origin: [f64; 3],
    pub nonzero: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTrs {
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformException {
    pub asset: String,
    pub transform_path_id: i64,
    pub game_object: String,
    pub kinds: Vec<String>,
    pub native: NativeTrs,
    pub parent_path_id: Option<i64>,
}
