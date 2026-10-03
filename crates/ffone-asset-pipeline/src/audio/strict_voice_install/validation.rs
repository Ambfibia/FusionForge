use super::*;

pub(super) fn validate_cook_report(report: &CookReport) -> Result<()> {
    if report.schema != "ffone.cook-report.v1"
        || report.build_uuid.trim().is_empty()
        || report.locale.trim().is_empty()
        || report.counts.audio == 0
    {
        return invalid("fresh cook report identity/counts are incomplete");
    }
    let audio_mappings = report
        .mappings
        .iter()
        .filter(|mapping| mapping.kind == "audio")
        .count() as u64;
    if audio_mappings < report.counts.audio {
        return invalid(format!(
            "fresh cook report advertises {} audio assets but contains only {audio_mappings} audio mappings",
            report.counts.audio
        ));
    }
    Ok(())
}

pub(super) fn validate_strict_catalog(catalog: &StrictAudioCatalog) -> Result<()> {
    if !matches!(
        catalog.schema.as_str(),
        STRICT_AUDIO_CATALOG_SCHEMA_V3
            | STRICT_AUDIO_CATALOG_SCHEMA_V4
            | STRICT_AUDIO_CATALOG_SCHEMA
    ) || catalog.fallback_locale != DEFAULT_LOCALE
        || catalog
            .locale_fallbacks
            .get(RUSSIAN_LOCALE)
            .map(String::as_str)
            != Some(DEFAULT_LOCALE)
    {
        return invalid("strict audio catalog locale contract is invalid");
    }
    let mut keys = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for asset in &catalog.assets {
        if catalog.schema == STRICT_AUDIO_CATALOG_SCHEMA_V3 && !asset.aliases.is_empty() {
            return invalid(format!(
                "strict audio aliases require schema v4 for {:?}",
                asset.logical_key
            ));
        }
        if asset.logical_key.trim().is_empty()
            || asset.true_name.trim().is_empty()
            || asset.owner.trim().is_empty()
        {
            return invalid(format!(
                "strict audio logical identity is invalid for {:?}",
                asset.logical_key
            ));
        }
        for key in std::iter::once(&asset.logical_key).chain(&asset.aliases) {
            if key.trim().is_empty() || !keys.insert(folded(key)) {
                return invalid(format!(
                    "strict audio logical key or alias collides at {key:?}"
                ));
            }
        }
        if asset.files.is_empty() {
            return invalid(format!(
                "strict audio asset has no files: {:?}",
                asset.logical_key
            ));
        }
        for file in &asset.files {
            validate_relative_path(&file.path)?;
            if file.path.contains("/variants/")
                || !file.path.ends_with(".ogg")
                || file.bytes == 0
                || !valid_hash(&file.blake3)
                || !paths.insert(folded(&file.path))
            {
                return invalid(format!("invalid strict runtime audio file {:?}", file.path));
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
                let prefix = format!("audio/voice/{locale}/{}/", asset.owner);
                if !file.path.starts_with(&prefix) {
                    return invalid(format!(
                        "voice path {:?} is outside {:?}",
                        file.path, prefix
                    ));
                }
            }
            for left in &asset.files {
                for right in &asset.files {
                    if left.locale != right.locale
                        && left.blake3 == right.blake3
                        && !(left.language_neutral && right.language_neutral)
                    {
                        return invalid(format!(
                            "identical locale payloads require explicit languageNeutral for {:?}",
                            asset.logical_key
                        ));
                    }
                }
            }
        } else if asset.files.len() != 1 || asset.files[0].locale.is_some() {
            return invalid(format!(
                "non-voice asset must have exactly one locale-free file: {:?}",
                asset.logical_key
            ));
        }
    }
    let actual = strict_catalog_counts(&catalog.assets)?;
    if actual != catalog.counts {
        return invalid(format!(
            "strict catalog counts disagree: catalog={:?}, actual={actual:?}",
            catalog.counts
        ));
    }
    Ok(())
}

pub(super) fn validate_installed_catalog_identity(
    manifest: &ProjectAssetManifest,
    catalog_bytes: &[u8],
) -> Result<()> {
    let entries = manifest
        .files
        .iter()
        .filter(|entry| {
            entry.path == SEMANTIC_AUDIO_CATALOG || entry.path == STRICT_AUDIO_CATALOG_PATH
        })
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        return invalid("manifest must contain exactly one installed audio registry entry");
    };
    let hash = blake3::hash(catalog_bytes).to_hex().to_string();
    if entry.bytes != catalog_bytes.len() as u64 || entry.blake3 != hash {
        return invalid("installed audio catalog identity disagrees with the manifest");
    }
    Ok(())
}

pub(super) fn validate_manifest(manifest: &ProjectAssetManifest) -> Result<()> {
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "unsupported project manifest schema {:?}",
            manifest.schema
        ));
    }
    let mut paths = BTreeSet::new();
    for entry in &manifest.files {
        validate_relative_path(&entry.path)?;
        if !paths.insert(folded(&entry.path)) {
            return invalid(format!(
                "project manifest path collision at {:?}",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_file_identity(path: &Path, file: &StrictAudioFile) -> Result<()> {
    let metadata = fs::metadata(path).map_err(|source| io_at(path, source))?;
    if !metadata.is_file() || metadata.len() != file.bytes {
        return invalid(format!(
            "runtime audio file identity mismatch at {}",
            path.display()
        ));
    }
    let hash = hash_file(path)?;
    if hash != file.blake3 {
        return invalid(format!(
            "runtime audio BLAKE3 mismatch at {}",
            path.display()
        ));
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

pub(super) fn validate_source_build(source_build: &str) -> Result<()> {
    if source_build.trim().is_empty()
        || source_build.contains('/')
        || source_build.contains('\\')
        || source_build.chars().any(char::is_control)
    {
        return invalid(format!("invalid source build label {source_build:?}"));
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::ProjectAssetManifest(message.into())
}
