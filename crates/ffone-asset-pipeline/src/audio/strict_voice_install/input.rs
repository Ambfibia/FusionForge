use super::*;

pub(super) fn load_base_assets(path: &Path, bytes: &[u8]) -> Result<Vec<BaseAsset>> {
    let header: CatalogHeader =
        serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
            path: path.display().to_string(),
            source,
        })?;
    match header.schema.as_str() {
        LOCALIZED_AUDIO_CATALOG_SCHEMA => {
            let catalog: LocalizedAudioCatalog =
                serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
                    path: path.display().to_string(),
                    source,
                })?;
            Ok(catalog
                .assets
                .into_iter()
                .map(|asset| {
                    let semantic_context =
                        semantic_context_from_provenance(&asset.source_provenance);
                    BaseAsset {
                        scope: (asset.classification.category_rule
                            == "character-creation-assetbundle-container")
                            .then(|| CHARACTER_CREATION_SCOPE.to_owned()),
                        file: (asset.category != SemanticAudioCategory::Voice).then(|| {
                            StrictAudioFile {
                                locale: None,
                                path: asset.path,
                                bytes: asset.source_bytes,
                                blake3: asset.source_blake3,
                                language_neutral: false,
                            }
                        }),
                        true_name: asset.true_name,
                        category: asset.category,
                        owner: asset.owner,
                        semantic_context,
                    }
                })
                .collect())
        }
        SEMANTIC_AUDIO_CATALOG_SCHEMA => {
            let catalog: SemanticAudioCatalog =
                serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
                    path: path.display().to_string(),
                    source,
                })?;
            Ok(catalog
                .assets
                .into_iter()
                .map(|asset| {
                    let semantic_context =
                        semantic_context_from_provenance(&asset.source_provenance);
                    BaseAsset {
                        scope: (asset.classification.category_rule
                            == "character-creation-assetbundle-container")
                            .then(|| CHARACTER_CREATION_SCOPE.to_owned()),
                        file: (asset.category != SemanticAudioCategory::Voice).then(|| {
                            StrictAudioFile {
                                locale: None,
                                path: asset.path,
                                bytes: asset.source_bytes,
                                blake3: asset.source_blake3,
                                language_neutral: false,
                            }
                        }),
                        true_name: asset.true_name,
                        category: asset.category,
                        owner: asset.owner,
                        semantic_context,
                    }
                })
                .collect())
        }
        STRICT_AUDIO_CATALOG_SCHEMA_V3 | STRICT_AUDIO_CATALOG_SCHEMA_V4 => {
            let catalog: StrictAudioCatalog =
                serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
                    path: path.display().to_string(),
                    source,
                })?;
            Ok(catalog
                .assets
                .into_iter()
                .map(|asset| {
                    let file = (asset.category != SemanticAudioCategory::Voice)
                        .then(|| asset.files.into_iter().next())
                        .flatten();
                    BaseAsset {
                        true_name: asset.true_name,
                        category: asset.category,
                        owner: asset.owner,
                        scope: asset.scope,
                        semantic_context: None,
                        file,
                    }
                })
                .collect())
        }
        STRICT_AUDIO_CATALOG_SCHEMA => {
            let catalog = editable_catalog_with_disk_identities(path, bytes)?;
            Ok(catalog
                .assets
                .into_iter()
                .map(|asset| {
                    let file = (asset.category != SemanticAudioCategory::Voice)
                        .then(|| asset.files.into_iter().next())
                        .flatten();
                    BaseAsset {
                        true_name: asset.true_name,
                        category: asset.category,
                        owner: asset.owner,
                        scope: asset.scope,
                        semantic_context: None,
                        file,
                    }
                })
                .collect())
        }
        schema => invalid(format!(
            "unsupported source audio catalog {schema:?}; expected v1, v2, v3, or v4"
        )),
    }
}

