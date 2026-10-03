use super::*;

pub(super) fn match_russian_sources(
    assets: &[LocalizedAudioAsset],
    sources: Vec<RussianSourceFile>,
) -> Result<(
    Vec<PendingMatch>,
    Vec<LocalizedVoiceUnmatchedFile>,
    Vec<LocalizedVoiceAmbiguousFile>,
)> {
    let (primary, fallback) = build_source_indexes(assets)?;
    let mut matched = Vec::new();
    let mut unmatched = Vec::new();
    let mut ambiguous = Vec::new();
    for source in sources {
        let (Some(container), Some(path_id), Some(true_name)) = (
            source.container.as_ref(),
            source.path_id,
            source.true_name.as_ref(),
        ) else {
            unmatched.push(unmatched_record(
                &source,
                "filename must be <pathId>__<true_name>.ogg directly below CustomAssetBundle-*",
            ));
            continue;
        };
        let primary_key = PrimaryKey {
            container: folded(container),
            path_id,
            true_name: true_name.clone(),
        };
        let mut candidates = unique_candidates(primary.get(&primary_key));
        let mode = if candidates.is_empty() {
            let fallback_key = FallbackKey {
                container: folded(container),
                folded_true_name: folded(true_name),
            };
            candidates = unique_candidates(fallback.get(&fallback_key));
            LocalizedVoiceMatchMode::ContainerCasefoldNameFallback
        } else {
            LocalizedVoiceMatchMode::Primary
        };
        match candidates.as_slice() {
            [asset_index] => matched.push(PendingMatch {
                source,
                asset_index: *asset_index,
                match_mode: mode,
            }),
            [] => unmatched.push(unmatched_record(
                &source,
                "no catalog provenance matched the strict primary or casefold fallback identity",
            )),
            _ => ambiguous.push(ambiguous_record(
                &source,
                "multiple catalog assets match the selected identity",
                &candidates,
                assets,
            )),
        }
    }
    Ok((matched, unmatched, ambiguous))
}

pub(super) fn build_source_indexes(
    assets: &[LocalizedAudioAsset],
) -> Result<(
    BTreeMap<PrimaryKey, Vec<usize>>,
    BTreeMap<FallbackKey, Vec<usize>>,
)> {
    let mut primary = BTreeMap::<PrimaryKey, Vec<usize>>::new();
    let mut fallback = BTreeMap::<FallbackKey, Vec<usize>>::new();
    for (asset_index, asset) in assets.iter().enumerate() {
        for source in &asset.source_provenance {
            let Some(file) = source.get("file").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let Some((container, path_id, _embedded_name)) = parse_provenance_file(file) else {
                continue;
            };
            if let Some(declared_path_id) = source.get("pathId").and_then(serde_json::Value::as_u64)
                && declared_path_id != path_id
            {
                return invalid(format!(
                    "catalog provenance pathId mismatch for {:?}: field={declared_path_id}, file={path_id}",
                    asset.true_name
                ));
            }
            primary
                .entry(PrimaryKey {
                    container: folded(&container),
                    path_id,
                    true_name: asset.true_name.clone(),
                })
                .or_default()
                .push(asset_index);
            fallback
                .entry(FallbackKey {
                    container: folded(&container),
                    folded_true_name: folded(&asset.true_name),
                })
                .or_default()
                .push(asset_index);
        }
    }
    Ok((primary, fallback))
}

pub(super) fn unique_candidates(candidates: Option<&Vec<usize>>) -> Vec<usize> {
    candidates
        .into_iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn prepare_translations(
    pending: Vec<PendingMatch>,
    assets: &[LocalizedAudioAsset],
    unmatched: &mut Vec<LocalizedVoiceUnmatchedFile>,
    _ambiguous: &mut Vec<LocalizedVoiceAmbiguousFile>,
) -> Result<Vec<PreparedTranslation>> {
    let mut output = Vec::new();
    for matched in pending {
        let bytes = fs::read(&matched.source.absolute_path)
            .map_err(|error| io_at(&matched.source.absolute_path, error))?;
        if !bytes.starts_with(b"OggS") {
            unmatched.push(unmatched_record(
                &matched.source,
                "matched file is not an Ogg bitstream",
            ));
            continue;
        }
        if assets.get(matched.asset_index).is_none() {
            return invalid("internal localized voice match index is out of bounds");
        }
        output.push(PreparedTranslation {
            source: matched.source,
            asset_index: matched.asset_index,
            match_mode: matched.match_mode,
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        });
    }
    Ok(output)
}

pub(super) fn is_proven_tutorial_dialogue(asset: &LocalizedAudioAsset) -> bool {
    if !folded(&asset.true_name).contains("_tut") {
        return false;
    }
    asset.source_provenance.iter().any(|source| {
        source
            .get("bundle")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|bundle| {
                bundle.eq_ignore_ascii_case("TutorialAudio.resourceFile")
                    || bundle.eq_ignore_ascii_case("Tutorial.resourceFile")
            })
            || source
                .get("file")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|file| {
                    file.replace('\\', "/")
                        .to_ascii_lowercase()
                        .starts_with("tutorial.resourcefile/")
                })
    })
}

