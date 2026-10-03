use super::*;

pub(super) const RUNTIME_TEXTURE_SOURCE_METADATA_SCHEMA: &str = "ffone.offline.chartexture-metadata.v1";

pub(super) const RUNTIME_TEXTURE_SOURCE_METADATA_JSON: &str =
    include_str!("../../../fixtures/character_creation/retrobution-20260613-chartexture-metadata.json");

pub(super) const UNEQUIPPED_FEMALE_TEXTURE_SOURCE_METADATA_JSON: &str = include_str!(
    "../../../fixtures/character_creation/retrobution-20260613-f-naked-texture-metadata.json"
);

pub(super) const EQUIPMENT_TEXTURE_SOURCE_METADATA_SCHEMA: &str =
    "ffone.offline.player-equipment-texture-evidence.v1";

pub(super) const EQUIPMENT_TEXTURE_SOURCE_METADATA_JSON: &str = include_str!(
    "../../../fixtures/character_creation/retrobution-20260613-player-equipment-texture-metadata.json"
);

#[derive(Clone, Copy)]
pub(super) struct RuntimeTextureSentinel {
    pub(super) true_name: &'static str,
    pub(super) native_png_sha256: &'static str,
    pub(super) path_id: i64,
    pub(super) source_chain_sha256: &'static str,
}

