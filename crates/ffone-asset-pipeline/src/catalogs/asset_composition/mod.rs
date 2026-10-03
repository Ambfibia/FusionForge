//! Atomic composition of independently proven native asset roots.
//!
//! A composition never mutates either input. The base owns every path except
//! the explicitly declared overlay prefixes; the overlay owns every path below
//! those prefixes. This makes audio/UI upgrades deterministic without copying a
//! recovered raw tree over hand-authored semantic world and character assets.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::Serialize;

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, SourcePackIdentity, error::io_at,
};

pub const ASSET_COMPOSITION_SCHEMA: &str = "ffone.asset-composition.v1";
pub const ASSET_COMPOSITION_REPORT: &str = "data/catalog/asset-composition.json";
const REPORT_FIXED_POINT_LIMIT: usize = 32;
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct AssetCompositionOptions {
    pub base_root: PathBuf,
    pub overlay_root: PathBuf,
    pub output_root: PathBuf,
    pub overlay_prefixes: Vec<String>,
}

impl AssetCompositionOptions {
    pub fn new(
        base_root: impl Into<PathBuf>,
        overlay_root: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            base_root: base_root.into(),
            overlay_root: overlay_root.into(),
            output_root: output_root.into(),
            overlay_prefixes: Vec::new(),
        }
    }

    pub fn with_overlay_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.overlay_prefixes.push(prefix.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetCompositionReport {
    pub schema: &'static str,
    pub base_manifest_blake3: String,
    pub overlay_manifest_blake3: String,
    pub overlay_prefixes: Vec<String>,
    pub base_files_retained: u64,
    pub base_files_replaced: u64,
    pub overlay_files_selected: u64,
    pub resolved_path_conflicts: u64,
    pub output_files: u64,
    pub output_bytes: u64,
    pub hard_linked_files: u64,
    pub copied_files: u64,
}

#[derive(Clone)]
struct SelectedAsset {
    entry: ProjectAssetFile,
    root: PathBuf,
}

pub fn compose_asset_roots(options: &AssetCompositionOptions) -> Result<AssetCompositionReport> {
    let base_root =
        fs::canonicalize(&options.base_root).map_err(|error| io_at(&options.base_root, error))?;
    let overlay_root = fs::canonicalize(&options.overlay_root)
        .map_err(|error| io_at(&options.overlay_root, error))?;
    if base_root == overlay_root {
        return invalid("base and overlay asset roots must be different");
    }

    let output_name = options
        .output_root
        .file_name()
        .ok_or_else(|| PipelineError::InvalidOutputPath(options.output_root.clone()))?;
    let output_parent = options
        .output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let output_parent =
        fs::canonicalize(output_parent).map_err(|error| io_at(output_parent, error))?;
    let output_root = output_parent.join(output_name);
    if output_root.exists() {
        return Err(PipelineError::OutputExists(output_root));
    }
    if output_root.starts_with(&base_root) || output_root.starts_with(&overlay_root) {
        return invalid("composition output must not be inside either input asset root");
    }

    let prefixes = normalize_prefixes(&options.overlay_prefixes)?;
    let (base_manifest, base_manifest_bytes) = read_manifest(&base_root)?;
    let (overlay_manifest, overlay_manifest_bytes) = read_manifest(&overlay_root)?;
    validate_compatible_manifests(&base_manifest, &overlay_manifest)?;

    let mut stage = StagingDirectory::create(&output_parent, output_name)?;
    let report = stage_composition(
        stage.path(),
        &base_root,
        &base_manifest,
        &base_manifest_bytes,
        &overlay_root,
        &overlay_manifest,
        &overlay_manifest_bytes,
        &prefixes,
    )?;
    stage.publish(&output_root)?;
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn stage_composition(
    stage: &Path,
    base_root: &Path,
    base_manifest: &ProjectAssetManifest,
    base_manifest_bytes: &[u8],
    overlay_root: &Path,
    overlay_manifest: &ProjectAssetManifest,
    overlay_manifest_bytes: &[u8],
    prefixes: &[String],
) -> Result<AssetCompositionReport> {
    let mut selected = BTreeMap::<String, SelectedAsset>::new();
    let mut base_owned_paths = BTreeSet::new();
    let mut base_files_retained = 0_u64;
    let mut base_files_replaced = 0_u64;

    for entry in &base_manifest.files {
        let folded = fold_path(&entry.path);
        if is_overlay_owned(&folded, prefixes) {
            base_files_replaced += 1;
            base_owned_paths.insert(folded);
        } else {
            insert_selected(&mut selected, entry, base_root)?;
            base_files_retained += 1;
        }
    }

    let mut prefix_coverage = vec![0_u64; prefixes.len()];
    let mut overlay_files_selected = 0_u64;
    let mut resolved_path_conflicts = 0_u64;
    for entry in &overlay_manifest.files {
        let folded = fold_path(&entry.path);
        let Some(prefix_index) = prefixes
            .iter()
            .position(|prefix| folded.starts_with(prefix))
        else {
            continue;
        };
        prefix_coverage[prefix_index] += 1;
        if base_owned_paths.contains(&folded) {
            resolved_path_conflicts += 1;
        }
        insert_selected(&mut selected, entry, overlay_root)?;
        overlay_files_selected += 1;
    }
    for (prefix, count) in prefixes.iter().zip(prefix_coverage) {
        if count == 0 {
            return invalid(format!(
                "overlay manifest contains no files below required prefix {prefix:?}"
            ));
        }
    }
    if selected.contains_key(&fold_path(ASSET_COMPOSITION_REPORT)) {
        return invalid(format!(
            "selected inputs already own reserved composition report path {ASSET_COMPOSITION_REPORT:?}"
        ));
    }
    if selected.contains_key(&fold_path(ASSET_MANIFEST_FILE)) {
        return invalid(format!(
            "selected inputs already own reserved project manifest path {ASSET_MANIFEST_FILE:?}"
        ));
    }
    validate_selected_path_tree(&selected)?;

    let mut hard_linked_files = 0_u64;
    let mut copied_files = 0_u64;
    let mut selected_bytes = 0_u64;
    for selected_asset in selected.values() {
        let source = validate_source_asset(&selected_asset.root, &selected_asset.entry)?;
        let target = join_manifest_path(stage, &selected_asset.entry.path);
        let parent = target
            .parent()
            .ok_or_else(|| invalid_error("selected asset target has no parent"))?;
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
        if materialize_verified_asset(&source, &target, &selected_asset.entry)? {
            hard_linked_files += 1;
        } else {
            copied_files += 1;
        }
        selected_bytes = selected_bytes
            .checked_add(selected_asset.entry.bytes)
            .ok_or_else(|| invalid_error("composed asset byte total overflows u64"))?;
    }

    let mut report = AssetCompositionReport {
        schema: ASSET_COMPOSITION_SCHEMA,
        base_manifest_blake3: blake3::hash(base_manifest_bytes).to_hex().to_string(),
        overlay_manifest_blake3: blake3::hash(overlay_manifest_bytes).to_hex().to_string(),
        overlay_prefixes: prefixes.to_vec(),
        base_files_retained,
        base_files_replaced,
        overlay_files_selected,
        resolved_path_conflicts,
        output_files: u64::try_from(selected.len())
            .map_err(|_| invalid_error("composed asset count overflows u64"))?
            .checked_add(1)
            .ok_or_else(|| invalid_error("composed asset count overflows u64"))?,
        output_bytes: 0,
        hard_linked_files,
        copied_files,
    };
    let report_bytes = serialize_stable_report(&mut report, selected_bytes)?;
    let report_path = join_manifest_path(stage, ASSET_COMPOSITION_REPORT);
    let report_parent = report_path
        .parent()
        .ok_or_else(|| invalid_error("composition report target has no parent"))?;
    fs::create_dir_all(report_parent).map_err(|error| io_at(report_parent, error))?;
    write_new_file(&report_path, &report_bytes)?;

    let report_entry = ProjectAssetFile {
        source_path: "asset-composition/report".to_owned(),
        path: ASSET_COMPOSITION_REPORT.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: report_bytes.len() as u64,
        blake3: blake3::hash(&report_bytes).to_hex().to_string(),
    };
    let mut files = selected
        .into_values()
        .map(|asset| asset.entry)
        .collect::<Vec<_>>();
    files.push(report_entry);
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: base_manifest.protocol,
        locale: base_manifest.locale.clone(),
        source_pack: SourcePackIdentity {
            schema: ASSET_COMPOSITION_SCHEMA.to_owned(),
            manifest_blake3: blake3::hash(&report_bytes).to_hex().to_string(),
        },
        files,
    };
    let mut manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|source| PipelineError::Json {
            path: ASSET_MANIFEST_FILE.to_owned(),
            source,
        })?;
    manifest_bytes.push(b'\n');
    write_new_file(&stage.join(ASSET_MANIFEST_FILE), &manifest_bytes)?;
    Ok(report)
}