pub(super) fn commit_transaction(
    audio_root: &Path,
    stage: &Path,
    backup: &Path,
    manifest_path: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<()> {
    fs::create_dir(backup).map_err(|error| io_at(backup, error))?;
    let mut backed_up = Vec::new();
    for target in AUDIO_TARGETS {
        let current = audio_root.join(target);
        if current.exists() {
            let saved = backup.join(target);
            if let Err(error) = fs::rename(&current, &saved) {
                rollback_components(audio_root, stage, backup, &[], &backed_up);
                return Err(io_at(&current, error));
            }
            backed_up.push((*target).to_owned());
        }
    }
    let mut installed = Vec::new();
    for target in AUDIO_TARGETS {
        let source = stage.join(target);
        let destination = audio_root.join(target);
        if let Err(error) = fs::rename(&source, &destination) {
            rollback_components(audio_root, stage, backup, &installed, &backed_up);
            return Err(io_at(&destination, error));
        }
        installed.push((*target).to_owned());
    }
    if let Err(error) = replace_manifest(manifest_path, manifest) {
        rollback_components(audio_root, stage, backup, &installed, &backed_up);
        return Err(error);
    }
    fs::remove_dir_all(backup).map_err(|error| io_at(backup, error))?;
    fs::remove_dir(stage).map_err(|error| io_at(stage, error))
}

pub(super) fn rollback_components(
    audio_root: &Path,
    stage: &Path,
    backup: &Path,
    installed: &[String],
    backed_up: &[String],
) {
    for target in installed.iter().rev() {
        let _ = fs::rename(audio_root.join(target), stage.join(target));
    }
    for target in backed_up.iter().rev() {
        let _ = fs::rename(backup.join(target), audio_root.join(target));
    }
}

pub(super) fn hash_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path).map_err(|error| io_at(path, error))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| io_at(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn is_owned_entry(entry: &ProjectAssetFile) -> bool {
    OWNED_FILES.contains(&entry.path.as_str())
        || OWNED_PREFIXES
            .iter()
            .any(|prefix| entry.path.starts_with(prefix))
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    if !canonical.is_dir() {
        return invalid(format!("{label} is not a directory: {}", path.display()));
    }
    Ok(canonical)
}

pub(super) fn portable_relative(root: &Path, path: &Path) -> Result<String> {
    path.strip_prefix(root)
        .map_err(|_| invalid_error(format!("path escaped root: {}", path.display())))?
        .components()
        .map(|component| match component {
            Component::Normal(part) => part.to_str().map(str::to_owned).ok_or_else(|| {
                invalid_error(format!("path is not valid UTF-8: {}", path.display()))
            }),
            _ => Err(invalid_error(format!(
                "path is not a strict relative path: {}",
                path.display()
            ))),
        })
        .collect::<Result<Vec<_>>>()
        .map(|components| components.join("/"))
}

pub(super) fn folded(value: &str) -> String {
    value.nfkc().flat_map(char::to_lowercase).collect()
}

pub(super) fn portable_component(value: &str, context: &str) -> Result<String> {
    let normalized = value.nfkc().collect::<String>();
    let mut output = String::new();
    let mut separator_pending = false;
    for character in normalized.chars() {
        if character.is_alphanumeric() {
            if separator_pending && !output.is_empty() {
                output.push('_');
            }
            separator_pending = false;
            output.extend(character.to_lowercase());
        } else {
            separator_pending = true;
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    if output.is_empty() || output.len() > 180 {
        return invalid(format!(
            "{context} cannot be represented as a portable component"
        ));
    }
    Ok(output)
}

pub(super) fn unmatched_record(source: &RussianSourceFile, reason: &str) -> LocalizedVoiceUnmatchedFile {
    LocalizedVoiceUnmatchedFile {
        relative_path: source.relative_path.clone(),
        container: source.container.clone(),
        path_id: source.path_id,
        true_name: source.true_name.clone(),
        reason: reason.to_owned(),
    }
}

pub(super) fn ambiguous_record(
    source: &RussianSourceFile,
    reason: &str,
    candidates: &[usize],
    assets: &[LocalizedAudioAsset],
) -> LocalizedVoiceAmbiguousFile {
    let mut candidate_native_keys = candidates
        .iter()
        .filter_map(|index| assets.get(*index))
        .map(|asset| asset.native_key.clone())
        .collect::<Vec<_>>();
    let mut candidate_paths = candidates
        .iter()
        .filter_map(|index| assets.get(*index))
        .map(|asset| asset.path.clone())
        .collect::<Vec<_>>();
    candidate_native_keys.sort();
    candidate_native_keys.dedup();
    candidate_paths.sort();
    candidate_paths.dedup();
    LocalizedVoiceAmbiguousFile {
        relative_path: source.relative_path.clone(),
        container: source.container.clone().unwrap_or_default(),
        path_id: source.path_id.unwrap_or_default(),
        true_name: source.true_name.clone().unwrap_or_default(),
        reason: reason.to_owned(),
        candidate_native_keys,
        candidate_paths,
    }
}

pub(super) fn matching_policy() -> LocalizedAudioMatchingPolicy {
    LocalizedAudioMatchingPolicy {
        primary: "(casefold(container), pathId, exact true_name)".to_owned(),
        fallback: "(casefold(container), Unicode-NFKC-casefold(true_name)); only when primary has no candidates"
            .to_owned(),
        unmatched: "kept as an explicit gap; never guessed by pathId, filename, or hash alone"
            .to_owned(),
        ambiguous: "kept as an explicit gap; no candidate is selected".to_owned(),
    }
}

pub(super) fn append_policy_once(existing: String, clause: &str) -> String {
    if existing.contains(clause) {
        existing
    } else {
        format!("{existing}; {clause}")
    }
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
