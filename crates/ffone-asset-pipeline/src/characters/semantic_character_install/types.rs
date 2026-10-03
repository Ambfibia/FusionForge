use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishedCharacterClassification {
    pub method: String,
    pub table_reference_count: u64,
    pub observed_roles: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishedCharacter {
    pub id: String,
    pub logical_name: String,
    pub category: RuntimeCharacterCategory,
    pub legacy_route: String,
    pub glb: String,
    pub glb_blake3: String,
    pub source_sha256: String,
    pub runtime_spawn_policy: String,
    pub classification: PublishedCharacterClassification,
    pub authored_root: SemanticAuthoredRoot,
    pub rig: SemanticRigSummary,
    pub animations: SemanticAnimationSummary,
    pub materials: PublishedCharacterMaterialSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterSkipReason {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkippedCharacter {
    pub logical_name: String,
    pub legacy_route: String,
    pub candidate_glb: Option<String>,
    pub reasons: Vec<CharacterSkipReason>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum TableRole {
    Nano,
    Npc,
    Mob,
    Hnpc,
}

impl TableRole {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Nano => "nano",
            Self::Npc => "npc",
            Self::Mob => "mob",
            Self::Hnpc => "hnpc",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TableReference {
    pub(super) role: TableRole,
    pub(super) row_index: usize,
    pub(super) entity_number: Option<i64>,
    pub(super) mesh_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Classification {
    pub(super) category: Option<RuntimeCharacterCategory>,
    pub(super) method: &'static str,
    pub(super) references: Vec<TableReference>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PreservedCharacterContent {
    pub(super) entries: Vec<ProjectAssetFile>,
    pub(super) models: Vec<RuntimeCharacterModel>,
}
