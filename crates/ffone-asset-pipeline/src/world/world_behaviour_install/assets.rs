use super::*;

pub const WORLD_BEHAVIOUR_OWNERSHIP_PATH: &str = "world/behaviours/install-manifest.json";

pub(super) const RUNTIME_WORLD_REGISTRY_PATH: &str = "_runtime/world.json";

pub(super) const RUNTIME_WORLD_REGISTRY_SCHEMA: &str = "ffone.runtime-world.v1";

/// A tile's published hierarchy, reduced to what the behaviour conversion needs.
#[derive(Clone, Debug, Default)]
pub(super) struct HierarchyIndex {
    pub(super) world_matrix_by_node: BTreeMap<String, [[f64; 4]; 4]>,
    pub(super) model_ids_by_node: BTreeMap<String, Vec<String>>,
    pub(super) direct_model_ids_by_node: BTreeMap<String, Vec<String>>,
    pub(super) node_by_component_path_id: BTreeMap<i64, String>,
    pub(super) nodes: BTreeMap<String, HierarchyNode>,
    pub(super) children_by_node: BTreeMap<String, Vec<String>>,
}

pub(super) fn relative_hierarchy_path(
    owner: &str,
    descendant: &str,
    hierarchy: &HierarchyIndex,
) -> Result<Option<String>> {
    let mut current = descendant;
    let mut segments = Vec::new();
    let mut visited = BTreeSet::new();
    while current != owner {
        if !visited.insert(current.to_owned()) {
            return invalid(format!(
                "hierarchy cycle while resolving {descendant:?} below {owner:?}"
            ));
        }
        let Some(node) = hierarchy.nodes.get(current) else {
            return Ok(None);
        };
        segments.push(node.name.clone());
        let Some(parent) = node.parent.as_deref() else {
            return Ok(None);
        };
        current = parent;
    }
    segments.reverse();
    Ok(Some(segments.join("/")))
}

pub(super) fn load_registry(asset_root: &Path) -> Result<Vec<(String, String)>> {
    let path = asset_root.join(RUNTIME_WORLD_REGISTRY_PATH);
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    let registry: JsonValue = parse_json(&bytes, &asset_root.join(RUNTIME_WORLD_REGISTRY_PATH))?;
    if registry.get("schema").and_then(JsonValue::as_str) != Some(RUNTIME_WORLD_REGISTRY_SCHEMA) {
        return invalid("runtime world registry has an unexpected schema");
    }
    let mut tiles = Vec::new();
    for entry in registry
        .get("entries")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error("runtime world registry has no entries"))?
    {
        let id = entry
            .get("id")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error("runtime world entry has no id"))?;
        let scope = entry
            .get("scope")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error("runtime world entry has no scope"))?;
        tiles.push((id.to_owned(), scope.to_owned()));
    }
    tiles.sort();
    Ok(tiles)
}
