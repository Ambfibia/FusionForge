use super::*;

pub(in super::super) fn run(command_args: &[String]) -> Result<(), String> {
    let args = command_args.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    let [project_root] = args.as_slice() else {
        return Err("usage: repair_avatar_equipment_audit_gaps <PROJECT_ROOT>".to_owned());
    };
    let project_root = canonical_directory(Path::new(project_root), "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let avatar_path = canonical_file(&asset_root.join(AVATAR_ITEMS_PATH), "avatar items")?;
    let runtime_path = canonical_file(&asset_root.join(RUNTIME_TEXTURES_PATH), "runtime textures")?;
    let evidence_path = project_root.join(EVIDENCE_PATH);
    let prior_evidence = if evidence_path.is_file() {
        Some(read_value(&evidence_path)?)
    } else {
        None
    };

    let original_avatar = fs::read(&avatar_path)
        .map_err(|error| format!("cannot read {}: {error}", avatar_path.display()))?;
    let original_runtime = fs::read(&runtime_path)
        .map_err(|error| format!("cannot read {}: {error}", runtime_path.display()))?;
    let mut avatar: CharacterCreationAvatarItems = serde_json::from_slice(&original_avatar)
        .map_err(|error| format!("invalid {}: {error}", avatar_path.display()))?;
    let mut runtime: CharacterCreationRuntimeTextures =
        serde_json::from_slice(&original_runtime)
            .map_err(|error| format!("invalid {}: {error}", runtime_path.display()))?;

    let texture_index = avatar_texture_index(&avatar)?;
    let mut repairs = Vec::<Value>::new();
    for repair in MODEL_REPAIRS {
        apply_model_repair(&mut avatar, *repair, &mut repairs)?;
    }
    for repair in TEXTURE_REPAIRS {
        apply_texture_repair(&mut avatar, *repair, &mut repairs)?;
    }
    for repair in WHITE_SLOT_REPAIRS {
        let donor = texture_index
            .get(&repair.donor_texture_true_name.to_ascii_lowercase())
            .cloned()
            .ok_or_else(|| {
                format!(
                    "verified donor texture {:?} is absent",
                    repair.donor_texture_true_name
                )
            })?;
        apply_white_slot_repair(&mut avatar, *repair, &donor, &mut repairs)?;
    }

    avatar.counts = avatar_item_counts(&avatar.items);
    avatar.lookup_complete = avatar.counts.model_references == avatar.counts.resolved_models
        && avatar.counts.texture_references == avatar.counts.resolved_textures
        && avatar.counts.icon_references == avatar.counts.resolved_icons;
    refresh_runtime_coverage(&avatar, &mut runtime)?;
    verify_repaired_contracts(&asset_root, &avatar, &runtime)?;

    let avatar_bytes = pretty_json(&avatar)?;
    let runtime_bytes = pretty_json(&runtime)?;
    write_replace(&avatar_path, &avatar_bytes)?;
    if let Err(error) = write_replace(&runtime_path, &runtime_bytes) {
        let _ = write_replace(&avatar_path, &original_avatar);
        return Err(error);
    }

    let installed_check = (|| -> Result<(), String> {
        let installed_avatar: CharacterCreationAvatarItems = read_typed(&avatar_path)?;
        let installed_runtime: CharacterCreationRuntimeTextures = read_typed(&runtime_path)?;
        verify_repaired_contracts(&asset_root, &installed_avatar, &installed_runtime)
    })();
    if let Err(error) = installed_check {
        let _ = write_replace(&avatar_path, &original_avatar);
        let _ = write_replace(&runtime_path, &original_runtime);
        return Err(format!("installed repair failed verification: {error}"));
    }

    let mut evidence = json!({
        "schema": "ffone.avatar-equipment-native-donor-repair.v1",
        "status": "installed-native-donor-extension-awaiting-gpu-acceptance",
        "sourceAuthority": {
            "primary": "builds/retrobution-20260613",
            "patched": "builds/retrobution-20260613.ffclient",
            "alternate": "builds/6543a2bb-d154-4087-b9ee-3c8aa778580a",
            "exactMissingNameSearch": {
                "queries": 45,
                "exactMatches": 0,
                "note": "The only substring match was the distinct primary terracottamountainarmor family; no missing requested object/container identity was found."
            }
        },
        "primaryParity": false,
        "intentionalDivergence": "The patched item rows reference model and texture names absent from primary, patched, and alternate source inventories. Each missing route is retained as the requested runtime route, but a byte-verified native donor from the same equipment category is used explicitly. No donor is represented as recovered primary ownership.",
        "policy": {
            "model": "same-category, same-gender donor; requested exactRoute retained; donor trueName/nativeAsset recorded",
            "texture": "published runtime texture donor with exact native byte/hash contract",
            "whiteSlots": "existing source-authored or same-model runtime texture; no generated bitmap"
        },
        "conversionCommand": "cargo run -p ffone-asset-pipeline --bin repair_avatar_equipment_audit_gaps -- <project-root>",
        "conversionVersion": "ffone.avatar-equipment-native-donor-repair.v1",
        "input": {
            "avatarItems": artifact_identity(AVATAR_ITEMS_PATH, &original_avatar),
            "runtimeTextures": artifact_identity(RUNTIME_TEXTURES_PATH, &original_runtime)
        },
        "output": {
            "avatarItems": artifact_identity(AVATAR_ITEMS_PATH, &avatar_bytes),
            "runtimeTextures": artifact_identity(RUNTIME_TEXTURES_PATH, &runtime_bytes)
        },
        "counts": {
            "modelRepairItems": MODEL_REPAIRS.len(),
            "modelRepairVisuals": MODEL_REPAIRS.len() * 2,
            "textureRepairItems": TEXTURE_REPAIRS.len(),
            "textureRepairVisuals": TEXTURE_REPAIRS.len() * 2,
            "whiteSlotRepairItems": WHITE_SLOT_REPAIRS.len(),
            "whiteSlotRepairVisuals": WHITE_SLOT_REPAIRS.len() * 2,
            "records": repairs.len()
        },
        "postCounts": avatar.counts,
        "runtimeCoverage": runtime.coverage,
        "repairs": repairs
    });
    if let Some(gpu_acceptance) = verified_gpu_acceptance(&project_root)? {
        evidence["status"] =
            Value::String("installed-gpu-accepted-native-donor-extension".to_owned());
        evidence["gpuAcceptance"] = gpu_acceptance;
    }
    if let Some(prior) = prior_evidence.as_ref() {
        merge_prior_evidence(&mut evidence, prior)?;
    }
    write_replace(&evidence_path, &pretty_json(&evidence)?)?;
    println!(
        "Installed {} documented avatar equipment donor repairs; evidence={}",
        MODEL_REPAIRS.len() * 2 + TEXTURE_REPAIRS.len() * 2 + WHITE_SLOT_REPAIRS.len() * 2,
        evidence_path.display()
    );
    Ok(())
}

pub(super) fn avatar_item_counts(items: &[AvatarItemLookup]) -> CharacterCreationAvatarItemCounts {
    let mut counts = CharacterCreationAvatarItemCounts {
        categories: items
            .iter()
            .map(|item| item.category)
            .collect::<BTreeSet<_>>()
            .len() as u64,
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
            for texture in [&visual.primary_texture, &visual.secondary_texture]
                .into_iter()
                .flatten()
            {
                counts.texture_references += 1;
                if texture.status == NativeLookupStatus::VerifiedUnique {
                    counts.resolved_textures += 1;
                }
            }
        }
    }
    counts
}

