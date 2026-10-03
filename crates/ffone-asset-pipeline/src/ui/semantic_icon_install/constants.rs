use super::*;

pub(super) const TABLE_SET_SCHEMA: &str = "ffone.table-set.v1";

pub(super) const ICON_ROOT: &str = "icons";

pub(super) const OWNED_SOURCE_PREFIX: &str = "semantic-icons/";

pub(super) const CATEGORY_DIRECTORIES: &[&str] = &[
    "items/weapons",
    "transport",
    "items/vehicles",
    "items/cosmetics",
    "items/quest",
    "items/general",
    "entities/mobs",
    "entities/npc",
    "entities/hnpc",
    "entities/nanos",
    "entities/fusion",
    "skills",
];

pub(super) const LEGACY_ICON_KINDS: &[LegacyIconKind] = &[
    LegacyIconKind {
        icon_type: 0,
        prefix: "wpnicon",
        category: SemanticIconCategory::ItemWeapon,
    },
    LegacyIconKind {
        icon_type: 1,
        prefix: "nanoicon",
        category: SemanticIconCategory::EntityNano,
    },
    LegacyIconKind {
        icon_type: 2,
        prefix: "skillicon",
        category: SemanticIconCategory::Skill,
    },
    LegacyIconKind {
        icon_type: 3,
        prefix: "cosicon",
        category: SemanticIconCategory::ItemCosmetic,
    },
    LegacyIconKind {
        icon_type: 4,
        prefix: "npcicon",
        category: SemanticIconCategory::EntityNpc,
    },
    LegacyIconKind {
        icon_type: 5,
        prefix: "nanoready",
        category: SemanticIconCategory::EntityNano,
    },
    LegacyIconKind {
        icon_type: 6,
        prefix: "questitemicon",
        category: SemanticIconCategory::ItemQuest,
    },
    LegacyIconKind {
        icon_type: 7,
        prefix: "generalitemicon",
        category: SemanticIconCategory::ItemGeneral,
    },
    LegacyIconKind {
        icon_type: 8,
        prefix: "mobicon",
        category: SemanticIconCategory::EntityMob,
    },
    LegacyIconKind {
        icon_type: 9,
        prefix: "fusionicon",
        category: SemanticIconCategory::EntityFusion,
    },
    LegacyIconKind {
        icon_type: 10,
        prefix: "hnpcicon",
        category: SemanticIconCategory::EntityHnpc,
    },
    LegacyIconKind {
        icon_type: 11,
        prefix: "transport",
        category: SemanticIconCategory::ItemTransport,
    },
    LegacyIconKind {
        icon_type: 12,
        prefix: "vehicle",
        category: SemanticIconCategory::ItemVehicle,
    },
];
