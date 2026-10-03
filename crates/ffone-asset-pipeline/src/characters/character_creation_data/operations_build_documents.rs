use super::*;

pub(super) fn build_documents(
    asset_root: &Path,
    tables: &Map<String, Value>,
    provenance: CharacterCreationProvenance,
    manifest: &[ProjectAssetFile],
    manifest_index: &BTreeMap<String, ProjectAssetFile>,
    equipment_models: &[PlayerEquipmentCatalogModel],
) -> Result<CatalogDocuments> {
    let name_table = object(tables, "m_pNameTable")?;
    let first_names = name_entries(array(name_table, "m_pFirstName")?)?;
    let middle_names = name_entries(array(name_table, "m_pMiddleName")?)?;
    let last_names = name_entries(array(name_table, "m_pLastName")?)?;
    let name_wheel = CharacterCreationNameWheel {
        schema: CHARACTER_CREATION_NAME_WHEEL_SCHEMA.to_owned(),
        protocol: PROTOCOL_0104,
        provenance: provenance.clone(),
        first_names,
        middle_names,
        last_names,
    };

    let creation_table = object(tables, "m_pCreationItemTable")?;
    let raw_rows = array(creation_table, "m_pCreationItemData")?;
    let creation_rows = raw_rows
        .iter()
        .enumerate()
        .map(|(index, value)| creation_row(index, value))
        .collect::<Result<Vec<_>>>()?;
    let maxima = maxima_from_row(
        creation_rows
            .get(1)
            .ok_or_else(|| invalid_error("creation table has no maxima row at index 1"))?,
    )?;

    let texture_index = texture_index(manifest);
    let model_index = equipment_model_index(equipment_models, manifest)?;
    let mut all_items = Vec::new();
    for category in AVATAR_ITEM_CATEGORIES {
        all_items.extend(build_item_category(
            tables,
            category,
            &texture_index,
            &model_index,
        )?);
    }
    all_items.sort_by(|left, right| {
        left.category
            .cmp(&right.category)
            .then_with(|| left.item_number.cmp(&right.item_number))
    });
    repair_exact_equipment_texture_routes(&mut all_items)?;
    let item_lookup = all_items
        .iter()
        .map(|item| ((item.category, item.item_number), item))
        .collect::<BTreeMap<_, _>>();
    let choices = build_choices(&creation_rows, &maxima, &item_lookup)?;
    let appearance = CharacterCreationAppearance {
        schema: CHARACTER_CREATION_APPEARANCE_SCHEMA.to_owned(),
        protocol: PROTOCOL_0104,
        provenance: provenance.clone(),
        constraints: CharacterAppearanceConstraints {
            gender_codes: vec![1, 2],
            body_codes: (0..=2).collect(),
            height_codes: (0..=4).collect(),
            skin_color_codes: (1..=12).collect(),
            hair_color_codes: (1..=18).collect(),
            eye_color_codes: (1..=5).collect(),
        },
        color_contract: exact_color_contract(),
        texture_rules: CharacterTextureRules {
            face_eye_suffix_by_code: ["a", "b", "c", "d", "e"]
                .into_iter()
                .enumerate()
                .map(|(index, suffix)| CharacterTextureSuffix {
                    code: index as u8 + 1,
                    suffix: suffix.to_owned(),
                })
                .collect(),
            hair_eye_suffix: "a".to_owned(),
            male_skin_texture: required_texture_reference("m_skin", &texture_index)?,
            female_skin_texture: required_texture_reference("f_skin", &texture_index)?,
        },
        maxima,
        creation_rows,
        choices,
    };

    let counts = avatar_item_counts(&all_items);
    let avatar_items = CharacterCreationAvatarItems {
        schema: CHARACTER_CREATION_AVATAR_ITEMS_SCHEMA.to_owned(),
        protocol: PROTOCOL_0104,
        provenance: provenance.clone(),
        lookup_complete: counts.model_references == counts.resolved_models
            && counts.texture_references == counts.resolved_textures
            && counts.icon_references == counts.resolved_icons,
        counts,
        items: all_items,
    };
    let runtime_textures = build_runtime_texture_contracts(
        asset_root,
        provenance.clone(),
        manifest_index,
        &texture_index,
        &appearance,
        &avatar_items,
    )?;

    Ok(CatalogDocuments {
        name_wheel,
        appearance,
        avatar_items,
        runtime_textures,
    })
}

