use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct ModelRepair {
    pub(super) category: AvatarItemCategory,
    pub(super) item_number: u32,
    pub(super) donor_item_number: u32,
}

pub(super) const MODEL_REPAIRS: &[ModelRepair] = &[
    ModelRepair {
        category: AvatarItemCategory::Back,
        item_number: 168,
        donor_item_number: 8,
    },
    ModelRepair {
        category: AvatarItemCategory::Glasses,
        item_number: 120,
        donor_item_number: 19,
    },
    ModelRepair {
        category: AvatarItemCategory::Hat,
        item_number: 202,
        donor_item_number: 251,
    },
    ModelRepair {
        category: AvatarItemCategory::Hat,
        item_number: 354,
        donor_item_number: 12,
    },
    ModelRepair {
        category: AvatarItemCategory::Hat,
        item_number: 355,
        donor_item_number: 318,
    },
    ModelRepair {
        category: AvatarItemCategory::Hat,
        item_number: 356,
        donor_item_number: 205,
    },
    ModelRepair {
        category: AvatarItemCategory::Hat,
        item_number: 357,
        donor_item_number: 302,
    },
    ModelRepair {
        category: AvatarItemCategory::Pants,
        item_number: 552,
        donor_item_number: 345,
    },
    ModelRepair {
        category: AvatarItemCategory::Pants,
        item_number: 555,
        donor_item_number: 525,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 539,
        donor_item_number: 133,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 639,
        donor_item_number: 526,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 640,
        donor_item_number: 570,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 641,
        donor_item_number: 369,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 642,
        donor_item_number: 472,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 645,
        donor_item_number: 563,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 648,
        donor_item_number: 631,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 650,
        donor_item_number: 632,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 652,
        donor_item_number: 547,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 655,
        donor_item_number: 548,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 656,
        donor_item_number: 633,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 657,
        donor_item_number: 385,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 662,
        donor_item_number: 634,
    },
    ModelRepair {
        category: AvatarItemCategory::Shirt,
        item_number: 663,
        donor_item_number: 386,
    },
    ModelRepair {
        category: AvatarItemCategory::Shoes,
        item_number: 555,
        donor_item_number: 351,
    },
    ModelRepair {
        category: AvatarItemCategory::Shoes,
        item_number: 558,
        donor_item_number: 122,
    },
];

pub(super) fn apply_model_repair(
    avatar: &mut CharacterCreationAvatarItems,
    repair: ModelRepair,
    evidence: &mut Vec<Value>,
) -> Result<(), String> {
    for gender in RepairGender::ALL {
        let donor = visual(avatar, repair.category, repair.donor_item_number, gender)?.clone();
        if donor.model_status != NativeLookupStatus::VerifiedUnique || donor.models.len() != 1 {
            return Err(format!(
                "donor {:?}/{} {} is not one verified model",
                repair.category,
                repair.donor_item_number,
                gender.label()
            ));
        }
        let target_name = item(avatar, repair.category, repair.item_number)?
            .name
            .clone();
        let donor_name = item(avatar, repair.category, repair.donor_item_number)?
            .name
            .clone();
        let target = visual_mut(avatar, repair.category, repair.item_number, gender)?;
        let original = target.clone();
        let requested_true_name = target.source_model_true_name.clone().ok_or_else(|| {
            format!(
                "target {:?}/{} {} has no requested model",
                repair.category,
                repair.item_number,
                gender.label()
            )
        })?;
        let requested_route = format!("wear/{requested_true_name}.nif");
        let is_missing =
            target.model_status == NativeLookupStatus::Missing && target.models.is_empty();
        let is_installed = target.model_status == NativeLookupStatus::VerifiedUnique
            && target.models.len() == 1
            && target.models[0].exact_route == requested_route;
        if !is_missing && !is_installed {
            return Err(format!(
                "target {:?}/{} {} is neither missing nor an installed donor repair",
                repair.category,
                repair.item_number,
                gender.label()
            ));
        }
        let donor_model = &donor.models[0];
        target.model_status = NativeLookupStatus::VerifiedUnique;
        target.models = vec![ffone_runtime_contracts::AvatarModelReference {
            true_name: donor_model.true_name.clone(),
            exact_route: requested_route.clone(),
            native_asset: donor_model.native_asset.clone(),
        }];
        let donor_primary = donor.primary_texture.clone().ok_or_else(|| {
            format!(
                "donor {:?}/{} {} has no primary texture",
                repair.category,
                repair.donor_item_number,
                gender.label()
            )
        })?;
        require_verified_texture(&donor_primary, "model donor primary")?;
        target.primary_texture = Some(donor_primary.clone());
        if original.secondary_texture.is_some() {
            let replacement = donor
                .secondary_texture
                .clone()
                .unwrap_or_else(|| donor_primary.clone());
            require_verified_texture(&replacement, "model donor secondary")?;
            target.secondary_texture = Some(replacement);
        }
        evidence.push(json!({
            "kind": "model-and-compatible-texture-donor",
            "category": repair.category,
            "itemNumber": repair.item_number,
            "itemName": target_name,
            "gender": gender,
            "requestedModelTrueName": requested_true_name,
            "requestedExactRoute": requested_route,
            "requestedPrimaryTexture": original.primary_texture.as_ref().map(|texture| texture.true_name.as_str()),
            "requestedSecondaryTexture": original.secondary_texture.as_ref().map(|texture| texture.true_name.as_str()),
            "donorItemNumber": repair.donor_item_number,
            "donorItemName": donor_name,
            "donorModelTrueName": donor_model.true_name,
            "donorModelExactRoute": donor_model.exact_route,
            "donorModelNativeAsset": donor_model.native_asset,
            "donorPrimaryTexture": donor_primary
        }));
    }
    Ok(())
}
