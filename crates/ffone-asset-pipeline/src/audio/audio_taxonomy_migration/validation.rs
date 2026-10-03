use super::*;

pub(super) fn validate_source_documents(
    asset_root: &Path,
    catalog: &StrictAudioCatalog,
    manifest: &ProjectAssetManifest,
) -> Result<()> {
    if !matches!(
        catalog.schema.as_str(),
        STRICT_AUDIO_CATALOG_SCHEMA_V3
            | STRICT_AUDIO_CATALOG_SCHEMA_V4
            | STRICT_AUDIO_CATALOG_SCHEMA
    ) {
        return invalid(format!(
            "audio taxonomy requires strict catalog v3/v4/v5, found {:?}",
            catalog.schema
        ));
    }
    if catalog.schema == STRICT_AUDIO_CATALOG_SCHEMA_V3
        && catalog.assets.iter().any(|asset| !asset.aliases.is_empty())
    {
        return invalid("audio catalog v3 cannot contain aliases");
    }
    if catalog.fallback_locale != DEFAULT_LOCALE
        || catalog
            .locale_fallbacks
            .get(RUSSIAN_LOCALE)
            .map(String::as_str)
            != Some(DEFAULT_LOCALE)
    {
        return invalid("audio catalog locale/fallback contract is invalid");
    }
    if catalog_counts(&catalog.assets)? != catalog.counts {
        return invalid("source audio catalog counts are stale");
    }
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "unsupported project asset manifest schema {:?}",
            manifest.schema
        ));
    }
    let mut manifest_paths = BTreeSet::new();
    for entry in &manifest.files {
        validate_relative_path(&entry.path)?;
        if !manifest_paths.insert(folded(&entry.path)) {
            return invalid(format!("manifest path collision at {:?}", entry.path));
        }
    }

    let catalog_absolute = asset_root.join(native_path(STRICT_AUDIO_CATALOG_PATH));
    let catalog_metadata =
        fs::metadata(&catalog_absolute).map_err(|source| io_at(&catalog_absolute, source))?;
    let catalog_hash = hash_file(&catalog_absolute)?;
    let catalog_manifest = manifest
        .files
        .iter()
        .filter(|entry| entry.path == STRICT_AUDIO_CATALOG_PATH)
        .collect::<Vec<_>>();
    let [catalog_manifest] = catalog_manifest.as_slice() else {
        return invalid("manifest must contain exactly one _runtime/audio.json entry");
    };
    if catalog_manifest.kind != ProjectAssetKind::Data
        || catalog_manifest.bytes != catalog_metadata.len()
        || catalog_manifest.blake3 != catalog_hash
    {
        return invalid("runtime audio catalog identity disagrees with asset-manifest.json");
    }

    let by_path = manifest
        .files
        .iter()
        .map(|entry| (folded(&entry.path), entry))
        .collect::<BTreeMap<_, _>>();
    let mut keys = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for asset in &catalog.assets {
        register_keys(asset, &mut keys)?;
        if asset.logical_key.trim().is_empty()
            || asset.true_name.trim().is_empty()
            || asset.owner.trim().is_empty()
            || asset.files.is_empty()
        {
            return invalid(format!(
                "incomplete source audio asset {:?}",
                asset.logical_key
            ));
        }
        for file in &asset.files {
            validate_relative_path(&file.path)?;
            if !paths.insert(folded(&file.path)) {
                return invalid(format!("source audio path collision at {:?}", file.path));
            }
            let entry = by_path.get(&folded(&file.path)).ok_or_else(|| {
                invalid_error(format!(
                    "source audio is absent from manifest: {:?}",
                    file.path
                ))
            })?;
            if entry.kind != ProjectAssetKind::Audio
                || entry.bytes != file.bytes
                || entry.blake3 != file.blake3
            {
                return invalid(format!(
                    "catalog/manifest identity mismatch for {:?}",
                    file.path
                ));
            }
            validate_file_identity(&asset_root.join(native_path(&file.path)), file)?;
        }
    }
    Ok(())
}

