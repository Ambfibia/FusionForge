use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct TextureRepair {
    pub(super) category: AvatarItemCategory,
    pub(super) item_number: u32,
    pub(super) donor_item_number: u32,
}

pub(super) const TEXTURE_REPAIRS: &[TextureRepair] = &[
    TextureRepair {
        category: AvatarItemCategory::Pants,
        item_number: 553,
        donor_item_number: 457,
    },
    TextureRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 644,
        donor_item_number: 666,
    },
    TextureRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 646,
        donor_item_number: 666,
    },
    TextureRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 647,
        donor_item_number: 67,
    },
    TextureRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 649,
        donor_item_number: 69,
    },
    TextureRepair {
        category: AvatarItemCategory::Shoes,
        item_number: 557,
        donor_item_number: 417,
    },
];

pub(super) fn apply_texture_repair(
    avatar: &mut CharacterCreationAvatarItems,
    repair: TextureRepair,
    evidence: &mut Vec<Value>,
) -> Result<(), String> {
    for gender in RepairGender::ALL {
        let donor = visual(avatar, repair.category, repair.donor_item_number, gender)?.clone();
        let donor_texture = donor.primary_texture.clone().ok_or_else(|| {
            format!(
                "texture donor {:?}/{} {} has no primary",
                repair.category,
                repair.donor_item_number,
                gender.label()
            )
        })?;
        require_verified_texture(&donor_texture, "texture donor")?;
        let target_name = item(avatar, repair.category, repair.item_number)?
            .name
            .clone();
        let donor_name = item(avatar, repair.category, repair.donor_item_number)?
            .name
            .clone();
        let target = visual_mut(avatar, repair.category, repair.item_number, gender)?;
        let original = target.primary_texture.clone().ok_or_else(|| {
            format!(
                "texture target {:?}/{} {} has no requested primary",
                repair.category,
                repair.item_number,
                gender.label()
            )
        })?;
        let is_missing =
            original.status == NativeLookupStatus::Missing && original.candidates.is_empty();
        let is_installed =
            original.status == NativeLookupStatus::VerifiedUnique && original.candidates.len() == 1;
        if !is_missing && !is_installed {
            return Err(format!(
                "texture target {:?}/{} {} is neither missing nor an installed donor repair",
                repair.category,
                repair.item_number,
                gender.label()
            ));
        }
        target.primary_texture = Some(donor_texture.clone());
        evidence.push(json!({
            "kind": "compatible-texture-donor",
            "category": repair.category,
            "itemNumber": repair.item_number,
            "itemName": target_name,
            "gender": gender,
            "requestedTextureTrueName": original.true_name,
            "donorItemNumber": repair.donor_item_number,
            "donorItemName": donor_name,
            "donorTexture": donor_texture
        }));
    }
    Ok(())
}

pub(super) fn avatar_texture_index(
    avatar: &CharacterCreationAvatarItems,
) -> Result<BTreeMap<String, AvatarTextureReference>, String> {
    let requested = WHITE_SLOT_REPAIRS
        .iter()
        .map(|repair| repair.donor_texture_true_name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut index = BTreeMap::new();
    for item in &avatar.items {
        for visual in [&item.male, &item.female] {
            for texture in [&visual.primary_texture, &visual.secondary_texture]
                .into_iter()
                .flatten()
            {
                if texture.status != NativeLookupStatus::VerifiedUnique
                    || texture.candidates.len() != 1
                {
                    continue;
                }
                let key = texture.true_name.to_ascii_lowercase();
                if !requested.contains(&key) {
                    continue;
                }
                if let Some(previous) = index.insert(key.clone(), texture.clone())
                    && previous != *texture
                {
                    return Err(format!(
                        "verified avatar texture {key:?} has contradictory references"
                    ));
                }
            }
        }
    }
    Ok(index)
}

pub(super) fn verify_published_texture(
    asset_root: &Path,
    published: &BTreeSet<&str>,
    texture: Option<&AvatarTextureReference>,
) -> Result<(), String> {
    let texture = texture.ok_or_else(|| "repaired visual has no primary texture".to_owned())?;
    require_verified_texture(texture, "repaired texture")?;
    let reference = &texture.candidates[0];
    if !published.contains(reference.path.as_str()) {
        return Err(format!(
            "repaired texture {:?} is not runtime-published",
            texture.true_name
        ));
    }
    verify_asset(asset_root, reference)
}

pub(super) fn require_verified_texture(texture: &AvatarTextureReference, label: &str) -> Result<(), String> {
    if texture.status != NativeLookupStatus::VerifiedUnique || texture.candidates.len() != 1 {
        return Err(format!(
            "{label} {:?} is {:?} with {} candidates",
            texture.true_name,
            texture.status,
            texture.candidates.len()
        ));
    }
    Ok(())
}
