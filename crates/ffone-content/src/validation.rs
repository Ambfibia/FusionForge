use std::{
    collections::HashSet,
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use crate::{
    CONTENT_PACK_SCHEMA, CONTENT_PROTOCOL, ContentError, ContentFileEntry, ContentManifest, Result,
    error::io_at,
    policy::{
        normalize_content_path, reject_legacy_json, reject_legacy_magic, validate_content_path,
    },
};

pub const MANIFEST_FILE: &str = "content-pack.json";

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn validate_pack(root: impl AsRef<Path>) -> Result<ContentManifest> {
    let root = canonical_pack_root(root.as_ref())?;

    let manifest_path = root.join(MANIFEST_FILE);
    let (mut manifest_file, _) = open_contained_regular_file(&root, MANIFEST_FILE)?;
    let mut manifest_bytes = Vec::new();
    manifest_file
        .read_to_end(&mut manifest_bytes)
        .map_err(|source| io_at(&manifest_path, source))?;
    let manifest: ContentManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| ContentError::Json {
            path: manifest_path.clone(),
            source,
        })?;
    validate_manifest_shape(&manifest)?;

    let actual_files = collect_pack_files_canonical(&root)?;
    let listed: HashSet<&str> = manifest
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    for actual in &actual_files {
        if actual != MANIFEST_FILE && !listed.contains(actual.as_str()) {
            return Err(ContentError::UnlistedPackFile(actual.clone()));
        }
    }

    for entry in &manifest.files {
        let inspected = inspect_native_file(&root, &entry.path)?;
        if inspected.bytes != entry.bytes {
            return Err(ContentError::SizeMismatch {
                path: entry.path.clone(),
                expected: entry.bytes,
                actual: inspected.bytes,
            });
        }
        if inspected.blake3 != entry.blake3 {
            return Err(ContentError::HashMismatch {
                path: entry.path.clone(),
                expected: entry.blake3.clone(),
                actual: inspected.blake3,
            });
        }
        validate_native_file_semantics(&root, &entry.path)?;
    }

    Ok(manifest)
}

pub(crate) fn validate_manifest_shape(manifest: &ContentManifest) -> Result<()> {
    if manifest.schema != CONTENT_PACK_SCHEMA {
        return Err(ContentError::WrongSchema {
            expected: CONTENT_PACK_SCHEMA,
            actual: manifest.schema.clone(),
        });
    }
    if manifest.protocol != CONTENT_PROTOCOL {
        return Err(ContentError::WrongProtocol {
            expected: CONTENT_PROTOCOL,
            actual: manifest.protocol,
        });
    }
    validate_locale(&manifest.locale)?;
    validate_provenance(manifest)?;

    if manifest.files.is_empty() {
        return Err(ContentError::EmptyPack);
    }

    let mut folded_paths = HashSet::with_capacity(manifest.files.len());
    for entry in &manifest.files {
        let normalized = normalize_content_path(&entry.path)?;
        if entry.path.eq_ignore_ascii_case(MANIFEST_FILE) {
            return Err(ContentError::UnsafePath {
                path: entry.path.clone(),
                reason: "the manifest filename is reserved",
            });
        }
        validate_digest(&format!("file {}", entry.path), &entry.blake3)?;
        if !folded_paths.insert(normalized) {
            return Err(ContentError::DuplicatePath(entry.path.clone()));
        }
    }
    for pair in manifest.files.windows(2) {
        if pair[0].path >= pair[1].path {
            return Err(ContentError::UnsortedFiles(pair[1].path.clone()));
        }
    }

    Ok(())
}

pub(crate) struct InspectedFile {
    pub bytes: u64,
    pub blake3: String,
}

pub(crate) fn inspect_native_file(root: &Path, relative: &str) -> Result<InspectedFile> {
    let (mut file, absolute) = open_contained_regular_file(root, relative)?;
    let mut hasher = blake3::Hasher::new();
    let mut bytes = 0_u64;
    let mut prefix = Vec::with_capacity(32);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| io_at(&absolute, source))?;
        if read == 0 {
            break;
        }
        if prefix.len() < 32 {
            let take = (32 - prefix.len()).min(read);
            prefix.extend_from_slice(&buffer[..take]);
            if prefix.len() >= 8 {
                reject_legacy_magic(relative, &prefix)?;
            }
        }
        hasher.update(&buffer[..read]);
        bytes += read as u64;
    }
    reject_legacy_magic(relative, &prefix)?;
    Ok(InspectedFile {
        bytes,
        blake3: hasher.finalize().to_hex().to_string(),
    })
}

pub(crate) fn validate_native_file_semantics(root: &Path, relative: &str) -> Result<()> {
    if !relative.to_ascii_lowercase().ends_with(".json") {
        return Ok(());
    }
    let (mut file, absolute) = open_contained_regular_file(root, relative)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| io_at(&absolute, source))?;
    reject_legacy_json(relative, &bytes)
}

