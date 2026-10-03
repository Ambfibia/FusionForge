use super::*;

pub(super) fn validate_base_assets(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    assets: &[LocalizedAudioAsset],
) -> Result<()> {
    let manifest_by_path = manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for asset in assets {
        validate_relative_path(&asset.path)?;
        if !seen.insert(path_identity(&asset.path)) {
            return invalid(format!(
                "source catalog contains a case-insensitive path collision at {:?}",
                asset.path
            ));
        }
        let entry = manifest_by_path.get(asset.path.as_str()).ok_or_else(|| {
            invalid_error(format!(
                "source catalog path {:?} is absent from the project manifest",
                asset.path
            ))
        })?;
        if entry.kind != ProjectAssetKind::Audio
            || entry.bytes != asset.source_bytes
            || entry.blake3 != asset.source_blake3
        {
            return invalid(format!(
                "source catalog/manifest identity mismatch for {:?}",
                asset.path
            ));
        }
        let source = asset_root.join(native_path(&asset.path));
        let metadata = fs::metadata(&source).map_err(|error| io_at(&source, error))?;
        if !metadata.is_file() || metadata.len() != asset.source_bytes {
            return invalid(format!(
                "source semantic audio is missing or has the wrong size: {}",
                source.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn reject_duplicate_asset_matches(
    pending: Vec<PendingMatch>,
    assets: &[LocalizedAudioAsset],
    ambiguous: &mut Vec<LocalizedVoiceAmbiguousFile>,
) -> Vec<PendingMatch> {
    let mut by_asset = BTreeMap::<usize, Vec<PendingMatch>>::new();
    for matched in pending {
        by_asset
            .entry(matched.asset_index)
            .or_default()
            .push(matched);
    }
    let mut accepted = Vec::new();
    for (asset_index, matches) in by_asset {
        if matches.len() == 1 {
            accepted.extend(matches);
            continue;
        }
        for matched in matches {
            ambiguous.push(ambiguous_record(
                &matched.source,
                "multiple Russian source files target the same catalog asset",
                &[asset_index],
                assets,
            ));
        }
    }
    accepted.sort_by(|left, right| left.source.relative_path.cmp(&right.source.relative_path));
    accepted
}

pub(super) fn validate_staged_files(stage: &Path, entries: &[ProjectAssetFile]) -> Result<()> {
    let mut expected = entries
        .iter()
        .map(|entry| {
            let relative = entry.path.strip_prefix("audio/").unwrap_or("catalog.json");
            (
                path_identity(relative),
                (relative.to_owned(), entry.bytes, entry.blake3.clone()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let files = collect_files(stage)?;
    if files.len() != expected.len() {
        return invalid(format!(
            "staged localized audio file count mismatch: expected {}, found {}",
            expected.len(),
            files.len()
        ));
    }
    for relative in files {
        let Some((expected_path, expected_bytes, expected_hash)) =
            expected.remove(&path_identity(&relative))
        else {
            return invalid(format!(
                "unexpected staged localized audio file {relative:?}"
            ));
        };
        if relative != expected_path {
            return invalid(format!(
                "case/normalization alias in staged localized audio: expected {expected_path:?}, got {relative:?}"
            ));
        }
        let path = stage.join(native_path(&relative));
        let metadata = fs::metadata(&path).map_err(|error| io_at(&path, error))?;
        let hash = hash_file(&path)?;
        if metadata.len() != expected_bytes || hash != expected_hash {
            return invalid(format!(
                "staged localized audio identity mismatch for {relative:?}"
            ));
        }
    }
    if !expected.is_empty() {
        return invalid("one or more staged localized audio files are missing");
    }
    Ok(())
}

pub(super) fn validate_manifest_paths(manifest: &ProjectAssetManifest) -> Result<()> {
    let mut seen = BTreeMap::<String, String>::new();
    for entry in &manifest.files {
        validate_relative_path(&entry.path)?;
        if let Some(first) = seen.insert(path_identity(&entry.path), entry.path.clone()) {
            return invalid(format!(
                "project manifest path collision: {first:?} and {:?}",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_unowned_collisions(
    previous: &ProjectAssetManifest,
    next: &ProjectAssetManifest,
) -> Result<()> {
    let unowned = previous
        .files
        .iter()
        .filter(|entry| !is_owned_entry(entry))
        .map(|entry| path_identity(&entry.path))
        .collect::<BTreeSet<_>>();
    for entry in next.files.iter().filter(|entry| is_owned_entry(entry)) {
        if unowned.contains(&path_identity(&entry.path)) {
            return invalid(format!(
                "localized audio destination collides with an unowned manifest entry: {:?}",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty() || path.contains('\\') {
        return invalid(format!("invalid localized audio path {path:?}"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe localized audio path {path:?}"));
    }
    Ok(())
}

pub(super) fn validate_source_build(source_build: &str) -> Result<()> {
    if source_build.is_empty()
        || source_build.len() > 128
        || source_build.contains(['/', '\\'])
        || source_build == "."
        || source_build == ".."
    {
        return invalid(format!("invalid source build identity {source_build:?}"));
    }
    Ok(())
}

pub(super) fn reject_stale_transactions(asset_root: &Path) -> Result<()> {
    let entries = fs::read_dir(asset_root)
        .map_err(|error| io_at(asset_root, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(asset_root, error))?;
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".localized-voice-stage-")
            || name.starts_with(".localized-voice-backup-")
            || name.starts_with(".asset-manifest.localized-voice-next-")
            || name.starts_with(".asset-manifest.localized-voice-backup-")
        {
            return invalid(format!(
                "stale localized-voice transaction artifact exists at {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::ProjectAssetManifest(format!(
        "localized voice install failed: {}",
        message.into()
    ))
}