pub(super) fn validate_output_catalog(catalog: &StrictAudioCatalog) -> Result<()> {
    if catalog.schema != STRICT_AUDIO_CATALOG_SCHEMA
        || catalog.fallback_locale != DEFAULT_LOCALE
        || catalog
            .locale_fallbacks
            .get(RUSSIAN_LOCALE)
            .map(String::as_str)
            != Some(DEFAULT_LOCALE)
    {
        return invalid("output audio catalog v4 locale contract is invalid");
    }
    let mut keys = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut voice_names = BTreeMap::<String, Vec<(bool, String)>>::new();
    for asset in &catalog.assets {
        register_keys(asset, &mut keys)?;
        if asset.logical_key.trim().is_empty()
            || asset.true_name.trim().is_empty()
            || asset.owner.trim().is_empty()
            || asset.files.is_empty()
        {
            return invalid(format!(
                "invalid output audio identity: {:?}",
                asset.logical_key
            ));
        }
        for file in &asset.files {
            validate_relative_path(&file.path)?;
            if !file.path.ends_with(".ogg")
                || file.path.contains("/variants/")
                || file.bytes == 0
                || !valid_hash(&file.blake3)
                || !paths.insert(folded(&file.path))
            {
                return invalid(format!("invalid output audio file: {:?}", file.path));
            }
        }
        if asset.category == SemanticAudioCategory::Voice {
            let locales = asset
                .files
                .iter()
                .map(|file| file.locale.as_deref().unwrap_or_default())
                .collect::<BTreeSet<_>>();
            if !locales.contains(DEFAULT_LOCALE)
                || locales.len() != asset.files.len()
                || locales.iter().any(|locale| !valid_locale_id(locale))
            {
                return invalid(format!(
                    "voice locale closure is invalid for {:?}",
                    asset.logical_key
                ));
            }
            for file in &asset.files {
                let locale = file.locale.as_deref().unwrap_or_default();
                let prefix = strict_voice_prefix(asset, locale);
                if !file.path.starts_with(&prefix) {
                    return invalid(format!(
                        "voice path {:?} is outside {:?}",
                        file.path, prefix
                    ));
                }
            }
            let fallback_hash = asset
                .files
                .iter()
                .find(|file| file.locale.as_deref() == Some(DEFAULT_LOCALE))
                .map(|file| file.blake3.clone())
                .ok_or_else(|| invalid_error("voice asset lost its fallback file"))?;
            voice_names
                .entry(folded(&asset.true_name))
                .or_default()
                .push((
                    asset.scope.as_deref() == Some("alternate_take"),
                    fallback_hash,
                ));
            for (index, left) in asset.files.iter().enumerate() {
                for right in asset.files.iter().skip(index + 1) {
                    if left.locale != right.locale
                        && left.blake3 == right.blake3
                        && !(left.language_neutral && right.language_neutral)
                    {
                        return invalid(format!(
                            "identical locale payloads require languageNeutral for {:?}",
                            asset.logical_key
                        ));
                    }
                }
            }
        } else if asset.files.len() != 1 || asset.files[0].locale.is_some() {
            return invalid(format!(
                "non-voice asset must have one locale-free file: {:?}",
                asset.logical_key
            ));
        }
    }
    for (true_name, takes) in voice_names {
        let primary_count = takes.iter().filter(|(alternate, _)| !alternate).count();
        let unique_hashes = takes.iter().map(|(_, hash)| hash).collect::<BTreeSet<_>>();
        if primary_count != 1 || unique_hashes.len() != takes.len() {
            return invalid(format!(
                "voice take set {true_name:?} must contain one primary and distinct explicit alternate_take hashes"
            ));
        }
    }
    if catalog_counts(&catalog.assets)? != catalog.counts {
        return invalid("output audio catalog counts are stale");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_production_closure(
    source_already_migrated: bool,
    recovery_scanned: u64,
    nano_voice: u64,
    nano_skills: u64,
    computress: u64,
    recovery_exact: u64,
    recovery_voice_added: u64,
    recovery_skills_added: u64,
) -> Result<()> {
    if recovery_scanned != 2_332 {
        return Ok(());
    }
    let actual = if source_already_migrated {
        (
            nano_voice + nano_skills,
            computress,
            recovery_exact,
            recovery_voice_added + recovery_skills_added,
        )
    } else {
        if (nano_voice, nano_skills) != (2_234, 63) {
            return invalid(format!(
                "audited current Nano taxonomy changed: expected=(2234, 63), actual=({nano_voice}, {nano_skills})"
            ));
        }
        (
            nano_voice + nano_skills,
            computress,
            recovery_exact,
            recovery_voice_added + recovery_skills_added,
        )
    };
    let expected = if source_already_migrated {
        (2_557, 180, 2_332, 0)
    } else {
        (2_297, 180, 2_072, 260)
    };
    if actual != expected {
        return invalid(format!(
            "audited Nano audio closure changed: expected={expected:?}, actual={actual:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_exact_recovery_skill_routes(assets: &[StrictAudioAsset]) -> Result<()> {
    for (true_name, power, owner) in RECOVERY_NANO_SKILLS {
        let expected_key = format!("sfx/nano_skills/{power}/{owner}/{true_name}");
        if !assets.iter().any(|asset| {
            asset.category == SemanticAudioCategory::Sfx
                && folded(&asset.true_name) == folded(true_name)
                && asset.logical_key.starts_with(&expected_key)
        }) {
            return invalid(format!(
                "exact recovery Nano skill did not reach its audited route: {true_name:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_generic_recovery_metadata(
    assets: &[StrictAudioAsset],
    files: &[PlannedFile],
    recovery_files_scanned: u64,
) -> Result<()> {
    if assets
        .iter()
        .any(|asset| asset.scope.as_deref() == Some("retrobution_nano_recovery"))
        || files
            .iter()
            .any(|file| file.source_path.starts_with("retrobution-nano-recovery/"))
    {
        return invalid("source-specific Retrobution Nano recovery metadata remains in runtime");
    }
    if recovery_files_scanned != 2_332 {
        return Ok(());
    }
    let generic_scopes = assets
        .iter()
        .filter(|asset| asset.scope.as_deref() == Some("nano_recovery"))
        .count();
    let generic_source_paths = files
        .iter()
        .filter(|file| file.source_path.starts_with("nano-recovery/"))
        .count();
    if (generic_scopes, generic_source_paths) != (260, 260) {
        return invalid(format!(
            "generic Nano recovery metadata closure changed: expected=(260, 260), actual=({generic_scopes}, {generic_source_paths})"
        ));
    }
    Ok(())
}

pub(super) fn validate_output_manifest(manifest: &ProjectAssetManifest) -> Result<()> {
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid("output project asset manifest schema changed");
    }
    let mut paths = BTreeSet::new();
    for entry in &manifest.files {
        validate_relative_path(&entry.path)?;
        let is_audio_authority =
            entry.path.starts_with("audio/") || entry.path == STRICT_AUDIO_CATALOG_PATH;
        if (is_audio_authority && entry.bytes == 0)
            || !valid_hash(&entry.blake3)
            || !paths.insert(folded(&entry.path))
        {
            return invalid(format!("invalid output manifest entry: {:?}", entry.path));
        }
    }
    Ok(())
}

pub(super) fn validate_file_identity(path: &Path, file: &StrictAudioFile) -> Result<()> {
    validate_identity(path, file.bytes, &file.blake3)
}

pub(super) fn validate_identity(path: &Path, expected_bytes: u64, expected_blake3: &str) -> Result<()> {
    let metadata = fs::metadata(path).map_err(|source| io_at(path, source))?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return invalid(format!(
            "audio byte identity mismatch at {}",
            path.display()
        ));
    }
    if hash_file(path)? != expected_blake3 {
        return invalid(format!("audio BLAKE3 mismatch at {}", path.display()));
    }
    Ok(())
}

pub(super) fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty() || path.contains('\\') {
        return invalid(format!("invalid portable path {path:?}"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe relative path {path:?}"));
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::ProjectAssetManifest(message.into())
}
