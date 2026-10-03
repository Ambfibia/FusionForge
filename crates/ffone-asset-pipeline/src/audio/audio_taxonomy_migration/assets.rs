use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Route {
    Preserve,
    NanoVoice { canonical_owner: String },
    NanoSkill { power: String, skill_owner: String },
    Computress { section: String },
}

pub(super) fn nano_skill_route_from_owner(owner: &str) -> Result<(String, String)> {
    let owner = owner.strip_prefix("nano_").unwrap_or(owner);
    let (power, skill_owner) = owner.split_once('_').ok_or_else(|| {
        invalid_error(format!(
            "proven Nano skill owner has no power/owner split: {owner:?}"
        ))
    })?;
    Ok((
        power.to_owned(),
        canonical_skill_owner(skill_owner).to_owned(),
    ))
}

pub(super) fn routed_file_path(
    route: &Route,
    owner: &str,
    file: &StrictAudioFile,
    line: &str,
) -> Result<String> {
    match route {
        Route::Preserve => Ok(file.path.clone()),
        Route::NanoVoice { .. } => {
            let locale = file.locale.as_deref().ok_or_else(|| {
                invalid_error(format!("Nano voice file has no locale: {:?}", file.path))
            })?;
            Ok(format!("audio/voice/{locale}/{owner}/{line}.ogg"))
        }
        Route::NanoSkill { power, skill_owner } => Ok(format!(
            "audio/sfx/nano_skills/{power}/{skill_owner}/{line}.ogg"
        )),
        Route::Computress { section } => {
            let locale = file.locale.as_deref().ok_or_else(|| {
                invalid_error(format!(
                    "Computress voice file has no locale: {:?}",
                    file.path
                ))
            })?;
            let physical_line = if section == "summon"
                && locale == DEFAULT_LOCALE
                && line.starts_with("computer_summon")
            {
                line.replacen("computer_summon", "computress_summon", 1)
            } else {
                line.to_owned()
            };
            Ok(format!("audio/voice/{locale}/{owner}/{physical_line}.ogg"))
        }
    }
}

pub(super) fn recovery_asset(source: &RecoveryOgg) -> Result<(StrictAudioAsset, Route)> {
    let line = portable_component(&source.true_name, "recovery true name")?;
    if let Some((_, power, owner)) = RECOVERY_NANO_SKILLS
        .iter()
        .find(|(name, _, _)| *name == line)
    {
        let route = Route::NanoSkill {
            power: (*power).to_owned(),
            skill_owner: (*owner).to_owned(),
        };
        let path = format!("audio/sfx/nano_skills/{power}/{owner}/{line}.ogg");
        return Ok((
            StrictAudioAsset {
                logical_key: format!("sfx/nano_skills/{power}/{owner}/{line}"),
                aliases: Vec::new(),
                true_name: source.true_name.clone(),
                category: SemanticAudioCategory::Sfx,
                owner: "nano_skills".to_owned(),
                scope: Some("nano_recovery".to_owned()),
                files: vec![StrictAudioFile {
                    locale: None,
                    path,
                    bytes: source.bytes,
                    blake3: source.blake3.clone(),
                    language_neutral: true,
                }],
            },
            route,
        ));
    }

    let canonical_owner = nano_owner_from_true_name(&source.true_name).ok_or_else(|| {
        invalid_error(format!(
            "recovery OGG is neither an exact catalog identity nor a proven Nano family: {} ({:?})",
            source.relative_path, source.true_name
        ))
    })?;
    let owner = format!("nano_{canonical_owner}");
    let route = Route::NanoVoice {
        canonical_owner: canonical_owner.clone(),
    };
    Ok((
        StrictAudioAsset {
            logical_key: format!("voice/nanos/{owner}/{line}"),
            aliases: Vec::new(),
            true_name: source.true_name.clone(),
            category: SemanticAudioCategory::Voice,
            owner: owner.clone(),
            scope: Some("nano_recovery".to_owned()),
            files: vec![StrictAudioFile {
                locale: Some(DEFAULT_LOCALE.to_owned()),
                path: format!("audio/voice/en/{owner}/{line}.ogg"),
                bytes: source.bytes,
                blake3: source.blake3.clone(),
                language_neutral: false,
            }],
        },
        route,
    ))
}

pub(super) fn normalize_recovery_source_path(source_path: &str) -> (String, bool) {
    let Some(tail) = source_path.strip_prefix("retrobution-nano-recovery/") else {
        return (source_path.to_owned(), false);
    };
    (format!("nano-recovery/{tail}"), true)
}

pub(super) fn catalog_counts(assets: &[StrictAudioAsset]) -> Result<StrictAudioCatalogCounts> {
    let mut counts = StrictAudioCatalogCounts {
        assets: assets.len() as u64,
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
        language_neutral_voice_assets: 0,
    };
    for asset in assets {
        match asset.category {
            SemanticAudioCategory::Music => counts.music += 1,
            SemanticAudioCategory::Ambient => counts.ambient += 1,
            SemanticAudioCategory::Voice => counts.voice += 1,
            SemanticAudioCategory::Sfx => counts.sfx += 1,
        }
        counts.files = counts
            .files
            .checked_add(asset.files.len() as u64)
            .ok_or_else(|| invalid_error("audio file count overflow"))?;
        for file in &asset.files {
            counts.file_bytes = counts
                .file_bytes
                .checked_add(file.bytes)
                .ok_or_else(|| invalid_error("audio byte count overflow"))?;
            if asset.category == SemanticAudioCategory::Voice {
                counts.english_voice_files +=
                    u64::from(file.locale.as_deref() == Some(DEFAULT_LOCALE));
                counts.russian_voice_files +=
                    u64::from(file.locale.as_deref() == Some(RUSSIAN_LOCALE));
            }
        }
        if asset.category == SemanticAudioCategory::Voice {
            let has_ru = asset
                .files
                .iter()
                .any(|file| file.locale.as_deref() == Some(RUSSIAN_LOCALE));
            counts.translated_voice_assets += u64::from(has_ru);
            counts.fallback_only_voice_assets += u64::from(!has_ru);
            counts.language_neutral_voice_assets +=
                u64::from(asset.files.iter().any(|file| file.language_neutral));
        }
    }
    Ok(counts)
}

pub(super) fn native_path(value: &str) -> PathBuf {
    value.split('/').collect()
}
