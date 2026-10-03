use std::path::{Path, PathBuf};

use crate::{
    ContentError, ContentFileEntry, ContentManifest, Result,
    validation::{canonical_pack_root, read_verified_file, validate_pack},
};

/// A fully validated, directory-backed native pack.
#[derive(Debug)]
pub struct ContentPack {
    root: PathBuf,
    manifest: ContentManifest,
}

impl ContentPack {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = canonical_pack_root(root.as_ref())?;
        let manifest = validate_pack(&root)?;
        Ok(Self { root, manifest })
    }

    pub fn manifest(&self) -> &ContentManifest {
        &self.manifest
    }

    pub fn entry(&self, path: &str) -> Option<&ContentFileEntry> {
        self.manifest
            .files
            .binary_search_by(|entry| entry.path.as_str().cmp(path))
            .ok()
            .map(|index| &self.manifest.files[index])
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let entry = self
            .entry(path)
            .ok_or_else(|| ContentError::NotManifestListed(path.to_owned()))?;
        read_verified_file(&self.root, entry)
    }
}
