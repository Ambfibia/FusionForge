use super::*;

pub const SEMANTIC_CHARACTER_CATALOG_SCHEMA: &str = "ffone.semantic-character-catalog.v1";

pub const SEMANTIC_CHARACTER_CATALOG_PATH: &str = "characters/catalog.json";

pub const CHARACTER_SCHEMA_UPGRADE_REPORT_PATH: &str = "characters/schema-upgrade-violations.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticCharacterCatalog {
    pub schema: String,
    pub status: String,
    pub runtime_accepted: bool,
    pub production_approved: bool,
    pub visual_parity_pending: bool,
    pub schema_upgrade_required: bool,
    /// External URI normalization into `model/`, `textures/` and `materials/`
    /// must be re-audited before replacing the byte-exact archived GLB closure.
    pub readable_sidecar_layout_pending: bool,
    pub source_build: String,
    pub coordinate_contract: String,
    pub runtime_spawn_policy: String,
    pub proofs: SemanticCharacterProofs,
    pub models: Vec<SemanticCharacterModel>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CharacterRoute {
    pub(super) source_directory: String,
    pub(super) destination_directory: String,
}

pub(super) fn replace_manifest(asset_root: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let path = asset_root.join(ASSET_MANIFEST_FILE);
    let next = asset_root.join(".asset-manifest.logical-characters.next");
    let backup = asset_root.join(".asset-manifest.logical-characters.backup");
    if fs::symlink_metadata(&next).is_ok() || fs::symlink_metadata(&backup).is_ok() {
        return invalid("stale logical-character manifest transaction files exist");
    }
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)
        .map_err(|error| io_at(&next, error))?;
    output
        .write_all(&bytes)
        .map_err(|error| io_at(&next, error))?;
    output.sync_all().map_err(|error| io_at(&next, error))?;
    fs::rename(&path, &backup).map_err(|error| io_at(&path, error))?;
    if let Err(error) = fs::rename(&next, &path) {
        let _ = fs::rename(&backup, &path);
        return Err(io_at(&path, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}

pub(super) fn reserve_path(
    existing: &BTreeSet<String>,
    planned: &mut BTreeSet<String>,
    path: &str,
) -> Result<()> {
    let folded = path.to_ascii_lowercase();
    if existing.contains(&folded) || !planned.insert(folded) {
        return invalid(format!("project asset path collision at {path:?}"));
    }
    Ok(())
}

pub(super) fn destination_file_path(source: &str, routes: &[CharacterRoute]) -> Result<String> {
    validate_relative(source)?;
    let mut matches = routes.iter().filter_map(|route| {
        source
            .strip_prefix(&format!("{}/", route.source_directory))
            .map(|tail| (route, tail))
    });
    let (route, tail) = matches
        .next()
        .ok_or_else(|| invalid_error(format!("no semantic character route owns {source:?}")))?;
    if matches.next().is_some() {
        return invalid(format!("multiple semantic character routes own {source:?}"));
    }
    validate_relative(tail)?;
    Ok(format!("{}/{tail}", route.destination_directory))
}

pub(super) fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid_error("candidate path escaped its root"))?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => parts.push(
                value
                    .to_str()
                    .ok_or_else(|| invalid_error("candidate path is not UTF-8"))?,
            ),
            _ => return invalid("candidate path contains a non-normal component"),
        }
    }
    Ok(parts.join("/"))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