fn read_manifest(root: &Path) -> Result<(ProjectAssetManifest, Vec<u8>)> {
    let path = root.join(ASSET_MANIFEST_FILE);
    let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!(
            "project-asset manifest must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    let manifest = serde_json::from_slice::<ProjectAssetManifest>(&bytes).map_err(|source| {
        PipelineError::Json {
            path: path.display().to_string(),
            source,
        }
    })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "{} uses unsupported schema {:?}; expected {PROJECT_ASSET_SCHEMA:?}",
            path.display(),
            manifest.schema
        ));
    }
    validate_manifest_entries(&manifest)?;
    Ok((manifest, bytes))
}

fn validate_compatible_manifests(
    base: &ProjectAssetManifest,
    overlay: &ProjectAssetManifest,
) -> Result<()> {
    if base.protocol != overlay.protocol {
        return invalid(format!(
            "base/overlay protocol mismatch: {} != {}",
            base.protocol, overlay.protocol
        ));
    }
    if base.locale != overlay.locale {
        return invalid(format!(
            "base/overlay locale mismatch: {:?} != {:?}",
            base.locale, overlay.locale
        ));
    }
    Ok(())
}

fn normalize_prefixes(prefixes: &[String]) -> Result<Vec<String>> {
    if prefixes.is_empty() {
        return invalid("at least one overlay prefix is required");
    }
    let mut normalized = BTreeSet::new();
    for prefix in prefixes {
        if !is_safe_relative_directory(prefix) {
            return invalid(format!(
                "overlay prefix must be a safe slash-terminated relative directory: {prefix:?}"
            ));
        }
        let folded = fold_path(prefix);
        if normalized
            .iter()
            .any(|existing: &String| folded.starts_with(existing) || existing.starts_with(&folded))
        {
            return invalid(format!("overlay prefixes must not overlap: {prefixes:?}"));
        }
        normalized.insert(folded);
    }
    Ok(normalized.into_iter().collect())
}

