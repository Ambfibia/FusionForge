use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KfmPayloadEvidence {
    pub ownership_evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_asset_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_path_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_object_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<usize>,
    pub references: Vec<String>,
    pub nif_references: Vec<String>,
    pub pointer_traversal: KfmPointerTraversalEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preload_ownership: Option<KfmPreloadOwnershipEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_contained_game_object: Option<SelfContainedGameObjectEvidence>,
}

pub(super) fn exact_kfm_text_asset_payload(
    env: &super::super::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    visited: &mut BTreeSet<(usize, i64)>,
) -> Option<(crate::ContainerAssetBytesResolution, Vec<String>)> {
    let mut pending = vec![(asset_index, path_id)];
    while let Some((current_asset_index, current_path_id)) = pending.pop() {
        if !visited.insert((current_asset_index, current_path_id)) {
            continue;
        }
        let asset = env.assets.get(current_asset_index)?;
        let info = asset.objects.get(&current_path_id)?;
        let object_type = asset.object_type_name(info);
        let body = asset.read_object(current_asset_index, info).ok()?;
        if object_type == "TextAsset" {
            if let Some(bytes) = crate::object_payload_bytes(&body) {
                let references = crate::kfm_reference_paths_exact(&bytes);
                if references
                    .iter()
                    .any(|reference| normalize_logical_model_route(reference).ends_with(".nif"))
                {
                    return Some((
                        crate::ContainerAssetBytesResolution {
                            bytes,
                            asset_index: current_asset_index,
                            path_id: current_path_id,
                            object_type,
                        },
                        references,
                    ));
                }
            }
        }
        let mut pointers = Vec::new();
        crate::collect_value_pointers(&body, &mut pointers);
        let resolved = pointers
            .into_iter()
            .filter_map(|pointer| env.resolve_pointer(&pointer).ok())
            .map(|key| (key.asset, key.path_id))
            .collect::<Vec<_>>();
        for key in resolved.into_iter().rev() {
            if !visited.contains(&key) {
                pending.push(key);
            }
        }
    }
    None
}
