use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialUnityObjectProof {
    pub asset: String,
    pub path_id: i64,
    pub type_id: i64,
    pub class_id: i64,
    pub object_type: String,
    pub name: String,
    pub canonical_blake3: String,
    pub value: JsonValue,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DumpObject {
    pub(super) asset: String,
    pub(super) path_id: i64,
    pub(super) type_id: i64,
    pub(super) class_id: i64,
    #[serde(rename = "type")]
    pub(super) object_type: String,
    pub(super) name: String,
    pub(super) value: JsonValue,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct UnityObjectKey {
    pub(super) asset: String,
    pub(super) path_id: i64,
}

pub(super) fn collect_container_routes(
    container: &JsonValue,
    source_asset: &str,
) -> Result<BTreeMap<String, Vec<UnityObjectKey>>> {
    let array = container.as_array().ok_or_else(|| {
        PipelineError::InvalidManifest("AssetBundle.m_Container is not an array".to_owned())
    })?;
    let mut routes = BTreeMap::<String, Vec<UnityObjectKey>>::new();
    for (index, entry) in array.iter().enumerate() {
        let pair = entry.as_array().ok_or_else(|| {
            PipelineError::InvalidManifest(format!(
                "AssetBundle.m_Container[{index}] is not a serialized pair"
            ))
        })?;
        if pair.len() != 2 {
            return invalid(format!(
                "AssetBundle.m_Container[{index}] has {} pair elements",
                pair.len()
            ));
        }
        let route = pair[0].as_str().ok_or_else(|| {
            PipelineError::InvalidManifest(format!(
                "AssetBundle.m_Container[{index}] route is not a string"
            ))
        })?;
        let mut pointers = Vec::new();
        collect_pointers(&pair[1], "$", &mut pointers);
        let roots = pointers
            .into_iter()
            .filter(|pointer| pointer.path_id != 0)
            .filter_map(|pointer| {
                let target_asset = if pointer.file_id == 0 {
                    Some(source_asset)
                } else {
                    referenced_asset_name(source_asset, pointer.file_id)
                }?;
                Some(UnityObjectKey {
                    asset: target_asset.to_owned(),
                    path_id: pointer.path_id,
                })
            })
            .collect::<Vec<_>>();
        if roots.len() > 1 {
            return invalid(format!(
                "AssetBundle route {route:?} resolves {} internal non-null roots",
                roots.len()
            ));
        }
        routes.entry(route.to_owned()).or_default().extend(roots);
    }
    Ok(routes)
}