pub(super) fn verify_repaired_contracts(
    asset_root: &Path,
    avatar: &CharacterCreationAvatarItems,
    runtime: &CharacterCreationRuntimeTextures,
) -> Result<(), String> {
    let published = runtime
        .textures
        .iter()
        .map(|texture| texture.native_asset.path.as_str())
        .collect::<BTreeSet<_>>();
    for repair in MODEL_REPAIRS {
        for gender in RepairGender::ALL {
            let visual = visual(avatar, repair.category, repair.item_number, gender)?;
            if visual.model_status != NativeLookupStatus::VerifiedUnique || visual.models.len() != 1
            {
                return Err(format!(
                    "repaired {:?}/{} {} model is not unique",
                    repair.category,
                    repair.item_number,
                    gender.label()
                ));
            }
            verify_asset(asset_root, &visual.models[0].native_asset)?;
            verify_published_texture(asset_root, &published, visual.primary_texture.as_ref())?;
            if visual.secondary_texture.is_some() {
                verify_published_texture(
                    asset_root,
                    &published,
                    visual.secondary_texture.as_ref(),
                )?;
            }
        }
    }
    for repair in TEXTURE_REPAIRS {
        for gender in RepairGender::ALL {
            let visual = visual(avatar, repair.category, repair.item_number, gender)?;
            verify_published_texture(asset_root, &published, visual.primary_texture.as_ref())?;
        }
    }
    for repair in WHITE_SLOT_REPAIRS {
        for gender in RepairGender::ALL {
            let visual = visual(avatar, repair.category, repair.item_number, gender)?;
            verify_published_texture(asset_root, &published, visual.primary_texture.as_ref())?;
        }
    }
    if runtime.coverage.avatar_deferred_verified_routes != 0
        || runtime.coverage.avatar_verified_unique_routes
            != runtime.coverage.avatar_published_routes
    {
        return Err("repaired avatar texture closure is not fully published".to_owned());
    }
    Ok(())
}

