use super::*;

pub const LOCALIZED_AUDIO_CATALOG_SCHEMA: &str = "ffone.semantic-audio-catalog.v2";

pub const LOCALIZED_VOICE_REPORT: &str = "audio/voice/localization-report.json";

pub const LOCALIZED_VOICE_REPORT_SCHEMA: &str = "ffone.localized-voice-report.v1";

pub const DEFAULT_VOICE_LOCALE: &str = "en";

pub const RUSSIAN_VOICE_LOCALE: &str = "ru";

#[derive(Clone, Debug)]
pub struct LocalizedVoiceInstallOptions {
    pub asset_root: PathBuf,
    pub russian_source_root: PathBuf,
    pub source_build: String,
    pub source_catalog: Option<PathBuf>,
}

impl LocalizedVoiceInstallOptions {
    #[must_use]
    pub fn new(
        asset_root: impl Into<PathBuf>,
        russian_source_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            russian_source_root: russian_source_root.into(),
            source_build: source_build.into(),
            source_catalog: None,
        }
    }

    #[must_use]
    pub fn with_source_catalog(mut self, source_catalog: impl Into<PathBuf>) -> Self {
        self.source_catalog = Some(source_catalog.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedVoiceInstallReport {
    pub source_build: String,
    pub logical_audio_assets: u64,
    pub installed_audio_files: u64,
    pub installed_audio_bytes: u64,
    pub english_voice_files: u64,
    pub russian_voice_files: u64,
    pub primary_matches: u64,
    pub casefold_fallback_matches: u64,
    pub unmatched_russian_files: u64,
    pub ambiguous_russian_files: u64,
    pub reclassified_voice_assets: u64,
    pub catalog_path: String,
    pub report_path: String,
    pub manifest_files: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizedVoiceMatchMode {
    Primary,
    ContainerCasefoldNameFallback,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedVoiceSourceProof {
    pub relative_path: String,
    pub container: String,
    pub path_id: u64,
    pub true_name: String,
    pub match_mode: LocalizedVoiceMatchMode,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedAudioVariant {
    pub locale: String,
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translated_source: Option<LocalizedVoiceSourceProof>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedAudioAsset {
    pub true_name: String,
    pub path: String,
    pub category: SemanticAudioCategory,
    pub owner: String,
    pub classification: SemanticAudioClassificationProof,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<SemanticAudioVariant>,
    pub source_manifest_path: String,
    pub source_manifest_source_path: String,
    pub source_bytes: u64,
    pub source_blake3: String,
    pub native_key: String,
    pub native_path: String,
    pub source_provenance: Vec<BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locale_variants: Vec<LocalizedAudioVariant>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedAudioCatalogCounts {
    pub assets: u64,
    pub base_bytes: u64,
    pub files: u64,
    pub file_bytes: u64,
    pub music: u64,
    pub ambient: u64,
    pub voice: u64,
    pub sfx: u64,
    pub english_voice_files: u64,
    pub russian_voice_files: u64,
    pub translated_voice_assets: u64,
    pub fallback_only_voice_assets: u64,
    pub primary_matches: u64,
    pub casefold_fallback_matches: u64,
    pub unmatched_russian_files: u64,
    pub ambiguous_russian_files: u64,
    pub reclassified_voice_assets: u64,
    pub explicit_shared_fallbacks: u64,
    pub variant_assets: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedAudioSourceCatalogProof {
    pub schema: String,
    pub blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedAudioMatchingPolicy {
    pub primary: String,
    pub fallback: String,
    pub unmatched: String,
    pub ambiguous: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedAudioCatalog {
    pub schema: String,
    pub source_build: String,
    pub source_manifest: SemanticAudioManifestProof,
    pub source_cook_report: SemanticAudioCookReportProof,
    pub source_catalog: LocalizedAudioSourceCatalogProof,
    pub fallback_locale: String,
    pub supported_locales: Vec<String>,
    pub locale_fallbacks: BTreeMap<String, String>,
    pub path_policy: String,
    pub duplicate_policy: String,
    pub classification_policy: String,
    pub matching_policy: LocalizedAudioMatchingPolicy,
    pub counts: LocalizedAudioCatalogCounts,
    pub assets: Vec<LocalizedAudioAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedVoiceReportCounts {
    pub source_files: u64,
    pub matched_source_files: u64,
    pub matched_assets: u64,
    pub primary_matches: u64,
    pub casefold_fallback_matches: u64,
    pub unmatched: u64,
    pub ambiguous: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedVoiceMatchedFile {
    pub relative_path: String,
    pub container: String,
    pub path_id: u64,
    pub source_true_name: String,
    pub catalog_true_name: String,
    pub catalog_native_key: String,
    pub english_path: String,
    pub russian_path: String,
    pub match_mode: LocalizedVoiceMatchMode,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedVoiceUnmatchedFile {
    pub relative_path: String,
    pub container: Option<String>,
    pub path_id: Option<u64>,
    pub true_name: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedVoiceAmbiguousFile {
    pub relative_path: String,
    pub container: String,
    pub path_id: u64,
    pub true_name: String,
    pub reason: String,
    pub candidate_native_keys: Vec<String>,
    pub candidate_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizedVoiceReportDocument {
    pub schema: String,
    pub source_build: String,
    pub source_root: String,
    pub matching_policy: LocalizedAudioMatchingPolicy,
    pub counts: LocalizedVoiceReportCounts,
    pub matched: Vec<LocalizedVoiceMatchedFile>,
    pub unmatched: Vec<LocalizedVoiceUnmatchedFile>,
    pub ambiguous: Vec<LocalizedVoiceAmbiguousFile>,
}

#[derive(Clone, Debug)]
pub(super) struct PreparedTranslation {
    pub(super) source: RussianSourceFile,
    pub(super) asset_index: usize,
    pub(super) match_mode: LocalizedVoiceMatchMode,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

pub fn install_localized_voice(
    options: &LocalizedVoiceInstallOptions,
) -> Result<LocalizedVoiceInstallReport> {
    validate_source_build(&options.source_build)?;
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    reject_stale_transactions(&asset_root)?;
    let russian_root = canonical_directory(&options.russian_source_root, "Russian voice root")?;
    let audio_root = asset_root.join("audio");
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_before =
        fs::read(&manifest_path).map_err(|source| io_at(&manifest_path, source))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_before).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "unsupported project-asset manifest schema {:?}; expected {PROJECT_ASSET_SCHEMA:?}",
            manifest.schema
        ));
    }
    validate_manifest_paths(&manifest)?;

    let (catalog_path, catalog_bytes) = resolve_source_catalog(options, &asset_root, &manifest)?;
    let mut base = load_base_catalog(&catalog_path, &catalog_bytes)?;
    if base.source_build != options.source_build {
        return invalid(format!(
            "source build mismatch: command={:?}, catalog={:?}",
            options.source_build, base.source_build
        ));
    }
    validate_base_assets(&asset_root, &manifest, &base.assets)?;

    let matching_policy = matching_policy();
    let russian_files = collect_russian_sources(&russian_root)?;
    let (pending, mut unmatched, mut ambiguous) =
        match_russian_sources(&base.assets, russian_files)?;
    let pending = reject_duplicate_asset_matches(pending, &base.assets, &mut ambiguous);
    let translations = prepare_translations(pending, &base.assets, &mut unmatched, &mut ambiguous)?;

    let translated_indices = translations
        .iter()
        .map(|translation| translation.asset_index)
        .collect::<BTreeSet<_>>();
    let mut proven_voice_indices = translated_indices.clone();
    proven_voice_indices.extend(
        base.assets
            .iter()
            .enumerate()
            .filter(|(_, asset)| is_proven_tutorial_dialogue(asset))
            .map(|(index, _)| index),
    );
    let reclassified_voice_assets = proven_voice_indices
        .iter()
        .filter(|index| base.assets[**index].category != SemanticAudioCategory::Voice)
        .count() as u64;
    reclassify_proven_voice(&mut base.assets, &proven_voice_indices, &translated_indices)?;
    route_english_voice(&mut base.assets)?;
    let russian_destinations = route_russian_voice(&base.assets, &translations, &mut ambiguous)?;
    let accepted_translations = translations
        .into_iter()
        .filter(|translation| russian_destinations.contains_key(&translation.asset_index))
        .collect::<Vec<_>>();

    let stamp = transaction_stamp();
    let stage = asset_root.join(format!(".localized-voice-stage-{stamp}"));
    let backup = asset_root.join(format!(".localized-voice-backup-{stamp}"));
    if stage.exists() || backup.exists() {
        return invalid(format!(
            "localized voice transaction path already exists: {} or {}",
            stage.display(),
            backup.display()
        ));
    }
    fs::create_dir(&stage).map_err(|source| io_at(&stage, source))?;

    let staged = stage_localized_audio(
        &asset_root,
        &russian_root,
        &stage,
        &manifest,
        base,
        accepted_translations,
        russian_destinations,
        unmatched,
        ambiguous,
        matching_policy,
    );
    let StagedLocalizedAudio {
        entries,
        catalog,
        report,
        installed_audio_bytes,
    } = match staged {
        Ok(staged) => staged,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
    };

    let mut next_manifest = manifest.clone();
    next_manifest.files.retain(|entry| !is_owned_entry(entry));
    next_manifest.files.extend(entries);
    next_manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    validate_manifest_paths(&next_manifest)?;
    validate_unowned_collisions(&manifest, &next_manifest)?;

    let current_manifest =
        fs::read(&manifest_path).map_err(|source| io_at(&manifest_path, source))?;
    if current_manifest != manifest_before {
        let _ = fs::remove_dir_all(&stage);
        return invalid("asset manifest changed while localized voice files were staged");
    }
    commit_transaction(&audio_root, &stage, &backup, &manifest_path, &next_manifest)?;

    Ok(LocalizedVoiceInstallReport {
        source_build: options.source_build.clone(),
        logical_audio_assets: catalog.counts.assets,
        installed_audio_files: catalog.counts.files,
        installed_audio_bytes,
        english_voice_files: catalog.counts.english_voice_files,
        russian_voice_files: catalog.counts.russian_voice_files,
        primary_matches: report.counts.primary_matches,
        casefold_fallback_matches: report.counts.casefold_fallback_matches,
        unmatched_russian_files: report.counts.unmatched,
        ambiguous_russian_files: report.counts.ambiguous,
        reclassified_voice_assets,
        catalog_path: SEMANTIC_AUDIO_CATALOG.to_owned(),
        report_path: LOCALIZED_VOICE_REPORT.to_owned(),
        manifest_files: next_manifest.files.len() as u64,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stage_localized_audio(
    asset_root: &Path,
    russian_root: &Path,
    stage: &Path,
    manifest: &ProjectAssetManifest,
    mut base: BaseCatalog,
    translations: Vec<PreparedTranslation>,
    russian_destinations: BTreeMap<usize, String>,
    mut unmatched: Vec<LocalizedVoiceUnmatchedFile>,
    mut ambiguous: Vec<LocalizedVoiceAmbiguousFile>,
    matching_policy: LocalizedAudioMatchingPolicy,
) -> Result<StagedLocalizedAudio> {
    for directory in ["music", "ambient", "voice", "sfx"] {
        let target = stage.join(directory);
        fs::create_dir(&target).map_err(|error| io_at(&target, error))?;
    }
    let manifest_by_path = manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut entries = Vec::new();
    let mut installed_audio_bytes = 0_u64;

    for (index, asset) in base.assets.iter().enumerate() {
        let source_path = base
            .runtime_paths
            .get(index)
            .ok_or_else(|| invalid_error("base catalog runtime path index is out of bounds"))?;
        let source = asset_root.join(native_path(source_path));
        let target = stage.join(
            asset
                .path
                .strip_prefix("audio/")
                .ok_or_else(|| invalid_error("semantic audio path lost audio/ prefix"))?,
        );
        let (bytes, hash) = copy_verified_ogg(&source, &target)?;
        if bytes != asset.source_bytes || hash != asset.source_blake3 {
            return invalid(format!(
                "English source identity mismatch for {:?}: catalog bytes={} blake3={}, disk bytes={bytes} blake3={hash}",
                source_path, asset.source_bytes, asset.source_blake3
            ));
        }
        let old_entry = manifest_by_path.get(source_path.as_str()).ok_or_else(|| {
            invalid_error(format!("missing source manifest entry for {source_path:?}"))
        })?;
        entries.push(ProjectAssetFile {
            source_path: old_entry.source_path.clone(),
            path: asset.path.clone(),
            kind: ProjectAssetKind::Audio,
            bytes,
            blake3: hash,
        });
        installed_audio_bytes = installed_audio_bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid_error("localized audio byte count overflow"))?;
    }

    let mut matched_report = Vec::new();
    let mut accepted_indices = BTreeSet::new();
    for translation in translations {
        let Some(destination) = russian_destinations.get(&translation.asset_index) else {
            continue;
        };
        let asset = &mut base.assets[translation.asset_index];
        let target = stage.join(
            destination
                .strip_prefix("audio/")
                .ok_or_else(|| invalid_error("Russian voice path lost audio/ prefix"))?,
        );
        let (bytes, hash) = copy_verified_ogg(&translation.source.absolute_path, &target)?;
        if bytes != translation.bytes || hash != translation.blake3 {
            return invalid(format!(
                "Russian source changed during import: {}",
                translation.source.absolute_path.display()
            ));
        }
        let source_proof = LocalizedVoiceSourceProof {
            relative_path: translation.source.relative_path.clone(),
            container: translation.source.container.clone().unwrap_or_default(),
            path_id: translation.source.path_id.unwrap_or_default(),
            true_name: translation.source.true_name.clone().unwrap_or_default(),
            match_mode: translation.match_mode.clone(),
        };
        asset.locale_variants.push(LocalizedAudioVariant {
            locale: RUSSIAN_VOICE_LOCALE.to_owned(),
            path: destination.clone(),
            bytes,
            blake3: hash.clone(),
            translated_source: Some(source_proof.clone()),
        });
        asset
            .locale_variants
            .sort_by(|left, right| left.locale.cmp(&right.locale));
        entries.push(ProjectAssetFile {
            source_path: format!("localized-voice/ru/{}", translation.source.relative_path),
            path: destination.clone(),
            kind: ProjectAssetKind::Audio,
            bytes,
            blake3: hash.clone(),
        });
        installed_audio_bytes = installed_audio_bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid_error("localized audio byte count overflow"))?;
        accepted_indices.insert(translation.asset_index);
        matched_report.push(LocalizedVoiceMatchedFile {
            relative_path: translation.source.relative_path,
            container: source_proof.container,
            path_id: source_proof.path_id,
            source_true_name: source_proof.true_name,
            catalog_true_name: asset.true_name.clone(),
            catalog_native_key: asset.native_key.clone(),
            english_path: asset.path.clone(),
            russian_path: destination.clone(),
            match_mode: translation.match_mode,
            bytes,
            blake3: hash,
        });
    }

    matched_report.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    unmatched.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    ambiguous.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let primary_matches = matched_report
        .iter()
        .filter(|record| record.match_mode == LocalizedVoiceMatchMode::Primary)
        .count() as u64;
    let fallback_matches = matched_report.len() as u64 - primary_matches;
    let report = LocalizedVoiceReportDocument {
        schema: LOCALIZED_VOICE_REPORT_SCHEMA.to_owned(),
        source_build: base.source_build.clone(),
        source_root: russian_root.display().to_string(),
        matching_policy: matching_policy.clone(),
        counts: LocalizedVoiceReportCounts {
            source_files: matched_report.len() as u64
                + unmatched.len() as u64
                + ambiguous.len() as u64,
            matched_source_files: matched_report.len() as u64,
            matched_assets: accepted_indices.len() as u64,
            primary_matches,
            casefold_fallback_matches: fallback_matches,
            unmatched: unmatched.len() as u64,
            ambiguous: ambiguous.len() as u64,
        },
        matched: matched_report,
        unmatched,
        ambiguous,
    };
    let mut report_bytes =
        serde_json::to_vec_pretty(&report).map_err(|source| PipelineError::Json {
            path: LOCALIZED_VOICE_REPORT.to_owned(),
            source,
        })?;
    report_bytes.push(b'\n');
    let report_target = stage.join("voice/localization-report.json");
    write_new(&report_target, &report_bytes)?;
    entries.push(ProjectAssetFile {
        source_path: "localized-voice-installer/report".to_owned(),
        path: LOCALIZED_VOICE_REPORT.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: report_bytes.len() as u64,
        blake3: blake3::hash(&report_bytes).to_hex().to_string(),
    });

    base.assets
        .sort_by(|left, right| left.path.cmp(&right.path));
    let counts = catalog_counts(&base.assets, &report)?;
    let catalog = LocalizedAudioCatalog {
        schema: LOCALIZED_AUDIO_CATALOG_SCHEMA.to_owned(),
        source_build: base.source_build,
        source_manifest: base.source_manifest,
        source_cook_report: base.source_cook_report,
        source_catalog: base.source_catalog,
        fallback_locale: DEFAULT_VOICE_LOCALE.to_owned(),
        supported_locales: vec![
            DEFAULT_VOICE_LOCALE.to_owned(),
            RUSSIAN_VOICE_LOCALE.to_owned(),
        ],
        locale_fallbacks: BTreeMap::from([(
            RUSSIAN_VOICE_LOCALE.to_owned(),
            DEFAULT_VOICE_LOCALE.to_owned(),
        )]),
        path_policy: append_policy_once(
            base.path_policy,
            "voice is rooted at audio/voice/<locale>/<owner>",
        ),
        duplicate_policy: base.duplicate_policy,
        classification_policy: append_policy_once(
            base.classification_policy,
            "exact translated source provenance reclassifies locale-sensitive vocal SFX as voice",
        ),
        matching_policy,
        counts,
        assets: base.assets,
    };
    let mut catalog_bytes =
        serde_json::to_vec_pretty(&catalog).map_err(|source| PipelineError::Json {
            path: SEMANTIC_AUDIO_CATALOG.to_owned(),
            source,
        })?;
    catalog_bytes.push(b'\n');
    let catalog_target = stage.join("catalog.json");
    write_new(&catalog_target, &catalog_bytes)?;
    entries.push(ProjectAssetFile {
        source_path: "localized-voice-installer/catalog-v2".to_owned(),
        path: SEMANTIC_AUDIO_CATALOG.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: catalog_bytes.len() as u64,
        blake3: blake3::hash(&catalog_bytes).to_hex().to_string(),
    });

    entries.sort_by(|left, right| left.path.cmp(&right.path));
    validate_staged_files(stage, &entries)?;
    Ok(StagedLocalizedAudio {
        entries,
        catalog,
        report,
        installed_audio_bytes,
    })
}

pub(super) struct StagedLocalizedAudio {
    pub(super) entries: Vec<ProjectAssetFile>,
    pub(super) catalog: LocalizedAudioCatalog,
    pub(super) report: LocalizedVoiceReportDocument,
    pub(super) installed_audio_bytes: u64,
}
