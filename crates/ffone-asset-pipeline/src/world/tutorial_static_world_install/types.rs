use super::*;

/// Publication scope of one audited static-world tile set.
///
/// Both scopes share the complete audit, merge and transaction logic. They
/// differ only in the already-installed native tree they enrich, the runtime
/// model root they own, and the ownership proof they publish. The scope is
/// derived from the tile identity so that a tutorial tile can never be
/// published into the world-map tree, or the reverse.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum StaticWorldScope {
    Tutorial,
    WorldMap,
}

impl StaticWorldScope {
    pub(super) fn of_tile(tile_id: &str) -> Result<Self> {
        if tile_id.starts_with("tile_") {
            Ok(Self::Tutorial)
        } else if tile_id.starts_with("map_") {
            Ok(Self::WorldMap)
        } else {
            invalid(format!("unsupported static-world tile id {tile_id:?}"))
        }
    }

    pub(super) fn of_contracts(contracts: &[TileContract]) -> Result<Self> {
        let mut scopes = contracts
            .iter()
            .map(|contract| Self::of_tile(&contract.id))
            .collect::<Result<BTreeSet<_>>>()?;
        match (scopes.pop_first(), scopes.is_empty()) {
            (Some(scope), true) => Ok(scope),
            (Some(_), false) => invalid("static-world contract mixes tutorial and world-map tiles"),
            (None, _) => invalid("static-world contract contains no tiles"),
        }
    }

    pub(super) fn scene_scope(self) -> &'static str {
        match self {
            Self::Tutorial => "tutorial",
            Self::WorldMap => "worldMap",
        }
    }

    pub(super) fn world_reference_scope(self) -> WorldReferenceScope {
        match self {
            Self::Tutorial => WorldReferenceScope::Tutorial,
            Self::WorldMap => WorldReferenceScope::WorldMap,
        }
    }

    pub(super) fn ownership_path(self) -> &'static str {
        match self {
            Self::Tutorial => TUTORIAL_STATIC_WORLD_OWNERSHIP_PATH,
            Self::WorldMap => WORLD_MAP_STATIC_WORLD_OWNERSHIP_PATH,
        }
    }

    pub(super) fn ownership_schema(self) -> &'static str {
        match self {
            Self::Tutorial => TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA,
            Self::WorldMap => WORLD_MAP_STATIC_WORLD_OWNERSHIP_SCHEMA,
        }
    }

    pub(super) fn installer_id(self) -> &'static str {
        match self {
            Self::Tutorial => INSTALLER_ID,
            Self::WorldMap => WORLD_MAP_INSTALLER_ID,
        }
    }

    pub(super) fn static_root(self) -> &'static str {
        match self {
            Self::Tutorial => "world/tutorial/static",
            Self::WorldMap => "world/maps/static",
        }
    }

    pub(super) fn source_path_prefix(self) -> &'static str {
        match self {
            Self::Tutorial => "native-static-world-retro-tutorial-v1",
            Self::WorldMap => "native-static-world-retro-worldmap-v1",
        }
    }

    /// Only the tutorial tiles went through the one-time runtime-world
    /// migration and conversion-metadata cleanup, so only they may take the
    /// archived-scene recovery path.
    pub(super) fn supports_migrated_scene_recovery(self) -> bool {
        matches!(self, Self::Tutorial)
    }
}

#[derive(Clone, Debug)]
pub(super) struct TileContract {
    pub(super) id: Cow<'static, str>,
    pub(super) source_archive_blake3: Cow<'static, str>,
    pub(super) scene_nodes: u64,
    pub(super) exported_visuals: u64,
    pub(super) runtime_visuals: u64,
    pub(super) exported_colliders: u64,
    pub(super) runtime_colliders: u64,
    pub(super) exported_models: u64,
    pub(super) vertices: u64,
    pub(super) indices: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct WorldMapStaticWorldContract {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) tiles: Vec<WorldMapStaticWorldContractTile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct WorldMapStaticWorldContractTile {
    pub(super) id: String,
    pub(super) source_archive_blake3: String,
    pub(super) scene_nodes: u64,
    pub(super) exported_visuals: u64,
    pub(super) runtime_visuals: u64,
    pub(super) exported_colliders: u64,
    pub(super) runtime_colliders: u64,
    pub(super) exported_models: u64,
    pub(super) vertices: u64,
    pub(super) indices: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialStaticWorldOwnership {
    pub schema: String,
    pub installer: String,
    pub source_build: String,
    pub source_set_blake3: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub winding_repair: Option<StaticWorldWindingRepairProof>,
    pub tiles: Vec<TutorialStaticWorldTileProof>,
    pub owned_files: Vec<TutorialStaticWorldOwnedFile>,
    pub scenes: Vec<TutorialStaticWorldSceneProof>,
    #[serde(default)]
    pub references: Vec<TutorialStaticWorldReferenceProof>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticWorldWindingRepairProof {
    pub schema: String,
    pub tool: String,
    pub operation: String,
    pub source_contract: String,
    pub report_path: String,
    pub visual_files: u64,
    pub converted_files: u64,
    pub unchanged_files: u64,
    pub source_set_blake3: String,
    pub result_set_blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialStaticWorldReferenceProof {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    pub scene_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialStaticWorldTileProof {
    pub tile_id: String,
    pub source_archive_blake3: String,
    pub export_manifest_bytes: u64,
    pub export_manifest_blake3: String,
    pub export_files: u64,
    pub export_bytes: u64,
    pub source_scene_blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialStaticWorldOwnedFile {
    pub source_path: String,
    pub path: String,
    pub kind: ProjectAssetKind,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialStaticWorldSceneProof {
    pub tile_id: String,
    pub path: String,
    pub source_scene_blake3: String,
    pub static_fields_blake3: String,
    pub installed_bytes: u64,
    pub installed_blake3: String,
    pub model_count: u64,
    pub visual_count: u64,
    pub collider_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum WorldReferenceScope {
    WorldMap,
    Tutorial,
}

#[derive(Clone, Debug)]
pub(super) struct AuditedTile {
    pub(super) proof: TutorialStaticWorldTileProof,
    pub(super) selected: Vec<SourcePublication>,
    pub(super) scene: JsonValue,
    pub(super) scene_bytes: Vec<u8>,
    pub(super) scene_blake3: String,
    pub(super) model_count: u64,
    pub(super) visual_count: u64,
    pub(super) collider_count: u64,
}

#[derive(Clone, Debug)]
pub(super) struct SourcePublication {
    pub(super) source: PathBuf,
    pub(super) source_path: String,
    pub(super) path: String,
    pub(super) kind: ProjectAssetKind,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Debug)]
pub(super) enum PublicationContent {
    Source(PathBuf),
    Generated(Vec<u8>),
}

#[derive(Clone, Debug)]
pub(super) struct Publication {
    pub(super) entry: ProjectAssetFile,
    pub(super) content: PublicationContent,
}

#[derive(Clone, Debug)]
pub(super) struct ExpectedSceneReference {
    pub(super) tile_id: String,
    pub(super) tile: [i32; 2],
    pub(super) blake3: String,
}

#[derive(Clone, Debug)]
pub(super) struct CleanupArchivedOwnership {
    pub(super) ownership: TutorialStaticWorldOwnership,
    pub(super) files_by_source: BTreeMap<String, ArchivedRuntimeMetadata>,
}