pub(super) fn avatar_item_counts(items: &[AvatarItemLookup]) -> CharacterCreationAvatarItemCounts {
    let mut counts = CharacterCreationAvatarItemCounts {
        categories: AVATAR_ITEM_CATEGORIES.len() as u64,
        items: items.len() as u64,
        ..Default::default()
    };
    for item in items {
        if let Some(icon) = &item.icon {
            counts.icon_references += 1;
            if icon.status == NativeLookupStatus::VerifiedUnique {
                counts.resolved_icons += 1;
            }
        }
        for visual in [&item.male, &item.female] {
            if visual.source_model_true_name.is_some() {
                counts.model_references += 1;
            }
            if matches!(
                visual.model_status,
                NativeLookupStatus::VerifiedUnique | NativeLookupStatus::VerifiedVariants
            ) {
                counts.resolved_models += 1;
            }
            for texture in [&visual.primary_texture, &visual.secondary_texture] {
                if let Some(texture) = texture {
                    counts.texture_references += 1;
                    if texture.status == NativeLookupStatus::VerifiedUnique {
                        counts.resolved_textures += 1;
                    }
                }
            }
        }
    }
    counts
}

pub(super) fn valid_equipment_true_name_resolution(
    true_name: &str,
    source: &RuntimeTextureSourceMetadata,
    repair: Option<&EquipmentTextureTrueNameRepair>,
    exact_route: &str,
) -> bool {
    if true_name.eq_ignore_ascii_case(&source.true_name) {
        return repair.is_none();
    }
    repair.is_some_and(|repair| {
        repair.requested_true_name.eq_ignore_ascii_case(true_name)
            && repair
                .serialized_true_name
                .eq_ignore_ascii_case(&source.true_name)
            && repair
                .requested_container_route
                .eq_ignore_ascii_case(exact_route)
            && !repair.reason.trim().is_empty()
    })
}

pub(super) fn build_item_category(
    tables: &Map<String, Value>,
    category: AvatarItemCategory,
    textures: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
    models: &BTreeMap<(String, String), AvatarModelReference>,
) -> Result<Vec<AvatarItemLookup>> {
    let table_value = object(tables, category.table_name())?;
    let table = ItemTable {
        item_data: array(table_value, "m_pItemData")?,
        icon_data: array(table_value, "m_pItemIconData")?,
        mesh_data: array(table_value, "m_pItemMeshData")?,
        string_data: array(table_value, "m_pItemStringData")?,
    };
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for (row_index, value) in table.item_data.iter().enumerate() {
        let item = value.as_object().ok_or_else(|| {
            invalid_error(format!(
                "{} item row is not an object",
                category.table_name()
            ))
        })?;
        let serialized_item_number = uint(item, "m_iItemNumber")? as u32;
        let item_number = if matches!(
            category,
            AvatarItemCategory::Face | AvatarItemCategory::Head
        ) {
            row_index as u32
        } else {
            serialized_item_number
        };
        if item_number == 0 {
            continue;
        }
        if !seen.insert(item_number) {
            return invalid(format!(
                "{} contains duplicate item number {item_number}",
                category.table_name()
            ));
        }
        let string_index = uint(item, "m_iItemName")? as usize;
        let item_string = table
            .string_data
            .get(string_index)
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid_error(format!(
                    "{} item {item_number} has invalid string index {string_index}",
                    category.table_name()
                ))
            })?;
        let mesh_index = uint(item, "m_iMesh")? as usize;
        let mesh = table
            .mesh_data
            .get(mesh_index)
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid_error(format!(
                    "{} item {item_number} has invalid mesh index {mesh_index}",
                    category.table_name()
                ))
            })?;
        let icon = icon_reference(item, &table, textures)?;
        let male = visual_reference(
            category,
            string(mesh, "m_pstrMMeshModelString")?,
            string(mesh, "m_pstrMTextureString")?,
            string(mesh, "m_pstrMTextureString2")?,
            textures,
            models,
        )?;
        let female = visual_reference(
            category,
            string(mesh, "m_pstrFMeshModelString")?,
            string(mesh, "m_pstrFTextureString")?,
            string(mesh, "m_pstrFTextureString2")?,
            textures,
            models,
        )?;
        items.push(AvatarItemLookup {
            category,
            item_number,
            level: uint(item, "m_iMinReqLev")? as u16,
            required_gender: uint(item, "m_iReqSex")? as u8,
            equip_type: u8::try_from(uint(item, "m_iEquipType")?).map_err(|_| {
                invalid_error(format!(
                    "{} item {item_number} has an out-of-range equip type",
                    category.table_name()
                ))
            })?,
            name: string(item_string, "m_strName")?.to_owned(),
            description: string(item_string, "m_strComment")?.to_owned(),
            icon,
            male,
            female,
        });
    }
    Ok(items)
}

