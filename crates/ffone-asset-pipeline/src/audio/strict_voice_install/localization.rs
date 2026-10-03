use super::*;

pub(super) const EXPECTED_LOCALIZED_KEYS: usize = 1_059;

pub(super) const DEFAULT_LOCALE: &str = "en";

pub(super) const RUSSIAN_LOCALE: &str = "ru";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LocalizedKeyProof {
    pub(super) logical_key: String,
    pub(super) true_name: String,
    pub(super) english_source: String,
    pub(super) english_blake3: String,
    pub(super) russian_source: String,
    pub(super) russian_blake3: String,
    pub(super) proof: String,
}

pub fn install_strict_localized_voice(
    options: &StrictVoiceInstallOptions,
) -> Result<StrictVoiceInstallReport> {
    validate_source_build(&options.source_build)?;
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    let russian_root = canonical_directory(&options.russian_source_root, "Russian voice root")?;
    let cook_pack_root = canonical_directory(&options.cook_pack_root, "fresh cook pack root")?;
    let english_root =
        canonical_directory(&options.english_source_root, "clean English audio root")?;
    let generated_target = absolute_output_path(&options.generated_report_root)?;
    if generated_target.starts_with(&asset_root) {
        return invalid(format!(
            "voice import evidence/quarantine must be outside runtime assets: {}",
            generated_target.display()
        ));
    }

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_before =
        fs::read(&manifest_path).map_err(|source| io_at(&manifest_path, source))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_before).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    validate_manifest(&manifest)?;

    let catalog_path = options.source_catalog.clone().unwrap_or_else(|| {
        let installed_v3 = asset_root.join(native_path(STRICT_AUDIO_CATALOG_PATH));
        if installed_v3.is_file() {
            installed_v3
        } else {
            asset_root.join(native_path(SEMANTIC_AUDIO_CATALOG))
        }
    });
    let catalog_bytes = fs::read(&catalog_path).map_err(|source| io_at(&catalog_path, source))?;
    if options.source_catalog.is_none() {
        validate_installed_catalog_identity(&manifest, &catalog_bytes)?;
    }
    let base_assets = load_base_assets(&catalog_path, &catalog_bytes)?;
    validate_non_voice_base(&asset_root, &manifest, &base_assets)?;

    let cook_report_bytes =
        fs::read(&options.cook_report).map_err(|source| io_at(&options.cook_report, source))?;
    let cook_report: CookReport =
        serde_json::from_slice(&cook_report_bytes).map_err(|source| PipelineError::Json {
            path: options.cook_report.display().to_string(),
            source,
        })?;
    validate_cook_report(&cook_report)?;

    let discovered_russian_sources = collect_named_oggs(&russian_root)?;
    if discovered_russian_sources.len() != EXPECTED_DISCOVERED_RUSSIAN_FILES {
        return invalid(format!(
            "strict voice preflight expected {EXPECTED_DISCOVERED_RUSSIAN_FILES} discovered Russian OGG files, found {}",
            discovered_russian_sources.len()
        ));
    }
    let russian_sources = discovered_russian_sources
        .iter()
        .filter(|source| !source.true_name.to_ascii_lowercase().ends_with(".ogg"))
        .cloned()
        .collect::<Vec<_>>();
    let ignored_russian_sources = discovered_russian_sources
        .iter()
        .filter(|source| source.true_name.to_ascii_lowercase().ends_with(".ogg"))
        .collect::<Vec<_>>();
    if russian_sources.len() != EXPECTED_RUSSIAN_SOURCE_FILES {
        return invalid(format!(
            "strict voice preflight expected {EXPECTED_RUSSIAN_SOURCE_FILES} Russian OGG files, found {}",
            russian_sources.len()
        ));
    }
    if ignored_russian_sources.len() != EXPECTED_IGNORED_RUSSIAN_FILES {
        return invalid(format!(
            "strict voice preflight expected {EXPECTED_IGNORED_RUSSIAN_FILES} malformed Russian OGG, found {}",
            ignored_russian_sources.len()
        ));
    }
    let english_sources = collect_named_oggs(&english_root)?;
    let fresh_sources = collect_fresh_audio(&cook_report, &cook_pack_root)?;

    let russian_by_name = group_named_oggs(&russian_sources);
    let english_by_name = group_named_oggs(&english_sources);
    let fresh_by_name = group_fresh_audio(&fresh_sources);
    let base_voice_by_name = group_base_voice(&base_assets);

    let mut voice_names = base_voice_by_name.keys().cloned().collect::<BTreeSet<_>>();
    voice_names.extend(russian_by_name.keys().cloned());

    let mut selections = Vec::new();
    let mut localized_proofs = Vec::new();
    let mut ambiguous_proofs = Vec::new();
    let mut quarantined = Vec::<(&NamedOgg, String)>::new();
    let mut ignored_quarantined = Vec::<(&NamedOgg, String)>::new();
    let mut unresolved_english = Vec::new();
    let identical_locale_payloads = 0_usize;

    for folded_name in voice_names {
        let base_group = base_voice_by_name.get(&folded_name);
        let russian_group = russian_by_name.get(&folded_name);
        let clean_group = english_by_name.get(&folded_name);
        let fresh_group = fresh_by_name.get(&folded_name);
        let true_name = canonical_true_name(base_group, russian_group, clean_group, fresh_group)?;
        let owner = canonical_owner(&true_name, base_group)?;
        let logical_key = voice_logical_key(&owner, &true_name)?;
        let scope = canonical_scope(base_group);

        if let Some(russian_group) = russian_group {
            let resolution = resolve_localized_voice(
                &logical_key,
                &true_name,
                russian_group,
                clean_group,
                fresh_group,
            )?;
            match resolution {
                LocalizedResolution::Paired {
                    english,
                    russian,
                    proof,
                } => {
                    if english.blake3 == russian.blake3 {
                        return invalid(format!(
                            "English and Russian payloads are identical without explicit languageNeutral for {logical_key:?}"
                        ));
                    }
                    localized_proofs.push(LocalizedKeyProof {
                        logical_key: logical_key.clone(),
                        true_name: true_name.clone(),
                        english_source: english.source_path.clone(),
                        english_blake3: english.blake3.clone(),
                        russian_source: russian.source_path.clone(),
                        russian_blake3: russian.blake3.clone(),
                        proof,
                    });
                    selections.push(VoiceSelection {
                        logical_key,
                        true_name,
                        owner,
                        scope,
                        english,
                        russian: Some(russian),
                    });
                }
                LocalizedResolution::Ambiguous {
                    english,
                    candidates,
                } => {
                    if !folded_name.starts_with("f_pirate2_") {
                        return invalid(format!(
                            "only the proven F_Pirate2 duplicate set may enter Russian quarantine; got {true_name:?}"
                        ));
                    }
                    let mut candidate_proofs = Vec::new();
                    for candidate in candidates {
                        let quarantine_path = format!(
                            "quarantine/ru-ambiguous/{}/{}__{}.ogg",
                            portable_component(&candidate.container, "Russian container")?,
                            candidate.path_id,
                            portable_component(&candidate.true_name, "Russian true name")?
                        );
                        candidate_proofs.push(QuarantineFileProof {
                            source: candidate.relative_path.clone(),
                            quarantine_path: quarantine_path.clone(),
                            bytes: candidate.bytes,
                            blake3: candidate.blake3.clone(),
                        });
                        quarantined.push((candidate, quarantine_path));
                    }
                    ambiguous_proofs.push(AmbiguousKeyProof {
                        logical_key: logical_key.clone(),
                        true_name: true_name.clone(),
                        english_source: english.source_path.clone(),
                        english_blake3: english.blake3.clone(),
                        russian_candidates: candidate_proofs,
                        reason: "two distinct Russian payloads share one legacy logical name; no authored takeId pairs them across locales".to_owned(),
                    });
                    selections.push(VoiceSelection {
                        logical_key,
                        true_name,
                        owner,
                        scope,
                        english,
                        russian: None,
                    });
                }
            }
            continue;
        }

        match resolve_clean_english(clean_group, fresh_group)? {
            Some(english) => selections.push(VoiceSelection {
                logical_key,
                true_name,
                owner,
                scope,
                english,
                russian: None,
            }),
            None => unresolved_english.push(true_name),
        }
    }

    localized_proofs.sort_by(|left, right| left.logical_key.cmp(&right.logical_key));
    ambiguous_proofs.sort_by(|left, right| left.logical_key.cmp(&right.logical_key));
    unresolved_english.sort();
    selections.sort_by(|left, right| left.logical_key.cmp(&right.logical_key));

    if localized_proofs.len() != EXPECTED_LOCALIZED_KEYS
        || ambiguous_proofs.len() != EXPECTED_AMBIGUOUS_KEYS
        || quarantined.len() != EXPECTED_QUARANTINED_FILES
        || identical_locale_payloads != 0
    {
        return invalid(format!(
            "strict voice preflight closure mismatch: localized={} (expected {EXPECTED_LOCALIZED_KEYS}), ambiguousKeys={} (expected {EXPECTED_AMBIGUOUS_KEYS}), quarantineFiles={} (expected {EXPECTED_QUARANTINED_FILES}), identicalLocalePayloads={identical_locale_payloads} (expected 0)",
            localized_proofs.len(),
            ambiguous_proofs.len(),
            quarantined.len()
        ));
    }

    validate_voice_selections(&selections)?;
    let nonvoice_destinations = plan_nonvoice_destinations(&base_assets)?;

    let ignored_proofs = ignored_russian_sources
        .iter()
        .map(|source| {
            let quarantine_path = format!(
                "quarantine/ru-ignored/{}/{}__{}.ogg",
                portable_component(&source.container, "ignored Russian container")?,
                source.path_id,
                portable_component(&source.true_name, "ignored Russian true name")?
            );
            ignored_quarantined.push((*source, quarantine_path.clone()));
            Ok(IgnoredFileProof {
                source: source.relative_path.clone(),
                quarantine_path,
                reason: "malformed double .ogg extension makes the parsed true name end in .ogg; no logical voice key may be inferred".to_owned(),
                bytes: source.bytes,
                blake3: source.blake3.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let report = StrictVoiceImportDocument {
        schema: STRICT_VOICE_REPORT_SCHEMA.to_owned(),
        source_build: options.source_build.clone(),
        inputs: BTreeMap::from([
            (
                "cleanEnglishAudio".to_owned(),
                ImportInputProof {
                    path: english_root.display().to_string(),
                    blake3: None,
                },
            ),
            (
                "freshCookPack".to_owned(),
                ImportInputProof {
                    path: cook_pack_root.display().to_string(),
                    blake3: None,
                },
            ),
            (
                "freshCookReport".to_owned(),
                ImportInputProof {
                    path: options.cook_report.display().to_string(),
                    blake3: Some(blake3::hash(&cook_report_bytes).to_hex().to_string()),
                },
            ),
            (
                "russianVoice".to_owned(),
                ImportInputProof {
                    path: russian_root.display().to_string(),
                    blake3: None,
                },
            ),
            (
                "sourceCatalog".to_owned(),
                ImportInputProof {
                    path: catalog_path.display().to_string(),
                    blake3: Some(blake3::hash(&catalog_bytes).to_hex().to_string()),
                },
            ),
        ]),
        policy: vec![
            "A voice logical key has exactly one authored line identity and no inferred variants."
                .to_owned(),
            "English is selected from the clean original client or from a fresh same-binding non-Russian payload."
                .to_owned(),
            "Russian is copied only from the supplied translation root after exact name/binding/hash checks."
                .to_owned(),
            "Container and pathId are import evidence, never take identity.".to_owned(),
            "Equal en/ru payloads are forbidden unless languageNeutral is explicitly authored."
                .to_owned(),
            "Ambiguous Russian payloads remain outside assets/game and runtime falls back to English."
                .to_owned(),
        ],
        counts: ImportReportCounts {
            discovered_russian_files: discovered_russian_sources.len() as u64,
            accepted_russian_files: russian_sources.len() as u64,
            ignored_russian_files: ignored_proofs.len() as u64,
            localized_keys: localized_proofs.len() as u64,
            english_only_ambiguous_keys: ambiguous_proofs.len() as u64,
            quarantined_russian_files: quarantined.len() as u64,
            english_voice_keys: selections.len() as u64,
            unresolved_english_keys: unresolved_english.len() as u64,
            identical_locale_payloads: identical_locale_payloads as u64,
        },
        localized: localized_proofs,
        ambiguous: ambiguous_proofs,
        ignored: ignored_proofs,
        unresolved_english,
    };

    if options.preflight_only {
        let non_voice = base_assets
            .iter()
            .filter(|asset| asset.category != SemanticAudioCategory::Voice)
            .collect::<Vec<_>>();
        let english_voice_files = selections.len() as u64;
        let russian_voice_files = selections
            .iter()
            .filter(|selection| selection.russian.is_some())
            .count() as u64;
        let non_voice_bytes = non_voice
            .iter()
            .filter_map(|asset| asset.file.as_ref())
            .try_fold(0_u64, |total, file| {
                total
                    .checked_add(file.bytes)
                    .ok_or_else(|| invalid_error("preflight non-voice byte count overflow"))
            })?;
        let voice_bytes = selections.iter().try_fold(0_u64, |total, selection| {
            let total = total
                .checked_add(selection.english.bytes)
                .ok_or_else(|| invalid_error("preflight voice byte count overflow"))?;
            selection.russian.as_ref().map_or(Ok(total), |russian| {
                total
                    .checked_add(russian.bytes)
                    .ok_or_else(|| invalid_error("preflight voice byte count overflow"))
            })
        })?;
        let retained_manifest_files = manifest
            .files
            .iter()
            .filter(|entry| {
                !is_semantic_audio_file(&entry.path)
                    && entry.path != SEMANTIC_AUDIO_CATALOG
                    && entry.path != STRICT_AUDIO_CATALOG_PATH
            })
            .count() as u64;
        return Ok(StrictVoiceInstallReport {
            committed: false,
            logical_audio_assets: non_voice.len() as u64 + english_voice_files,
            installed_audio_files: non_voice.len() as u64
                + english_voice_files
                + russian_voice_files,
            installed_audio_bytes: non_voice_bytes
                .checked_add(voice_bytes)
                .ok_or_else(|| invalid_error("preflight total audio byte count overflow"))?,
            english_voice_files,
            russian_voice_files,
            localized_voice_keys: report.counts.localized_keys,
            english_only_ambiguous_keys: report.counts.english_only_ambiguous_keys,
            quarantined_russian_files: report.counts.quarantined_russian_files,
            ignored_russian_files: report.counts.ignored_russian_files,
            unresolved_english_keys: report.counts.unresolved_english_keys,
            identical_locale_payloads: report.counts.identical_locale_payloads,
            catalog_path: STRICT_AUDIO_CATALOG_PATH.to_owned(),
            generated_report_path: generated_target.join("report.json").display().to_string(),
            manifest_files: retained_manifest_files
                + non_voice.len() as u64
                + english_voice_files
                + russian_voice_files
                + 1,
        });
    }

    let stamp = transaction_stamp();
    let stage = asset_root.join(format!(".strict-voice-stage-{stamp}"));
    let backup = asset_root.join(format!(".strict-voice-backup-{stamp}"));
    if stage.exists() || backup.exists() {
        return invalid("strict voice transaction path already exists");
    }
    fs::create_dir(&stage).map_err(|source| io_at(&stage, source))?;

    let staged = stage_strict_voice(
        &asset_root,
        &stage,
        &manifest,
        &base_assets,
        &nonvoice_destinations,
        &selections,
        &report,
        &quarantined,
        &ignored_quarantined,
    );
    let staged = match staged {
        Ok(staged) => staged,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
    };

    let mut next_manifest = manifest.clone();
    next_manifest.files.retain(|entry| {
        !is_semantic_audio_file(&entry.path)
            && entry.path != SEMANTIC_AUDIO_CATALOG
            && entry.path != STRICT_AUDIO_CATALOG_PATH
    });
    next_manifest.files.extend(staged.entries.clone());
    next_manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    validate_manifest(&next_manifest)?;

    let manifest_current =
        fs::read(&manifest_path).map_err(|source| io_at(&manifest_path, source))?;
    if manifest_current != manifest_before {
        let _ = fs::remove_dir_all(&stage);
        return invalid("asset manifest changed while strict voice v3 was staged");
    }

    commit_transaction(
        &asset_root.join("audio"),
        &asset_root.join(native_path(SEMANTIC_AUDIO_CATALOG)),
        &asset_root.join(native_path(STRICT_AUDIO_CATALOG_PATH)),
        &generated_target,
        &stage,
        &backup,
        &manifest_path,
        &next_manifest,
    )?;

    Ok(StrictVoiceInstallReport {
        committed: true,
        logical_audio_assets: staged.catalog.counts.assets,
        installed_audio_files: staged.catalog.counts.files,
        installed_audio_bytes: staged.catalog.counts.file_bytes,
        english_voice_files: staged.catalog.counts.english_voice_files,
        russian_voice_files: staged.catalog.counts.russian_voice_files,
        localized_voice_keys: report.counts.localized_keys,
        english_only_ambiguous_keys: report.counts.english_only_ambiguous_keys,
        quarantined_russian_files: report.counts.quarantined_russian_files,
        ignored_russian_files: report.counts.ignored_russian_files,
        unresolved_english_keys: report.counts.unresolved_english_keys,
        identical_locale_payloads: report.counts.identical_locale_payloads,
        catalog_path: STRICT_AUDIO_CATALOG_PATH.to_owned(),
        generated_report_path: generated_target.join("report.json").display().to_string(),
        manifest_files: next_manifest.files.len() as u64,
    })
}

pub(super) enum LocalizedResolution<'a> {
    Paired {
        english: VoiceSource,
        russian: VoiceSource,
        proof: String,
    },
    Ambiguous {
        english: VoiceSource,
        candidates: Vec<&'a NamedOgg>,
    },
}

pub(super) fn resolve_localized_voice<'a>(
    logical_key: &str,
    true_name: &str,
    russian: &'a [&'a NamedOgg],
    clean: Option<&Vec<&NamedOgg>>,
    fresh: Option<&Vec<&FreshAudio>>,
) -> Result<LocalizedResolution<'a>> {
    let russian_hashes = russian
        .iter()
        .map(|source| source.blake3.as_str())
        .collect::<BTreeSet<_>>();
    let bound_fresh = fresh
        .into_iter()
        .flatten()
        .copied()
        .filter(|candidate| {
            russian
                .iter()
                .any(|source| fresh_matches_binding(candidate, source))
        })
        .collect::<Vec<_>>();
    let considered_fresh = if bound_fresh.is_empty() {
        fresh.into_iter().flatten().copied().collect::<Vec<_>>()
    } else {
        bound_fresh
    };
    let fresh_other = unique_fresh_by_hash(
        considered_fresh
            .iter()
            .copied()
            .filter(|candidate| !russian_hashes.contains(candidate.blake3.as_str())),
    );
    let clean_other = unique_named_by_hash(
        clean
            .into_iter()
            .flatten()
            .copied()
            .filter(|candidate| !russian_hashes.contains(candidate.blake3.as_str())),
    );

    let (english, proof) = match (fresh_other.as_slice(), clean_other.as_slice()) {
        ([candidate], _) => (
            voice_source_from_fresh(candidate),
            "fresh cook contains exactly one non-Russian payload for the same logical name/binding"
                .to_owned(),
        ),
        ([], [candidate]) => (
            voice_source_from_named(candidate, "clean-en"),
            "fresh cook contains only the translated payload; English is the unique distinct payload in the clean original client".to_owned(),
        ),
        ([], []) => {
            return invalid(format!(
                "no distinct English payload is proven for localized key {logical_key:?} ({true_name:?})"
            ));
        }
        (many, clean_candidates) => {
            let common = many
                .iter()
                .copied()
                .filter(|fresh_candidate| {
                    clean_candidates
                        .iter()
                        .any(|clean_candidate| clean_candidate.blake3 == fresh_candidate.blake3)
                })
                .collect::<Vec<_>>();
            let common = unique_fresh_by_hash(common.into_iter());
            let [candidate] = common.as_slice() else {
                return invalid(format!(
                    "multiple non-Russian payloads remain for localized key {logical_key:?}"
                ));
            };
            (
                voice_source_from_fresh(candidate),
                "one payload is independently present in both the fresh cook and clean original client"
                    .to_owned(),
            )
        }
    };

    match russian.len() {
        1 => Ok(LocalizedResolution::Paired {
            english,
            russian: voice_source_from_named(russian[0], "ru-translation"),
            proof,
        }),
        2 if russian_hashes.len() == 2 => Ok(LocalizedResolution::Ambiguous {
            english,
            candidates: russian.to_vec(),
        }),
        count => invalid(format!(
            "localized key {logical_key:?} has {count} source files / {} distinct payloads; no authored takeId pairing exists",
            russian_hashes.len()
        )),
    }
}

pub(super) fn valid_locale_id(locale: &str) -> bool {
    !locale.is_empty()
        && !locale.starts_with('-')
        && !locale.ends_with('-')
        && !locale.contains("--")
        && locale
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
