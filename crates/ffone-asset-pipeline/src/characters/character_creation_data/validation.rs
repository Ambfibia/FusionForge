use super::*;

pub(super) fn validate_documents(documents: &CatalogDocuments) -> Result<()> {
    let names = &documents.name_wheel;
    if names.first_names.len() != EXPECTED_FIRST_NAMES
        || names.middle_names.len() != EXPECTED_MIDDLE_NAMES
        || names.last_names.len() != EXPECTED_LAST_NAMES
    {
        return invalid(format!(
            "Retrobution name-wheel counts changed: first={}, middle={}, last={}",
            names.first_names.len(),
            names.middle_names.len(),
            names.last_names.len()
        ));
    }
    for entries in [&names.first_names, &names.middle_names, &names.last_names] {
        if entries
            .iter()
            .enumerate()
            .any(|(code, entry)| entry.code as usize != code)
        {
            return invalid("name-wheel numeric codes do not equal source array indices");
        }
    }
    if names.first_names[0].value != ""
        || names.middle_names[0].value != ""
        || names.middle_names[1].value != " "
        || names.last_names[0].value != ""
        || names.last_names[1].value != " "
        || names.first_names[1].value != "Abbey"
        || names.first_names[600].value != "Zora"
        || names.middle_names[600].value != "Zort"
        || names.last_names[601].value != "Zon"
    {
        return invalid("Retrobution name-wheel sentinel or boundary codes changed");
    }
    if documents.appearance.creation_rows.len() != EXPECTED_CREATION_ROWS {
        return invalid(format!(
            "Retrobution creation row count changed: {}",
            documents.appearance.creation_rows.len()
        ));
    }
    if documents.runtime_textures.schema != CHARACTER_CREATION_RUNTIME_TEXTURES_SCHEMA
        || documents.runtime_textures.protocol != PROTOCOL_0104
        || documents.runtime_textures.textures.len() < EXPECTED_CREATOR_RUNTIME_TEXTURES
        || documents.runtime_textures.coverage.creator_choices
            != documents.appearance.choices.len() as u64
        || documents
            .runtime_textures
            .coverage
            .creator_required_textures
            != EXPECTED_CREATOR_RUNTIME_TEXTURES as u64
        || documents
            .runtime_textures
            .coverage
            .creator_published_textures
            != EXPECTED_CREATOR_RUNTIME_TEXTURES as u64
        || documents.runtime_textures.coverage.avatar_published_routes
            + documents
                .runtime_textures
                .coverage
                .avatar_deferred_verified_routes
            != documents
                .runtime_textures
                .coverage
                .avatar_verified_unique_routes
    {
        return invalid("character runtime texture contract is incomplete");
    }
    for texture in &documents.runtime_textures.textures {
        if texture.usage_color_space != TextureColorSpace::Srgb
            || texture.published_mip_policy != PublishedMipPolicy::BaseLevelOnly
            || texture.source.source_mip_count == 0
            || texture.sampler.name != texture.true_name
            || runtime_texture_format_name(texture.source.texture_format)
                != Some(texture.source.texture_format_name.as_str())
        {
            return invalid(format!(
                "creator texture {} contradicts audited source/sampler/color contract",
                texture.true_name
            ));
        }
    }
    let expected_choices = 5 + 5 + 23 + 21 + 30 + 30 + 30 + 30 + 30 + 27;
    if documents.appearance.choices.len() != expected_choices {
        return invalid(format!(
            "Retrobution creation choice count changed: {}",
            documents.appearance.choices.len()
        ));
    }
    let items = documents
        .avatar_items
        .items
        .iter()
        .map(|item| ((item.category, item.item_number), item))
        .collect::<BTreeMap<_, _>>();
    for choice in &documents.appearance.choices {
        let item_category = match choice.category {
            CharacterAppearanceCategory::Face => AvatarItemCategory::Face,
            CharacterAppearanceCategory::Hair => AvatarItemCategory::Head,
            CharacterAppearanceCategory::Shirt => AvatarItemCategory::Shirt,
            CharacterAppearanceCategory::Pants => AvatarItemCategory::Pants,
            CharacterAppearanceCategory::Shoes => AvatarItemCategory::Shoes,
        };
        let _item = items
            .get(&(item_category, choice.value))
            .ok_or_else(|| invalid_error("starter choice item disappeared from avatar lookup"))?;
        // Every selector must remain representable even while the independently
        // published native equipment catalog still carries a typed Missing or
        // Ambiguous route.  Rejecting such rows here would silently truncate the
        // exact Retrobution selector wheel.  The runtime resolver reports the
        // typed route blocker only when that specific look is requested.
        if choice
            .icon
            .as_ref()
            .is_some_and(|icon| icon.status != NativeLookupStatus::VerifiedUnique)
        {
            return invalid(format!(
                "starter choice {:?}/{:?}/{} icon is not uniquely resolved",
                choice.gender, choice.category, choice.creation_index
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_equipment_catalog(
    asset_root: &Path,
    manifest: &BTreeMap<String, ProjectAssetFile>,
    catalog: &PlayerEquipmentCatalog,
) -> Result<()> {
    if catalog.schema != PLAYER_EQUIPMENT_CATALOG_SCHEMA
        || !catalog.runtime_accepted
        || !catalog.standalone_gpu_passed
        || catalog.models.len() as u64 != catalog.counts.models
    {
        return invalid("installed player-equipment catalog is not a complete standalone GPU pass");
    }
    for model in &catalog.models {
        let entry = manifest.get(&model.glb).ok_or_else(|| {
            invalid_error(format!(
                "equipment model {} is not manifest-listed",
                model.glb
            ))
        })?;
        if entry.kind != ProjectAssetKind::Model || entry.blake3 != model.glb_blake3 {
            return invalid(format!(
                "equipment model manifest mismatch for {}",
                model.glb
            ));
        }
        let path = asset_root.join(&model.glb);
        let metadata = fs::metadata(&path).map_err(|error| io_at(&path, error))?;
        if metadata.len() != entry.bytes {
            return invalid(format!(
                "equipment model byte length changed: {}",
                model.glb
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_relative(path: &str) -> Result<()> {
    let value = Path::new(path);
    if path.is_empty()
        || path.contains('\\')
        || value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("invalid portable relative path {path:?}"));
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::CharacterCreationData(message.into())
}