pub(super) fn item(
    avatar: &CharacterCreationAvatarItems,
    category: AvatarItemCategory,
    item_number: u32,
) -> Result<&AvatarItemLookup, String> {
    avatar
        .items
        .iter()
        .find(|item| item.category == category && item.item_number == item_number)
        .ok_or_else(|| format!("avatar item {category:?}/{item_number} is absent"))
}

pub(super) fn visual(
    avatar: &CharacterCreationAvatarItems,
    category: AvatarItemCategory,
    item_number: u32,
    gender: RepairGender,
) -> Result<&AvatarItemVisual, String> {
    let item = item(avatar, category, item_number)?;
    Ok(match gender {
        RepairGender::Male => &item.male,
        RepairGender::Female => &item.female,
    })
}

pub(super) fn visual_mut(
    avatar: &mut CharacterCreationAvatarItems,
    category: AvatarItemCategory,
    item_number: u32,
    gender: RepairGender,
) -> Result<&mut AvatarItemVisual, String> {
    let item = avatar
        .items
        .iter_mut()
        .find(|item| item.category == category && item.item_number == item_number)
        .ok_or_else(|| format!("avatar item {category:?}/{item_number} is absent"))?;
    Ok(match gender {
        RepairGender::Male => &mut item.male,
        RepairGender::Female => &mut item.female,
    })
}

pub(super) fn artifact_identity(path: &str, bytes: &[u8]) -> Value {
    json!({
        "path": path,
        "bytes": bytes.len(),
        "blake3": blake3::hash(bytes).to_hex().to_string(),
        "sha256": format!("{:x}", Sha256::digest(bytes))
    })
}