pub(crate) fn read_verified_file(root: &Path, entry: &ContentFileEntry) -> Result<Vec<u8>> {
    let (file, absolute) = open_contained_regular_file(root, &entry.path)?;
    let limit = entry.bytes.saturating_add(1);
    let mut reader = file.take(limit);
    let initial_capacity = usize::try_from(entry.bytes.min(1024 * 1024)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(initial_capacity);
    reader
        .read_to_end(&mut bytes)
        .map_err(|source| io_at(&absolute, source))?;

    reject_legacy_magic(&entry.path, &bytes[..bytes.len().min(32)])?;
    let actual = bytes.len() as u64;
    if actual != entry.bytes {
        return Err(ContentError::SizeMismatch {
            path: entry.path.clone(),
            expected: entry.bytes,
            actual,
        });
    }
    let actual_digest = blake3::hash(&bytes).to_hex().to_string();
    if actual_digest != entry.blake3 {
        return Err(ContentError::HashMismatch {
            path: entry.path.clone(),
            expected: entry.blake3.clone(),
            actual: actual_digest,
        });
    }
    Ok(bytes)
}

pub(crate) fn collect_pack_files(root: &Path) -> Result<Vec<String>> {
    let root = canonical_pack_root(root)?;
    collect_pack_files_canonical(&root)
}

fn collect_pack_files_canonical(root: &Path) -> Result<Vec<String>> {
    ensure_canonical_root(root)?;
    let mut files = Vec::new();
    walk_directory(root, root, "", &mut files)?;
    files.sort();
    Ok(files)
}

pub(crate) fn write_manifest(root: &Path, manifest: &ContentManifest) -> Result<()> {
    validate_manifest_shape(manifest)?;
    let root = canonical_pack_root(root)?;
    let manifest_path = root.join(MANIFEST_FILE);
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| ContentError::Json {
        path: manifest_path.clone(),
        source,
    })?;
    bytes.push(b'\n');
    atomic_write_manifest(&root, &manifest_path, &bytes)
}

pub(crate) fn canonical_pack_root(root: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(root).map_err(|source| io_at(root, source))?;
    reject_link_or_reparse(root, &metadata)?;
    if !metadata.is_dir() {
        return Err(io_at(
            root,
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "content pack root is not a directory",
            ),
        ));
    }
    let canonical = fs::canonicalize(root).map_err(|source| io_at(root, source))?;
    ensure_canonical_root(&canonical)?;
    Ok(canonical)
}

fn ensure_canonical_root(root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(root).map_err(|source| io_at(root, source))?;
    reject_link_or_reparse(root, &metadata)?;
    if !metadata.is_dir() {
        return Err(io_at(
            root,
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "content pack root is not a directory",
            ),
        ));
    }
    Ok(())
}

fn walk_directory(
    root: &Path,
    absolute: &Path,
    relative: &str,
    files: &mut Vec<String>,
) -> Result<()> {
    let entries = fs::read_dir(absolute).map_err(|source| io_at(absolute, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| io_at(absolute, source))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|name| ContentError::NonUtf8Path(PathBuf::from(name)))?;
        let child_relative = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        validate_content_path(&child_relative)?;

        let child_absolute = entry.path();
        let metadata = fs::symlink_metadata(&child_absolute)
            .map_err(|source| io_at(&child_absolute, source))?;
        reject_link_or_reparse(&child_absolute, &metadata)?;
        let canonical_child =
            fs::canonicalize(&child_absolute).map_err(|source| io_at(&child_absolute, source))?;
        ensure_contained(root, &canonical_child, &child_relative)?;
        if metadata.is_dir() {
            walk_directory(root, &canonical_child, &child_relative, files)?;
        } else if metadata.is_file() {
            files.push(child_relative);
        } else {
            return Err(ContentError::NotAFile(child_relative));
        }
    }
    Ok(())
}

fn open_contained_regular_file(root: &Path, relative: &str) -> Result<(File, PathBuf)> {
    validate_content_path(relative)?;
    ensure_canonical_root(root)?;

    let mut candidate = root.to_path_buf();
    for component in relative.split('/') {
        candidate.push(component);
        let metadata = match fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(ContentError::MissingFile(relative.to_owned()));
            }
            Err(source) => return Err(io_at(&candidate, source)),
        };
        reject_link_or_reparse(&candidate, &metadata)?;
    }

    let canonical = fs::canonicalize(&candidate).map_err(|source| io_at(&candidate, source))?;
    ensure_contained(root, &canonical, relative)?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|source| io_at(&canonical, source))?;
    reject_link_or_reparse(&canonical, &metadata)?;
    if !metadata.is_file() {
        return Err(ContentError::NotAFile(relative.to_owned()));
    }
    let file = File::open(&canonical).map_err(|source| io_at(&canonical, source))?;
    Ok((file, canonical))
}

fn ensure_contained(root: &Path, candidate: &Path, relative: &str) -> Result<()> {
    if !candidate.starts_with(root) {
        return Err(ContentError::OutsidePack(relative.to_owned()));
    }
    Ok(())
}

