use super::*;

#[derive(Clone, Debug)]
pub(super) struct BaseCatalog {
    pub(super) source_build: String,
    pub(super) source_manifest: SemanticAudioManifestProof,
    pub(super) source_cook_report: SemanticAudioCookReportProof,
    pub(super) source_catalog: LocalizedAudioSourceCatalogProof,
    pub(super) path_policy: String,
    pub(super) duplicate_policy: String,
    pub(super) classification_policy: String,
    pub(super) assets: Vec<LocalizedAudioAsset>,
    pub(super) runtime_paths: Vec<String>,
}

pub(super) fn resolve_source_catalog(
    options: &LocalizedVoiceInstallOptions,
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<(PathBuf, Vec<u8>)> {
    let installed = asset_root.join(native_path(SEMANTIC_AUDIO_CATALOG));
    let path = if installed.is_file() {
        installed
    } else {
        options.source_catalog.clone().ok_or_else(|| {
            invalid_error(format!(
                "{} is missing; pass --source-catalog <V1_OR_V2_CATALOG>",
                installed.display()
            ))
        })?
    };
    let bytes = fs::read(&path).map_err(|source| io_at(&path, source))?;
    let manifest_entries = manifest
        .files
        .iter()
        .filter(|entry| entry.path == SEMANTIC_AUDIO_CATALOG)
        .collect::<Vec<_>>();
    let [manifest_entry] = manifest_entries.as_slice() else {
        return invalid(format!(
            "project manifest must contain exactly one {:?} entry; found {}",
            SEMANTIC_AUDIO_CATALOG,
            manifest_entries.len()
        ));
    };
    let actual_hash = blake3::hash(&bytes).to_hex().to_string();
    if manifest_entry.bytes != bytes.len() as u64 || manifest_entry.blake3 != actual_hash {
        return invalid(format!(
            "source catalog identity disagrees with manifest: path={}, manifest bytes={} blake3={}, actual bytes={} blake3={actual_hash}",
            path.display(),
            manifest_entry.bytes,
            manifest_entry.blake3,
            bytes.len()
        ));
    }
    Ok((path, bytes))
}

pub(super) fn load_base_catalog(path: &Path, bytes: &[u8]) -> Result<BaseCatalog> {
    let header: CatalogHeader =
        serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
            path: path.display().to_string(),
            source,
        })?;
    match header.schema.as_str() {
        SEMANTIC_AUDIO_CATALOG_SCHEMA => {
            let source: SemanticAudioCatalog =
                serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
                    path: path.display().to_string(),
                    source,
                })?;
            let runtime_paths = source
                .assets
                .iter()
                .map(|asset| asset.path.clone())
                .collect();
            Ok(BaseCatalog {
                source_build: source.source_build,
                source_manifest: source.source_manifest,
                source_cook_report: source.source_cook_report,
                source_catalog: LocalizedAudioSourceCatalogProof {
                    schema: source.schema,
                    blake3: blake3::hash(bytes).to_hex().to_string(),
                },
                path_policy: source.path_policy,
                duplicate_policy: source.duplicate_policy,
                classification_policy: source.classification_policy,
                assets: source
                    .assets
                    .into_iter()
                    .map(|asset| LocalizedAudioAsset {
                        true_name: asset.true_name,
                        path: asset.path,
                        category: asset.category,
                        owner: asset.owner,
                        classification: asset.classification,
                        variant: asset.variant,
                        source_manifest_path: asset.source_manifest_path,
                        source_manifest_source_path: asset.source_manifest_source_path,
                        source_bytes: asset.source_bytes,
                        source_blake3: asset.source_blake3,
                        native_key: asset.native_key,
                        native_path: asset.native_path,
                        source_provenance: asset.source_provenance,
                        locale_variants: Vec::new(),
                    })
                    .collect(),
                runtime_paths,
            })
        }
        LOCALIZED_AUDIO_CATALOG_SCHEMA => {
            let source: LocalizedAudioCatalog =
                serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
                    path: path.display().to_string(),
                    source,
                })?;
            let runtime_paths = source
                .assets
                .iter()
                .map(|asset| {
                    if asset.category == SemanticAudioCategory::Voice {
                        asset
                            .locale_variants
                            .iter()
                            .find(|variant| variant.locale == DEFAULT_VOICE_LOCALE)
                            .map_or_else(|| asset.path.clone(), |variant| variant.path.clone())
                    } else {
                        asset.path.clone()
                    }
                })
                .collect();
            Ok(BaseCatalog {
                source_build: source.source_build,
                source_manifest: source.source_manifest,
                source_cook_report: source.source_cook_report,
                source_catalog: source.source_catalog,
                path_policy: source.path_policy,
                duplicate_policy: source.duplicate_policy,
                classification_policy: source.classification_policy,
                assets: source.assets,
                runtime_paths,
            })
        }
        schema => invalid(format!(
            "unsupported source audio catalog schema {schema:?}; expected {SEMANTIC_AUDIO_CATALOG_SCHEMA:?} or {LOCALIZED_AUDIO_CATALOG_SCHEMA:?}"
        )),
    }
}

