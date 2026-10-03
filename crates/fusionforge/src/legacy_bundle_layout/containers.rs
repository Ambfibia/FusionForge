use super::*;

#[derive(Debug, Clone)]
pub(super) struct DanglingObjectPointer {
    pub(super) source: SourcePointerKey,
    pub(super) description: String,
}

#[derive(Debug, Clone)]
pub(super) struct SourceBundle {
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) size: u64,
    /// Exact CachingManifest phases in which the legacy bundle was loaded.  This is
    /// runtime lifecycle data, not merely reporting metadata: loading a PaidZone-only
    /// graph during CharacterCreation makes Unity 2.x eagerly activate assets which the
    /// original client deliberately kept dormant.
    pub(super) sections: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LegacyBundleDependency {
    pub bundle: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LegacyRoutedBundle {
    pub name: String,
    pub sections: Vec<String>,
}

pub(super) fn dong_tile_from_bundle(bundle: &str, prefix: &str) -> Option<String> {
    let stem = bundle
        .strip_suffix(".resourceFile")
        .or_else(|| bundle.strip_suffix(".resourcefile"))?;
    stem.to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
        .then(|| stem.to_string())
}

/// These filenames are polled literally by legacy coroutines. They therefore remain as
/// tiny route-less AssetBundle shells even after their payload is moved into semantic packs.
pub(super) const COMPAT_BUNDLE_NAMES: &[&str] = &[
    "CharacterCreation.resourceFile",
    "CharacterSelection.resourceFile",
    "Tutorial.resourceFile",
    "NpcTexture.resourceFile",
];

pub(super) fn harmless_object_size_mismatch(raw: &[u8], consumed: usize, expected: usize) -> bool {
    let trailing_zero_padding = raw
        .get(consumed..)
        .is_some_and(|tail| tail.iter().all(|byte| *byte == 0));
    let implicit_final_alignment =
        consumed > expected && consumed == (expected.saturating_add(3) & !3);
    trailing_zero_padding || implicit_final_alignment
}

pub(super) fn object_roundtrip_is_lossless(raw: &[u8], rebuilt: &[u8]) -> bool {
    if raw == rebuilt {
        return true;
    }
    // Some legacy ObjectInfo sizes include an unused zero tail that is not represented by
    // the TypeTree. Re-emitting the parsed value without that tail is lossless.
    if raw.starts_with(rebuilt) && raw[rebuilt.len()..].iter().all(|byte| *byte == 0) {
        return true;
    }
    // Conversely, a few Unity 2.x ObjectInfo sizes omit only the final align4 padding.
    let aligned_raw_len = raw.len().saturating_add(3) & !3;
    rebuilt.len() == aligned_raw_len
        && rebuilt.starts_with(raw)
        && rebuilt[raw.len()..].iter().all(|byte| *byte == 0)
}

pub(super) fn invalid_serialized_object_name(name: &str) -> bool {
    name.len() > 4096 || name.chars().any(|ch| ch == '\u{fffd}' || ch.is_control())
}

pub(super) fn strict_read_object(
    asset: &Asset,
    asset_index: usize,
    info: &ObjectInfo,
) -> Result<UnityValue, String> {
    let object_type = asset.object_type_name(info);
    let result = asset
        .read_object_with_size(asset_index, info)
        .map_err(|err| format!("{}#{} {}: {err}", asset.name, info.path_id, object_type))?;
    let raw = asset.object_raw_data(info)?;
    if result.consumed != result.expected {
        // Unity 2.x occasionally excludes the final 1-3 alignment bytes from an
        // ObjectInfo size. BinaryReader::align4 is the only object reader operation
        // allowed to seek past the slice, so reaching exactly align4(expected) means
        // every declared byte was parsed and only the absent inter-object padding was
        // crossed. The rebuilt object writes that padding explicitly.
        let harmless_padding = harmless_object_size_mismatch(raw, result.consumed, result.expected);
        if !harmless_padding {
            return Err(format!(
                "{}#{} {} consumed {} of {} bytes; refusing lossy reserialization",
                asset.name, info.path_id, object_type, result.consumed, result.expected
            ));
        }
    }
    if let Some(name) = result.value.get("m_Name").and_then(UnityValue::as_str) {
        if invalid_serialized_object_name(name) {
            return Err(format!(
                "{}#{} {} has an invalid serialized m_Name; refusing binary-data reinterpretation",
                asset.name, info.path_id, object_type
            ));
        }
    }
    let rebuilt = asset.serialize_object_value(asset_index, info, &result.value)?;
    if !object_roundtrip_is_lossless(raw, &rebuilt) {
        return Err(format!(
            "{}#{} {} is not byte-stable through its TypeTree ({} source bytes, {} rebuilt); refusing lossy reserialization",
            asset.name,
            info.path_id,
            object_type,
            raw.len(),
            rebuilt.len()
        ));
    }
    Ok(result.value)
}

pub(super) fn source_bundle_name<'a>(inventory: &'a Inventory, root: &Root) -> &'a str {
    let asset = &inventory.assets[root.source_asset];
    &inventory.bundles[asset.bundle_index].name
}

pub(super) fn retained_bundle_paths(out_dir: &Path) -> Result<Vec<PathBuf>, String> {
    const RETAINED_RESOURCES: &[&str] = &[
        "futuremusic.resourcefile",
        "lobbymusic.resourcefile",
        "pastmusic.resourcefile",
        "retromusic.resourcefile",
        "tabledata.resourcefile",
    ];
    let mut paths = fs::read_dir(out_dir)
        .map_err(|err| format!("{}: {err}", out_dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            let lower = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            lower.ends_with(".unity3d") || RETAINED_RESOURCES.contains(&lower.as_str())
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}
