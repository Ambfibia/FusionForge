use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use crate::{
    ContentError, ContentFileEntry, ContentKind, ContentManifest, Provenance, Result,
    policy::normalize_content_path,
    validation::{
        MANIFEST_FILE, canonical_pack_root, collect_pack_files, inspect_native_file,
        validate_native_file_semantics, validate_pack, write_manifest,
    },
};

/// Inventories an already-native staging directory and writes only its deterministic manifest.
///
/// The builder deliberately has no import or copy API. Every staged file must be explicitly
/// registered with a native [`ContentKind`], and every staged file is checked for legacy formats.
#[derive(Debug)]
pub struct ContentPackBuilder {
    locale: String,
    provenance: Provenance,
    registrations: BTreeMap<String, ContentKind>,
    normalized_registrations: BTreeSet<String>,
}

impl ContentPackBuilder {
    pub fn new(locale: impl Into<String>, provenance: Provenance) -> Self {
        Self {
            locale: locale.into(),
            provenance,
            registrations: BTreeMap::new(),
            normalized_registrations: BTreeSet::new(),
        }
    }

    pub fn register(&mut self, path: impl Into<String>, kind: ContentKind) -> Result<&mut Self> {
        let path = path.into();
        let normalized = normalize_content_path(&path)?;
        if path.eq_ignore_ascii_case(MANIFEST_FILE) {
            return Err(ContentError::UnsafePath {
                path,
                reason: "the manifest filename is reserved",
            });
        }
        if !self.normalized_registrations.insert(normalized) {
            return Err(ContentError::DuplicateRegistration(path));
        }
        self.registrations.insert(path, kind);
        Ok(self)
    }

    pub fn write(&self, staging_dir: impl AsRef<Path>) -> Result<ContentManifest> {
        let staging_dir = canonical_pack_root(staging_dir.as_ref())?;
        let staged_files = collect_pack_files(&staging_dir)?;
        let staged_set: BTreeSet<&str> = staged_files.iter().map(String::as_str).collect();

        for staged in &staged_files {
            if staged != MANIFEST_FILE && !self.registrations.contains_key(staged) {
                return Err(ContentError::UnregisteredStagingFile(staged.clone()));
            }
        }
        for registered in self.registrations.keys() {
            if !staged_set.contains(registered.as_str()) {
                return Err(ContentError::RegisteredFileMissing(registered.clone()));
            }
        }

        let mut files = Vec::with_capacity(self.registrations.len());
        for (path, kind) in &self.registrations {
            let inspected = inspect_native_file(&staging_dir, path)?;
            validate_native_file_semantics(&staging_dir, path)?;
            files.push(ContentFileEntry {
                path: path.clone(),
                kind: *kind,
                bytes: inspected.bytes,
                blake3: inspected.blake3,
            });
        }

        let manifest = ContentManifest::new(self.locale.clone(), self.provenance.clone(), files);
        write_manifest(&staging_dir, &manifest)?;
        validate_pack(&staging_dir)
    }
}
