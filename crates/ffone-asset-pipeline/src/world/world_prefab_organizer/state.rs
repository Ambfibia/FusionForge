use super::*;

#[derive(Default)]
pub(super) struct Inventory {
    pub(super) resources: BTreeMap<ResourceKey, ResourceDraft>,
    pub(super) prefabs: BTreeMap<Vec<PrefabPartKey>, PrefabDraft>,
    pub(super) placements: Vec<PlacementDraft>,
    pub(super) tiles: BTreeSet<(String, String)>,
    pub(super) counts: WorldPrefabOrganizerCounts,
    pub(super) source_set: blake3::Hasher,
}
