use super::*;

pub(super) fn resolve_creator_models(
    appearance: &CharacterCreationAppearance,
    avatar_items: &CharacterCreationAvatarItems,
    creator_models: &[CreatorModelCandidate],
    spec: GenderSpec,
) -> Result<(
    Vec<PlayerRigCreatorChoiceContract>,
    Vec<CreatorModelCandidate>,
)> {
    let character_gender = match spec.gender {
        PlayerRigGender::Male => CharacterGender::Male,
        PlayerRigGender::Female => CharacterGender::Female,
    };
    let source_choices = appearance
        .choices
        .iter()
        .filter(|choice| choice.gender == character_gender)
        .collect::<Vec<_>>();
    if source_choices.len() != spec.expected_creator_choices {
        return rig_error(format!(
            "{:?} creator choice count drifted: expected {}, found {}",
            spec.gender,
            spec.expected_creator_choices,
            source_choices.len()
        ));
    }

    let mut choices = Vec::with_capacity(source_choices.len());
    let mut unique_models = BTreeMap::<String, CreatorModelCandidate>::new();
    for choice in source_choices {
        let item_category = match choice.category {
            CharacterAppearanceCategory::Face => AvatarItemCategory::Face,
            CharacterAppearanceCategory::Hair => AvatarItemCategory::Head,
            CharacterAppearanceCategory::Shirt => AvatarItemCategory::Shirt,
            CharacterAppearanceCategory::Pants => AvatarItemCategory::Pants,
            CharacterAppearanceCategory::Shoes => AvatarItemCategory::Shoes,
        };
        let item_matches = avatar_items
            .items
            .iter()
            .filter(|item| item.category == item_category && item.item_number == choice.value)
            .collect::<Vec<_>>();
        let [item] = item_matches.as_slice() else {
            return rig_error(format!(
                "{:?}/{:?}/{} resolves {} avatar items",
                spec.gender,
                choice.category,
                choice.value,
                item_matches.len()
            ));
        };
        let visual = match spec.gender {
            PlayerRigGender::Male => &item.male,
            PlayerRigGender::Female => &item.female,
        };

        let candidate = if matches!(
            visual.model_status,
            NativeLookupStatus::VerifiedUnique | NativeLookupStatus::VerifiedVariants
        ) {
            let selected = visual
                .models
                .iter()
                .find(|model| model.true_name.ends_with("_type01"))
                .or_else(|| visual.models.first())
                .ok_or_else(|| {
                    rig_message(format!(
                        "{:?}/{:?}/{} has verified status but no model reference",
                        spec.gender, choice.category, choice.value
                    ))
                })?;
            let matches = creator_models
                .iter()
                .filter(|model| {
                    model.gender.is_none_or(|gender| gender == spec.gender)
                        && model.exact_route == selected.exact_route
                        && model.glb == selected.native_asset.path
                        && model.glb_blake3 == selected.native_asset.blake3
                })
                .collect::<Vec<_>>();
            let [candidate] = matches.as_slice() else {
                return rig_error(format!(
                    "{:?}/{:?}/{} native model {:?} resolves {} published candidates",
                    spec.gender,
                    choice.category,
                    choice.value,
                    selected.exact_route,
                    matches.len()
                ));
            };
            (*candidate).clone()
        } else if visual.model_status == NativeLookupStatus::Missing {
            let source_true_name = visual.source_model_true_name.as_deref().ok_or_else(|| {
                rig_message(format!(
                    "{:?}/{:?}/{} has no source model true name",
                    spec.gender, choice.category, choice.value
                ))
            })?;
            let matches = creator_models
                .iter()
                .filter(|model| {
                    model.gender == Some(spec.gender) && model.true_name == source_true_name
                })
                .collect::<Vec<_>>();
            let [candidate] = matches.as_slice() else {
                return rig_error(format!(
                    "{:?}/{:?}/{} missing source model {source_true_name:?} resolves {} exact supplemental publications",
                    spec.gender,
                    choice.category,
                    choice.value,
                    matches.len()
                ));
            };
            (*candidate).clone()
        } else {
            return rig_error(format!(
                "{:?}/{:?}/{} has unsupported model status {:?}",
                spec.gender, choice.category, choice.value, visual.model_status
            ));
        };

        if let Some(previous) =
            unique_models.insert(candidate.exact_route.clone(), candidate.clone())
            && (previous.glb != candidate.glb
                || previous.glb_blake3 != candidate.glb_blake3
                || previous.true_name != candidate.true_name)
        {
            return rig_error(format!(
                "{:?} creator route {:?} resolves inconsistent native models",
                spec.gender, candidate.exact_route
            ));
        }
        choices.push(PlayerRigCreatorChoiceContract {
            appearance_category: choice.category,
            creation_index: choice.creation_index,
            item_number: choice.value,
            exact_route: candidate.exact_route.clone(),
            glb: candidate.glb.clone(),
        });
    }
    if unique_models.len() != spec.expected_unique_creator_parts {
        return rig_error(format!(
            "{:?} unique creator part count drifted: expected {}, found {}",
            spec.gender,
            spec.expected_unique_creator_parts,
            unique_models.len()
        ));
    }
    Ok((choices, unique_models.into_values().collect()))
}

pub(super) fn collect_transforms(catalog: &DumpCatalog, transform: i64, output: &mut Vec<i64>) -> Result<()> {
    output.push(transform);
    let body = catalog.value(transform)?;
    for child in optional_array(&body, "m_Children") {
        let child = pointer_path_id(child)?;
        collect_transforms(catalog, child, output)?;
    }
    Ok(())
}

pub(super) fn read_packed_bits(value: &Value) -> Result<Vec<u32>> {
    let count = usize_number(
        value
            .get("m_NumItems")
            .ok_or_else(|| rig_message("packed bit count is absent"))?,
        "packed bit count",
    )?;
    let bit_size = u32::try_from(usize_number(
        value
            .get("m_BitSize")
            .ok_or_else(|| rig_message("packed bit size is absent"))?,
        "packed bit size",
    )?)
    .map_err(|_| rig_message("packed bit size exceeds u32"))?;
    if count == 0 || bit_size == 0 || bit_size > 32 {
        return rig_error("packed bit vector has invalid count/bit size");
    }
    let data = byte_payload(
        value
            .get("m_Data")
            .ok_or_else(|| rig_message("packed bit data is absent"))?,
    )?;
    let mut reader = BitReader::new(&data, bit_size);
    Ok((0..count).map(|_| reader.read()).collect())
}

pub(super) fn read_packed_floats(value: &Value) -> Result<Vec<f64>> {
    let bit_size = u32::try_from(usize_number(
        value
            .get("m_BitSize")
            .ok_or_else(|| rig_message("packed float bit size is absent"))?,
        "packed float bit size",
    )?)
    .map_err(|_| rig_message("packed float bit size exceeds u32"))?;
    let values = read_packed_bits(value)?;
    let max = if bit_size == 32 {
        u32::MAX as f64
    } else {
        ((1_u64 << bit_size) - 1) as f64
    };
    let range = finite(value.get("m_Range"), "packed float range")? / max.max(1.0);
    let start = finite(value.get("m_Start"), "packed float start")?;
    Ok(values
        .into_iter()
        .map(|value| f64::from(value) * range + start)
        .collect())
}
