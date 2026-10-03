use super::*;

pub(super) fn validate_equipment_texture_source_metadata(
    document: &EquipmentTextureSourceMetadataDocument,
) -> Result<()> {
    if document.schema != EQUIPMENT_TEXTURE_SOURCE_METADATA_SCHEMA
        || document.source_build != "retrobution-20260613"
        || document.source_alias != "primary"
        || document.exact_entries != 2_421
        || document.entries.len() != document.exact_entries
    {
        return invalid("primary player-equipment Texture2D evidence header/count changed");
    }
    let mut identities = BTreeSet::new();
    for entry in &document.entries {
        let source = &entry.source;
        let texture = &entry.texture;
        if entry.true_name.trim().is_empty()
            || entry.native_asset.path.trim().is_empty()
            || entry.native_asset.bytes == 0
            || entry.native_asset.blake3.len() != 64
            || texture.native_png_bytes != Some(entry.native_asset.bytes)
            || texture.native_png_blake3.as_deref() != Some(entry.native_asset.blake3.as_str())
            || source.source_alias != "primary"
            || !source
                .raw_resource_file
                .starts_with("builds/retrobution-20260613/")
            || source.raw_resource_file_bytes == 0
            || source.raw_resource_file_sha256.len() != 64
            || source.navigation_cache_alias != "patched"
            || source.navigation_cache_relative_path.trim().is_empty()
            || source.navigation_cache_relative_path.contains("..")
            || source.source_asset.trim().is_empty()
            || source.source_file_bytes == 0
            || source.source_file_sha256.len() != 64
            || runtime_texture_format_name(texture.texture_format).is_none()
            || !valid_equipment_route_resolution(entry)
            || !identities.insert((
                entry.true_name.to_ascii_lowercase(),
                entry.native_asset.blake3.to_ascii_lowercase(),
                entry.native_asset.bytes,
            ))
        {
            return invalid(format!(
                "primary player-equipment Texture2D evidence for {:?} is incomplete",
                entry.true_name
            ));
        }
        let _ = runtime_texture_sampler(texture)?;
    }
    Ok(())
}

pub(super) fn validate_runtime_texture_sentinels(
    asset_root: &Path,
    texture_index: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
    source_index: &BTreeMap<String, Vec<&RuntimeTextureSourceMetadata>>,
) -> Result<()> {
    for sentinel in RUNTIME_TEXTURE_SENTINELS {
        let sources = source_index
            .get(&sentinel.true_name.to_ascii_lowercase())
            .ok_or_else(|| invalid_error("runtime texture sentinel source disappeared"))?;
        if sources.len() != 1
            || sources[0].path_id != sentinel.path_id
            || sources[0].source_chain_sha256 != sentinel.source_chain_sha256
        {
            return invalid(format!(
                "runtime texture sentinel {} source evidence changed",
                sentinel.true_name
            ));
        }
        let reference = required_texture_reference(sentinel.true_name, texture_index)?;
        let entry = &reference.candidates[0];
        let bytes = fs::read(asset_root.join(&entry.path))
            .map_err(|error| io_at(&asset_root.join(&entry.path), error))?;
        if format!("{:x}", Sha256::digest(&bytes)) != sentinel.native_png_sha256 {
            return invalid(format!(
                "runtime texture sentinel {} native PNG changed",
                sentinel.true_name
            ));
        }
    }
    Ok(())
}

pub(super) fn runtime_texture_sampler(source: &RuntimeTextureSourceMetadata) -> Result<NativeSampler> {
    let has_mips = source.source_mip_count > 1;
    let mag_filter = if source.filter_mode == 0 {
        SamplerMagFilter::Nearest
    } else {
        SamplerMagFilter::Linear
    };
    let min_filter = match (source.filter_mode, has_mips) {
        (0, false) => SamplerMinFilter::Nearest,
        (0, true) => SamplerMinFilter::NearestMipmapNearest,
        (1, false) | (2, false) => SamplerMinFilter::Linear,
        (1, true) => SamplerMinFilter::LinearMipmapNearest,
        (2, true) => SamplerMinFilter::LinearMipmapLinear,
        _ => {
            return invalid(format!(
                "source Texture2D {:?} has unsupported filter mode {}",
                source.true_name, source.filter_mode
            ));
        }
    };
    let wrap = match source.wrap_mode {
        0 => SamplerWrapMode::Repeat,
        1 => SamplerWrapMode::ClampToEdge,
        _ => {
            return invalid(format!(
                "source Texture2D {:?} has unsupported wrap mode {}",
                source.true_name, source.wrap_mode
            ));
        }
    };
    if !source.mip_map && source.source_mip_count != 1 {
        return invalid(format!(
            "source Texture2D {:?} contradicts its mip flag/count",
            source.true_name
        ));
    }
    Ok(NativeSampler {
        name: source.true_name.clone(),
        mag_filter,
        min_filter,
        wrap_s: wrap,
        wrap_t: wrap,
        legacy_filter_mode: source.filter_mode,
        legacy_wrap_mode: source.wrap_mode,
        anisotropy_level: u32::try_from(source.anisotropy_level).map_err(|_| {
            invalid_error(format!(
                "source Texture2D {:?} has negative anisotropy",
                source.true_name
            ))
        })?,
        mip_map_bias: source.mip_map_bias,
    })
}

