use super::*;

pub(super) const EQUIPMENT_SOURCE_BATCH_SCHEMA: &str = "ffclient.equipment-model-source-batch.v1";

pub(super) const EQUIPMENT_TAXONOMY: &str = "characters/player/equipment/{hat,mask,glasses,back,head,shirt,pants,shoes,weapon,vehicle}/[<xdt-route-qualifier>/]<true-name>";

pub(super) const ALLOWED_CATEGORIES: &[&str] = &[
    "back", "glasses", "hat", "head", "mask", "pants", "shirt", "shoes", "vehicle", "weapon",
];

pub(super) static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);
