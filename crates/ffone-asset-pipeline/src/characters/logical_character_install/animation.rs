use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticRigSummary {
    pub skinned_meshes: u64,
    pub joints: u64,
    pub inverse_bind_matrices: u64,
    pub weighted_vertices: u64,
    pub skinning_preserved: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAnimationSummary {
    pub clips: u64,
    pub channels: u64,
    pub keyframes: u64,
    pub names: Vec<String>,
}

pub(super) fn glb_animation_names(bytes: &[u8]) -> Result<Vec<String>> {
    if bytes.len() < 20 || &bytes[..4] != b"glTF" {
        return invalid("logical character file is not GLB 2.0");
    }
    let json_length = u32::from_le_bytes(
        bytes[12..16]
            .try_into()
            .map_err(|_| invalid_error("invalid GLB JSON length"))?,
    ) as usize;
    if &bytes[16..20] != b"JSON" || 20_usize.saturating_add(json_length) > bytes.len() {
        return invalid("logical character GLB has no valid JSON chunk");
    }
    let document: Value =
        serde_json::from_slice(&bytes[20..20 + json_length]).map_err(|source| {
            PipelineError::Json {
                path: "logical-character.glb#JSON".to_owned(),
                source,
            }
        })?;
    document
        .get("animations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|animation| {
            animation
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| invalid_error("logical character animation has no exact name"))
        })
        .collect::<Result<Vec<_>>>()
}
