use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticIconCategory {
    ItemWeapon,
    ItemTransport,
    ItemVehicle,
    ItemCosmetic,
    ItemQuest,
    ItemGeneral,
    EntityMob,
    EntityNpc,
    EntityHnpc,
    EntityNano,
    EntityFusion,
    Skill,
}

impl SemanticIconCategory {
    pub(super) fn directory(self) -> &'static str {
        match self {
            Self::ItemWeapon => "items/weapons",
            Self::ItemTransport => "transport",
            Self::ItemVehicle => "items/vehicles",
            Self::ItemCosmetic => "items/cosmetics",
            Self::ItemQuest => "items/quest",
            Self::ItemGeneral => "items/general",
            Self::EntityMob => "entities/mobs",
            Self::EntityNpc => "entities/npc",
            Self::EntityHnpc => "entities/hnpc",
            Self::EntityNano => "entities/nanos",
            Self::EntityFusion => "entities/fusion",
            Self::Skill => "skills",
        }
    }

    pub(super) fn all() -> [Self; 12] {
        [
            Self::ItemWeapon,
            Self::ItemTransport,
            Self::ItemVehicle,
            Self::ItemCosmetic,
            Self::ItemQuest,
            Self::ItemGeneral,
            Self::EntityMob,
            Self::EntityNpc,
            Self::EntityHnpc,
            Self::EntityNano,
            Self::EntityFusion,
            Self::Skill,
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticIconUnmatchedReason {
    MissingTexture,
    AmbiguousTexture,
    UnsupportedIconType,
    UnreferencedLegacyTexture,
    NameOnlyNoTableDataProof,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconTableReference {
    pub table_document: String,
    pub table: String,
    pub array: String,
    pub row_index: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconClassificationProof {
    pub method: &'static str,
    pub legacy_icon_type: u8,
    pub legacy_icon_prefix: String,
    pub category: SemanticIconCategory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIconUnmatched {
    pub reason: SemanticIconUnmatchedReason,
    pub legacy_true_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_icon_type: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_icon_number: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<SemanticIconSourceAsset>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub table_references: Vec<SemanticIconTableReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TableSetDocument {
    pub(super) schema: String,
    pub(super) tables: Vec<TableSetEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TableSetEntry {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) value: Value,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LegacyIconKind {
    pub(super) icon_type: u8,
    pub(super) prefix: &'static str,
    pub(super) category: SemanticIconCategory,
}

#[derive(Clone, Debug)]
pub(super) struct IconReferenceGroup {
    pub(super) icon_type: u8,
    pub(super) icon_number: u32,
    pub(super) references: Vec<SemanticIconTableReference>,
}

#[derive(Clone, Debug)]
pub(super) struct PreparedIcon {
    pub(super) source: ProjectAssetFile,
    pub(super) destination: String,
    pub(super) catalog: SemanticIconAsset,
}