#[derive(Deserialize)]
pub(super) struct CatalogHeader {
    pub(super) schema: String,
}

pub(super) fn parse_russian_source_path(path: &Path) -> Option<(String, u64, String)> {
    let container = path.parent()?.file_name()?.to_str()?.to_owned();
    if !folded(&container).starts_with("customassetbundle-") {
        return None;
    }
    let filename = path.file_name()?.to_str()?;
    let (path_id, true_name) = parse_ogg_filename(filename)?;
    Some((container, path_id, true_name))
}

pub(super) fn catalog_counts(
    assets: &[LocalizedAudioAsset],
    report: &LocalizedVoiceReportDocument,
) -> Result<LocalizedAudioCatalogCounts> {
    let mut counts = LocalizedAudioCatalogCounts {
        assets: assets.len() as u64,
        base_bytes: 0,
        files: 0,
        file_bytes: 0,
        music: 0,
        ambient: 0,
        voice: 0,
        sfx: 0,
        english_voice_files: 0,
        russian_voice_files: 0,
        translated_voice_assets: 0,
        fallback_only_voice_assets: 0,
        primary_matches: report.counts.primary_matches,
        casefold_fallback_matches: report.counts.casefold_fallback_matches,
        unmatched_russian_files: report.counts.unmatched,
        ambiguous_russian_files: report.counts.ambiguous,
        reclassified_voice_assets: 0,
        explicit_shared_fallbacks: 0,
        variant_assets: 0,
    };
    for asset in assets {
        counts.base_bytes = counts
            .base_bytes
            .checked_add(asset.source_bytes)
            .ok_or_else(|| invalid_error("base audio byte count overflow"))?;
        match asset.category {
            SemanticAudioCategory::Music => counts.music += 1,
            SemanticAudioCategory::Ambient => counts.ambient += 1,
            SemanticAudioCategory::Voice => counts.voice += 1,
            SemanticAudioCategory::Sfx => counts.sfx += 1,
        }
        if matches!(
            asset.classification.category_rule.as_str(),
            "localized-voice-source" | "tutorial-dialogue-source"
        ) {
            counts.reclassified_voice_assets += 1;
        }
        if asset.classification.uncertain {
            counts.explicit_shared_fallbacks += 1;
        }
        if asset.variant.is_some() {
            counts.variant_assets += 1;
        }
        if asset.category == SemanticAudioCategory::Voice {
            let english = asset
                .locale_variants
                .iter()
                .find(|variant| variant.locale == DEFAULT_VOICE_LOCALE)
                .ok_or_else(|| {
                    invalid_error(format!(
                        "voice asset {:?} has no English variant",
                        asset.path
                    ))
                })?;
            counts.english_voice_files += 1;
            counts.files += 1;
            counts.file_bytes = counts
                .file_bytes
                .checked_add(english.bytes)
                .ok_or_else(|| invalid_error("localized audio file byte count overflow"))?;
            if let Some(russian) = asset
                .locale_variants
                .iter()
                .find(|variant| variant.locale == RUSSIAN_VOICE_LOCALE)
            {
                counts.russian_voice_files += 1;
                counts.translated_voice_assets += 1;
                counts.files += 1;
                counts.file_bytes = counts
                    .file_bytes
                    .checked_add(russian.bytes)
                    .ok_or_else(|| invalid_error("localized audio file byte count overflow"))?;
            } else {
                counts.fallback_only_voice_assets += 1;
            }
        } else {
            counts.files += 1;
            counts.file_bytes = counts
                .file_bytes
                .checked_add(asset.source_bytes)
                .ok_or_else(|| invalid_error("localized audio file byte count overflow"))?;
        }
    }
    Ok(counts)
}

pub(super) fn replace_manifest(path: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("asset manifest has no parent"))?;
    let stamp = transaction_stamp();
    let next = parent.join(format!(".asset-manifest.localized-voice-next-{stamp}"));
    let backup = parent.join(format!(".asset-manifest.localized-voice-backup-{stamp}"));
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    write_new(&next, &bytes)?;
    fs::rename(path, &backup).map_err(|error| io_at(path, error))?;
    if let Err(error) = fs::rename(&next, path) {
        let _ = fs::rename(&backup, path);
        let _ = fs::remove_file(&next);
        return Err(io_at(path, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}

pub(super) fn native_path(path: &str) -> PathBuf {
    path.split('/').collect()
}

pub(super) fn path_identity(path: &str) -> String {
    folded(&path.replace('\\', "/"))
}
