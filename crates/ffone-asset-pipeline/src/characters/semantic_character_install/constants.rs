use super::*;

pub(super) const BATCH_REPORT_FILE: &str = "logical-model-batch-report.json";

pub(super) const TABLE_SET_SCHEMA: &str = "ffone.table-set.v1";

pub(super) const CONSOLIDATED_TABLE: &str = "npc_imports_consolidated";

pub(super) const COORDINATE_CONTRACT: &str = "ffone.native-coordinate-contract.v1 / H=diag(-1,1,1)";

pub(super) const SEMANTIC_CHARACTER_SOURCE_PREFIX: &str = "native-semantic-characters/";

pub(super) const MANAGED_TARGETS: &[&str] = &[
    "characters/mob",
    "characters/npc",
    "characters/nano",
    "characters/mobs",
    "characters/fusions",
    "characters/npcs",
    "characters/nanos",
    "characters/shared",
    "characters/catalog.json",
    "characters/registry.json",
    "characters/schema-upgrade-violations.json",
    SEMANTIC_CHARACTER_REGISTRY_PATH,
];

pub(super) const NEW_TARGETS: &[&str] = &[
    "characters/mobs",
    "characters/fusions",
    "characters/npcs",
    "characters/nanos",
    "characters/shared",
    SEMANTIC_CHARACTER_REGISTRY_PATH,
];

pub(super) const MANIFESTLESS_EXTERNAL_NON_PACKAGE_ROOTS: &[&str] = &["characters/shared/runtime-textures"];

pub(super) static TRANSACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);
