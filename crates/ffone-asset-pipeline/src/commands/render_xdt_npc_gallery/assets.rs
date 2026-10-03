use super::*;

pub(super) const REGISTRY_SCHEMA: &str = "ffone.semantic-character-registry.v2";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeRegistry {
    pub(super) schema: String,
    pub(super) models: Vec<RuntimeModel>,
}

pub(super) fn index_registry(registry: &RuntimeRegistry) -> Result<BTreeMap<String, &RuntimeModel>, String> {
    let mut result = BTreeMap::new();
    for model in &registry.models {
        if !is_blake3(&model.glb_blake3) {
            return Err(format!(
                "runtime model {:?} has an invalid glbBlake3 value {:?}",
                model.id, model.glb_blake3
            ));
        }
        let (_, stem) = model
            .id
            .split_once('/')
            .ok_or_else(|| format!("runtime model id has no category prefix: {:?}", model.id))?;
        for name in std::iter::once(stem).chain(model.legacy_aliases.iter().map(String::as_str)) {
            let key = name.to_lowercase();
            if let Some(previous) = result.insert(key, model)
                && previous.glb != model.glb
            {
                return Err(format!(
                    "runtime registry has a case-insensitive model-stem/alias collision at {name:?}"
                ));
            }
        }
    }
    Ok(result)
}

pub(super) fn checked_registry_output_base(id: &str) -> Result<PathBuf, String> {
    let path = Path::new(id);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe runtime model id {id:?}"));
    }
    Ok(path.to_path_buf())
}

pub(super) fn normalized_route(model_stem: &str) -> String {
    format!("mob/{}.kfm", model_stem.to_lowercase())
}