/// Sentinels prove that the compact all-Texture2D evidence was extracted from
/// the same Retrobution object payloads as the original six-object audit.
pub(super) const RUNTIME_TEXTURE_SENTINELS: [RuntimeTextureSentinel; 6] = [
    RuntimeTextureSentinel {
        true_name: "m_face_001_a",
        native_png_sha256: "a036a57aa2eabb399d2ec3cd950c7ee6439f5d313e1e054189f0cb10119403f8",
        path_id: 161,
        source_chain_sha256: "a9aa0bc70b8ac3a680860d82ec0c60642b76025fc1f00274baf3fc3cd0648ae7",
    },
    RuntimeTextureSentinel {
        true_name: "m_head_001_a",
        native_png_sha256: "9ce4409e677dd1ca8159acb778c8187c624278f1329415f30966e41e9e9b0e80",
        path_id: 131,
        source_chain_sha256: "4a22e0ddfd533ecd13c616f83c5ad094133d640e140618426823d322b697814d",
    },
    RuntimeTextureSentinel {
        true_name: "m_skin",
        native_png_sha256: "f17b023acc22d4f61dc6a14ee67a9d570378a705ea1d69780cb86b0499d72716",
        path_id: 118,
        source_chain_sha256: "3cede1a60dd63aa40587bfdb26e8de49f6f93618c9ebc15b7b5f9e947f188241",
    },
    RuntimeTextureSentinel {
        true_name: "m_shirt_coolshirt",
        native_png_sha256: "abb028674ddc904448e030b8da70cd8abdd7fee957827d71fe712732f722963f",
        path_id: 360,
        source_chain_sha256: "9915f59c4484389decec5382081b36f37d17641e8046e1a5c0be2a360e013d2f",
    },
    RuntimeTextureSentinel {
        true_name: "m_pants_camo",
        native_png_sha256: "1172a7c1065d54abf72c472645478c5b487a7712a466f971c92e07f886d36fb8",
        path_id: 1712,
        source_chain_sha256: "018d79d1a7543c3fb2dac2d17af0076be1e1daafb5a306409439db29448cca52",
    },
    RuntimeTextureSentinel {
        true_name: "helmet_electronic",
        native_png_sha256: "12f4b0cf93dd4c00c36c8e0e31ed7235c36edabfb4806b58e8a6233c39cacb2b",
        path_id: 855,
        source_chain_sha256: "321a6f42b069cef2dea5a648ed6b0ba4e11ae5ee66db5f24d8032e54db74f90d",
    },
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterTextureRules {
    pub face_eye_suffix_by_code: Vec<CharacterTextureSuffix>,
    pub hair_eye_suffix: String,
    pub male_skin_texture: AvatarTextureReference,
    pub female_skin_texture: AvatarTextureReference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterTextureSuffix {
    pub code: u8,
    pub suffix: String,
}

/// Exact native texture interpretation for Texture2D assets assigned by
/// `ActorSkinCombiner` at runtime instead of being serialized on a material.
///
/// The legacy source locator is evidence only. Bevy loads `native_asset.path`;
/// no Unity archive is a runtime dependency.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRuntimeTextureContract {
    pub true_name: String,
    pub native_asset: CharacterCreationAssetReference,
    pub native_png_sha256: String,
    pub source: CharacterRuntimeTextureSource,
    pub usage_color_space: TextureColorSpace,
    pub usage_color_space_source: String,
    pub sampler: NativeSampler,
    pub published_mip_policy: PublishedMipPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRuntimeTextureSource {
    pub asset: String,
    pub container_route: String,
    pub path_id: i64,
    pub width: u32,
    pub height: u32,
    pub texture_format: i32,
    pub texture_format_name: String,
    pub complete_image_size: u64,
    pub source_chain_sha256: String,
    pub mip_map: bool,
    pub source_mip_count: u32,
    pub image_count: u32,
    pub texture_dimension: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRuntimeTextureCoverage {
    pub creator_choices: u64,
    pub creator_required_textures: u64,
    pub creator_published_textures: u64,
    pub avatar_texture_references: u64,
    pub avatar_verified_unique_routes: u64,
    pub avatar_published_routes: u64,
    pub avatar_deferred_verified_routes: u64,
    pub avatar_missing_true_names: Vec<String>,
    pub avatar_ambiguous_true_names: Vec<String>,
    pub avatar_missing_source_metadata: Vec<String>,
    pub source_metadata_textures: u64,
    pub source_metadata_unreadable: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeTextureSourceMetadataDocument {
    pub(super) schema: String,
    pub(super) source_path: String,
    pub(super) source_file_sha256: String,
    pub(super) source_asset: String,
    pub(super) texture_count: usize,
    pub(super) unreadable_texture_count: usize,
    pub(super) textures: Vec<RuntimeTextureSourceMetadata>,
    pub(super) unreadable_textures: Vec<RuntimeTextureUnreadable>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeTextureSourceMetadata {
    pub(super) true_name: String,
    pub(super) path_id: i64,
    pub(super) container_routes: Vec<String>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) texture_format: i32,
    pub(super) complete_image_size: u64,
    pub(super) source_chain_sha256: String,
    #[serde(default)]
    pub(super) native_png_bytes: Option<u64>,
    #[serde(default)]
    pub(super) native_png_blake3: Option<String>,
    pub(super) mip_map: bool,
    pub(super) source_mip_count: u32,
    pub(super) image_count: u32,
    pub(super) texture_dimension: i32,
    pub(super) filter_mode: i32,
    pub(super) wrap_mode: i32,
    pub(super) anisotropy_level: i32,
    pub(super) mip_map_bias: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeTextureUnreadable {
    pub(super) path_id: i64,
    pub(super) error: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentTextureSourceMetadataDocument {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) source_alias: String,
    pub(super) exact_entries: usize,
    pub(super) entries: Vec<EquipmentTextureSourceMetadata>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentTextureSourceMetadata {
    pub(super) true_name: String,
    pub(super) native_asset: CharacterCreationAssetReference,
    pub(super) source: EquipmentTextureSourceOwner,
    pub(super) texture: RuntimeTextureSourceMetadata,
    pub(super) route_repair: Option<EquipmentTextureRouteRepair>,
    pub(super) true_name_repair: Option<EquipmentTextureTrueNameRepair>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentTextureSourceOwner {
    pub(super) source_alias: String,
    pub(super) raw_resource_file: String,
    pub(super) raw_resource_file_bytes: u64,
    pub(super) raw_resource_file_sha256: String,
    pub(super) navigation_cache_alias: String,
    pub(super) navigation_cache_relative_path: String,
    pub(super) source_asset: String,
    pub(super) source_file_bytes: u64,
    pub(super) source_file_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentTextureRouteRepair {
    pub(super) kind: String,
    pub(super) requested_container_route: String,
    pub(super) serialized_container_routes: Vec<String>,
    pub(super) reason: String,
    pub(super) route_owner: Option<EquipmentTextureRouteOwner>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentTextureTrueNameRepair {
    pub(super) requested_true_name: String,
    pub(super) serialized_true_name: String,
    pub(super) requested_container_route: String,
    pub(super) reason: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentTextureRouteOwner {
    pub(super) source_alias: String,
    pub(super) raw_resource_file: String,
    pub(super) raw_resource_file_bytes: u64,
    pub(super) raw_resource_file_sha256: String,
    pub(super) source_asset: String,
    pub(super) source_file_bytes: u64,
    pub(super) source_file_sha256: String,
    pub(super) asset_bundle_path_id: i64,
    pub(super) external_file_id: i64,
    pub(super) external_path_id: i64,
    pub(super) target_source_asset: String,
    pub(super) target_path_id: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvatarTextureReference {
    pub true_name: String,
    pub status: NativeLookupStatus,
    pub candidates: Vec<CharacterCreationAssetReference>,
}

pub(super) fn build_runtime_texture_contracts(
    asset_root: &Path,
    provenance: CharacterCreationProvenance,
    manifest: &BTreeMap<String, ProjectAssetFile>,
    texture_index: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
    appearance: &CharacterCreationAppearance,
    avatar_items: &CharacterCreationAvatarItems,
) -> Result<CharacterCreationRuntimeTextures> {
    if provenance.source_build != "retrobution-20260613" {
        return invalid(format!(
            "creator texture sampler evidence belongs to retrobution-20260613, not {}",
            provenance.source_build
        ));
    }
    let source_document: RuntimeTextureSourceMetadataDocument =
        serde_json::from_str(RUNTIME_TEXTURE_SOURCE_METADATA_JSON).map_err(|source| {
            PipelineError::Json {
        path: "crates/ffone-asset-pipeline/fixtures/character_creation/retrobution-20260613-chartexture-metadata.json"
            .to_owned(),
        source,
    }
        })?;
    validate_runtime_texture_source_metadata(&source_document)?;
    let female_unequipped_source: RuntimeTextureSourceMetadataDocument =
        serde_json::from_str(UNEQUIPPED_FEMALE_TEXTURE_SOURCE_METADATA_JSON).map_err(|source| {
            PipelineError::Json {
        path:
            "crates/ffone-asset-pipeline/fixtures/character_creation/retrobution-20260613-f-naked-texture-metadata.json"
                .to_owned(),
        source,
    }
        })?;
    if female_unequipped_source.schema != RUNTIME_TEXTURE_SOURCE_METADATA_SCHEMA
        || female_unequipped_source.source_path
            != "retrobution-20260613/work/ffclienteditor/fusionforge-cli/NpcTexture_18c28fe0436740ec/CustomAssetBundle-d304c52c4bae348e38c743762c1bd818"
        || female_unequipped_source.source_file_sha256
            != "883659790a9cbc8562f4ad8122066df3e94e55d7a48b36b2b5222e5e05760b94"
        || female_unequipped_source.source_asset
            != "CustomAssetBundle-d304c52c4bae348e38c743762c1bd818"
        || female_unequipped_source.texture_count != 1
        || female_unequipped_source.textures.len() != 1
        || female_unequipped_source.unreadable_texture_count != 0
        || !female_unequipped_source.unreadable_textures.is_empty()
    {
        return invalid("Retrobution f_naked Texture2D metadata header/count/hash changed");
    }
    let female_source = &female_unequipped_source.textures[0];
    if female_source.true_name != "f_naked"
        || female_source.path_id != 48
        || female_source.container_routes != ["texture/f_naked.dds"]
        || female_source.width != 256
        || female_source.height != 256
        || female_source.texture_format != 10
        || female_source.complete_image_size != 32_768
        || female_source.source_chain_sha256
            != "04fcce5c70735e988fd987e94df0e7ba62dc357f08cd7d01b3500c981d577b78"
        || female_source.mip_map
        || female_source.source_mip_count != 1
        || female_source.image_count != 1
        || female_source.texture_dimension != 2
    {
        return invalid("Retrobution f_naked Texture2D source evidence changed");
    }
    let _ = runtime_texture_sampler(female_source)?;
    let equipment_source_document: EquipmentTextureSourceMetadataDocument =
        serde_json::from_str(EQUIPMENT_TEXTURE_SOURCE_METADATA_JSON).map_err(|source| {
            PipelineError::Json {
                path: "crates/ffone-asset-pipeline/fixtures/character_creation/retrobution-20260613-player-equipment-texture-metadata.json"
                    .to_owned(),
                source,
            }
        })?;
    validate_equipment_texture_source_metadata(&equipment_source_document)?;
    let mut equipment_source_index: BTreeMap<
        (String, String, u64),
        Vec<&EquipmentTextureSourceMetadata>,
    > = BTreeMap::new();
    for source in &equipment_source_document.entries {
        equipment_source_index
            .entry((
                source.true_name.to_ascii_lowercase(),
                source.native_asset.blake3.to_ascii_lowercase(),
                source.native_asset.bytes,
            ))
            .or_default()
            .push(source);
    }
    let mut source_index: BTreeMap<String, Vec<&RuntimeTextureSourceMetadata>> = BTreeMap::new();
    for source in &source_document.textures {
        source_index
            .entry(source.true_name.to_ascii_lowercase())
            .or_default()
            .push(source);
    }
    for source in &female_unequipped_source.textures {
        source_index
            .entry(source.true_name.to_ascii_lowercase())
            .or_default()
            .push(source);
    }
    validate_runtime_texture_sentinels(asset_root, texture_index, &source_index)?;

    let creator_routes = creator_runtime_texture_routes(appearance, avatar_items, texture_index)?;
    if creator_routes.len() != EXPECTED_CREATOR_RUNTIME_TEXTURES {
        return invalid(format!(
            "creator runtime texture closure changed: expected {EXPECTED_CREATOR_RUNTIME_TEXTURES}, found {}",
            creator_routes.len()
        ));
    }
    let published_routes =
        avatar_runtime_texture_routes(avatar_items, &creator_routes, &equipment_source_index)?;
    let mut textures = Vec::with_capacity(published_routes.len());
    for (true_name_key, (true_name, reference)) in &published_routes {
        let equipment_sources = equipment_source_index.get(&(
            true_name_key.clone(),
            reference.blake3.to_ascii_lowercase(),
            reference.bytes,
        ));
        let (source, source_asset, route_repair, true_name_repair) =
            if let Some(equipment_sources) = equipment_sources {
                if equipment_sources.len() != 1 {
                    return invalid(format!(
                        "equipment texture {true_name:?} has {} exact primary source candidates",
                        equipment_sources.len()
                    ));
                }
                let evidence = equipment_sources[0];
                if evidence.native_asset != *reference {
                    return invalid(format!(
                        "equipment texture {true_name:?} contradicts its exact native route"
                    ));
                }
                let resource_file = Path::new(&evidence.source.raw_resource_file)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| {
                        invalid_error("equipment source resource-file path is invalid")
                    })?;
                (
                    &evidence.texture,
                    format!("{resource_file}/{}", evidence.source.source_asset),
                    evidence.route_repair.as_ref(),
                    evidence.true_name_repair.as_ref(),
                )
            } else {
                let sources = source_index.get(true_name_key).ok_or_else(|| {
                    invalid_error(format!(
                        "runtime texture {true_name:?} has no exact source Texture2D metadata"
                    ))
                })?;
                if sources.len() != 1 {
                    return invalid(format!(
                        "runtime texture {true_name:?} has {} source Texture2D metadata candidates",
                        sources.len()
                    ));
                }
                let source_asset = if true_name.eq_ignore_ascii_case("f_naked") {
                    format!(
                        "NpcTexture.resourceFile/{}",
                        female_unequipped_source.source_asset
                    )
                } else {
                    format!("CharTexture.resourceFile/{}", source_document.source_asset)
                };
                (sources[0], source_asset, None, None)
            };
        let entry = manifest.get(&reference.path).ok_or_else(|| {
            invalid_error(format!(
                "creator texture {} is not manifest-listed",
                reference.path
            ))
        })?;
        if entry.kind != ProjectAssetKind::Texture {
            return invalid(format!(
                "creator texture {} is not typed as texture",
                reference.path
            ));
        }
        if entry.bytes != reference.bytes || entry.blake3 != reference.blake3 {
            return invalid(format!(
                "creator texture {} contradicts its native reference",
                reference.path
            ));
        }
        let bytes = read_verified(asset_root, entry)?;
        let native_png_sha256 = format!("{:x}", Sha256::digest(&bytes));
        let texture_format_name =
            runtime_texture_format_name(source.texture_format).ok_or_else(|| {
                invalid_error(format!(
                    "creator texture {true_name:?} uses unsupported source format {}",
                    source.texture_format
                ))
            })?;
        let container_route = effective_equipment_container_route(
            true_name,
            source,
            route_repair,
            true_name_repair,
        )
        .ok_or_else(|| {
            invalid_error(format!(
                "creator texture {true_name:?} has no exact or repaired AssetBundle container route"
            ))
        })?;
        let mut sampler = runtime_texture_sampler(source)?;
        sampler.name = true_name.clone();
        textures.push(CharacterRuntimeTextureContract {
            true_name: true_name.clone(),
            native_asset: native_reference(entry),
            native_png_sha256,
            source: CharacterRuntimeTextureSource {
                asset: source_asset,
                container_route,
                path_id: source.path_id,
                width: source.width,
                height: source.height,
                texture_format: source.texture_format,
                texture_format_name: texture_format_name.to_owned(),
                complete_image_size: source.complete_image_size,
                source_chain_sha256: source.source_chain_sha256.clone(),
                mip_map: source.mip_map,
                source_mip_count: source.source_mip_count,
                image_count: source.image_count,
                texture_dimension: source.texture_dimension,
            },
            usage_color_space: TextureColorSpace::Srgb,
            usage_color_space_source:
                "ActorSkinCombiner runtime assignment to ShaderLab _MainTex; same sRGB slot interpretation as audited static bindings"
                    .to_owned(),
            sampler,
            published_mip_policy: PublishedMipPolicy::BaseLevelOnly,
        });
    }
    textures.sort_by(|left, right| left.native_asset.path.cmp(&right.native_asset.path));
    let coverage = runtime_texture_coverage(
        appearance,
        avatar_items,
        &creator_routes,
        &published_routes,
        &source_document,
    );
    Ok(CharacterCreationRuntimeTextures {
        schema: CHARACTER_CREATION_RUNTIME_TEXTURES_SCHEMA.to_owned(),
        protocol: PROTOCOL_0104,
        provenance,
        coverage,
        textures,
    })
}

/// Extends the mandatory creator closure with every avatar-item texture whose
/// native PNG route and primary Texture2D metadata are both exact and unique.
/// Missing/ambiguous extension content remains explicitly deferred instead of
/// being assigned a guessed sampler contract.
pub(super) fn avatar_runtime_texture_routes(
    avatar_items: &CharacterCreationAvatarItems,
    creator_routes: &BTreeMap<String, (String, CharacterCreationAssetReference)>,
    source_index: &BTreeMap<(String, String, u64), Vec<&EquipmentTextureSourceMetadata>>,
) -> Result<BTreeMap<String, (String, CharacterCreationAssetReference)>> {
    let mut routes = creator_routes.clone();
    for item in &avatar_items.items {
        for visual in [&item.male, &item.female] {
            for reference in [&visual.primary_texture, &visual.secondary_texture]
                .into_iter()
                .flatten()
            {
                if reference.status != NativeLookupStatus::VerifiedUnique
                    || reference.candidates.len() != 1
                {
                    continue;
                }
                let candidate = &reference.candidates[0];
                let source = source_index
                    .get(&(
                        reference.true_name.to_ascii_lowercase(),
                        candidate.blake3.to_ascii_lowercase(),
                        candidate.bytes,
                    ))
                    .filter(|sources| sources.len() == 1)
                    .map(|sources| sources[0])
                    .filter(|source| source.native_asset == *candidate);
                if source.is_none_or(|source| {
                    runtime_texture_format_name(source.texture.texture_format).is_none()
                        || runtime_texture_sampler(&source.texture).is_err()
                        || effective_equipment_container_route(
                            &reference.true_name,
                            &source.texture,
                            source.route_repair.as_ref(),
                            source.true_name_repair.as_ref(),
                        )
                        .is_none()
                }) {
                    continue;
                }
                add_required_runtime_texture(
                    &mut routes,
                    reference,
                    &format!(
                        "avatar {:?} item {} texture",
                        item.category, item.item_number
                    ),
                )?;
            }
        }
    }
    Ok(routes)
}

pub(super) fn creator_runtime_texture_routes(
    appearance: &CharacterCreationAppearance,
    avatar_items: &CharacterCreationAvatarItems,
    textures: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
) -> Result<BTreeMap<String, (String, CharacterCreationAssetReference)>> {
    let items = avatar_items
        .items
        .iter()
        .map(|item| ((item.category, item.item_number), item))
        .collect::<BTreeMap<_, _>>();
    let mut routes = BTreeMap::new();
    add_required_runtime_texture(
        &mut routes,
        &appearance.texture_rules.male_skin_texture,
        "male skin",
    )?;
    add_required_runtime_texture(
        &mut routes,
        &appearance.texture_rules.female_skin_texture,
        "female skin",
    )?;
    for (true_name, label) in [
        ("m_naked", "male unequipped clothing"),
        ("f_naked", "female unequipped clothing"),
    ] {
        let reference = required_texture_reference(true_name, textures)?;
        add_required_runtime_texture(&mut routes, &reference, label)?;
    }
    for choice in &appearance.choices {
        let category = match choice.category {
            CharacterAppearanceCategory::Face => AvatarItemCategory::Face,
            CharacterAppearanceCategory::Hair => AvatarItemCategory::Head,
            CharacterAppearanceCategory::Shirt => AvatarItemCategory::Shirt,
            CharacterAppearanceCategory::Pants => AvatarItemCategory::Pants,
            CharacterAppearanceCategory::Shoes => AvatarItemCategory::Shoes,
        };
        let item = items.get(&(category, choice.value)).ok_or_else(|| {
            invalid_error(format!(
                "creator {:?}/{:?}/{} has no avatar item",
                choice.gender, choice.category, choice.value
            ))
        })?;
        let visual = match choice.gender {
            CharacterGender::Male => &item.male,
            CharacterGender::Female => &item.female,
        };
        match choice.category {
            CharacterAppearanceCategory::Face => {
                let base = visual.primary_texture.as_ref().ok_or_else(|| {
                    invalid_error(format!(
                        "creator {:?} face {} has no texture basename",
                        choice.gender, choice.value
                    ))
                })?;
                for suffix in &appearance.texture_rules.face_eye_suffix_by_code {
                    let reference = required_texture_reference(
                        &format!("{}_{}", base.true_name, suffix.suffix),
                        textures,
                    )?;
                    add_required_runtime_texture(
                        &mut routes,
                        &reference,
                        "creator face eye variant",
                    )?;
                }
            }
            CharacterAppearanceCategory::Hair => {
                let base = visual.primary_texture.as_ref().ok_or_else(|| {
                    invalid_error(format!(
                        "creator {:?} hair {} has no texture basename",
                        choice.gender, choice.value
                    ))
                })?;
                let reference = required_texture_reference(
                    &format!(
                        "{}_{}",
                        base.true_name, appearance.texture_rules.hair_eye_suffix
                    ),
                    textures,
                )?;
                add_required_runtime_texture(&mut routes, &reference, "creator hair variant")?;
            }
            CharacterAppearanceCategory::Shirt
            | CharacterAppearanceCategory::Pants
            | CharacterAppearanceCategory::Shoes => {
                for (slot, reference) in [
                    ("primary", &visual.primary_texture),
                    ("secondary", &visual.secondary_texture),
                ] {
                    if let Some(reference) = reference {
                        add_required_runtime_texture(
                            &mut routes,
                            reference,
                            &format!(
                                "creator {:?} {:?} {} {slot}",
                                choice.gender, choice.category, choice.value
                            ),
                        )?;
                    }
                }
            }
        }
    }
    Ok(routes)
}

pub(super) fn add_required_runtime_texture(
    routes: &mut BTreeMap<String, (String, CharacterCreationAssetReference)>,
    reference: &AvatarTextureReference,
    label: &str,
) -> Result<()> {
    if reference.status != NativeLookupStatus::VerifiedUnique || reference.candidates.len() != 1 {
        return invalid(format!(
            "{label} texture {:?} is {:?} with {} native candidates",
            reference.true_name,
            reference.status,
            reference.candidates.len()
        ));
    }
    let candidate = reference.candidates[0].clone();
    let key = reference.true_name.to_ascii_lowercase();
    if let Some((existing_name, existing)) = routes.get(&key) {
        if existing != &candidate {
            return invalid(format!(
                "{label} texture {:?} contradicts existing route {}",
                existing_name, existing.path
            ));
        }
        return Ok(());
    }
    routes.insert(key, (reference.true_name.clone(), candidate));
    Ok(())
}

pub(super) fn validate_runtime_texture_source_metadata(
    document: &RuntimeTextureSourceMetadataDocument,
) -> Result<()> {
    if document.schema != RUNTIME_TEXTURE_SOURCE_METADATA_SCHEMA
        || document.source_path
            != "retrobution-20260613/CharTexture.resourceFile/CustomAssetBundle-aa120043d3c634fe9adfb5cbe08e6970"
        || document.source_file_sha256
            != "5319d19e18eecf7223a1997d5e88cfa355b8715d68a217f961a102125f803d6d"
        || document.source_asset != "CustomAssetBundle-aa120043d3c634fe9adfb5cbe08e6970"
        || document.texture_count != 2_160
        || document.textures.len() != document.texture_count
        || document.unreadable_texture_count != 54
        || document.unreadable_textures.len() != document.unreadable_texture_count
    {
        return invalid("Retrobution CharTexture metadata header/count/hash changed");
    }
    if document
        .unreadable_textures
        .iter()
        .any(|entry| entry.path_id <= 0 || entry.error.trim().is_empty())
    {
        return invalid("Retrobution CharTexture unreadable-object report is invalid");
    }
    for source in &document.textures {
        if source.true_name.trim().is_empty()
            || source.path_id <= 0
            || source.container_routes.is_empty()
            || source.width == 0
            || source.height == 0
            || source.complete_image_size == 0
            || source.source_chain_sha256.len() != 64
            || source.source_mip_count == 0
            || source.image_count == 0
            || source.texture_dimension != 2
            || runtime_texture_format_name(source.texture_format).is_none()
        {
            return invalid(format!(
                "source Texture2D metadata for {:?} is incomplete",
                source.true_name
            ));
        }
        let _ = runtime_texture_sampler(source)?;
    }
    Ok(())
}