pub(super) fn icon_reference(
    item: &Map<String, Value>,
    table: &ItemTable<'_>,
    textures: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
) -> Result<Option<AvatarIconReference>> {
    let icon_index = uint(item, "m_iIcon")? as usize;
    let Some(icon) = table.icon_data.get(icon_index).and_then(Value::as_object) else {
        return Ok(None);
    };
    let icon_type = uint(icon, "m_iIconType")? as u8;
    let icon_number = uint(icon, "m_iIconNumber")? as u32;
    if icon_type == 0 && icon_number == 0 {
        return Ok(None);
    }
    let prefix = match icon_type {
        0 => "wpnicon",
        1 => "nanoicon",
        2 => "skillicon",
        3 => "cosicon",
        4 => "npcicon",
        5 => "nanoready",
        6 => "questitemicon",
        7 => "generalitemicon",
        8 => "mobicon",
        9 => "fusionicon",
        10 => "hnpcicon",
        11 => "transport",
        12 => "vehicle",
        other => return invalid(format!("unsupported legacy icon type {other}")),
    };
    let true_name = format!("{prefix}_{icon_number:02}");
    let candidates = texture_candidates(textures, &true_name);
    let status = candidate_status(&candidates);
    Ok(Some(AvatarIconReference {
        icon_type,
        icon_number,
        true_name,
        status,
        candidates,
    }))
}

pub(super) fn visual_reference(
    category: AvatarItemCategory,
    model_name: &str,
    primary_texture: &str,
    secondary_texture: &str,
    textures: &BTreeMap<String, Vec<CharacterCreationAssetReference>>,
    models: &BTreeMap<(String, String), AvatarModelReference>,
) -> Result<AvatarItemVisual> {
    let model_true_name = true_name(model_name);
    let (model_status, model_variants) = if model_true_name.is_empty() {
        (NativeLookupStatus::Missing, Vec::new())
    } else {
        let resolved_true_name =
            model_alias(category, &model_true_name).unwrap_or(model_true_name.as_str());
        let category_name = category.equipment_category().to_owned();
        let mut variants = Vec::new();
        if let Some(model) = models.get(&(
            category_name.clone(),
            resolved_true_name.to_ascii_lowercase(),
        )) {
            variants.push(model.clone());
        } else if matches!(
            category,
            AvatarItemCategory::Head | AvatarItemCategory::Face
        ) {
            for suffix in ["type01", "type02"] {
                if let Some(model) = models.get(&(
                    category_name.clone(),
                    format!("{resolved_true_name}_{suffix}").to_ascii_lowercase(),
                )) {
                    variants.push(model.clone());
                }
            }
        }
        let status = match variants.len() {
            0 => NativeLookupStatus::Missing,
            1 => NativeLookupStatus::VerifiedUnique,
            _ => NativeLookupStatus::VerifiedVariants,
        };
        (status, variants)
    };
    let primary_texture = texture_reference(primary_texture, textures)?;
    let secondary_texture = texture_reference(secondary_texture, textures)?;
    Ok(AvatarItemVisual {
        source_model_true_name: (!model_true_name.is_empty()).then_some(model_true_name),
        model_status,
        models: model_variants,
        primary_texture,
        secondary_texture,
    })
}

