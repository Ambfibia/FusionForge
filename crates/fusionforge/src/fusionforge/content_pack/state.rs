use super::*;

#[derive(Clone, Debug)]
pub(super) struct InventoryEntry {
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) expected: LauncherFileInfo,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InventoryReportEntry {
    pub(super) file: String,
    pub(super) size: u64,
    pub(super) sha256: String,
}

pub(super) struct CookState {
    pub(super) staging: PathBuf,
    pub(super) builder: ContentPackBuilder,
    pub(super) registered: BTreeSet<String>,
    pub(super) assets: BTreeMap<String, NativeAssetEntry>,
    pub(super) font_metadata: BTreeMap<String, FontIndexEntry>,
    pub(super) coverage: CoverageCounters,
    pub(super) counts: BTreeMap<String, u64>,
}

impl CookState {
    pub(super) fn new(staging: PathBuf, locale: &str, provenance: Provenance) -> Self {
        Self {
            staging,
            builder: ContentPackBuilder::new(locale, provenance),
            registered: BTreeSet::new(),
            assets: BTreeMap::new(),
            font_metadata: BTreeMap::new(),
            coverage: CoverageCounters::default(),
            counts: BTreeMap::new(),
        }
    }

    pub(super) fn emit_asset(
        &mut self,
        directory: &str,
        semantic_name: &str,
        extension: &str,
        kind: ContentKind,
        bytes: &[u8],
    ) -> Result<EmittedAsset, String> {
        validate_native_payload(extension, bytes)?;
        self.emit_validated_asset(directory, semantic_name, extension, kind, bytes)
    }

    pub(super) fn emit_validated_asset(
        &mut self,
        directory: &str,
        semantic_name: &str,
        extension: &str,
        kind: ContentKind,
        bytes: &[u8],
    ) -> Result<EmittedAsset, String> {
        let identity = native_asset_identity(kind, semantic_name, bytes);
        let relative = format!(
            "{directory}/{}--{}.{}",
            identity.safe_name,
            &identity.content_hash[..16],
            extension.to_ascii_lowercase()
        );
        let kind_name = content_kind_name(kind);

        let newly_registered = self.write_registered(&relative, kind, bytes)?;
        self.assets
            .entry(identity.key.clone())
            .or_insert(NativeAssetEntry {
                key: identity.key.clone(),
                kind: kind_name.to_string(),
                name: identity.display_name,
                path: relative,
            });
        if newly_registered {
            *self.counts.entry(kind_name.to_string()).or_default() += 1;
        }
        let asset = self
            .assets
            .get(&identity.key)
            .ok_or_else(|| "native catalog registration was lost".to_string())?;
        Ok(EmittedAsset {
            key: asset.key.clone(),
            name: asset.name.clone(),
            path: asset.path.clone(),
        })
    }

    pub(super) fn register_font_metadata(
        &mut self,
        family: &str,
        bytes: &[u8],
        include_russian: bool,
        replace_ascii: bool,
        vertical_offset: f64,
        target_name: Option<&str>,
    ) {
        let family = neutral_semantic_label(family, "font");
        let asset_key = native_asset_identity(ContentKind::Font, &family, bytes).key;
        let target_key = target_name
            .map(|value| neutral_semantic_label(value, "font-target"))
            .map(|value| stable_key("ffone.font-target.v1", &[value.as_bytes()]));
        let metadata_key = stable_key(
            "ffone.font-metadata.v1",
            &[
                asset_key.as_bytes(),
                &[u8::from(include_russian), u8::from(replace_ascii)],
                &vertical_offset.to_le_bytes(),
                target_key.as_deref().unwrap_or_default().as_bytes(),
            ],
        );
        self.font_metadata
            .entry(metadata_key.clone())
            .or_insert(FontIndexEntry {
                key: metadata_key,
                family,
                asset_key,
                include_russian,
                replace_ascii,
                vertical_offset,
                target_key,
            });
    }

    pub(super) fn emit_font_index(&mut self) -> Result<(), String> {
        if self.font_metadata.is_empty() {
            return Ok(());
        }
        let value = json!({
            "schema": "ffone.font-index.v1",
            "fonts": self.font_metadata.values().cloned().collect::<Vec<_>>(),
        });
        let bytes = json_bytes(&value)?;
        validate_native_payload("json", &bytes)?;
        let hash = blake3_hex(&bytes);
        let relative = format!("fonts/font-index--{}.json", &hash[..16]);
        self.write_registered(&relative, ContentKind::Font, &bytes)?;
        *self.counts.entry("font_metadata".to_string()).or_default() += 1;
        Ok(())
    }