pub(super) fn collect_named_oggs(root: &Path) -> Result<Vec<NamedOgg>> {
    fn visit(root: &Path, current: &Path, output: &mut Vec<NamedOgg>) -> Result<()> {
        let mut entries = fs::read_dir(current)
            .map_err(|source| io_at(current, source))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|source| io_at(current, source))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| io_at(&path, source))?;
            if file_type.is_dir() {
                visit(root, &path, output)?;
                continue;
            }
            if !file_type.is_file()
                || path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_none_or(|value| !value.eq_ignore_ascii_case("ogg"))
            {
                continue;
            }
            let filename = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    invalid_error(format!("non-UTF-8 OGG filename at {}", path.display()))
                })?;
            let (path_id, true_name) = parse_named_ogg_stem(filename).ok_or_else(|| {
                invalid_error(format!(
                    "expected <pathId>__<trueName>.ogg at {}",
                    path.display()
                ))
            })?;
            let container = path
                .parent()
                .and_then(Path::file_name)
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    invalid_error(format!(
                        "OGG source has no UTF-8 container at {}",
                        path.display()
                    ))
                })?
                .to_owned();
            let (bytes, blake3) = inspect_ogg(&path)?;
            output.push(NamedOgg {
                absolute_path: path.clone(),
                relative_path: portable_relative(root, &path)?,
                container,
                path_id,
                true_name,
                bytes,
                blake3,
            });
        }
        Ok(())
    }
    let mut output = Vec::new();
    visit(root, root, &mut output)?;
    output.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(output)
}

pub(super) fn resolve_clean_english(
    clean: Option<&Vec<&NamedOgg>>,
    fresh: Option<&Vec<&FreshAudio>>,
) -> Result<Option<VoiceSource>> {
    let clean = unique_named_by_hash(clean.into_iter().flatten().copied());
    match clean.as_slice() {
        [] => Ok(None),
        [candidate] => Ok(Some(voice_source_from_named(candidate, "clean-en"))),
        many => {
            let fresh_hashes = fresh
                .into_iter()
                .flatten()
                .map(|candidate| candidate.blake3.as_str())
                .collect::<BTreeSet<_>>();
            let common = many
                .iter()
                .copied()
                .filter(|candidate| fresh_hashes.contains(candidate.blake3.as_str()))
                .collect::<Vec<_>>();
            let common = unique_named_by_hash(common.into_iter());
            match common.as_slice() {
                [candidate] => Ok(Some(voice_source_from_named(candidate, "clean-en"))),
                _ => Ok(None),
            }
        }
    }
}

pub(super) fn collect_tree_identities(root: &Path) -> Result<BTreeMap<String, (u64, String)>> {
    fn visit(
        root: &Path,
        current: &Path,
        output: &mut BTreeMap<String, (u64, String)>,
    ) -> Result<()> {
        let mut entries = fs::read_dir(current)
            .map_err(|source| io_at(current, source))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|source| io_at(current, source))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| io_at(&path, source))?;
            if file_type.is_dir() {
                visit(root, &path, output)?;
            } else if file_type.is_file() {
                let relative = portable_relative(root, &path)?;
                let metadata = entry.metadata().map_err(|source| io_at(&path, source))?;
                if output
                    .insert(relative.clone(), (metadata.len(), hash_file(&path)?))
                    .is_some()
                {
                    return invalid(format!("duplicate transaction tree path {relative:?}"));
                }
            } else {
                return invalid(format!(
                    "transaction tree contains a non-file entry at {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }
    let mut output = BTreeMap::new();
    visit(root, root, &mut output)?;
    Ok(output)
}

pub(super) fn parse_named_ogg_stem(stem: &str) -> Option<(u64, String)> {
    let (path_id, true_name) = stem.split_once("__")?;
    let path_id = path_id.parse().ok()?;
    (!true_name.trim().is_empty()).then(|| (path_id, true_name.to_owned()))
}
