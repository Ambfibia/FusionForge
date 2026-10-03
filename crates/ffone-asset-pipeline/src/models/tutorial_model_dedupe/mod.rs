use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, error::io_at,
};

pub const TUTORIAL_CHARACTER_MODEL_DEDUPE_SCHEMA: &str =
    "ffone.tutorial-character-model-dedupe-report.v1";

const DUPLICATE_PACKAGES: &[DuplicatePackage] = &[
    duplicate(
        "mob_oilmonster",
        "tutorial/models/mob/mob_oilmonster",
        "characters/mobs/mob_oilmonster",
    ),
    duplicate(
        "npc_dexbot1",
        "tutorial/models/mob/npc_dexbot1",
        "characters/npcs/npc_dexbot1",
    ),
    duplicate(
        "npc_dexter",
        "tutorial/models/mob/npc_dexter",
        "characters/npcs/npc_dexter",
    ),
    duplicate(
        "npc_samuraijack",
        "tutorial/models/mob/npc_samuraijack",
        "characters/npcs/npc_samuraijack",
    ),
    duplicate(
        "objectnpc1",
        "tutorial/models/mob/objectnpc1",
        "characters/npcs/objectnpc1",
    ),
    duplicate(
        "t_blossom",
        "tutorial/models/mob/t_blossom",
        "characters/npcs/t_blossom",
    ),
    duplicate(
        "t_bubbles",
        "tutorial/models/mob/t_bubbles",
        "characters/npcs/t_bubbles",
    ),
    duplicate(
        "t_buttercup",
        "tutorial/models/mob/t_buttercup",
        "characters/npcs/t_buttercup",
    ),
    duplicate(
        "t_dexterpistol",
        "tutorial/models/mob/t_dexterpistol",
        "characters/npcs/t_dexterpistol",
    ),
    duplicate(
        "t_dextersword",
        "tutorial/models/mob/t_dextersword",
        "characters/npcs/t_dextersword",
    ),
    duplicate(
        "t_numbuhone",
        "tutorial/models/mob/t_numbuhone",
        "characters/npcs/t_numbuhone",
    ),
    duplicate(
        "t_numbuhtwo",
        "tutorial/models/mob/t_numbuhtwo",
        "characters/npcs/t_numbuhtwo",
    ),
];

#[derive(Clone, Copy, Debug)]
struct DuplicatePackage {
    id: &'static str,
    redundant_root: &'static str,
    canonical_root: &'static str,
}

