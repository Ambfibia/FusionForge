use super::*;

pub const STRICT_AUDIO_CATALOG_SCHEMA_V3: &str = "ffone.semantic-audio-catalog.v3";

pub const STRICT_AUDIO_CATALOG_SCHEMA_V4: &str = "ffone.semantic-audio-catalog.v4";

pub const STRICT_AUDIO_CATALOG_SCHEMA: &str = "ffone.semantic-audio-catalog.v5";

pub const STRICT_AUDIO_CATALOG_PATH: &str = "_runtime/audio.json";

pub const STRICT_VOICE_REPORT_SCHEMA: &str = "ffone.voice-localization-import.v3";

pub const DEFAULT_STRICT_VOICE_REPORT_DIRECTORY: &str = "target/ffone-audits/voice-localization-v3";

#[derive(Clone, Debug)]
pub struct StrictVoiceInstallOptions {
    pub asset_root: PathBuf,
    pub russian_source_root: PathBuf,
    pub source_build: String,
    pub cook_report: PathBuf,
    pub cook_pack_root: PathBuf,
    pub english_source_root: PathBuf,
    pub generated_report_root: PathBuf,
    pub source_catalog: Option<PathBuf>,
    pub preflight_only: bool,
}

impl StrictVoiceInstallOptions {
    #[must_use]
    pub fn new(
        asset_root: impl Into<PathBuf>,
        russian_source_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
        cook_report: impl Into<PathBuf>,
        cook_pack_root: impl Into<PathBuf>,
        english_source_root: impl Into<PathBuf>,
        generated_report_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            russian_source_root: russian_source_root.into(),
            source_build: source_build.into(),
            cook_report: cook_report.into(),
            cook_pack_root: cook_pack_root.into(),
            english_source_root: english_source_root.into(),
            generated_report_root: generated_report_root.into(),
            source_catalog: None,
            preflight_only: false,
        }
    }

    #[must_use]
    pub fn with_source_catalog(mut self, source_catalog: impl Into<PathBuf>) -> Self {
        self.source_catalog = Some(source_catalog.into());
        self
    }

    #[must_use]
    pub fn preflight_only(mut self) -> Self {
        self.preflight_only = true;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrictVoiceInstallReport {
    pub committed: bool,
    pub logical_audio_assets: u64,
    pub installed_audio_files: u64,
    pub installed_audio_bytes: u64,
    pub english_voice_files: u64,
    pub russian_voice_files: u64,
    pub localized_voice_keys: u64,
    pub english_only_ambiguous_keys: u64,
    pub quarantined_russian_files: u64,
    pub ignored_russian_files: u64,
    pub unresolved_english_keys: u64,
    pub identical_locale_payloads: u64,
    pub catalog_path: String,
    pub generated_report_path: String,
    pub manifest_files: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrictAudioCatalog {
    pub schema: String,
    pub fallback_locale: String,
    pub locale_fallbacks: BTreeMap<String, String>,
    pub counts: StrictAudioCatalogCounts,
    pub assets: Vec<StrictAudioAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrictAudioCatalogCounts {
    pub assets: u64,
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
    pub language_neutral_voice_assets: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrictAudioAsset {
    pub logical_key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub true_name: String,
    pub category: SemanticAudioCategory,
    pub owner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub files: Vec<StrictAudioFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrictAudioFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub language_neutral: bool,
}

#[derive(Clone, Debug)]
pub(super) struct FreshAudio {
    pub(super) absolute_path: PathBuf,
    pub(super) native_path: String,
    pub(super) true_name: String,
    pub(super) native_key: String,
    pub(super) sources: Vec<BTreeMap<String, serde_json::Value>>,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Debug)]
pub(super) struct VoiceSource {
    pub(super) absolute_path: PathBuf,
    pub(super) source_path: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Debug)]
pub(super) struct VoiceSelection {
    pub(super) logical_key: String,
    pub(super) true_name: String,
    pub(super) owner: String,
    pub(super) scope: Option<String>,
    pub(super) english: VoiceSource,
    pub(super) russian: Option<VoiceSource>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StrictVoiceImportDocument {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) inputs: BTreeMap<String, ImportInputProof>,
    pub(super) policy: Vec<String>,
    pub(super) counts: ImportReportCounts,
    pub(super) localized: Vec<LocalizedKeyProof>,
    pub(super) ambiguous: Vec<AmbiguousKeyProof>,
    pub(super) ignored: Vec<IgnoredFileProof>,
    pub(super) unresolved_english: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EditableAudioCatalog {
    pub(super) schema: String,
    pub(super) fallback_locale: String,
    #[serde(default)]
    pub(super) locale_fallbacks: BTreeMap<String, String>,
    pub(super) assets: Vec<EditableAudioAsset>,
    pub(super) counts: EditableAudioCounts,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EditableAudioCounts {
    pub(super) assets: u64,
    pub(super) music: u64,
    pub(super) ambient: u64,
    pub(super) voice: u64,
    pub(super) sfx: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EditableAudioAsset {
    pub(super) logical_key: String,
    #[serde(default)]
    pub(super) aliases: Vec<String>,
    pub(super) true_name: String,
    pub(super) category: SemanticAudioCategory,
    pub(super) owner: String,
    #[serde(default)]
    pub(super) scope: Option<String>,
    pub(super) files: Vec<EditableAudioFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EditableAudioFile {
    #[serde(default)]
    pub(super) locale: Option<String>,
    pub(super) path: String,
}

pub(super) fn validate_non_voice_base(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    assets: &[BaseAsset],
) -> Result<()> {
    let manifest_by_path = manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut paths = BTreeSet::new();
    for asset in assets
        .iter()
        .filter(|asset| asset.category != SemanticAudioCategory::Voice)
    {
        let file = asset
            .file
            .as_ref()
            .ok_or_else(|| invalid_error("non-voice base asset has no runtime file"))?;
        validate_relative_path(&file.path)?;
        if !paths.insert(folded(&file.path)) {
            return invalid(format!(
                "non-voice catalog path collision at {:?}",
                file.path
            ));
        }
        let entry = manifest_by_path.get(file.path.as_str()).ok_or_else(|| {
            invalid_error(format!(
                "non-voice catalog path {:?} is absent from the manifest",
                file.path
            ))
        })?;
        if entry.kind != ProjectAssetKind::Audio
            || entry.bytes != file.bytes
            || entry.blake3 != file.blake3
        {
            return invalid(format!(
                "non-voice catalog/manifest identity mismatch for {:?}",
                file.path
            ));
        }
        validate_file_identity(&asset_root.join(native_path(&file.path)), file)?;
    }
    Ok(())
}

pub(super) fn collect_fresh_audio(report: &CookReport, pack_root: &Path) -> Result<Vec<FreshAudio>> {
    let mut grouped = BTreeMap::<String, FreshAudio>::new();
    for mapping in report
        .mappings
        .iter()
        .filter(|mapping| mapping.kind == "audio")
    {
        validate_relative_path(&mapping.native_path)?;
        if !mapping.native_path.starts_with("audio/") || !mapping.native_path.ends_with(".ogg") {
            return invalid(format!(
                "fresh audio mapping has an invalid nativePath {:?}",
                mapping.native_path
            ));
        }
        match grouped.get_mut(&mapping.native_path) {
            Some(existing) => {
                if folded(&existing.true_name) != folded(&mapping.name)
                    || existing.native_key != mapping.native_key
                {
                    return invalid(format!(
                        "fresh cook mappings disagree for {:?}",
                        mapping.native_path
                    ));
                }
                existing.sources.push(mapping.source.clone());
            }
            None => {
                let absolute_path = resolve_pack_audio(pack_root, &mapping.native_path);
                let (bytes, blake3) = inspect_ogg(&absolute_path)?;
                let expected_prefix = mapping
                    .native_path
                    .rsplit_once("--")
                    .and_then(|(_, suffix)| suffix.strip_suffix(".ogg"))
                    .unwrap_or_default();
                if expected_prefix.len() != 16 || !blake3.starts_with(expected_prefix) {
                    return invalid(format!(
                        "fresh cook nativePath/hash mismatch for {:?}: actual {blake3}",
                        mapping.native_path
                    ));
                }
                grouped.insert(
                    mapping.native_path.clone(),
                    FreshAudio {
                        absolute_path,
                        native_path: mapping.native_path.clone(),
                        true_name: mapping.name.clone(),
                        native_key: mapping.native_key.clone(),
                        sources: vec![mapping.source.clone()],
                        bytes,
                        blake3,
                    },
                );
            }
        }
    }
    Ok(grouped.into_values().collect())
}

pub(super) fn resolve_pack_audio(pack_root: &Path, native_path_value: &str) -> PathBuf {
    if pack_root
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("audio"))
    {
        return pack_root.join(native_path(
            native_path_value
                .strip_prefix("audio/")
                .unwrap_or(native_path_value),
        ));
    }
    pack_root.join(native_path(native_path_value))
}

pub(super) fn group_fresh_audio<'a>(sources: &'a [FreshAudio]) -> BTreeMap<String, Vec<&'a FreshAudio>> {
    let mut grouped = BTreeMap::<String, Vec<&FreshAudio>>::new();
    for source in sources {
        grouped
            .entry(folded(&source.true_name))
            .or_default()
            .push(source);
    }
    grouped
}

pub(super) fn group_base_voice<'a>(assets: &'a [BaseAsset]) -> BTreeMap<String, Vec<&'a BaseAsset>> {
    let mut grouped = BTreeMap::<String, Vec<&BaseAsset>>::new();
    for asset in assets
        .iter()
        .filter(|asset| asset.category == SemanticAudioCategory::Voice)
    {
        grouped
            .entry(folded(&asset.true_name))
            .or_default()
            .push(asset);
    }
    grouped
}

pub(super) fn voice_source_from_named(source: &NamedOgg, label: &str) -> VoiceSource {
    VoiceSource {
        absolute_path: source.absolute_path.clone(),
        source_path: format!("{label}/{}", source.relative_path),
        bytes: source.bytes,
        blake3: source.blake3.clone(),
    }
}

pub(super) fn voice_source_from_fresh(source: &FreshAudio) -> VoiceSource {
    VoiceSource {
        absolute_path: source.absolute_path.clone(),
        source_path: format!("fresh-cook/{}", source.native_path),
        bytes: source.bytes,
        blake3: source.blake3.clone(),
    }
}

pub(super) fn infer_voice_owner(true_name: &str) -> Result<String> {
    let components = true_name.split('_').collect::<Vec<_>>();
    let owner = if components.len() >= 2
        && matches!(components[0].to_ascii_lowercase().as_str(), "f" | "m")
    {
        format!("{}_{}", components[0], components[1])
    } else {
        components.first().copied().unwrap_or_default().to_owned()
    };
    portable_component(&owner, "inferred voice owner")
}

pub(super) fn voice_logical_key(owner: &str, true_name: &str) -> Result<String> {
    Ok(format!(
        "voice/{owner}/{}",
        portable_component(true_name, "voice line")?
    ))
}

pub(super) struct StagedVoice {
    pub(super) entries: Vec<ProjectAssetFile>,
    pub(super) catalog: StrictAudioCatalog,
}

pub(super) fn stage_strict_voice(
    asset_root: &Path,
    stage: &Path,
    manifest: &ProjectAssetManifest,
    base_assets: &[BaseAsset],
    nonvoice_destinations: &BTreeMap<String, String>,
    selections: &[VoiceSelection],
    report: &StrictVoiceImportDocument,
    quarantined: &[(&NamedOgg, String)],
    ignored_quarantined: &[(&NamedOgg, String)],
) -> Result<StagedVoice> {
    let voice_stage = stage.join("voice");
    let generated_stage = stage.join("generated");
    fs::create_dir_all(&voice_stage).map_err(|source| io_at(&voice_stage, source))?;
    fs::create_dir_all(&generated_stage).map_err(|source| io_at(&generated_stage, source))?;

    let mut entries = Vec::new();
    let mut catalog_assets = Vec::new();
    let mut logical_keys = BTreeSet::new();
    let manifest_by_path = manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();

    for base in base_assets
        .iter()
        .filter(|asset| asset.category != SemanticAudioCategory::Voice)
    {
        let source_file = base
            .file
            .as_ref()
            .ok_or_else(|| invalid_error("non-voice asset lost its runtime file"))?;
        let mut file = source_file.clone();
        file.path = nonvoice_destinations
            .get(&source_file.path)
            .cloned()
            .ok_or_else(|| {
                invalid_error(format!(
                    "non-voice destination plan is missing {:?}",
                    source_file.path
                ))
            })?;
        let logical_key = file
            .path
            .strip_prefix("audio/")
            .and_then(|value| value.strip_suffix(".ogg"))
            .ok_or_else(|| invalid_error(format!("invalid non-voice path {:?}", file.path)))?
            .to_owned();
        if !logical_keys.insert(folded(&logical_key)) {
            return invalid(format!("duplicate runtime logical key {logical_key:?}"));
        }
        let target =
            stage.join(native_path(file.path.strip_prefix("audio/").ok_or_else(
                || invalid_error("non-voice destination lost audio/ prefix"),
            )?));
        let source = VoiceSource {
            absolute_path: asset_root.join(native_path(&source_file.path)),
            source_path: source_file.path.clone(),
            bytes: source_file.bytes,
            blake3: source_file.blake3.clone(),
        };
        copy_and_verify(&source, &target)?;
        let previous_entry = manifest_by_path
            .get(source_file.path.as_str())
            .ok_or_else(|| {
                invalid_error(format!(
                    "non-voice source manifest entry is missing for {:?}",
                    source_file.path
                ))
            })?;
        entries.push(ProjectAssetFile {
            source_path: previous_entry.source_path.clone(),
            path: file.path.clone(),
            kind: ProjectAssetKind::Audio,
            bytes: file.bytes,
            blake3: file.blake3.clone(),
        });
        catalog_assets.push(StrictAudioAsset {
            logical_key,
            aliases: Vec::new(),
            true_name: base.true_name.clone(),
            category: base.category.clone(),
            owner: base.owner.clone(),
            scope: base.scope.clone(),
            files: vec![file],
        });
    }

    for voice in selections {
        if !logical_keys.insert(folded(&voice.logical_key)) {
            return invalid(format!(
                "duplicate runtime logical key {:?}",
                voice.logical_key
            ));
        }
        let line = portable_component(&voice.true_name, "voice line")?;
        let english_path = format!("audio/voice/en/{}/{line}.ogg", voice.owner);
        let english_target = stage.join(native_path(
            english_path
                .strip_prefix("audio/")
                .ok_or_else(|| invalid_error("English destination lost audio/ prefix"))?,
        ));
        copy_and_verify(&voice.english, &english_target)?;
        let english_file = StrictAudioFile {
            locale: Some(DEFAULT_LOCALE.to_owned()),
            path: english_path.clone(),
            bytes: voice.english.bytes,
            blake3: voice.english.blake3.clone(),
            language_neutral: false,
        };
        entries.push(ProjectAssetFile {
            source_path: voice.english.source_path.clone(),
            path: english_path,
            kind: ProjectAssetKind::Audio,
            bytes: voice.english.bytes,
            blake3: voice.english.blake3.clone(),
        });
        let mut files = vec![english_file];
        if let Some(russian) = &voice.russian {
            let russian_path = format!("audio/voice/ru/{}/{line}.ogg", voice.owner);
            let russian_target = stage.join(native_path(
                russian_path
                    .strip_prefix("audio/")
                    .ok_or_else(|| invalid_error("Russian destination lost audio/ prefix"))?,
            ));
            copy_and_verify(russian, &russian_target)?;
            files.push(StrictAudioFile {
                locale: Some(RUSSIAN_LOCALE.to_owned()),
                path: russian_path.clone(),
                bytes: russian.bytes,
                blake3: russian.blake3.clone(),
                language_neutral: false,
            });
            entries.push(ProjectAssetFile {
                source_path: russian.source_path.clone(),
                path: russian_path,
                kind: ProjectAssetKind::Audio,
                bytes: russian.bytes,
                blake3: russian.blake3.clone(),
            });
        }
        catalog_assets.push(StrictAudioAsset {
            logical_key: voice.logical_key.clone(),
            aliases: Vec::new(),
            true_name: voice.true_name.clone(),
            category: SemanticAudioCategory::Voice,
            owner: voice.owner.clone(),
            scope: voice.scope.clone(),
            files,
        });
    }

    catalog_assets.sort_by(|left, right| left.logical_key.cmp(&right.logical_key));
    let counts = strict_catalog_counts(&catalog_assets)?;
    let catalog = StrictAudioCatalog {
        schema: STRICT_AUDIO_CATALOG_SCHEMA.to_owned(),
        fallback_locale: DEFAULT_LOCALE.to_owned(),
        locale_fallbacks: BTreeMap::from([(RUSSIAN_LOCALE.to_owned(), DEFAULT_LOCALE.to_owned())]),
        counts,
        assets: catalog_assets,
    };
    validate_strict_catalog(&catalog)?;
    let catalog_bytes = serialize_editable_runtime_catalog(&catalog)?;
    write_new(&stage.join("runtime/audio.json"), &catalog_bytes)?;
    entries.push(ProjectAssetFile {
        source_path: "voice-v3/runtime-catalog".to_owned(),
        path: STRICT_AUDIO_CATALOG_PATH.to_owned(),
        kind: ProjectAssetKind::Data,
        bytes: catalog_bytes.len() as u64,
        blake3: blake3::hash(&catalog_bytes).to_hex().to_string(),
    });

    let mut report_bytes =
        serde_json::to_vec_pretty(report).map_err(|source| PipelineError::Json {
            path: "voice-localization-v3/report.json".to_owned(),
            source,
        })?;
    report_bytes.push(b'\n');
    write_new(&generated_stage.join("report.json"), &report_bytes)?;
    for (source, relative) in quarantined.iter().chain(ignored_quarantined) {
        let target = generated_stage.join(native_path(relative));
        let proof = VoiceSource {
            absolute_path: source.absolute_path.clone(),
            source_path: source.relative_path.clone(),
            bytes: source.bytes,
            blake3: source.blake3.clone(),
        };
        copy_and_verify(&proof, &target)?;
    }

    entries.sort_by(|left, right| left.path.cmp(&right.path));
    validate_staged_voice(stage, &entries)?;
    Ok(StagedVoice { entries, catalog })
}

pub(super) fn validate_voice_selections(selections: &[VoiceSelection]) -> Result<()> {
    let mut keys = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for selection in selections {
        if !keys.insert(folded(&selection.logical_key)) {
            return invalid(format!(
                "duplicate strict voice key {:?}",
                selection.logical_key
            ));
        }
        let line = portable_component(&selection.true_name, "voice line")?;
        for locale in [
            Some(DEFAULT_LOCALE),
            selection.russian.as_ref().map(|_| RUSSIAN_LOCALE),
        ]
        .into_iter()
        .flatten()
        {
            let path = format!("audio/voice/{locale}/{}/{line}.ogg", selection.owner);
            if path.contains("/variants/") || !paths.insert(folded(&path)) {
                return invalid(format!("strict voice destination collision at {path:?}"));
            }
        }
    }
    Ok(())
}

pub(super) fn is_semantic_audio_file(path: &str) -> bool {
    [
        "audio/music/",
        "audio/ambient/",
        "audio/voice/",
        "audio/sfx/",
    ]
    .iter()
    .any(|prefix| path.starts_with(prefix))
}

pub(super) fn validate_staged_voice(stage: &Path, entries: &[ProjectAssetFile]) -> Result<()> {
    for entry in entries {
        let staged = if entry.path == STRICT_AUDIO_CATALOG_PATH {
            stage.join("runtime/audio.json")
        } else {
            stage.join(native_path(entry.path.strip_prefix("audio/").ok_or_else(
                || invalid_error("staged voice path lost audio/ prefix"),
            )?))
        };
        let metadata = fs::metadata(&staged).map_err(|source| io_at(&staged, source))?;
        let hash = hash_file(&staged)?;
        if !metadata.is_file() || metadata.len() != entry.bytes || hash != entry.blake3 {
            return invalid(format!(
                "staged strict voice identity mismatch for {:?}",
                entry.path
            ));
        }
    }
    Ok(())
}
