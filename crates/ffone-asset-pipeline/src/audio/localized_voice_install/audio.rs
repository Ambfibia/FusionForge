use super::*;

pub(super) const AUDIO_TARGETS: &[&str] = &["music", "ambient", "voice", "sfx", "catalog.json"];

pub(super) fn reclassify_proven_voice(
    assets: &mut [LocalizedAudioAsset],
    proven_voice_indices: &BTreeSet<usize>,
    translated_indices: &BTreeSet<usize>,
) -> Result<()> {
    for index in proven_voice_indices {
        let asset = assets
            .get_mut(*index)
            .ok_or_else(|| invalid_error("translated asset index is out of bounds"))?;
        if asset.category == SemanticAudioCategory::Voice {
            continue;
        }
        let owner = voice_owner_from_true_name(&asset.true_name)?;
        asset.category = SemanticAudioCategory::Voice;
        asset.owner = owner;
        asset.classification = if translated_indices.contains(index) {
            SemanticAudioClassificationProof {
                category_rule: "localized-voice-source".to_owned(),
                owner_rule:
                    "speaker prefix preceding the first underscore in translated true m_Name"
                        .to_owned(),
                certainty: ClassificationCertainty::SourceProvenance,
                evidence: vec![
                    "a Russian OGG matched exact CustomAssetBundle provenance and identifies this asset as localized voice"
                        .to_owned(),
                ],
                reason:
                    "The translated source is direct provenance that this vocal clip is locale-sensitive voice."
                        .to_owned(),
                uncertain: false,
            }
        } else {
            SemanticAudioClassificationProof {
                category_rule: "tutorial-dialogue-source".to_owned(),
                owner_rule:
                    "speaker prefix preceding the first underscore in exact tutorial dialogue m_Name"
                        .to_owned(),
                certainty: ClassificationCertainty::SourceProvenance,
                evidence: vec![
                    format!("true m_Name {:?} contains the explicit _Tut dialogue marker", asset.true_name),
                    "serialized source provenance identifies TutorialAudio.resourceFile or Tutorial.resourceFile"
                        .to_owned(),
                ],
                reason:
                    "The exact tutorial dialogue marker and tutorial audio container jointly prove voice ownership."
                        .to_owned(),
                uncertain: false,
            }
        };
    }
    Ok(())
}

pub(super) fn route_english_voice(assets: &mut [LocalizedAudioAsset]) -> Result<()> {
    let mut destinations = BTreeMap::<String, String>::new();
    for asset in assets {
        asset.locale_variants.clear();
        if asset.category != SemanticAudioCategory::Voice {
            continue;
        }
        let tail = semantic_audio_tail(asset)?;
        let destination = format!(
            "audio/voice/{DEFAULT_VOICE_LOCALE}/{}/{}",
            asset.owner, tail
        );
        validate_relative_path(&destination)?;
        let identity = path_identity(&destination);
        if let Some(first) = destinations.insert(identity, asset.native_key.clone()) {
            return invalid(format!(
                "English voice destination collision for {destination:?}: {first:?} and {:?}",
                asset.native_key
            ));
        }
        asset.path = destination.clone();
        asset.locale_variants.push(LocalizedAudioVariant {
            locale: DEFAULT_VOICE_LOCALE.to_owned(),
            path: destination,
            bytes: asset.source_bytes,
            blake3: asset.source_blake3.clone(),
            translated_source: None,
        });
    }
    Ok(())
}

pub(super) fn route_russian_voice(
    assets: &[LocalizedAudioAsset],
    translations: &[PreparedTranslation],
    ambiguous: &mut Vec<LocalizedVoiceAmbiguousFile>,
) -> Result<BTreeMap<usize, String>> {
    let mut by_destination = BTreeMap::<String, Vec<&PreparedTranslation>>::new();
    for translation in translations {
        let asset = &assets[translation.asset_index];
        let english_prefix = format!("audio/voice/{DEFAULT_VOICE_LOCALE}/{}/", asset.owner);
        let tail = asset
            .path
            .strip_prefix(&english_prefix)
            .ok_or_else(|| invalid_error("English voice path has an invalid owner prefix"))?;
        let destination = format!(
            "audio/voice/{RUSSIAN_VOICE_LOCALE}/{}/{}",
            asset.owner, tail
        );
        by_destination
            .entry(path_identity(&destination))
            .or_default()
            .push(translation);
    }
    let mut accepted = BTreeMap::new();
    for translations in by_destination.into_values() {
        if translations.len() == 1 {
            let translation = translations[0];
            let asset = &assets[translation.asset_index];
            let english_prefix = format!("audio/voice/{DEFAULT_VOICE_LOCALE}/{}/", asset.owner);
            let tail = asset
                .path
                .strip_prefix(&english_prefix)
                .ok_or_else(|| invalid_error("English voice path has an invalid owner prefix"))?;
            let destination = format!(
                "audio/voice/{RUSSIAN_VOICE_LOCALE}/{}/{}",
                asset.owner, tail
            );
            validate_relative_path(&destination)?;
            accepted.insert(translation.asset_index, destination);
            continue;
        }
        let indices = translations
            .iter()
            .map(|translation| translation.asset_index)
            .collect::<Vec<_>>();
        for translation in translations {
            ambiguous.push(ambiguous_record(
                &translation.source,
                "multiple catalog assets route to the same Russian destination",
                &indices,
                assets,
            ));
        }
    }
    Ok(accepted)
}

pub(super) fn voice_owner_from_true_name(true_name: &str) -> Result<String> {
    let candidate = true_name.split('_').next().unwrap_or_default();
    portable_component(candidate, "translated voice owner")
}

pub(super) fn semantic_audio_tail(asset: &LocalizedAudioAsset) -> Result<String> {
    let components = asset.path.split('/').collect::<Vec<_>>();
    if let Some(variants_index) = components
        .iter()
        .position(|component| *component == "variants")
    {
        return Ok(components[variants_index..].join("/"));
    }
    asset
        .path
        .rsplit('/')
        .next()
        .map(str::to_owned)
        .ok_or_else(|| invalid_error("semantic audio path has no filename"))
}
