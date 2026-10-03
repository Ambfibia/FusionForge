use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PayloadKind {
    Visual,
    Collider,
}

#[derive(Debug, Clone)]
pub(super) struct PendingPayload {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) hierarchy_node_id: String,
    pub(super) kind: PayloadKind,
    pub(super) source_component: ObjectKey,
    pub(super) source_game_object: ObjectKey,
    pub(super) source_mesh: ObjectKey,
    pub(super) source_mesh_filter: Option<ObjectKey>,
    pub(super) source_renderer: Option<ObjectKey>,
    pub(super) world_matrix: Matrix4,
    pub(super) effective_active: bool,
    pub(super) component_enabled: bool,
    pub(super) is_trigger: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PublishedPayload {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) hierarchy_node_id: String,
    pub(super) source_component: SourceIdentity,
    pub(super) source_game_object: SourceIdentity,
    pub(super) source_mesh: SourceIdentity,
    pub(super) source_mesh_filter: Option<SourceIdentity>,
    pub(super) source_renderer: Option<SourceIdentity>,
    pub(super) world_matrix: Matrix4,
    pub(super) transform_policy: String,
    pub(super) effective_active: bool,
    pub(super) component_enabled: bool,
    pub(super) runtime_published: bool,
    pub(super) geometry_status: String,
    pub(super) model_id: Option<String>,
    pub(super) model_path: Option<String>,
    pub(super) model_blake3: Option<String>,
    pub(super) root_name: Option<String>,
    pub(super) vertex_count: usize,
    pub(super) index_count: usize,
    pub(super) primitive_count: usize,
    pub(super) material_ids: Vec<Option<String>>,
    pub(super) is_trigger: bool,
}

pub(super) fn encode_glb(document: JsonValue, mut binary: Vec<u8>) -> Result<Vec<u8>, String> {
    pad_four(&mut binary, 0);
    let mut json_bytes =
        serde_json::to_vec(&document).map_err(|err| format!("could not encode GLB JSON: {err}"))?;
    pad_four(&mut json_bytes, b' ');
    let json_len =
        u32::try_from(json_bytes.len()).map_err(|_| "GLB JSON exceeds u32".to_string())?;
    let bin_len = u32::try_from(binary.len()).map_err(|_| "GLB binary exceeds u32".to_string())?;
    let total = 12usize
        .checked_add(8)
        .and_then(|value| value.checked_add(json_bytes.len()))
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(binary.len()))
        .ok_or_else(|| "GLB total length overflows usize".to_string())?;
    let total = u32::try_from(total).map_err(|_| "GLB total exceeds u32".to_string())?;
    let mut glb = Vec::with_capacity(total as usize);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&total.to_le_bytes());
    glb.extend_from_slice(&json_len.to_le_bytes());
    glb.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&bin_len.to_le_bytes());
    glb.extend_from_slice(&0x004e_4942u32.to_le_bytes());
    glb.extend_from_slice(&binary);
    if glb.len() != total as usize {
        return Err("internal GLB length mismatch".to_string());
    }
    Ok(glb)
}
