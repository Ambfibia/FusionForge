use super::*;

#[derive(Clone, Debug)]
pub(super) struct BaseAsset {
    pub(super) true_name: String,
    pub(super) category: SemanticAudioCategory,
    pub(super) owner: String,
    pub(super) scope: Option<String>,
    pub(super) semantic_context: Option<String>,
    pub(super) file: Option<StrictAudioFile>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CatalogHeader {
    pub(super) schema: String,
}

pub(crate) fn editable_catalog_with_disk_identities(
    path: &Path,
    bytes: &[u8],
) -> Result<StrictAudioCatalog> {
    let source: EditableAudioCatalog =
        serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
            path: path.display().to_string(),
            source,
        })?;
    if source.schema != STRICT_AUDIO_CATALOG_SCHEMA {
        return invalid(format!(
            "unsupported editable audio catalog schema {:?}",
            source.schema
        ));
    }
    let asset_root = path.parent().and_then(Path::parent).ok_or_else(|| {
        invalid_error(format!(
            "audio catalog has no asset root: {}",
            path.display()
        ))
    })?;
    let voice_root = asset_root.join("audio/voice");
    let mut discovered_locales = Vec::new();
    if voice_root.is_dir() {
        for entry in fs::read_dir(&voice_root).map_err(|source| io_at(&voice_root, source))? {
            let entry = entry.map_err(|source| io_at(&voice_root, source))?;
            if entry
                .file_type()
                .map_err(|source| io_at(&entry.path(), source))?
                .is_dir()
            {
                discovered_locales.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        discovered_locales.sort();
    }

    let mut assets = Vec::with_capacity(source.assets.len());
    for asset in source.assets {
        let mut files = Vec::with_capacity(asset.files.len());
        for file in asset.files {
            let absolute = asset_root.join(native_path(&file.path));
            let (bytes, blake3) = inspect_ogg(&absolute)?;
            files.push(StrictAudioFile {
                locale: file.locale,
                path: file.path,
                bytes,
                blake3,
                language_neutral: false,
            });
        }
        if asset.category == SemanticAudioCategory::Voice {
            let fallback = files
                .iter()
                .find(|file| file.locale.as_deref() == Some(source.fallback_locale.as_str()))
                .ok_or_else(|| {
                    invalid_error(format!(
                        "editable voice has no fallback file for {:?}",
                        asset.logical_key
                    ))
                })?;
            let filename = Path::new(&fallback.path)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| invalid_error(format!("invalid voice path {:?}", fallback.path)))?
                .to_owned();
            for locale in &discovered_locales {
                if files
                    .iter()
                    .any(|file| file.locale.as_deref() == Some(locale.as_str()))
                {
                    continue;
                }
                let candidate = format!("audio/voice/{locale}/{}/{filename}", asset.owner);
                let absolute = asset_root.join(native_path(&candidate));
                if !absolute.is_file() {
                    continue;
                }
                let (bytes, blake3) = inspect_ogg(&absolute)?;
                files.push(StrictAudioFile {
                    locale: Some(locale.clone()),
                    path: candidate,
                    bytes,
                    blake3,
                    language_neutral: false,
                });
            }
        }
        assets.push(StrictAudioAsset {
            logical_key: asset.logical_key,
            aliases: asset.aliases,
            true_name: asset.true_name,
            category: asset.category,
            owner: asset.owner,
            scope: asset.scope,
            files,
        });
    }
    let counts = strict_catalog_counts(&assets)?;
    let logical_counts = (
        counts.assets,
        counts.music,
        counts.ambient,
        counts.voice,
        counts.sfx,
    );
    let expected = (
        source.counts.assets,
        source.counts.music,
        source.counts.ambient,
        source.counts.voice,
        source.counts.sfx,
    );
    if logical_counts != expected {
        return invalid(format!(
            "editable audio logical counts disagree: catalog={expected:?}, disk={logical_counts:?}"
        ));
    }
    Ok(StrictAudioCatalog {
        schema: STRICT_AUDIO_CATALOG_SCHEMA.to_owned(),
        fallback_locale: source.fallback_locale,
        locale_fallbacks: source.locale_fallbacks,
        counts,
        assets,
    })
}