const fn duplicate(
    id: &'static str,
    redundant_root: &'static str,
    canonical_root: &'static str,
) -> DuplicatePackage {
    DuplicatePackage {
        id,
        redundant_root,
        canonical_root,
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialCharacterModelDedupeMode {
    DryRun,
    Apply,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialCharacterModelDedupeStatus {
    RemoveExactDuplicate,
    AlreadyDeduplicated,
}

#[derive(Clone, Debug)]
pub struct TutorialCharacterModelDedupeOptions {
    pub asset_root: PathBuf,
    pub apply: bool,
}

impl TutorialCharacterModelDedupeOptions {
    #[must_use]
    pub fn new(asset_root: impl Into<PathBuf>) -> Self {
        Self {
            asset_root: asset_root.into(),
            apply: false,
        }
    }

    #[must_use]
    pub fn with_apply(mut self, apply: bool) -> Self {
        self.apply = apply;
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialCharacterModelDedupePackage {
    pub id: String,
    pub redundant_root: String,
    pub canonical_root: String,
    pub status: TutorialCharacterModelDedupeStatus,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialCharacterModelDedupeCounts {
    pub proven_packages: u64,
    pub removed_packages: u64,
    pub already_deduplicated_packages: u64,
    pub removed_files: u64,
    pub removed_bytes: u64,
    pub manifest_files_before: u64,
    pub manifest_files_after: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialCharacterModelDedupeReport {
    pub schema: String,
    pub mode: TutorialCharacterModelDedupeMode,
    pub asset_root: String,
    pub requires_asset_index_regeneration: bool,
    pub counts: TutorialCharacterModelDedupeCounts,
    pub packages: Vec<TutorialCharacterModelDedupePackage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    bytes: u64,
    blake3: String,
}

#[derive(Debug)]
struct Removal {
    id: &'static str,
    redundant_root: &'static str,
}

#[derive(Debug)]
struct DedupePlan {
    manifest_path: PathBuf,
    manifest_before: Vec<u8>,
    next_manifest: ProjectAssetManifest,
    packages: Vec<TutorialCharacterModelDedupePackage>,
    removals: Vec<Removal>,
    counts: TutorialCharacterModelDedupeCounts,
}

pub fn dedupe_tutorial_character_models(
    options: &TutorialCharacterModelDedupeOptions,
) -> Result<TutorialCharacterModelDedupeReport> {
    let plan = build_plan(&options.asset_root, DUPLICATE_PACKAGES)?;
    let mode = if options.apply {
        TutorialCharacterModelDedupeMode::Apply
    } else {
        TutorialCharacterModelDedupeMode::DryRun
    };
    if options.apply && !plan.removals.is_empty() {
        apply_plan(&options.asset_root, &plan)?;
    }
    Ok(TutorialCharacterModelDedupeReport {
        schema: TUTORIAL_CHARACTER_MODEL_DEDUPE_SCHEMA.to_owned(),
        mode,
        asset_root: options.asset_root.display().to_string(),
        requires_asset_index_regeneration: !plan.removals.is_empty(),
        counts: plan.counts,
        packages: plan.packages,
    })
}

fn build_plan(asset_root: &Path, mappings: &[DuplicatePackage]) -> Result<DedupePlan> {
    let root_metadata =
        fs::symlink_metadata(asset_root).map_err(|source| io_at(asset_root, source))?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(dedupe_error(format!(
            "asset root must be a regular directory: {}",
            asset_root.display()
        )));
    }
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_before =
        fs::read(&manifest_path).map_err(|source| io_at(&manifest_path, source))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_before).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return Err(dedupe_error(format!(
            "unsupported schema {:?}; expected {:?}",
            manifest.schema, PROJECT_ASSET_SCHEMA
        )));
    }
    let manifest_by_path = manifest_index(&manifest)?;
    let mut next_manifest = manifest.clone();
    let mut packages = Vec::with_capacity(mappings.len());
    let mut removals = Vec::new();
    let mut removed_paths = BTreeSet::new();
    let mut counts = TutorialCharacterModelDedupeCounts {
        manifest_files_before: manifest.files.len() as u64,
        ..Default::default()
    };
    reject_runtime_references(asset_root, &manifest, mappings)?;

    for mapping in mappings {
        validate_manifest_root(mapping.redundant_root)?;
        validate_manifest_root(mapping.canonical_root)?;
        let canonical = collect_tree(&asset_root.join(native_path(mapping.canonical_root)))?;
        if canonical.is_empty() {
            return Err(dedupe_error(format!(
                "canonical package {:?} is empty",
                mapping.canonical_root
            )));
        }
        validate_tree_manifest(mapping.canonical_root, &canonical, &manifest_by_path)?;

        let redundant_path = asset_root.join(native_path(mapping.redundant_root));
        let redundant_exists = redundant_path
            .try_exists()
            .map_err(|source| io_at(&redundant_path, source))?;
        let redundant_manifest_paths = manifest_paths_below(&manifest, mapping.redundant_root);
        if !redundant_exists && redundant_manifest_paths.is_empty() {
            counts.already_deduplicated_packages += 1;
            counts.proven_packages += 1;
            packages.push(TutorialCharacterModelDedupePackage {
                id: mapping.id.to_owned(),
                redundant_root: mapping.redundant_root.to_owned(),
                canonical_root: mapping.canonical_root.to_owned(),
                status: TutorialCharacterModelDedupeStatus::AlreadyDeduplicated,
                files: canonical.len() as u64,
                bytes: canonical.values().map(|identity| identity.bytes).sum(),
            });
            continue;
        }
        if redundant_exists != !redundant_manifest_paths.is_empty() {
            return Err(dedupe_error(format!(
                "redundant package {:?} has inconsistent disk/manifest state: disk={}, manifestFiles={}",
                mapping.redundant_root,
                redundant_exists,
                redundant_manifest_paths.len()
            )));
        }

        let redundant = collect_tree(&redundant_path)?;
        validate_tree_manifest(mapping.redundant_root, &redundant, &manifest_by_path)?;
        if redundant != canonical {
            return Err(dedupe_error(describe_tree_difference(
                mapping, &redundant, &canonical,
            )));
        }
        validate_matching_manifest_kinds(mapping, &redundant, &manifest_by_path)?;
        let files = redundant.len() as u64;
        let bytes = redundant
            .values()
            .map(|identity| identity.bytes)
            .sum::<u64>();
        counts.proven_packages += 1;
        counts.removed_packages += 1;
        counts.removed_files += files;
        counts.removed_bytes = counts
            .removed_bytes
            .checked_add(bytes)
            .ok_or_else(|| dedupe_error("removed byte total overflow"))?;
        packages.push(TutorialCharacterModelDedupePackage {
            id: mapping.id.to_owned(),
            redundant_root: mapping.redundant_root.to_owned(),
            canonical_root: mapping.canonical_root.to_owned(),
            status: TutorialCharacterModelDedupeStatus::RemoveExactDuplicate,
            files,
            bytes,
        });
        removals.push(Removal {
            id: mapping.id,
            redundant_root: mapping.redundant_root,
        });
        removed_paths.extend(redundant_manifest_paths);
    }

    next_manifest
        .files
        .retain(|entry| !removed_paths.contains(&entry.path));
    counts.manifest_files_after = next_manifest.files.len() as u64;
    if counts.manifest_files_before - counts.manifest_files_after != counts.removed_files {
        return Err(dedupe_error(format!(
            "planned manifest removal mismatch: entries={}, files={}",
            counts.manifest_files_before - counts.manifest_files_after,
            counts.removed_files
        )));
    }

    Ok(DedupePlan {
        manifest_path,
        manifest_before,
        next_manifest,
        packages,
        removals,
        counts,
    })
}

fn apply_plan(asset_root: &Path, plan: &DedupePlan) -> Result<()> {
    let current =
        fs::read(&plan.manifest_path).map_err(|source| io_at(&plan.manifest_path, source))?;
    if current != plan.manifest_before {
        return Err(dedupe_error(
            "asset manifest changed after the dedupe plan was validated",
        ));
    }

    let staging = asset_root.join(format!(
        ".tutorial-model-dedupe-{}-{}",
        std::process::id(),
        unique_stamp()
    ));
    fs::create_dir(&staging).map_err(|source| io_at(&staging, source))?;
    let mut moved = Vec::<(&Removal, PathBuf, PathBuf)>::new();
    for removal in &plan.removals {
        let source = asset_root.join(native_path(removal.redundant_root));
        let destination = staging.join(removal.id);
        if let Err(source_error) = fs::rename(&source, &destination) {
            rollback_moves(&moved);
            let _ = fs::remove_dir(&staging);
            return Err(io_at(&source, source_error));
        }
        moved.push((removal, source, destination));
    }

    let mut manifest_bytes =
        serde_json::to_vec_pretty(&plan.next_manifest).map_err(|source| PipelineError::Json {
            path: plan.manifest_path.display().to_string(),
            source,
        })?;
    manifest_bytes.push(b'\n');
    if let Err(error) = replace_manifest_transactionally(&plan.manifest_path, &manifest_bytes) {
        rollback_moves(&moved);
        let _ = fs::remove_dir(&staging);
        return Err(error);
    }
    fs::remove_dir_all(&staging).map_err(|source| {
        dedupe_error(format!(
            "manifest committed, but exact-duplicate staging directory {} could not be removed: {source}",
            staging.display()
        ))
    })?;
    Ok(())
}

fn rollback_moves(moved: &[(&Removal, PathBuf, PathBuf)]) {
    for (_, source, destination) in moved.iter().rev() {
        let _ = fs::rename(destination, source);
    }
}

fn manifest_index(manifest: &ProjectAssetManifest) -> Result<BTreeMap<String, &ProjectAssetFile>> {
    let mut by_path = BTreeMap::new();
    for entry in &manifest.files {
        validate_manifest_path(&entry.path)?;
        if by_path.insert(entry.path.clone(), entry).is_some() {
            return Err(dedupe_error(format!(
                "duplicate project-asset manifest path {:?}",
                entry.path
            )));
        }
    }
    Ok(by_path)
}

fn manifest_paths_below(manifest: &ProjectAssetManifest, root: &str) -> BTreeSet<String> {
    let prefix = format!("{root}/");
    manifest
        .files
        .iter()
        .filter(|entry| entry.path.starts_with(&prefix))
        .map(|entry| entry.path.clone())
        .collect()
}

fn validate_tree_manifest(
    root: &str,
    tree: &BTreeMap<String, FileIdentity>,
    manifest: &BTreeMap<String, &ProjectAssetFile>,
) -> Result<()> {
    let manifest_paths = manifest
        .keys()
        .filter(|path| path.starts_with(&format!("{root}/")))
        .cloned()
        .collect::<BTreeSet<_>>();
    let disk_paths = tree
        .keys()
        .map(|relative| format!("{root}/{relative}"))
        .collect::<BTreeSet<_>>();
    if manifest_paths != disk_paths {
        return Err(dedupe_error(format!(
            "package {root:?} disk/manifest file set differs: disk={}, manifest={}",
            disk_paths.len(),
            manifest_paths.len()
        )));
    }
    for (relative, identity) in tree {
        let path = format!("{root}/{relative}");
        let entry = manifest
            .get(&path)
            .ok_or_else(|| dedupe_error(format!("missing manifest entry {path:?}")))?;
        if entry.bytes != identity.bytes || entry.blake3 != identity.blake3 {
            return Err(dedupe_error(format!(
                "manifest identity mismatch for {path:?}: manifest=({}, {}), disk=({}, {})",
                entry.bytes, entry.blake3, identity.bytes, identity.blake3
            )));
        }
    }
    Ok(())
}

fn validate_matching_manifest_kinds(
    mapping: &DuplicatePackage,
    redundant: &BTreeMap<String, FileIdentity>,
    manifest: &BTreeMap<String, &ProjectAssetFile>,
) -> Result<()> {
    for relative in redundant.keys() {
        let redundant_path = format!("{}/{relative}", mapping.redundant_root);
        let canonical_path = format!("{}/{relative}", mapping.canonical_root);
        let redundant_entry = manifest[&redundant_path];
        let canonical_entry = manifest[&canonical_path];
        if redundant_entry.kind != canonical_entry.kind {
            return Err(dedupe_error(format!(
                "manifest kind differs for exact duplicate {relative:?}: redundant={:?}, canonical={:?}",
                redundant_entry.kind, canonical_entry.kind
            )));
        }
    }
    Ok(())
}

fn reject_runtime_references(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    mappings: &[DuplicatePackage],
) -> Result<()> {
    let prefixes = mappings
        .iter()
        .map(|mapping| format!("{}/", mapping.redundant_root))
        .collect::<Vec<_>>();
    for entry in manifest
        .files
        .iter()
        .filter(|entry| entry.kind == ProjectAssetKind::Data)
    {
        let path = asset_root.join(native_path(&entry.path));
        let bytes = fs::read(&path).map_err(|source| io_at(&path, source))?;
        if let Ok(text) = std::str::from_utf8(&bytes) {
            for prefix in &prefixes {
                if text.contains(prefix) {
                    return Err(dedupe_error(format!(
                        "active data asset {:?} still references redundant package prefix {:?}",
                        entry.path, prefix
                    )));
                }
            }
        }
    }
    Ok(())
}

fn collect_tree(root: &Path) -> Result<BTreeMap<String, FileIdentity>> {
    let metadata = fs::symlink_metadata(root).map_err(|source| io_at(root, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(dedupe_error(format!(
            "package root must be a regular directory: {}",
            root.display()
        )));
    }
    let mut files = BTreeMap::new();
    collect_tree_at(root, root, &mut files)?;
    Ok(files)
}

fn collect_tree_at(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<String, FileIdentity>,
) -> Result<()> {
    let mut entries = fs::read_dir(current)
        .map_err(|source| io_at(current, source))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| io_at(current, source))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| io_at(&path, source))?;
        if metadata.file_type().is_symlink() {
            return Err(dedupe_error(format!(
                "symlinks are forbidden in model packages: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_tree_at(root, &path, files)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(dedupe_error(format!(
                "unsupported filesystem entry in model package: {}",
                path.display()
            )));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| dedupe_error(format!("file escaped package root: {}", path.display())))?;
        let relative = manifest_path(relative)?;
        let identity = FileIdentity {
            bytes: metadata.len(),
            blake3: hash_file(&path)?,
        };
        if files.insert(relative.clone(), identity).is_some() {
            return Err(dedupe_error(format!(
                "duplicate package-relative path {relative:?}"
            )));
        }
    }
    Ok(())
}

fn describe_tree_difference(
    mapping: &DuplicatePackage,
    redundant: &BTreeMap<String, FileIdentity>,
    canonical: &BTreeMap<String, FileIdentity>,
) -> String {
    let paths = redundant
        .keys()
        .chain(canonical.keys())
        .collect::<BTreeSet<_>>();
    let first = paths
        .into_iter()
        .find(|path| redundant.get(*path) != canonical.get(*path));
    format!(
        "refusing to remove {:?}: package differs from {:?} at {:?}",
        mapping.redundant_root, mapping.canonical_root, first
    )
}

fn validate_manifest_root(path: &str) -> Result<()> {
    validate_manifest_path(path)?;
    if path.ends_with('/') {
        return Err(dedupe_error(format!(
            "package root has a trailing slash: {path:?}"
        )));
    }
    Ok(())
}

fn validate_manifest_path(path: &str) -> Result<()> {
    if path.is_empty() || path.contains('\\') {
        return Err(dedupe_error(format!("invalid manifest path {path:?}")));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(dedupe_error(format!("unsafe manifest path {path:?}")));
    }
    Ok(())
}

fn native_path(path: &str) -> PathBuf {
    path.split('/').fold(PathBuf::new(), |mut output, part| {
        output.push(part);
        output
    })
}

fn manifest_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(dedupe_error(format!(
                "unsafe package-relative path {}",
                path.display()
            )));
        };
        parts.push(
            part.to_str()
                .ok_or_else(|| dedupe_error("package path is not valid UTF-8"))?,
        );
    }
    Ok(parts.join("/"))
}

