use super::*;

pub(super) const RETROBUTION_EXACT_20260613_PACK_MANIFEST_BLAKE3: &str =
    "629a7f764f27808f61a5c115dfa4c6dbb528b0d468b974e45cf2d88362a70148";

pub(super) const OWNED_MANIFEST_PREFIXES: &[&str] = &[
    "audio/music/",
    "audio/ambient/",
    "audio/voice/",
    "audio/sfx/",
];

pub(super) fn canonical_path_identity(path: &str) -> String {
    normalized_identity(&path.replace('\\', "/"))
}

pub(super) fn catalog_counts(assets: &[SemanticAudioAsset]) -> Result<SemanticAudioCatalogCounts> {
    let mut counts = SemanticAudioCatalogCounts {
        assets: assets.len() as u64,
        bytes: 0,
        music: 0,
        ambient: 0,
        voice: 0,
        sfx: 0,
        explicit_shared_fallbacks: 0,
        variant_assets: 0,
    };
    for asset in assets {
        counts.bytes = counts
            .bytes
            .checked_add(asset.source_bytes)
            .ok_or_else(|| invalid_error("semantic audio catalog byte count overflow"))?;
        match asset.category {
            SemanticAudioCategory::Music => counts.music += 1,
            SemanticAudioCategory::Ambient => counts.ambient += 1,
            SemanticAudioCategory::Voice => counts.voice += 1,
            SemanticAudioCategory::Sfx => counts.sfx += 1,
        }
        if asset.classification.uncertain {
            counts.explicit_shared_fallbacks += 1;
        }
        if asset.variant.is_some() {
            counts.variant_assets += 1;
        }
    }
    Ok(counts)
}

pub(super) fn join_manifest_path(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.contains('\\') {
        return invalid(format!(
            "manifest path must use forward slashes: {relative:?}"
        ));
    }
    join_relative(root, relative)
}

pub(super) fn replace_manifest(path: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("asset manifest has no parent"))?;
    let next = parent.join(".asset-manifest.semantic-audio.next");
    let backup = parent.join(".asset-manifest.semantic-audio.backup");
    if next.exists() || backup.exists() {
        return invalid(format!(
            "stale semantic-audio manifest transaction file at {} or {}",
            next.display(),
            backup.display()
        ));
    }
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    write_new(&next, &bytes)?;
    fs::rename(path, &backup).map_err(|error| io_at(path, error))?;
    if let Err(error) = fs::rename(&next, path) {
        let _ = fs::rename(&backup, path);
        let _ = fs::remove_file(&next);
        return Err(io_at(path, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}
