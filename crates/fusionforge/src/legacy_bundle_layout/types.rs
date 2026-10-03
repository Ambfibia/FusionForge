use super::*;

pub(super) type NodeKey = (usize, i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SourcePointerKey {
    pub(super) source_asset: usize,
    pub(super) file_id: i32,
    pub(super) path_id: i64,
}

impl From<&Pointer> for SourcePointerKey {
    fn from(pointer: &Pointer) -> Self {
        Self {
            source_asset: pointer.source_asset,
            file_id: pointer.file_id,
            path_id: pointer.path_id,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LegacyConfig {
    pub(super) enabled: bool,
    pub(super) max_part_bytes: u64,
    pub(super) preload_npc_bundles: bool,
    pub(super) load_npc_bundles_in_world: bool,
    pub(super) npc_bundle_manifest_sections: BTreeSet<String>,
    pub(super) core_name: String,
    pub(super) tutorial_audio_name: String,
    pub(super) ui_audio_name: String,
    pub(super) npc_voice_prefix: String,
    pub(super) world_shared_prefix: String,
    pub(super) dong_prefix: String,
    pub(super) npc_prefix: String,
    pub(super) hnpc_prefix: String,
    pub(super) nano_prefix: String,
    pub(super) player_prefix: String,
    pub(super) items_prefix: String,
    pub(super) icons_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Family {
    /// Path alone is ambiguous; ownership is inferred from graph consumers.
    Auto,
    Core,
    TutorialAudio,
    UiAudio,
    NpcVoice,
    WorldShared,
    Dong(String),
    Npc,
    Hnpc,
    Nano,
    Player,
    Items,
    Icons,
    /// Scene/UI compatibility bundles whose names are hardcoded by the legacy client.
    Compat(String),
}

#[derive(Debug, Clone)]
pub(super) struct Node {
    pub(super) key: NodeKey,
    pub(super) object_type: String,
    pub(super) name: String,
    pub(super) raw_hash: String,
    /// Canonical parsed Unity value for exact leaf objects. Unlike `raw_hash`, this is
    /// independent of serialized-file format, alignment padding, and rewritten PPtr IDs.
    pub(super) semantic_hash: Option<String>,
    /// SHA-1 of the actual embedded OGG/audio payload for AudioClip objects.
    pub(super) audio_payload_sha1: Option<String>,
    /// Serialized value with every PPtr zeroed; independent of source-local file/path IDs.
    pub(super) shape_hash: String,
    pub(super) type_tree_hash: String,
    pub(super) size: u64,
    pub(super) edges: Vec<NodeKey>,
}

#[derive(Debug, Clone)]
pub(super) struct Root {
    pub(super) path: String,
    pub(super) normalized_path: String,
    pub(super) target: NodeKey,
    pub(super) preload_roots: Vec<NodeKey>,
    pub(super) preserved_preloads: Vec<(AssetRef, i64)>,
    pub(super) source_asset: usize,
    pub(super) source_assetbundle: i64,
    pub(super) source_entry: UnityValue,
    pub(super) family: Family,
    pub(super) semantic_owner: SemanticOwnerHint,
    /// Source CachingManifest phases for every legacy declaration of this route.
    pub(super) sections: BTreeSet<String>,
    /// Assigned by the dependency-first packer. Keeping this explicit avoids the old
    /// "first part of the family" guess, which could introduce a synthetic part cycle.
    pub(super) output_part: Option<usize>,
}

#[derive(Debug, Clone)]
pub(super) struct Part {
    pub(super) family: Family,
    /// Exact runtime loading phases retained from the source graph. Parts with different
    /// phase sets are never coalesced, even when they belong to the same semantic family.
    pub(super) sections: BTreeSet<String>,
    pub(super) ordinal: usize,
    pub(super) nodes: Vec<NodeKey>,
    pub(super) estimated_bytes: u64,
    pub(super) output_name: String,
    pub(super) internal_name: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OutputLocation {
    pub(super) part: usize,
    pub(super) path_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LegacyLayoutPlan {
    /// Dependency-first order suitable for AssetLoader download/load registration.
    pub output_bundles: Vec<String>,
    pub routeable_bundles: Vec<String>,
    /// Inputs that the caller may delete only after managed loader/manifest patching succeeds.
    pub retired_bundles: Vec<String>,
    pub manifest_sections: Vec<LegacyRoutedBundle>,
    pub rewritten_retained_bundles: Vec<String>,
    pub dependencies: Vec<LegacyBundleDependency>,
    pub source_objects: usize,
    pub output_objects: usize,
    pub exact_objects_deduplicated: usize,
    pub same_name_different_content: usize,
    pub unresolved_pointers_preserved: usize,
    pub unreadable_orphans_removed: Vec<String>,
    pub dangling_preloads_removed: usize,
    pub dangling_preload_examples: Vec<String>,
    pub dangling_object_pointers_cleared: usize,
    pub dangling_object_pointer_examples: Vec<String>,
    pub translated_audio_pairs_preserved: usize,
    pub patched_audio_entries_preserved: usize,
    pub tile_scoped_audio_exceptions: Vec<String>,
    pub tile_scoped_route_exceptions: Vec<String>,
    pub route_conflicts: Vec<LegacyRouteConflict>,
    pub unclassified_roots: Vec<String>,
    pub oversized_parts: Vec<String>,
}

#[derive(Debug)]
pub(super) struct Plan {
    pub(super) canonical: BTreeMap<NodeKey, NodeKey>,
    pub(super) parts: Vec<Part>,
    pub(super) locations: BTreeMap<NodeKey, OutputLocation>,
    pub(super) roots: Vec<Root>,
    pub(super) exact_deduplicated: usize,
    pub(super) same_name_different_content: usize,
    pub(super) route_conflicts: Vec<LegacyRouteConflict>,
    pub(super) tile_scoped_audio_exceptions: Vec<String>,
    pub(super) tile_scoped_route_exceptions: Vec<String>,
    pub(super) unclassified_roots: Vec<String>,
    pub(super) oversized_parts: Vec<String>,
    pub(super) dangling_object_pointers_to_clear: BTreeMap<NodeKey, Vec<DanglingObjectPointer>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(super) enum CompactOwner {
    #[default]
    None,
    One(usize),
    Shared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct CompactUsage {
    pub(super) reachable: bool,
    pub(super) mask: u16,
    pub(super) dong: Option<usize>,
    pub(super) many_dongs: bool,
    pub(super) owner: CompactOwner,
}

#[derive(Debug, Clone)]
pub(super) struct PackingUnit {
    pub(super) family: Family,
    pub(super) sections: BTreeSet<String>,
    /// True when `sections` came from an actual source CachingManifest entry. Public root
    /// units keep that lifecycle boundary. Dependency-only SCC units may still be registered
    /// in an additional consumer phase after a monolithic source bundle is split: otherwise
    /// CharacterSelection/Tutorial can dereference an object whose new part was never loaded.
    pub(super) sections_pinned: bool,
    pub(super) nodes: Vec<NodeKey>,
    pub(super) roots: Vec<usize>,
    pub(super) estimated_bytes: u64,
    pub(super) owner: CompactOwner,
    /// Unit -> dependencies. The packer emits dependencies first.
    pub(super) dependencies: Vec<usize>,
}

impl PackingUnit {
    pub(super) fn empty(family: Family) -> Self {
        Self {
            family,
            sections: BTreeSet::new(),
            sections_pinned: false,
            nodes: Vec::new(),
            roots: Vec::new(),
            estimated_bytes: 0,
            owner: CompactOwner::None,
            dependencies: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PackedUnitsPart {
    pub(super) family: Family,
    pub(super) sections: BTreeSet<String>,
    pub(super) units: Vec<usize>,
    pub(super) estimated_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LayoutCacheFile {
    pub(super) name: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LayoutCacheReceipt {
    pub(super) format: String,
    pub(super) key: String,
    pub(super) complete: bool,
    pub(super) plan_sha256: String,
    pub(super) report_sha256: String,
    pub(super) files: Vec<LayoutCacheFile>,
}