pub(super) fn exact_color_contract() -> CharacterColorContract {
    let skin = [
        [0.2265625, 0.15234375, 0.09765625, 1.0],
        [0.35546875, 0.29296875, 0.16015625, 1.0],
        [0.39453125, 0.30859375, 0.2265625, 1.0],
        [0.59375, 0.40625, 0.328125, 1.0],
        [0.54296875, 0.46484375, 0.3828125, 1.0],
        [0.59375, 0.5078125, 0.32421875, 1.0],
        [0.71875, 0.5078125, 0.40625, 1.0],
        [0.7109375, 0.578125, 0.4296875, 1.0],
        [0.8984375, 0.73046875, 0.5546875, 1.0],
        [0.91015625, 0.8828125, 0.671875, 1.0],
        [0.97265625, 0.83984375, 0.80078125, 1.0],
        [0.828125, 0.859375, 0.87890625, 1.0],
    ];
    let hair = [
        [0.99609375, 0.99609375, 0.99609375, 1.0],
        [0.54296875, 0.56640625, 0.61328125, 1.0],
        [0.055, 0.055, 0.055, 1.0],
        [0.859375, 0.57421875, 0.609375, 1.0],
        [0.578125, 0.2890625, 0.19140625, 1.0],
        [0.84375, 0.2421875, 0.0390625, 1.0],
        [0.73046875, 0.78125, 0.90625, 1.0],
        [0.5390625, 0.078125, 0.625, 1.0],
        [0.2578125, 0.0390625, 0.62109375, 1.0],
        [0.43359375, 0.80859375, 0.703125, 1.0],
        [0.05859375, 0.8203125, 0.0390625, 1.0],
        [0.1015625, 0.48828125, 0.16796875, 1.0],
        [0.9375, 0.9140625, 0.671875, 1.0],
        [0.9375, 0.84765625, 0.41015625, 1.0],
        [0.04296875, 0.60546875, 0.96484375, 1.0],
        [0.9453125, 0.5234375, 0.03125, 1.0],
        [0.48828125, 0.31640625, 0.265625, 1.0],
        [0.3125, 0.18359375, 0.1640625, 1.0],
    ];
    CharacterColorContract {
        source_asset: "sharedassets0.assets/male.bsd".to_owned(),
        source_path_id: 1378,
        serialized_color_space: "unity_serialized_raw_rgba_f32".to_owned(),
        runtime_uniform_policy:
            "preserve_raw_components_as_bevy_linear_rgba_without_srgb_conversion".to_owned(),
        actor_skin_tint_multiplier: 0.5,
        skin: skin
            .into_iter()
            .enumerate()
            .map(|(index, rgba)| CharacterPaletteColor {
                code: index as u8 + 1,
                rgba,
            })
            .collect(),
        hair: hair
            .into_iter()
            .enumerate()
            .map(|(index, rgba)| CharacterPaletteColor {
                code: index as u8 + 1,
                rgba,
            })
            .collect(),
    }
}

pub(super) fn build_choices(
    rows: &[CharacterCreationRow],
    maxima: &CharacterCreationMaxima,
    items: &BTreeMap<(AvatarItemCategory, u32), &AvatarItemLookup>,
) -> Result<Vec<CharacterCreationChoice>> {
    let mut choices = Vec::new();
    for (gender, category, maximum) in [
        (
            CharacterGender::Male,
            CharacterAppearanceCategory::Face,
            maxima.male_face,
        ),
        (
            CharacterGender::Female,
            CharacterAppearanceCategory::Face,
            maxima.female_face,
        ),
        (
            CharacterGender::Male,
            CharacterAppearanceCategory::Hair,
            maxima.male_hair,
        ),
        (
            CharacterGender::Female,
            CharacterAppearanceCategory::Hair,
            maxima.female_hair,
        ),
        (
            CharacterGender::Male,
            CharacterAppearanceCategory::Shirt,
            maxima.male_shirts,
        ),
        (
            CharacterGender::Female,
            CharacterAppearanceCategory::Shirt,
            maxima.female_shirts,
        ),
        (
            CharacterGender::Male,
            CharacterAppearanceCategory::Pants,
            maxima.male_pants,
        ),
        (
            CharacterGender::Female,
            CharacterAppearanceCategory::Pants,
            maxima.female_pants,
        ),
        (
            CharacterGender::Male,
            CharacterAppearanceCategory::Shoes,
            maxima.male_shoes,
        ),
        (
            CharacterGender::Female,
            CharacterAppearanceCategory::Shoes,
            maxima.female_shoes,
        ),
    ] {
        for creation_index in 2..maximum + 2 {
            let row = rows.get(creation_index as usize).ok_or_else(|| {
                invalid_error(format!(
                    "{gender:?}/{category:?} creation index {creation_index} is absent"
                ))
            })?;
            let value = choice_value(row, gender, category);
            if value == 0 {
                return invalid(format!(
                    "{gender:?}/{category:?} creation index {creation_index} maps to zero"
                ));
            }
            let item_category = match category {
                CharacterAppearanceCategory::Face => AvatarItemCategory::Face,
                CharacterAppearanceCategory::Hair => AvatarItemCategory::Head,
                CharacterAppearanceCategory::Shirt => AvatarItemCategory::Shirt,
                CharacterAppearanceCategory::Pants => AvatarItemCategory::Pants,
                CharacterAppearanceCategory::Shoes => AvatarItemCategory::Shoes,
            };
            let item = items.get(&(item_category, value)).ok_or_else(|| {
                invalid_error(format!(
                    "creation choice {gender:?}/{category:?}/{creation_index} references absent item {value}"
                ))
            })?;
            choices.push(CharacterCreationChoice {
                category,
                gender,
                creation_index,
                value,
                label: item.name.clone(),
                icon: item.icon.clone(),
            });
        }
    }
    Ok(choices)
}

