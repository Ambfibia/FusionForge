use super::*;

pub const SEMANTIC_AUDIO_CATALOG: &str = "audio/catalog.json";

pub const SEMANTIC_AUDIO_CATALOG_SCHEMA: &str = "ffone.semantic-audio-catalog.v1";

pub const RETROBUTION_AUDIO_ASSET_COUNT: usize = 9_746;

pub(super) const RETROBUTION_EXACT_20260613_AUDIO_ASSET_COUNT: usize = 9_972;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CharacterCreationAudioKind {
    Music,
    Voice(&'static str),
    Sfx,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CharacterCreationAudioRoute {
    pub(super) true_name: &'static str,
    pub(super) path_id: i64,
    pub(super) container: &'static str,
    pub(super) kind: CharacterCreationAudioKind,
}

/// Exact `AssetBundle.m_Container["cc sound/..."]` closure from
/// `CharacterCreation.resourceFile/CustomAssetBundle-bd5f53480423447d7bcaed95cb2a96c8`.
///
/// The bundle preloads hundreds of shared clips, so bundle membership alone is not ownership
/// evidence. Only these 31 container entries are character-creation audio. PathID is part of the
/// identity so same-name tutorial/world variants are never silently routed to this screen.
pub(super) const CHARACTER_CREATION_AUDIO_ROUTES: &[CharacterCreationAudioRoute] = &[
    cc_audio(
        "Beeping",
        286,
        "cc sound/beeping.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "Button",
        236,
        "cc sound/button.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "CharacterCreation_Loop",
        567,
        "cc sound/charactercreation_loop.mp3",
        CharacterCreationAudioKind::Music,
    ),
    cc_audio(
        "Comp_PreTut",
        469,
        "cc sound/comp_pretut.wav",
        CharacterCreationAudioKind::Voice("computress"),
    ),
    cc_audio(
        "Computress_CC01",
        253,
        "cc sound/computress_cc01.wav",
        CharacterCreationAudioKind::Voice("computress"),
    ),
    cc_audio(
        "Computress_CC02",
        626,
        "cc sound/computress_cc02.wav",
        CharacterCreationAudioKind::Voice("computress"),
    ),
    cc_audio(
        "DeeDee_CC_Button",
        459,
        "cc sound/deedee_cc_button.wav",
        CharacterCreationAudioKind::Voice("deedee"),
    ),
    cc_audio(
        "DeeDee_CCBeforeButton",
        416,
        "cc sound/deedee_ccbeforebutton.wav",
        CharacterCreationAudioKind::Voice("deedee"),
    ),
    cc_audio(
        "DeeDee_CCButton",
        450,
        "cc sound/deedee_ccbutton.wav",
        CharacterCreationAudioKind::Voice("deedee"),
    ),
    cc_audio(
        "DeeDee_Laugh02",
        264,
        "cc sound/deedee_laugh02.wav",
        CharacterCreationAudioKind::Voice("deedee"),
    ),
    cc_audio(
        "DeeDee_Laugh_01",
        582,
        "cc sound/deedee_laugh_01.wav",
        CharacterCreationAudioKind::Voice("deedee"),
    ),
    cc_audio(
        "DeeDee_Laugh_03",
        547,
        "cc sound/deedee_laugh_03.wav",
        CharacterCreationAudioKind::Voice("deedee"),
    ),
    cc_audio(
        "DexlabsText_Typed",
        374,
        "cc sound/dexlabstext_typed.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "Dexter_CC01",
        402,
        "cc sound/dexter_cc01.wav",
        CharacterCreationAudioKind::Voice("dexter"),
    ),
    cc_audio(
        "Dexter_CCDeeDeeNo",
        521,
        "cc sound/dexter_ccdeedeeno.wav",
        CharacterCreationAudioKind::Voice("dexter"),
    ),
    cc_audio(
        "Dexter_CCEnginePwr",
        349,
        "cc sound/dexter_ccenginepwr.wav",
        CharacterCreationAudioKind::Voice("dexter"),
    ),
    cc_audio(
        "Dexter_CCHello",
        261,
        "cc sound/dexter_cchello.wav",
        CharacterCreationAudioKind::Voice("dexter"),
    ),
    cc_audio(
        "Dexter_CCInput",
        294,
        "cc sound/dexter_ccinput.wav",
        CharacterCreationAudioKind::Voice("dexter"),
    ),
    cc_audio(
        "Dexter_CCLaunch",
        377,
        "cc sound/dexter_cclaunch.wav",
        CharacterCreationAudioKind::Voice("dexter"),
    ),
    cc_audio(
        "Dexter_CCReadings",
        630,
        "cc sound/dexter_ccreadings.wav",
        CharacterCreationAudioKind::Voice("dexter"),
    ),
    cc_audio(
        "Engine",
        493,
        "cc sound/engine.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "HologramOff",
        542,
        "cc sound/hologramoff.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "HologramOn",
        247,
        "cc sound/hologramon.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "HOLOGRAMS 1",
        484,
        "cc sound/holograms 1.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "Machine_Running",
        575,
        "cc sound/machine_running.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "NanoComm_Check",
        456,
        "cc sound/nanocomm_check.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "Thunderous_Crash",
        227,
        "cc sound/thunderous_crash.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "TimeJumpText_Typed",
        574,
        "cc sound/timejumptext_typed.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "Vibration1",
        617,
        "cc sound/vibration1.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "Vibration2",
        595,
        "cc sound/vibration2.wav",
        CharacterCreationAudioKind::Sfx,
    ),
    cc_audio(
        "Warning_Siren",
        375,
        "cc sound/warning_siren.wav",
        CharacterCreationAudioKind::Sfx,
    ),
];

pub(super) const fn cc_audio(
    true_name: &'static str,
    path_id: i64,
    container: &'static str,
    kind: CharacterCreationAudioKind,
) -> CharacterCreationAudioRoute {
    CharacterCreationAudioRoute {
        true_name,
        path_id,
        container,
        kind,
    }
}

#[derive(Clone, Debug)]
pub struct SemanticAudioInstallOptions {
    pub asset_root: PathBuf,
    pub cook_report: PathBuf,
    pub source_build: String,
    pub(super) expected_audio_assets: usize,
}

impl SemanticAudioInstallOptions {
    pub fn new(
        asset_root: impl Into<PathBuf>,
        cook_report: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            cook_report: cook_report.into(),
            source_build: source_build.into(),
            expected_audio_assets: RETROBUTION_AUDIO_ASSET_COUNT,
        }
    }

    #[cfg(test)]
    pub(super) fn with_expected_audio_assets(mut self, expected: usize) -> Self {
        self.expected_audio_assets = expected;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticAudioCategory {
    Music,
    Ambient,
    Voice,
    Sfx,
}

impl SemanticAudioCategory {
    pub(super) fn directory(&self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Ambient => "ambient",
            Self::Voice => "voice",
            Self::Sfx => "sfx",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAudioClassificationProof {
    pub category_rule: String,
    pub owner_rule: String,
    pub certainty: ClassificationCertainty,
    pub evidence: Vec<String>,
    pub reason: String,
    pub uncertain: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAudioVariant {
    pub normalized_true_name: String,
    pub index: u32,
    pub total: u32,
    pub ordering: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAudioAsset {
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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAudioCatalogCounts {
    pub assets: u64,
    pub bytes: u64,
    pub music: u64,
    pub ambient: u64,
    pub voice: u64,
    pub sfx: u64,
    pub explicit_shared_fallbacks: u64,
    pub variant_assets: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAudioManifestProof {
    pub schema: String,
    pub blake3: String,
    pub original_hashed_ogg_entries: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAudioCookReportProof {
    pub schema: String,
    pub blake3: String,
    pub build_uuid: String,
    pub locale: String,
    pub declared_audio_assets: u64,
    pub audio_mapping_records: u64,
    pub unique_native_audio_paths: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAudioCatalog {
    pub schema: String,
    pub source_build: String,
    pub source_manifest: SemanticAudioManifestProof,
    pub source_cook_report: SemanticAudioCookReportProof,
    pub path_policy: String,
    pub duplicate_policy: String,
    pub classification_policy: String,
    pub counts: SemanticAudioCatalogCounts,
    pub assets: Vec<SemanticAudioAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticAudioInstallReport {
    pub source_build: String,
    pub installed_audio_files: u64,
    pub installed_audio_bytes: u64,
    pub manifest_files: u64,
    pub catalog_path: String,
    pub explicit_shared_fallbacks: u64,
    pub variant_assets: u64,
}

#[derive(Clone, Debug)]
pub(super) struct PreparedAudio {
    pub(super) source: ProjectAssetFile,
    pub(super) true_name: String,
    pub(super) native_key: String,
    pub(super) native_path: String,
    pub(super) provenance: Vec<BTreeMap<String, serde_json::Value>>,
    pub(super) classification: Classification,
    pub(super) variant: Option<SemanticAudioVariant>,
    pub(super) destination: String,
}

pub fn install_semantic_audio(
    options: &SemanticAudioInstallOptions,
) -> Result<SemanticAudioInstallReport> {
    validate_source_build(&options.source_build)?;
    if options.expected_audio_assets == 0 {
        return invalid("expected audio asset count must be positive");
    }

    let asset_root =
        fs::canonicalize(&options.asset_root).map_err(|error| io_at(&options.asset_root, error))?;
    reject_stale_transactions(&asset_root)?;
    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "project manifest schema must be {PROJECT_ASSET_SCHEMA:?}, got {:?}",
            manifest.schema
        ));
    }

    let cook_report_bytes =
        fs::read(&options.cook_report).map_err(|error| io_at(&options.cook_report, error))?;
    let cook_report: CookReport =
        serde_json::from_slice(&cook_report_bytes).map_err(|source| PipelineError::Json {
            path: options.cook_report.display().to_string(),
            source,
        })?;
    if cook_report.schema != COOK_REPORT_SCHEMA {
        return invalid(format!(
            "cook report schema must be {COOK_REPORT_SCHEMA:?}, got {:?}",
            cook_report.schema
        ));
    }
    let expected_audio_assets = select_audio_profile(
        options,
        &manifest,
        &cook_report,
        &sha256_hex(&cook_report_bytes),
    )?;

    let audio_root = asset_root.join("audio");
    fs::create_dir_all(&audio_root).map_err(|error| io_at(&audio_root, error))?;
    let previous = inspect_previous_install(&audio_root, &manifest)?;
    let mut next_manifest = manifest.clone();
    if previous.present {
        next_manifest.files.retain(|entry| !is_owned_entry(entry));
    }

    let mut prepared = prepare_audio(&manifest, &cook_report, expected_audio_assets)?;
    assign_destinations(&mut prepared)?;
    validate_manifest_destination_closure(&next_manifest, &prepared)?;

    let stage = asset_root.join(format!(".semantic-audio-stage-{}", std::process::id()));
    let backup = asset_root.join(format!(".semantic-audio-backup-{}", std::process::id()));
    if stage.exists() || backup.exists() {
        return invalid(format!(
            "stale semantic-audio transaction path exists at {} or {}",
            stage.display(),
            backup.display()
        ));
    }
    fs::create_dir(&stage).map_err(|error| io_at(&stage, error))?;

    let manifest_blake3 = blake3::hash(&manifest_bytes).to_hex().to_string();
    let cook_report_blake3 = blake3::hash(&cook_report_bytes).to_hex().to_string();
    let staged = stage_semantic_audio(
        &asset_root,
        &stage,
        &options.source_build,
        &prepared,
        &manifest,
        &cook_report,
        &manifest_blake3,
        &cook_report_blake3,
    );
    let (mut semantic_entries, catalog, installed_audio_bytes) = match staged {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
    };

    next_manifest.files.append(&mut semantic_entries);
    next_manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    validate_manifest_paths(&next_manifest)?;

    if let Err(error) =
        commit_transaction(&audio_root, &stage, &backup, &manifest_path, &next_manifest)
    {
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_dir_all(&backup);
        return Err(error);
    }

    Ok(SemanticAudioInstallReport {
        source_build: options.source_build.clone(),
        installed_audio_files: prepared.len() as u64,
        installed_audio_bytes,
        manifest_files: next_manifest.files.len() as u64,
        catalog_path: SEMANTIC_AUDIO_CATALOG.to_owned(),
        explicit_shared_fallbacks: catalog.counts.explicit_shared_fallbacks,
        variant_assets: catalog.counts.variant_assets,
    })
}

pub(super) fn select_audio_profile(
    options: &SemanticAudioInstallOptions,
    manifest: &ProjectAssetManifest,
    cook_report: &CookReport,
    cook_report_sha256: &str,
) -> Result<usize> {
    if cook_report.build_uuid != RETROBUTION_EXACT_20260613_BUILD_UUID {
        return Ok(options.expected_audio_assets);
    }

    let mut mismatches = Vec::new();
    if cook_report_sha256 != RETROBUTION_EXACT_20260613_COOK_REPORT_SHA256 {
        mismatches.push(format!(
            "cook-report SHA-256 {cook_report_sha256:?} != {:?}",
            RETROBUTION_EXACT_20260613_COOK_REPORT_SHA256
        ));
    }
    if manifest.source_pack.schema != CONTENT_PACK_SCHEMA {
        mismatches.push(format!(
            "source-pack schema {:?} != {CONTENT_PACK_SCHEMA:?}",
            manifest.source_pack.schema
        ));
    }
    if manifest.source_pack.manifest_blake3 != RETROBUTION_EXACT_20260613_PACK_MANIFEST_BLAKE3 {
        mismatches.push(format!(
            "source-pack manifest BLAKE3 {:?} != {:?}",
            manifest.source_pack.manifest_blake3, RETROBUTION_EXACT_20260613_PACK_MANIFEST_BLAKE3
        ));
    }
    if manifest.protocol != 104 {
        mismatches.push(format!("project protocol {} != 104", manifest.protocol));
    }
    if manifest.locale != "ru-RU" {
        mismatches.push(format!("project locale {:?} != \"ru-RU\"", manifest.locale));
    }
    if cook_report.locale != "ru-RU" {
        mismatches.push(format!(
            "cook-report locale {:?} != \"ru-RU\"",
            cook_report.locale
        ));
    }
    if !mismatches.is_empty() {
        return invalid(format!(
            "exact Retrobution 20260613 semantic-audio profile provenance mismatch: {}",
            mismatches.join("; ")
        ));
    }

    Ok(RETROBUTION_EXACT_20260613_AUDIO_ASSET_COUNT)
}

pub(super) fn prepare_audio(
    manifest: &ProjectAssetManifest,
    cook_report: &CookReport,
    expected: usize,
) -> Result<Vec<PreparedAudio>> {
    if cook_report.counts.audio != expected as u64 {
        return invalid(format!(
            "cook report declares {} audio assets, expected {expected}",
            cook_report.counts.audio
        ));
    }

    let mut groups = BTreeMap::<String, MappingGroup>::new();
    let mut audio_mapping_records = 0_usize;
    for mapping in cook_report
        .mappings
        .iter()
        .filter(|mapping| mapping.kind == "audio")
    {
        audio_mapping_records += 1;
        validate_mapping(mapping)?;
        match groups.get_mut(&mapping.native_path) {
            Some(group) => {
                if group.true_name != mapping.name || group.native_key != mapping.native_key {
                    return invalid(format!(
                        "cook mappings disagree about true m_Name/nativeKey for {:?}",
                        mapping.native_path
                    ));
                }
                group.provenance.push(mapping.source.clone());
            }
            None => {
                groups.insert(
                    mapping.native_path.clone(),
                    MappingGroup {
                        true_name: mapping.name.clone(),
                        native_key: mapping.native_key.clone(),
                        native_path: mapping.native_path.clone(),
                        provenance: vec![mapping.source.clone()],
                    },
                );
            }
        }
    }
    if audio_mapping_records == 0 || groups.len() != expected {
        return invalid(format!(
            "cook report has {audio_mapping_records} audio mapping records and {} unique native paths; expected {expected} unique paths",
            groups.len()
        ));
    }
    for group in groups.values_mut() {
        group.provenance.sort_by_key(canonical_provenance);
    }

    let originals = manifest
        .files
        .iter()
        .filter(|entry| is_original_hashed_ogg(entry))
        .cloned()
        .collect::<Vec<_>>();
    if originals.len() != expected {
        return invalid(format!(
            "project manifest has {} original hashed OGG entries; expected {expected}",
            originals.len()
        ));
    }

    let mut seen_source_paths = BTreeSet::new();
    let mut prepared = Vec::with_capacity(expected);
    for source in originals {
        if source.kind != ProjectAssetKind::Audio {
            return invalid(format!(
                "original OGG manifest entry {:?} is not kind=audio",
                source.path
            ));
        }
        if source.path != source.source_path {
            return invalid(format!(
                "original hashed OGG entry {:?} must retain identical path/source_path",
                source.path
            ));
        }
        if !seen_source_paths.insert(source.source_path.clone()) {
            return invalid(format!(
                "duplicate original OGG manifest source_path {:?}",
                source.source_path
            ));
        }
        validate_hash(&source.blake3, "manifest audio blake3")?;
        validate_hash_suffix(&source)?;
        let group = groups.remove(&source.source_path).ok_or_else(|| {
            invalid_error(format!(
                "manifest source_path {:?} has no exact cook mapping nativePath",
                source.source_path
            ))
        })?;
        let classification = classify(&group.true_name, &group.provenance)?;
        prepared.push(PreparedAudio {
            source,
            true_name: group.true_name,
            native_key: group.native_key,
            native_path: group.native_path,
            provenance: group.provenance,
            classification,
            variant: None,
            destination: String::new(),
        });
    }
    if let Some(unused) = groups.keys().next() {
        return invalid(format!(
            "cook mapping nativePath {unused:?} was not consumed by an original manifest source_path"
        ));
    }
    prepared.sort_by(|left, right| left.native_path.cmp(&right.native_path));
    Ok(prepared)
}

pub(super) fn is_hashed_audio_path(path: &str) -> bool {
    let Some(file) = path.strip_prefix("audio/") else {
        return false;
    };
    if file.contains('/') || !file.ends_with(".ogg") {
        return false;
    }
    let stem = &file[..file.len() - 4];
    let Some((name, suffix)) = stem.rsplit_once("--") else {
        return false;
    };
    !name.is_empty()
        && suffix.len() == 16
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn classify_character_creation_audio(
    true_name: &str,
    normalized_name: &str,
    provenance: &[BTreeMap<String, serde_json::Value>],
) -> Option<Classification> {
    let route = CHARACTER_CREATION_AUDIO_ROUTES.iter().find(|route| {
        normalized_identity(route.true_name) == normalized_name
            && provenance.iter().any(|source| {
                source.get("bundle").and_then(serde_json::Value::as_str)
                    == Some("CharacterCreation.resourceFile")
                    && source.get("pathId").and_then(serde_json::Value::as_i64)
                        == Some(route.path_id)
                    && source
                        .get("file")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|file| {
                            file.starts_with(
                                "CharacterCreation.resourceFile/CustomAssetBundle-bd5f53480423447d7bcaed95cb2a96c8/",
                            ) && file.contains(&format!("/{}__", route.path_id))
                        })
            })
    })?;

    let (category, owner, owner_rule) = match route.kind {
        CharacterCreationAudioKind::Music => (
            SemanticAudioCategory::Music,
            "shared",
            "character-creation music container entry",
        ),
        CharacterCreationAudioKind::Voice(owner) => (
            SemanticAudioCategory::Voice,
            owner,
            "speaker is explicit in the exact character-creation container entry",
        ),
        CharacterCreationAudioKind::Sfx => (
            SemanticAudioCategory::Sfx,
            "character_creation",
            "exact character-creation screen SFX container entry",
        ),
    };
    Some(Classification {
        category,
        owner: owner.to_owned(),
        proof: SemanticAudioClassificationProof {
            category_rule: "character-creation-assetbundle-container".to_owned(),
            owner_rule: owner_rule.to_owned(),
            certainty: ClassificationCertainty::SourceProvenance,
            evidence: vec![format!(
                "AssetBundle.m_Container[{container:?}] -> CharacterCreation.resourceFile/CustomAssetBundle-bd5f53480423447d7bcaed95cb2a96c8#{path_id} ({true_name:?})",
                container = route.container,
                path_id = route.path_id,
            )],
            reason:
                "The exact CharacterCreation AssetBundle container route proves screen ownership; preload-only clips are excluded."
                    .to_owned(),
            uncertain: false,
        },
    })
}

pub(super) fn voice_owner(normalized_name: &str, markers: &[&str]) -> Result<(String, String)> {
    let marker_position = markers
        .iter()
        .filter_map(|marker| {
            normalized_name
                .find(marker)
                .map(|position| (position, *marker))
        })
        .min_by_key(|(position, _)| *position);
    let candidate = marker_position
        .map(|(position, _)| &normalized_name[..position])
        .or_else(|| normalized_name.rsplit_once('_').map(|(owner, _)| owner))
        .unwrap_or("");
    let owner = if candidate.is_empty() {
        "shared".to_owned()
    } else {
        portable_component(candidate, "voice owner")?
    };
    let rule = if owner == "shared" {
        "voice m_Name contains no provable speaker prefix; explicit shared owner".to_owned()
    } else {
        "speaker prefix preceding the first explicit voice/action marker in true m_Name".to_owned()
    };
    Ok((owner, rule))
}
