use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DetailRoutes {
    pub(super) document_path: String,
    pub(super) document_blake3: String,
    pub(super) texture_path: String,
}

#[derive(Clone, Debug)]
pub(super) struct PackageFile {
    pub(super) member: String,
    pub(super) asset_path: String,
    pub(super) absolute: PathBuf,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Debug)]
pub(super) struct DetailPackage {
    pub(super) true_texture_name: String,
    pub(super) document_path: String,
    pub(super) package_relative: String,
    pub(super) closure_blake3: String,
    pub(super) files: Vec<PackageFile>,
    pub(super) document: JsonValue,
    pub(super) base_path: String,
    pub(super) mip_zero_path: String,
    pub(super) base_is_mip_zero: bool,
    pub(super) document_hash_was_stale: bool,
}

#[derive(Clone, Debug)]
pub(super) struct TileSource {
    pub(super) tile_id: String,
    pub(super) tile_route: String,
    pub(super) terrain_root: PathBuf,
    pub(super) terrain: JsonValue,
    pub(super) scene: JsonValue,
    pub(super) manifest: JsonValue,
    pub(super) packages: Vec<DetailPackage>,
}

#[derive(Debug)]
pub(super) struct MigrationPlan {
    pub(super) source_catalog_blake3: String,
    pub(super) result_catalog_blake3: String,
    pub(super) replacements: BTreeMap<String, Vec<u8>>,
    pub(super) additions: BTreeMap<String, Vec<u8>>,
    pub(super) removals: BTreeSet<String>,
    pub(super) counts: PublishedTerrainDedupCounts,
    pub(super) routes: Vec<PublishedTerrainDetailRoute>,
}

#[derive(Debug)]
pub(super) struct TransactionLock {
    pub(super) path: PathBuf,
}

impl Drop for TransactionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) struct RewrittenTile {
    pub(super) files: BTreeMap<String, Vec<u8>>,
    pub(super) manifest_artifact: JsonValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PhysicalArtifact {
    pub(super) bytes: u64,
    pub(super) blake3: String,
}