pub(crate) fn serialize_editable_runtime_catalog(catalog: &StrictAudioCatalog) -> Result<Vec<u8>> {
    let mut assets = Vec::with_capacity(catalog.assets.len());
    for asset in &catalog.assets {
        let default_file = if asset.category == SemanticAudioCategory::Voice {
            asset
                .files
                .iter()
                .find(|file| file.locale.as_deref() == Some(catalog.fallback_locale.as_str()))
        } else {
            asset.files.first()
        }
        .ok_or_else(|| {
            invalid_error(format!(
                "audio asset has no editable default file: {:?}",
                asset.logical_key
            ))
        })?;
        let default_filename = Path::new(&default_file.path).file_name();
        let runtime_files = std::iter::once(default_file)
            .chain(asset.files.iter().filter(|file| {
                file.path != default_file.path
                    && Path::new(&file.path).file_name() != default_filename
            }))
            .map(|file| {
                let mut value = serde_json::Map::new();
                if let Some(locale) = &file.locale {
                    value.insert(
                        "locale".to_owned(),
                        serde_json::Value::String(locale.clone()),
                    );
                }
                value.insert(
                    "path".to_owned(),
                    serde_json::Value::String(file.path.clone()),
                );
                serde_json::Value::Object(value)
            })
            .collect();
        let mut value = serde_json::Map::new();
        value.insert(
            "logicalKey".to_owned(),
            serde_json::Value::String(asset.logical_key.clone()),
        );
        if !asset.aliases.is_empty() {
            value.insert(
                "aliases".to_owned(),
                serde_json::to_value(&asset.aliases).map_err(|source| PipelineError::Json {
                    path: STRICT_AUDIO_CATALOG_PATH.to_owned(),
                    source,
                })?,
            );
        }
        value.insert(
            "trueName".to_owned(),
            serde_json::Value::String(asset.true_name.clone()),
        );
        value.insert(
            "category".to_owned(),
            serde_json::to_value(&asset.category).map_err(|source| PipelineError::Json {
                path: STRICT_AUDIO_CATALOG_PATH.to_owned(),
                source,
            })?,
        );
        value.insert(
            "owner".to_owned(),
            serde_json::Value::String(asset.owner.clone()),
        );
        if let Some(scope) = &asset.scope {
            value.insert("scope".to_owned(), serde_json::Value::String(scope.clone()));
        }
        value.insert("files".to_owned(), serde_json::Value::Array(runtime_files));
        assets.push(serde_json::Value::Object(value));
    }
    let document = serde_json::json!({
        "schema": STRICT_AUDIO_CATALOG_SCHEMA,
        "fallbackLocale": catalog.fallback_locale,
        "localeFallbacks": catalog.locale_fallbacks,
        "counts": {
            "assets": catalog.counts.assets,
            "music": catalog.counts.music,
            "ambient": catalog.counts.ambient,
            "voice": catalog.counts.voice,
            "sfx": catalog.counts.sfx,
        },
        "assets": assets,
    });
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|source| PipelineError::Json {
        path: STRICT_AUDIO_CATALOG_PATH.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn strict_catalog_counts(assets: &[StrictAudioAsset]) -> Result<StrictAudioCatalogCounts> {
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
        counts.files += asset.files.len() as u64;
        for file in &asset.files {
            counts.file_bytes = counts
                .file_bytes
                .checked_add(file.bytes)
                .ok_or_else(|| invalid_error("strict audio file byte count overflow"))?;
            if asset.category == SemanticAudioCategory::Voice {
                counts.english_voice_files += u64::from(file.locale.as_deref() == Some("en"));
                counts.russian_voice_files += u64::from(file.locale.as_deref() == Some("ru"));
            }
        }
        if asset.category == SemanticAudioCategory::Voice {
            let has_ru = asset
                .files
                .iter()
                .any(|file| file.locale.as_deref() == Some("ru"));
            counts.translated_voice_assets += u64::from(has_ru);
            counts.fallback_only_voice_assets += u64::from(!has_ru);
            counts.language_neutral_voice_assets +=
                u64::from(asset.files.iter().any(|file| file.language_neutral));
        }
    }
    Ok(counts)
}

pub(super) fn flatten_legacy_variant_path(path: &str) -> Result<String> {
    validate_relative_path(path)?;
    let parts = path.split('/').collect::<Vec<_>>();
    let mut output = Vec::with_capacity(parts.len());
    let mut index = 0;
    while index < parts.len() {
        if parts[index] == "variants"
            && parts
                .get(index + 1)
                .is_some_and(|part| part.starts_with("variant_"))
        {
            index += 2;
            continue;
        }
        output.push(parts[index]);
        index += 1;
    }
    let canonical = output.join("/");
    validate_relative_path(&canonical)?;
    Ok(canonical)
}

pub(super) fn remove_transaction_path(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path).map_err(|source| io_at(path, source))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|source| io_at(path, source))
    } else if metadata.is_file() {
        fs::remove_file(path).map_err(|source| io_at(path, source))
    } else {
        invalid(format!(
            "refusing to remove non-file transaction path {}",
            path.display()
        ))
    }
}

pub(super) fn replace_manifest(path: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("asset manifest has no parent"))?;
    let stamp = transaction_stamp();
    let next = parent.join(format!(".asset-manifest.voice-v3-next-{stamp}"));
    let backup = parent.join(format!(".asset-manifest.voice-v3-backup-{stamp}"));
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    write_new(&next, &bytes)?;
    fs::rename(path, &backup).map_err(|source| io_at(path, source))?;
    if let Err(source) = fs::rename(&next, path) {
        let _ = fs::rename(&backup, path);
        let _ = fs::remove_file(&next);
        return Err(io_at(path, source));
    }
    fs::remove_file(&backup).map_err(|source| io_at(&backup, source))
}

pub(super) fn absolute_output_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|source| io_at(path, source))
}

pub(super) fn native_path(value: &str) -> PathBuf {
    value.split('/').collect()
}