pub(super) fn runtime_texture_coverage(
    appearance: &CharacterCreationAppearance,
    avatar_items: &CharacterCreationAvatarItems,
    creator_routes: &BTreeMap<String, (String, CharacterCreationAssetReference)>,
    published_texture_routes: &BTreeMap<String, (String, CharacterCreationAssetReference)>,
    source_document: &RuntimeTextureSourceMetadataDocument,
) -> CharacterRuntimeTextureCoverage {
    let published_paths = published_texture_routes
        .values()
        .map(|(_, reference)| reference.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut texture_references = 0_u64;
    let mut verified_routes = BTreeSet::new();
    let mut published_routes = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut ambiguous = BTreeSet::new();
    let mut missing_source = BTreeSet::new();
    for item in &avatar_items.items {
        for visual in [&item.male, &item.female] {
            for reference in [&visual.primary_texture, &visual.secondary_texture]
                .into_iter()
                .flatten()
            {
                texture_references += 1;
                match reference.status {
                    NativeLookupStatus::VerifiedUnique if reference.candidates.len() == 1 => {
                        let path = reference.candidates[0].path.as_str();
                        verified_routes.insert(path);
                        if published_paths.contains(path) {
                            published_routes.insert(path);
                        }
                        if !published_paths.contains(path) {
                            missing_source.insert(reference.true_name.clone());
                        }
                    }
                    NativeLookupStatus::Missing => {
                        missing.insert(reference.true_name.clone());
                    }
                    _ => {
                        ambiguous.insert(reference.true_name.clone());
                    }
                }
            }
        }
    }
    CharacterRuntimeTextureCoverage {
        creator_choices: appearance.choices.len() as u64,
        creator_required_textures: creator_routes.len() as u64,
        creator_published_textures: creator_routes.len() as u64,
        avatar_texture_references: texture_references,
        avatar_verified_unique_routes: verified_routes.len() as u64,
        avatar_published_routes: published_routes.len() as u64,
        avatar_deferred_verified_routes: verified_routes
            .len()
            .saturating_sub(published_routes.len()) as u64,
        avatar_missing_true_names: missing.into_iter().collect(),
        avatar_ambiguous_true_names: ambiguous.into_iter().collect(),
        avatar_missing_source_metadata: missing_source.into_iter().collect(),
        source_metadata_textures: source_document.texture_count as u64,
        source_metadata_unreadable: source_document.unreadable_texture_count as u64,
    }
}

pub(super) fn texture_reference(
    source_name: &str,
    textures: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
) -> Result<Option<AvatarTextureReference>> {
    let true_name = true_name(source_name);
    if true_name.is_empty() {
        return Ok(None);
    }
    let candidates = texture_candidates(textures, &true_name);
    Ok(Some(AvatarTextureReference {
        status: candidate_status(&candidates),
        candidates,
        true_name,
    }))
}

pub(super) fn required_texture_reference(
    true_name: &str,
    textures: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
) -> Result<AvatarTextureReference> {
    let reference = texture_reference(true_name, textures)?
        .ok_or_else(|| invalid_error(format!("required texture {true_name:?} is null")))?;
    if reference.status != NativeLookupStatus::VerifiedUnique {
        return invalid(format!(
            "required texture {true_name:?} is not uniquely native-resolved"
        ));
    }
    Ok(reference)
}

pub(super) fn texture_index(
    manifest: &[ProjectAssetFile],
) -> BTreeMap<String, Vec<CharacterCreationAssetReference>> {
    let mut output: BTreeMap<String, Vec<CharacterCreationAssetReference>> = BTreeMap::new();
    for entry in manifest
        .iter()
        .filter(|entry| entry.kind == ProjectAssetKind::Texture)
    {
        let Some(stem) = Path::new(&entry.path)
            .file_stem()
            .and_then(|stem| stem.to_str())
        else {
            continue;
        };
        let logical = strip_collision_suffix(stem).to_ascii_lowercase();
        output
            .entry(logical)
            .or_default()
            .push(native_reference(entry));
    }
    for candidates in output.values_mut() {
        candidates.sort_by(|left, right| left.path.cmp(&right.path));
        candidates.dedup_by(|left, right| left.blake3 == right.blake3);
    }
    output
}

pub(super) fn texture_candidates(
    textures: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
    true_name: &str,
) -> Vec<CharacterCreationAssetReference> {
    let key = true_name.to_ascii_lowercase();
    textures
        .get(&key)
        .or_else(|| texture_alias(&key).and_then(|alias| textures.get(alias)))
        .cloned()
        .unwrap_or_default()
}

pub(super) fn repair_exact_creator_texture_aliases(
    avatar_items: &mut CharacterCreationAvatarItems,
    textures: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
) -> Result<()> {
    let exact_aliases = [
        (
            248,
            "f_shoes_pink lady_a",
            CharacterCreationAssetReference {
                path: "characters/player/shared/runtime-textures/f_shoes_pink_lady_a.png"
                    .to_owned(),
                bytes: 61_335,
                blake3: "af38502c43ec3463d4e61471f538314481bb32ca486a585af0b37af5dbaaaf50"
                    .to_owned(),
            },
        ),
        (
            250,
            "f_shoes_pink lady_b",
            CharacterCreationAssetReference {
                path: "characters/player/shared/runtime-textures/f_shoes_pink_lady_b.png"
                    .to_owned(),
                bytes: 61_377,
                blake3: "d418078ed62d4fb3770e6c5129ca6177a707f335c84663f468eb7b03751e9cca"
                    .to_owned(),
            },
        ),
    ];
    for (item_number, true_name, expected_native_asset) in exact_aliases {
        let mut matches = avatar_items.items.iter_mut().filter(|item| {
            item.category == AvatarItemCategory::Shoes && item.item_number == item_number
        });
        let item = matches.next().ok_or_else(|| {
            invalid_error(format!(
                "exact female creator shoe item {item_number} is absent"
            ))
        })?;
        if matches.next().is_some() {
            return invalid(format!(
                "exact female creator shoe item {item_number} is duplicated"
            ));
        }
        let current = item.female.primary_texture.as_mut().ok_or_else(|| {
            invalid_error(format!(
                "exact female creator shoe item {item_number} has no primary texture"
            ))
        })?;
        if current.true_name != true_name {
            return invalid(format!(
                "female creator shoe item {item_number} names {:?}, expected exact {:?}",
                current.true_name, true_name
            ));
        }
        let repaired = required_texture_reference(true_name, textures)?;
        if repaired.candidates.as_slice() != [expected_native_asset] {
            return invalid(format!(
                "exact female creator shoe alias {true_name:?} resolved to unexpected native evidence"
            ));
        }
        match current.status {
            NativeLookupStatus::Missing if current.candidates.is_empty() => {
                *current = repaired;
            }
            NativeLookupStatus::VerifiedUnique if *current == repaired => {}
            _ => {
                return invalid(format!(
                    "female creator shoe alias {true_name:?} has non-repairable status {:?} with {} candidates",
                    current.status,
                    current.candidates.len()
                ));
            }
        }
    }
    avatar_items.counts = avatar_item_counts(&avatar_items.items);
    avatar_items.lookup_complete = avatar_items.counts.model_references
        == avatar_items.counts.resolved_models
        && avatar_items.counts.texture_references == avatar_items.counts.resolved_textures
        && avatar_items.counts.icon_references == avatar_items.counts.resolved_icons;
    Ok(())
}

pub(super) fn repair_exact_equipment_texture_routes(items: &mut [AvatarItemLookup]) -> Result<()> {
    let document: EquipmentTextureSourceMetadataDocument =
        serde_json::from_str(EQUIPMENT_TEXTURE_SOURCE_METADATA_JSON).map_err(|source| {
            PipelineError::Json {
                path: "crates/ffone-asset-pipeline/fixtures/character_creation/retrobution-20260613-player-equipment-texture-metadata.json"
                    .to_owned(),
                source,
            }
        })?;
    validate_equipment_texture_source_metadata(&document)?;
    let exact_by_name = document
        .entries
        .iter()
        .map(|entry| (entry.true_name.to_ascii_lowercase(), &entry.native_asset))
        .collect::<BTreeMap<_, _>>();
    for item in items {
        for visual in [&mut item.male, &mut item.female] {
            for reference in [&mut visual.primary_texture, &mut visual.secondary_texture]
                .into_iter()
                .flatten()
            {
                let Some(native_asset) =
                    exact_by_name.get(&reference.true_name.to_ascii_lowercase())
                else {
                    continue;
                };
                if reference.status == NativeLookupStatus::VerifiedUnique
                    && reference.candidates.len() == 1
                    && (reference.candidates[0].bytes != native_asset.bytes
                        || !reference.candidates[0]
                            .blake3
                            .eq_ignore_ascii_case(&native_asset.blake3))
                {
                    return invalid(format!(
                        "avatar texture {:?} contradicts its exact primary table-route evidence",
                        reference.true_name
                    ));
                }
                reference.status = NativeLookupStatus::VerifiedUnique;
                reference.candidates = vec![(*native_asset).clone()];
            }
        }
    }
    Ok(())
}

pub(super) fn texture_alias(true_name: &str) -> Option<&'static str> {
    match true_name {
        // FusionForge preserves Texture2D.m_Name in the data contract while
        // the readable Windows-safe PNG route replaces the embedded space.
        "f_shoes_pink lady" => Some("f_shoes_pink_lady"),
        "f_shoes_pink lady_a" => Some("f_shoes_pink_lady_a"),
        "f_shoes_pink lady_b" => Some("f_shoes_pink_lady_b"),
        "f_shoes_pink lady_c" => Some("f_shoes_pink_lady_c"),
        "f_shoes_pink lady_d" => Some("f_shoes_pink_lady_d"),
        _ => None,
    }
}