pub(super) fn choice_value(
    row: &CharacterCreationRow,
    gender: CharacterGender,
    category: CharacterAppearanceCategory,
) -> u32 {
    match (gender, category) {
        (CharacterGender::Male, CharacterAppearanceCategory::Face) => row.male_face,
        (CharacterGender::Female, CharacterAppearanceCategory::Face) => row.female_face,
        (CharacterGender::Male, CharacterAppearanceCategory::Hair) => row.male_hair,
        (CharacterGender::Female, CharacterAppearanceCategory::Hair) => row.female_hair,
        (CharacterGender::Male, CharacterAppearanceCategory::Shirt) => row.male_shirt,
        (CharacterGender::Female, CharacterAppearanceCategory::Shirt) => row.female_shirt,
        (CharacterGender::Male, CharacterAppearanceCategory::Pants) => row.male_pants,
        (CharacterGender::Female, CharacterAppearanceCategory::Pants) => row.female_pants,
        (CharacterGender::Male, CharacterAppearanceCategory::Shoes) => row.male_shoes,
        (CharacterGender::Female, CharacterAppearanceCategory::Shoes) => row.female_shoes,
    }
}

pub(super) fn creation_row(index: usize, value: &Value) -> Result<CharacterCreationRow> {
    let row = value
        .as_object()
        .ok_or_else(|| invalid_error(format!("creation row {index} is not an object")))?;
    Ok(CharacterCreationRow {
        creation_index: index as u16,
        male_face: uint(row, "m_iFaceM")? as u32,
        female_face: uint(row, "m_iFaceF")? as u32,
        male_hair: uint(row, "m_iHairM")? as u32,
        female_hair: uint(row, "m_iHairF")? as u32,
        male_shirt: uint(row, "m_iShirtM")? as u32,
        female_shirt: uint(row, "m_iShirtF")? as u32,
        male_pants: uint(row, "m_iPantsM")? as u32,
        female_pants: uint(row, "m_iPantsF")? as u32,
        male_shoes: uint(row, "m_iShoesM")? as u32,
        female_shoes: uint(row, "m_iShoesF")? as u32,
        weapon: uint(row, "m_iWeapon")? as u32,
    })
}

pub(super) fn maxima_from_row(row: &CharacterCreationRow) -> Result<CharacterCreationMaxima> {
    let maxima = CharacterCreationMaxima {
        male_face: row.male_face as u16,
        female_face: row.female_face as u16,
        male_hair: row.male_hair as u16,
        female_hair: row.female_hair as u16,
        male_shirts: row.male_shirt as u16,
        female_shirts: row.female_shirt as u16,
        male_pants: row.male_pants as u16,
        female_pants: row.female_pants as u16,
        male_shoes: row.male_shoes as u16,
        female_shoes: row.female_shoes as u16,
    };
    if maxima
        != (CharacterCreationMaxima {
            male_face: 5,
            female_face: 5,
            male_hair: 23,
            female_hair: 21,
            male_shirts: 30,
            female_shirts: 30,
            male_pants: 30,
            female_pants: 30,
            male_shoes: 30,
            female_shoes: 27,
        })
    {
        return invalid(format!(
            "Retrobution 0104 creation maxima changed: {maxima:?}"
        ));
    }
    Ok(maxima)
}

