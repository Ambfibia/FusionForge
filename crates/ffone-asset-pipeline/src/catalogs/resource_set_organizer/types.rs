use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSetArtifact {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSetMember {
    pub id: String,
    pub name: String,
    pub definition: ResourceSetArtifact,
    pub files: Vec<ResourceSetArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSetDocument {
    pub schema: String,
    pub id: String,
    pub name: String,
    pub domain: String,
    pub category: String,
    pub prefix: String,
    pub family: String,
    pub textures: Vec<ResourceSetArtifact>,
    pub members: Vec<ResourceSetMember>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSetOrganizerReport {
    pub schema: String,
    pub map_sets: u64,
    pub map_objects: u64,
    pub map_textures_before: u64,
    pub map_textures_after: u64,
    pub duplicate_texture_files_removed: u64,
    pub catalog: ResourceSetArtifact,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerItemDefinition {
    pub schema: String,
    pub id: String,
    pub true_name: String,
    pub category: String,
    pub source_route: String,
    pub resource_set: String,
    pub model: ResourceSetArtifact,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerItemSetOrganizerReport {
    pub schema: String,
    pub sets: u64,
    pub models: u64,
    pub source_texture_files: u64,
    pub published_atlases: u64,
    pub duplicate_texture_files_removed: u64,
    pub rendering_textures: u64,
    #[serde(default)]
    pub conversion_reports_removed: u64,
    pub catalog: ResourceSetArtifact,
}

#[derive(Clone, Debug)]
pub(super) struct PlannedSet {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) category: String,
    pub(super) prefix: String,
    pub(super) family: String,
    pub(super) relative: String,
    pub(super) members: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(super) struct PlayerSetPlan {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) category: String,
    pub(super) relative: String,
    pub(super) members: Vec<usize>,
    pub(super) texture_hashes: BTreeSet<String>,
}