fn is_overlay_owned(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

fn insert_selected(
    selected: &mut BTreeMap<String, SelectedAsset>,
    entry: &ProjectAssetFile,
    root: &Path,
) -> Result<()> {
    let folded = fold_path(&entry.path);
    if let Some(previous) = selected.get(&folded) {
        return Err(PipelineError::OutputCollision {
            output: entry.path.clone(),
            first: previous.entry.source_path.clone(),
            second: entry.source_path.clone(),
        });
    }
    selected.insert(
        folded,
        SelectedAsset {
            entry: entry.clone(),
            root: root.to_owned(),
        },
    );
    Ok(())
}

fn validate_manifest_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe project-asset path {path:?}"));
    }
    Ok(())
}

fn validate_manifest_entries(manifest: &ProjectAssetManifest) -> Result<()> {
    let mut paths = BTreeMap::<String, &ProjectAssetFile>::new();
    for entry in &manifest.files {
        validate_manifest_path(&entry.path)?;
        let folded = fold_path(&entry.path);
        if let Some(previous) = paths.insert(folded, entry) {
            return Err(PipelineError::OutputCollision {
                output: entry.path.clone(),
                first: previous.source_path.clone(),
                second: entry.source_path.clone(),
            });
        }
    }
    Ok(())
}

fn validate_selected_path_tree(selected: &BTreeMap<String, SelectedAsset>) -> Result<()> {
    for (folded, asset) in selected {
        for (separator, _) in folded.match_indices('/') {
            if let Some(parent_asset) = selected.get(&folded[..separator]) {
                return Err(PipelineError::OutputCollision {
                    output: asset.entry.path.clone(),
                    first: parent_asset.entry.source_path.clone(),
                    second: asset.entry.source_path.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_source_asset(root: &Path, entry: &ProjectAssetFile) -> Result<PathBuf> {
    let path = join_manifest_path(root, &entry.path);
    validate_asset_file(&path, entry)?;
    Ok(path)
}

fn validate_asset_file(path: &Path, entry: &ProjectAssetFile) -> Result<()> {
    let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!(
            "composition source must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    if metadata.len() != entry.bytes {
        return invalid(format!(
            "asset length mismatch for {}: manifest={}, disk={}",
            path.display(),
            entry.bytes,
            metadata.len()
        ));
    }
    let file = File::open(&path).map_err(|error| io_at(&path, error))?;
    let actual = hash_reader(file, &path)?;
    if actual != entry.blake3 {
        return invalid(format!(
            "asset BLAKE3 mismatch for {}: manifest={}, disk={actual}",
            path.display(),
            entry.blake3
        ));
    }
    Ok(())
}

fn materialize_verified_asset(
    source: &Path,
    target: &Path,
    entry: &ProjectAssetFile,
) -> Result<bool> {
    materialize_verified_asset_with(source, target, entry, |source, target| {
        fs::hard_link(source, target)
    })
}

fn materialize_verified_asset_with(
    source: &Path,
    target: &Path,
    entry: &ProjectAssetFile,
    hard_link: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<bool> {
    let hard_linked = match hard_link(source, target) {
        Ok(()) => true,
        Err(_) => {
            let copied = fs::copy(source, target).map_err(|error| io_at(target, error))?;
            if copied != entry.bytes {
                return invalid(format!(
                    "short asset copy for {:?}: expected {}, copied {copied}",
                    entry.path, entry.bytes
                ));
            }
            false
        }
    };
    validate_asset_file(target, entry)?;
    Ok(hard_linked)
}

fn hash_reader(mut reader: impl Read, path: &Path) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| io_at(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn join_manifest_path(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

fn fold_path(path: &str) -> String {
    path.to_lowercase()
}

fn is_safe_relative_directory(prefix: &str) -> bool {
    prefix.ends_with('/')
        && !prefix.starts_with('/')
        && !prefix.contains('\\')
        && prefix
            .strip_suffix('/')
            .is_some_and(|body| !body.is_empty() && body.split('/').all(is_safe_component))
        && Path::new(prefix)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_safe_component(component: &str) -> bool {
    !component.is_empty() && component != "." && component != ".." && !component.contains('\0')
}

fn serialize_stable_report(
    report: &mut AssetCompositionReport,
    selected_bytes: u64,
) -> Result<Vec<u8>> {
    for _ in 0..REPORT_FIXED_POINT_LIMIT {
        let mut bytes =
            serde_json::to_vec_pretty(&*report).map_err(|source| PipelineError::Json {
                path: ASSET_COMPOSITION_REPORT.to_owned(),
                source,
            })?;
        bytes.push(b'\n');
        let total = selected_bytes
            .checked_add(
                u64::try_from(bytes.len())
                    .map_err(|_| invalid_error("composition report length overflows u64"))?,
            )
            .ok_or_else(|| invalid_error("composed asset byte total overflows u64"))?;
        if report.output_bytes == total {
            return Ok(bytes);
        }
        report.output_bytes = total;
    }
    invalid("composition report byte total did not converge")
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}

struct StagingDirectory {
    path: PathBuf,
    published: bool,
}

impl StagingDirectory {
    fn create(parent: &Path, output_name: &std::ffi::OsStr) -> Result<Self> {
        for _ in 0..64 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{}.asset-composition-stage-{}-{sequence}",
                output_name.to_string_lossy(),
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_at(path, error)),
            }
        }
        Err(PipelineError::StagingCollision(parent.join(output_name)))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn publish(&mut self, output: &Path) -> Result<()> {
        if fs::symlink_metadata(output).is_ok() {
            return Err(PipelineError::OutputExists(output.to_owned()));
        }
        fs::rename(&self.path, output).map_err(|error| io_at(output, error))?;
        self.published = true;
        Ok(())
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::ProjectAssetManifest(message.into())
}

#[cfg(test)]
mod tests;