use crate::shared::hash_file;

fn replace_manifest_transactionally(manifest_path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = manifest_path.parent().ok_or_else(|| {
        dedupe_error(format!(
            "manifest path has no parent: {}",
            manifest_path.display()
        ))
    })?;
    let stamp = format!("{}-{}", std::process::id(), unique_stamp());
    let next_path = parent.join(format!(".asset-manifest.next-{stamp}.json"));
    let backup_path = parent.join(format!(".asset-manifest.backup-{stamp}.json"));
    let mut next = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&next_path)
        .map_err(|source| io_at(&next_path, source))?;
    next.write_all(bytes)
        .map_err(|source| io_at(&next_path, source))?;
    next.sync_all()
        .map_err(|source| io_at(&next_path, source))?;
    drop(next);

    fs::rename(manifest_path, &backup_path).map_err(|source| io_at(manifest_path, source))?;
    if let Err(source) = fs::rename(&next_path, manifest_path) {
        let _ = fs::rename(&backup_path, manifest_path);
        let _ = fs::remove_file(&next_path);
        return Err(io_at(manifest_path, source));
    }
    fs::remove_file(&backup_path).map_err(|source| io_at(&backup_path, source))
}

fn unique_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn dedupe_error(message: impl Into<String>) -> PipelineError {
    PipelineError::ProjectAssetManifest(format!(
        "tutorial character-model dedupe failed: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests;
