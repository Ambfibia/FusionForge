use super::*;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HierarchyPayload {
    pub(super) id: String,
    #[serde(default)]
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) hierarchy_node_id: String,
    pub(super) source_mesh: WorldPrefabSourceIdentity,
    #[serde(default)]
    pub(super) material_ids: Vec<Option<String>>,
    pub(super) world_matrix: [[f64; 4]; 4],
    #[serde(default)]
    pub(super) runtime_published: bool,
    #[serde(default)]
    pub(super) model_path: Option<String>,
    #[serde(default)]
    pub(super) model_blake3: Option<String>,
    #[serde(default)]
    pub(super) is_trigger: bool,
}

pub(super) fn payload_has_runtime_model(payload: &HierarchyPayload) -> bool {
    payload.runtime_published && payload.model_path.is_some()
}

pub(super) fn is_legacy_collision_helper_payload(payload: &HierarchyPayload) -> bool {
    payload.kind == "visual"
        && payload.source_mesh.asset == LEGACY_COLLISION_HELPER_ASSET
        && LEGACY_COLLISION_HELPER_VISUALS
            .iter()
            .any(|&(mesh, material)| {
                payload.source_mesh.path_id == mesh
                    && payload.material_ids.as_slice()
                        == [Some(format!("{LEGACY_COLLISION_HELPER_ASSET}:{material}"))]
            })
}

pub(super) fn payload_aliases(
    payload: &HierarchyPayload,
    nodes: &BTreeMap<&str, &HierarchyNode>,
    materials: &BTreeMap<String, MaterialName>,
) -> BTreeSet<String> {
    let mut aliases = node_aliases(&payload.hierarchy_node_id, nodes);
    insert_alias(&mut aliases, &payload.name);
    for material_id in payload.material_ids.iter().flatten() {
        if let Some(material) = materials.get(material_id) {
            insert_alias(&mut aliases, &material.name);
        }
    }
    aliases
}

pub(super) fn encode_glb(parsed: &ParsedGlb, path: &str) -> Result<Vec<u8>> {
    let mut json = serde_json::to_vec(&parsed.document).map_err(generated_json_error)?;
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut binary = parsed.binary.clone();
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    if let Some(buffer) = parsed
        .document
        .get("buffers")
        .and_then(JsonValue::as_array)
        .and_then(|buffers| buffers.first())
        .and_then(JsonValue::as_object)
    {
        if buffer.get("byteLength").and_then(JsonValue::as_u64) != Some(parsed.binary.len() as u64)
        {
            return invalid(format!("{path:?} buffer byteLength changed unexpectedly"));
        }
    }
    let total = 12 + 8 + json.len() + 8 + binary.len();
    let total_u32 = u32::try_from(total).map_err(|_| invalid_error("GLB exceeds u32"))?;
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2_u32.to_le_bytes());
    output.extend_from_slice(&total_u32.to_le_bytes());
    output.extend_from_slice(&(json.len() as u32).to_le_bytes());
    output.extend_from_slice(&GLB_JSON_CHUNK.to_le_bytes());
    output.extend_from_slice(&json);
    output.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    output.extend_from_slice(&GLB_BIN_CHUNK.to_le_bytes());
    output.extend_from_slice(&binary);
    Ok(output)
}