pub(super) fn verified_gpu_acceptance(project_root: &Path) -> Result<Option<Value>, String> {
    let path = project_root.join(GPU_AUDIT_PATH);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let report: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid GPU audit {}: {error}", path.display()))?;
    if report.pointer("/schema").and_then(Value::as_str)
        != Some("ffone.avatar-equipment-gpu-audit.v1")
        || report.pointer("/complete").and_then(Value::as_bool) != Some(true)
    {
        return Err(format!(
            "GPU audit {} is not a complete v1 report",
            path.display()
        ));
    }
    let counts = report
        .get("counts")
        .ok_or_else(|| format!("GPU audit {} has no counts", path.display()))?;
    for field in ["queued", "completed", "rendered", "passed"] {
        if counts.get(field).and_then(Value::as_u64) != Some(4_376) {
            return Err(format!(
                "GPU audit {} has unexpected {field} count",
                path.display()
            ));
        }
    }
    if counts.get("failed").and_then(Value::as_u64) != Some(0) {
        return Err(format!(
            "GPU audit {} still contains failures",
            path.display()
        ));
    }
    let results = report["results"]
        .as_array()
        .ok_or_else(|| format!("GPU audit {} has no results array", path.display()))?;
    if results.len() != 4_376
        || results.iter().any(|result| {
            result.get("passed").and_then(Value::as_bool) != Some(true)
                || result.get("rendered").and_then(Value::as_bool) != Some(true)
        })
    {
        return Err(format!(
            "GPU audit {} does not contain 4,376 rendered passes",
            path.display()
        ));
    }
    let repair_acceptance = report
        .get("repairAcceptance")
        .ok_or_else(|| format!("GPU audit {} has no repair acceptance", path.display()))?;
    if repair_acceptance
        .get("repairedGenderVariants")
        .and_then(Value::as_u64)
        != Some(68)
        || repair_acceptance.get("gpuPassed").and_then(Value::as_u64) != Some(68)
        || repair_acceptance
            .get("visibleTexturedSurfaces")
            .and_then(Value::as_u64)
            != Some(68)
    {
        return Err(format!(
            "GPU audit {} does not accept all 68 repaired variants",
            path.display()
        ));
    }
    Ok(Some(json!({
        "audit": artifact_identity(GPU_AUDIT_PATH, &bytes),
        "counts": counts,
        "repairedGenderVariants": 68,
        "contactSheets": repair_acceptance.get("contactSheets")
    })))
}

pub(super) fn merge_prior_evidence(next: &mut Value, prior: &Value) -> Result<(), String> {
    if prior.pointer("/schema").and_then(Value::as_str)
        != Some("ffone.avatar-equipment-native-donor-repair.v1")
    {
        return Err(
            "existing avatar equipment repair evidence has an unexpected schema".to_owned(),
        );
    }
    if let Some(input) = prior.get("input") {
        next["input"] = input.clone();
    }
    let prior_repairs = prior["repairs"]
        .as_array()
        .ok_or_else(|| "existing repair evidence has no repairs array".to_owned())?;
    let mut prior_by_key = BTreeMap::<String, &Value>::new();
    for repair in prior_repairs {
        prior_by_key.insert(repair_evidence_key(repair)?, repair);
    }
    let next_repairs = next["repairs"]
        .as_array_mut()
        .ok_or_else(|| "new repair evidence has no repairs array".to_owned())?;
    for repair in next_repairs {
        let key = repair_evidence_key(repair)?;
        let Some(previous) = prior_by_key.get(&key) else {
            continue;
        };
        for field in [
            "requestedModelTrueName",
            "requestedExactRoute",
            "requestedPrimaryTexture",
            "requestedSecondaryTexture",
            "requestedTextureTrueName",
        ] {
            if let Some(value) = previous.get(field) {
                repair[field] = value.clone();
            }
        }
    }
    Ok(())
}

pub(super) fn repair_evidence_key(repair: &Value) -> Result<String, String> {
    let kind = repair["kind"]
        .as_str()
        .ok_or_else(|| "repair evidence has no kind".to_owned())?;
    let category = repair["category"]
        .as_str()
        .ok_or_else(|| "repair evidence has no category".to_owned())?;
    let item_number = repair["itemNumber"]
        .as_u64()
        .ok_or_else(|| "repair evidence has no itemNumber".to_owned())?;
    let gender = repair["gender"]
        .as_str()
        .ok_or_else(|| "repair evidence has no gender".to_owned())?;
    Ok(format!("{kind}|{category}|{item_number}|{gender}"))
}

pub(super) fn pretty_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve {label} {}: {error}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!(
            "{label} is not a directory: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

pub(super) fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve {label} {}: {error}", path.display()))?;
    if !canonical.is_file() {
        return Err(format!("{label} is not a file: {}", canonical.display()));
    }
    Ok(canonical)
}