    pub(super) fn emit_catalog(&mut self) -> Result<(), String> {
        let emitted = self.registered.len() as u64;
        let catalog = json!({
            "schema": "ffone.asset-index.v1",
            "profile": "core-v1",
            "complete": false,
            "nativeOnly": true,
            "coverage": {
                "emitted": emitted,
                "conversion": {
                    "sourceFilesScanned": self.coverage.bundle_files_scanned,
                    "sourceFileFailures": self.coverage.bundle_failures,
                    "sourceRecordsScanned": self.coverage.bundle_objects_scanned,
                    "supportedRecords": self.coverage.bundle_supported_objects,
                    "emittedRecords": self.coverage.bundle_emitted + self.coverage.overlay_emitted,
                    "skippedRecords": self.coverage.bundle_skipped + self.coverage.overlay_skipped,
                    "overlayEntriesScanned": self.coverage.overlay_entries_scanned,
                    "localizationFallbacks": self.coverage.localization_fallbacks,
                    "audioSizeMismatches": self.coverage.audio_size_mismatches,
                    "errors": self.coverage.errors
                },
                "counts": self.counts.clone(),
                "supportedKinds": ["audio", "font", "localization", "mesh", "table", "texture"],
                "omittedKinds": ["animation", "material", "shader", "world"],
                "fontMetadataAssociation": "overlay-only"
            },
            "assets": self.assets.values().cloned().collect::<Vec<_>>(),
        });
        let bytes = json_bytes(&catalog)?;
        validate_native_payload("json", &bytes)?;
        let hash = blake3_hex(&bytes);
        let relative = format!("catalog/content-index--{}.json", &hash[..16]);
        self.write_registered(&relative, ContentKind::Table, &bytes)?;
        *self.counts.entry("catalog".to_string()).or_default() += 1;
        Ok(())
    }

    pub(super) fn write_registered(
        &mut self,
        relative: &str,
        kind: ContentKind,
        bytes: &[u8],
    ) -> Result<bool, String> {
        validate_pack_relative_path(relative)?;
        let target = self
            .staging
            .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        if self.registered.contains(relative) {
            let existing =
                fs::read(&target).map_err(|err| format!("{}: {err}", target.display()))?;
            if existing != bytes {
                return Err(format!("native output collision at {relative}"));
            }
            return Ok(false);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        fs::write(&target, bytes).map_err(|err| format!("{}: {err}", target.display()))?;
        self.builder
            .register(relative.to_string(), kind)
            .map_err(|err| err.to_string())?;
        self.registered.insert(relative.to_string());
        Ok(true)
    }
}

pub(super) fn build_inventory(
    build_dir: &Path,
    launcher: &LauncherManifest,
) -> Result<Vec<InventoryEntry>, String> {
    let mut inventory = Vec::with_capacity(launcher.bundles.len() + 1);
    inventory.push(InventoryEntry {
        name: "main.unity3d".to_string(),
        path: build_dir.join("main.unity3d"),
        expected: launcher.main_file_info.clone(),
    });
    let mut folded = BTreeSet::new();
    folded.insert("main.unity3d".to_string());
    for (name, bundle) in &launcher.bundles {
        validate_source_filename(name)?;
        validate_sha256(&bundle.compressed_info.hash)?;
        if !folded.insert(name.to_ascii_lowercase()) {
            return Err(format!(
                "case-insensitive duplicate build file in manifest: {name}"
            ));
        }
        inventory.push(InventoryEntry {
            name: name.clone(),
            path: build_dir.join(name),
            expected: bundle.compressed_info.clone(),
        });
    }
    validate_sha256(&launcher.main_file_info.hash)?;
    Ok(inventory)
}

pub(super) fn build_inventory_fingerprint(inventory: &[InventoryEntry]) -> String {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"ffone.build-inventory.v1");
    for entry in inventory {
        hasher.update(&[0]);
        hasher.update(entry.name.as_bytes());
        hasher.update(&[0]);
        hasher.update(&entry.expected.size.to_le_bytes());
        hasher.update(entry.expected.hash.to_ascii_lowercase().as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

pub(super) fn verify_inventory_entry(entry: &InventoryEntry) -> Result<(), String> {
    validate_sha256(&entry.expected.hash)?;
    let metadata = fs::symlink_metadata(&entry.path)
        .map_err(|err| format!("{}: {err}", entry.path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "build inventory entry is not a regular file: {}",
            entry.path.display()
        ));
    }
    if metadata.len() != entry.expected.size {
        return Err(format!(
            "{} size mismatch: manifest {}, actual {}",
            entry.name,
            entry.expected.size,
            metadata.len()
        ));
    }
    let bytes = fs::read(&entry.path).map_err(|err| format!("{}: {err}", entry.path.display()))?;
    let actual = sha256_hex(&bytes);
    if !actual.eq_ignore_ascii_case(&entry.expected.hash) {
        return Err(format!(
            "{} SHA-256 mismatch: manifest {}, actual {}",
            entry.name, entry.expected.hash, actual
        ));
    }
    Ok(())
}
