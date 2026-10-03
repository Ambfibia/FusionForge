use super::*;

pub(super) const DEFAULT_MAX_PART_BYTES: u64 = 24 * 1024 * 1024;

/// The raw object payload sum does not include the serialized-file header, ObjectInfo table,
/// shared type trees, external-reference table, or the UnityWeb bundle wrapper.  Keep a fixed
/// margin in every planned part and still verify the two real artifacts after writing them.
pub(super) const PART_METADATA_RESERVE_BYTES: u64 = 1024 * 1024;

pub(super) const EXACT_LEAF_TYPES: &[&str] = &[
    "AudioClip",
    "Texture2D",
    "Mesh",
    "Shader",
    "TextAsset",
    "AnimationClip",
];

pub(super) const USE_CORE: u16 = 1 << 0;

pub(super) const USE_WORLD: u16 = 1 << 4;

pub(super) const USE_NPC: u16 = 1 << 5;

pub(super) const USE_NANO: u16 = 1 << 6;

pub(super) const USE_PLAYER: u16 = 1 << 7;

pub(super) const USE_ITEMS: u16 = 1 << 8;

pub(super) const USE_ICONS: u16 = 1 << 9;

pub(super) const USE_HNPC: u16 = 1 << 10;

pub(super) const LAYOUT_CACHE_FORMAT: &str = "ffclient.legacy-layout-result-cache.v1";

pub(super) static LAYOUT_CACHE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) const LAYOUT_CACHE_STALE_SECONDS: u64 = 24 * 60 * 60;
