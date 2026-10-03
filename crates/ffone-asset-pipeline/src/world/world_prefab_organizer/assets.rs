use super::*;

pub const WORLD_PREFAB_CATALOG_SCHEMA: &str = "ffone.map-catalog.v1";

pub const WORLD_PREFAB_CATALOG_PATH: &str = "map/catalog.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabCatalogPrefab {
    pub id: String,
    pub name: String,
    pub category: String,
    pub prefix: String,
    pub family: String,
    pub definition: WorldPrefabArtifact,
    pub part_count: u64,
    pub occurrence_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_set: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapCompositeObjectCatalogEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub prefix: String,
    pub family: String,
    pub definition: WorldPrefabArtifact,
    pub files: Vec<WorldPrefabArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_set: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldPrefabCatalog {
    pub schema: String,
    pub source_build: String,
    pub generated_by: String,
    pub coordinate_contract: String,
    pub compatibility_policy: String,
    pub reconstruction_proof: MapReconstructionProof,
    pub categories: BTreeMap<String, u64>,
    pub prefixes: BTreeMap<String, u64>,
    #[serde(default)]
    pub shared_files: Vec<WorldPrefabArtifact>,
    #[serde(default, rename = "resourceSets")]
    pub resource_sets: Vec<ResourceSetCatalogEntry>,
    #[serde(rename = "geometry")]
    pub resources: Vec<WorldPrefabResource>,
    #[serde(rename = "objects")]
    pub prefabs: Vec<WorldPrefabCatalogPrefab>,
    #[serde(default, rename = "compositeObjects")]
    pub composite_objects: Vec<MapCompositeObjectCatalogEntry>,
    #[serde(rename = "tiles")]
    pub placement_sets: Vec<WorldPrefabPlacementSetReference>,
}

pub(super) fn map_closure_path(path: &str) -> Result<String> {
    if path.starts_with("map/") || path.starts_with("objects/") {
        Ok(path.to_owned())
    } else {
        invalid(format!(
            "map-owned artifact escaped map/ and objects/: {path:?}"
        ))
    }
}

pub(super) fn relative_path(from: &Path, to: &Path) -> Result<String> {
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = Vec::<String>::new();
    for component in &from[common..] {
        if !matches!(component, Component::Normal(_)) {
            return invalid("relative GLB source path is not normalized");
        }
        parts.push("..".to_owned());
    }
    for component in &to[common..] {
        let Component::Normal(value) = component else {
            return invalid("relative GLB target path is not normalized");
        };
        parts.push(
            value
                .to_str()
                .ok_or_else(|| invalid_error("relative path is not UTF-8"))?
                .to_owned(),
        );
    }
    Ok(parts.join("/"))
}

pub(super) fn path_text(path: &Path) -> String {
    slash_path(path)
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