pub(super) fn name_entries(values: &[Value]) -> Result<Vec<NameWheelEntry>> {
    values
        .iter()
        .enumerate()
        .map(|(code, value)| {
            let row = value
                .as_object()
                .ok_or_else(|| invalid_error(format!("name row {code} is not an object")))?;
            Ok(NameWheelEntry {
                code: code as u16,
                value: string(row, "m_pstrNameString")?.to_owned(),
            })
        })
        .collect()
}

pub(super) fn select_table_root(document: &Value) -> Result<&Map<String, Value>> {
    let root = document
        .as_object()
        .ok_or_else(|| invalid_error("native table JSON root is not an object"))?;
    if root.contains_key("m_pNameTable") && root.contains_key("m_pCreationItemTable") {
        return Ok(root);
    }
    if root.get("schema").and_then(Value::as_str) != Some(TABLE_SET_SCHEMA) {
        return invalid("native table JSON is neither xdt root nor ffone.table-set.v1");
    }
    let tables = root
        .get("tables")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_error("ffone.table-set.v1 has no tables array"))?;
    let matches = tables
        .iter()
        .filter_map(|entry| entry.get("value").and_then(Value::as_object))
        .filter(|value| {
            value.contains_key("m_pNameTable") && value.contains_key("m_pCreationItemTable")
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return invalid(format!(
            "expected one TableData value, found {}",
            matches.len()
        ));
    }
    Ok(matches[0])
}

pub(super) fn array<'a>(parent: &'a Map<String, Value>, key: &str) -> Result<&'a [Value]> {
    parent
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid_error(format!("missing array {key}")))
}

pub(super) fn uint(parent: &Map<String, Value>, key: &str) -> Result<u64> {
    parent
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_error(format!("missing unsigned integer {key}")))
}

pub(super) fn string<'a>(parent: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    parent
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error(format!("missing string {key}")))
}

pub(super) fn true_name(source: &str) -> String {
    let trimmed = source.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
        return String::new();
    }
    let normalized = trimmed.replace('\\', "/");
    let file = normalized.rsplit('/').next().unwrap_or(&normalized);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    stem.to_owned()
}

pub(super) fn native_reference(entry: &ProjectAssetFile) -> CharacterCreationAssetReference {
    CharacterCreationAssetReference {
        path: entry.path.clone(),
        bytes: entry.bytes,
        blake3: entry.blake3.clone(),
    }
}

pub(super) fn pretty_json<T: Serialize>(value: &T, path: &str) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: path.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn create_stage(asset_root: &Path) -> Result<PathBuf> {
    for _ in 0..128 {
        let sequence = INSTALL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = asset_root.join(format!(
            ".character-creation-data-stage-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_at(&path, error)),
        }
    }
    Err(PipelineError::StagingCollision(
        asset_root.join(CHARACTER_CREATION_ROOT),
    ))
}

pub(super) fn replace_character_creation_document(
    asset_root: &Path,
    relative_path: &str,
    transaction_name: &str,
    bytes: &[u8],
) -> Result<()> {
    validate_relative(relative_path)?;
    if transaction_name.is_empty()
        || !transaction_name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    {
        return invalid("unsafe character-creation transaction name");
    }
    let path = asset_root.join(relative_path);
    let next = asset_root.join(format!(".character-creation-{transaction_name}.next"));
    let backup = asset_root.join(format!(".character-creation-{transaction_name}.backup"));
    if fs::symlink_metadata(&next).is_ok() || fs::symlink_metadata(&backup).is_ok() {
        return invalid(format!(
            "stale character-creation {transaction_name} transaction files exist"
        ));
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)
        .map_err(|error| io_at(&next, error))?;
    output
        .write_all(bytes)
        .map_err(|error| io_at(&next, error))?;
    output.sync_all().map_err(|error| io_at(&next, error))?;
    fs::rename(&path, &backup).map_err(|error| io_at(&path, error))?;
    if let Err(error) = fs::rename(&next, &path) {
        let _ = fs::rename(&backup, &path);
        return Err(io_at(&path, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|error| io_at(&canonical, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a regular directory"));
    }
    Ok(canonical)
}

pub(super) fn canonical_file(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|error| io_at(&canonical, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a regular file"));
    }
    Ok(canonical)
}