fn reject_link_or_reparse(path: &Path, metadata: &Metadata) -> Result<()> {
    if metadata.file_type().is_symlink() {
        return Err(ContentError::Symlink(path.to_path_buf()));
    }
    #[cfg(windows)]
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(ContentError::ReparsePoint(path.to_path_buf()));
    }
    Ok(())
}

fn atomic_write_manifest(root: &Path, manifest_path: &Path, bytes: &[u8]) -> Result<()> {
    let (temporary_path, mut temporary) = create_temporary_file(root, "tmp")?;
    if let Err(source) = temporary
        .write_all(bytes)
        .and_then(|_| temporary.sync_all())
    {
        drop(temporary);
        let _ = fs::remove_file(&temporary_path);
        return Err(io_at(&temporary_path, source));
    }
    drop(temporary);

    let replace_result = replace_prepared_manifest(root, &temporary_path, manifest_path);
    if replace_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    replace_result
}

fn create_temporary_file(root: &Path, role: &str) -> Result<(PathBuf, File)> {
    for _ in 0..32 {
        let path = unique_sibling_path(root, role);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(io_at(&path, source)),
        }
    }
    let path = root.join(format!(".{MANIFEST_FILE}.temporary"));
    Err(io_at(
        &path,
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique manifest temporary file",
        ),
    ))
}

#[cfg(not(windows))]
fn replace_prepared_manifest(_root: &Path, temporary: &Path, manifest: &Path) -> Result<()> {
    fs::rename(temporary, manifest).map_err(|source| io_at(manifest, source))
}

#[cfg(windows)]
fn replace_prepared_manifest(root: &Path, temporary: &Path, manifest: &Path) -> Result<()> {
    let existing = match fs::symlink_metadata(manifest) {
        Ok(metadata) => Some(metadata),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => return Err(io_at(manifest, source)),
    };
    let Some(existing) = existing else {
        return fs::rename(temporary, manifest).map_err(|source| io_at(manifest, source));
    };
    reject_link_or_reparse(manifest, &existing)?;
    if !existing.is_file() {
        return Err(ContentError::NotAFile(MANIFEST_FILE.to_owned()));
    }

    let backup = unique_sibling_path(root, "backup");
    fs::rename(manifest, &backup).map_err(|source| io_at(manifest, source))?;
    match fs::rename(temporary, manifest) {
        Ok(()) => fs::remove_file(&backup).map_err(|source| io_at(&backup, source)),
        Err(replace) => match fs::rename(&backup, manifest) {
            Ok(()) => Err(io_at(manifest, replace)),
            Err(rollback) => Err(ContentError::ManifestRollbackFailed {
                manifest: manifest.to_path_buf(),
                backup,
                replace: replace.to_string(),
                rollback: rollback.to_string(),
            }),
        },
    }
}

fn unique_sibling_path(root: &Path, role: &str) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    root.join(format!(
        ".{MANIFEST_FILE}.{}.{}.{}",
        std::process::id(),
        sequence,
        role
    ))
}

fn validate_locale(locale: &str) -> Result<()> {
    let valid = !locale.is_empty()
        && locale.len() <= 35
        && locale.is_ascii()
        && locale.split('-').all(|part| {
            !part.is_empty()
                && part.len() <= 8
                && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    if !valid {
        return Err(ContentError::InvalidLocale(locale.to_owned()));
    }
    Ok(())
}

fn validate_provenance(manifest: &ContentManifest) -> Result<()> {
    let provenance = &manifest.provenance;
    if provenance.producer.is_empty() || provenance.sources.is_empty() {
        return Err(ContentError::MissingProvenance);
    }
    validate_neutral_label("producer", &provenance.producer)?;

    let mut folded_labels = HashSet::with_capacity(provenance.sources.len());
    for source in &provenance.sources {
        validate_neutral_label("source", &source.label)?;
        validate_digest(&format!("source {}", source.label), &source.blake3)?;
        if !folded_labels.insert(source.label.to_lowercase()) {
            return Err(ContentError::DuplicateSource(source.label.clone()));
        }
    }
    for pair in provenance.sources.windows(2) {
        if pair[0].label >= pair[1].label {
            return Err(ContentError::UnsortedSources(pair[1].label.clone()));
        }
    }
    Ok(())
}

fn validate_neutral_label(context: &'static str, label: &str) -> Result<()> {
    let valid = !label.is_empty()
        && label.len() <= 128
        && label.trim() == label
        && !label
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':'));
    if !valid {
        return Err(ContentError::InvalidProvenanceLabel {
            context,
            label: label.to_owned(),
        });
    }
    Ok(())
}

fn validate_digest(context: &str, digest: &str) -> Result<()> {
    let valid = digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !valid {
        return Err(ContentError::InvalidDigest {
            context: context.to_owned(),
            digest: digest.to_owned(),
        });
    }
    Ok(())
}
