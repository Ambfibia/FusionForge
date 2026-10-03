use super::*;

pub(in super::super) fn run(command_args: &[String]) -> Result<(), String> {
    let args = command_args.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    let [
        project_root,
        alternate_root,
        exact_texture_report_root,
        candidate_root,
        source_manifest_path,
        gpu_audit_path,
    ] = args.as_slice()
    else {
        return Err(
            "usage: install_alternate_ultimate_cannonbolt <PROJECT_ROOT> <ALTERNATE_SOURCE_ROOT> <EXACT_TEXTURE_REPORT_ROOT> <CANDIDATE_MODEL_ROOT> <SOURCE_MANIFEST> <GPU_AUDIT>"
                .to_owned(),
        );
    };
    let project_root = canonical_directory(Path::new(project_root), "project root")?;
    let alternate_root = canonical_directory(Path::new(alternate_root), "alternate source root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let imported_root = canonical_directory(
        Path::new(exact_texture_report_root),
        "imported exact texture evidence",
    )?;
    let candidate_root = canonical_directory(Path::new(candidate_root), "candidate model root")?;
    let source_manifest_path = canonical_file(Path::new(source_manifest_path), "source manifest")?;
    let gpu_audit_path = canonical_file(Path::new(gpu_audit_path), "GPU audit")?;
    verify_batch_gate(&source_manifest_path, &gpu_audit_path)?;

    let items_root = canonical_directory(
        &asset_root.join("characters/player/items"),
        "player items root",
    )?;
    let catalog_path = canonical_file(&items_root.join("catalog.json"), "player item catalog")?;
    let avatar_path = canonical_file(
        &asset_root.join("data/character_creation/avatar_items.json"),
        "avatar catalog",
    )?;
    let runtime_path = canonical_file(
        &asset_root.join("data/character_creation/runtime_textures.json"),
        "runtime texture catalog",
    )?;
    let appearance_path = canonical_file(
        &asset_root.join("data/character_creation/appearance.json"),
        "appearance catalog",
    )?;
    let name_wheel_path = canonical_file(
        &asset_root.join("data/character_creation/name_wheel.json"),
        "name-wheel catalog",
    )?;
    let audit_path = project_root.join("target/ffone-audits/player-equipment-texture-audit.json");
    let retired_hnpc_store = asset_root.join("characters/player/hnpc-runtime-textures");
    let retired_hnpc_store_preexisted = retired_hnpc_store.is_dir();

    let protected_paths = [
        catalog_path.clone(),
        avatar_path.clone(),
        runtime_path.clone(),
        appearance_path.clone(),
        name_wheel_path.clone(),
        audit_path.clone(),
    ];
    let originals = protected_paths
        .iter()
        .filter(|path| path.is_file())
        .map(|path| {
            fs::read(path)
                .map(|bytes| (path.clone(), bytes))
                .map_err(|error| format!("cannot back up {}: {error}", path.display()))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    let staging_root = items_root.join(format!(
        ".alternate-ultimate-cannonbolt-stage-{}",
        std::process::id()
    ));
    if staging_root.exists() {
        return Err(format!(
            "staging path already exists: {}",
            staging_root.display()
        ));
    }
    let mut destinations = Vec::<PathBuf>::new();
    for spec in SETS {
        let destination = items_root.join(spec.category).join(spec.set_name);
        if destination.exists() {
            return Err(format!(
                "refusing to overwrite installed extension set {}",
                destination.display()
            ));
        }
        destinations.push(destination);
    }

    let install_result = (|| -> Result<Value, String> {
        fs::create_dir_all(&staging_root)
            .map_err(|error| format!("cannot create {}: {error}", staging_root.display()))?;
        let mut catalog: PlayerItemSetCatalog = read_json_typed(&catalog_path)?;
        if catalog.schema != PLAYER_ITEM_SET_CATALOG_SCHEMA {
            return Err("player item catalog has an unexpected schema".to_owned());
        }
        let mut avatar = read_json_value(&avatar_path)?;
        let mut runtime = read_json_value(&runtime_path)?;
        let mut extension_sets = Vec::new();

        for spec in SETS {
            let staged_set = staging_root.join(spec.category).join(spec.set_name);
            let target_relative_root = format!(
                "characters/player/items/{}/{}",
                spec.category, spec.set_name
            );
            fs::create_dir_all(staged_set.join("models"))
                .map_err(|error| format!("cannot create staged model root: {error}"))?;

            let report_path =
                imported_root.join(format!("{}.texture.json", spec.texture_true_name));
            let report = read_json_value(&report_path)?;
            validate_texture_report(spec, &report)?;
            let levels = decode_exact_mips(&report)?;
            let texture_relative = format!(
                "{target_relative_root}/textures/{}.png",
                spec.texture_true_name
            );
            let texture_path = staged_set
                .join("textures")
                .join(format!("{}.png", spec.texture_true_name));
            write_new(&texture_path, &levels[0])?;
            if levels.len() > 1 {
                let mip_root = staged_set
                    .join("textures")
                    .join(format!("{}.mips", spec.texture_true_name));
                for (level, bytes) in levels.iter().enumerate().skip(1) {
                    write_new(&mip_root.join(format!("mip-{level:02}.png")), bytes)?;
                }
            }
            let texture_artifact = artifact(texture_relative.clone(), &levels[0]);
            let set_id = format!(
                "player-item-set-{}",
                blake3::hash(format!("alternate\0{}\0{}", spec.category, spec.set_name).as_bytes())
                    .to_hex()
            );
            let mut members = Vec::<ResourceSetMember>::new();

            for model in spec.models {
                let candidate_dir = candidate_root
                    .join("characters/player/equipment")
                    .join(spec.category)
                    .join(model.true_name);
                let candidate_glb = canonical_file(
                    &candidate_dir.join(format!("{}.glb", model.true_name)),
                    "candidate GLB",
                )?;
                let model_relative = format!(
                    "{target_relative_root}/models/{}/model.glb",
                    model.true_name
                );
                let model_dir = staged_set.join("models").join(model.true_name);
                fs::create_dir_all(&model_dir)
                    .map_err(|error| format!("cannot create {}: {error}", model_dir.display()))?;
                let glb_bytes = fs::read(&candidate_glb)
                    .map_err(|error| format!("cannot read {}: {error}", candidate_glb.display()))?;
                write_new(&model_dir.join("model.glb"), &glb_bytes)?;
                let model_artifact = artifact(model_relative.clone(), &glb_bytes);
                let mut payload = vec![model_artifact.clone()];
                let candidate_textures =
                    candidate_dir.join(format!("{}.textures", model.true_name));
                if candidate_textures.is_dir() {
                    copy_support_tree(
                        &candidate_textures,
                        &model_dir.join(format!("{}.textures", model.true_name)),
                        &format!(
                            "{target_relative_root}/models/{}/{}.textures",
                            model.true_name, model.true_name
                        ),
                        &mut payload,
                    )?;
                }
                payload.sort_by(|left, right| left.path.cmp(&right.path));
                let source_route = format!(
                    "{}/{}/{}.glb",
                    spec.category, model.true_name, model.true_name
                );
                let item_id = format!(
                    "player-item-{}",
                    blake3::hash(
                        format!("alternate\0{}\0{}", source_route, model.exact_route).as_bytes()
                    )
                    .to_hex()
                );
                let definition_relative = format!(
                    "{target_relative_root}/models/{}/item.json",
                    model.true_name
                );
                let definition = PlayerItemDefinition {
                    schema: PLAYER_ITEM_SCHEMA.to_owned(),
                    id: item_id.clone(),
                    true_name: model.true_name.to_owned(),
                    category: spec.category.to_owned(),
                    source_route: source_route.clone(),
                    resource_set: set_id.clone(),
                    model: model_artifact.clone(),
                };
                let definition_bytes = pretty_json(&definition)?;
                write_new(&model_dir.join("item.json"), &definition_bytes)?;
                let definition_artifact = artifact(definition_relative, &definition_bytes);
                members.push(ResourceSetMember {
                    id: item_id,
                    name: model.true_name.to_owned(),
                    definition: definition_artifact,
                    files: payload.clone(),
                });
                catalog.models.push(PlayerItemCatalogModel {
                    category: spec.category.to_owned(),
                    true_name: model.true_name.to_owned(),
                    source_route,
                    resource_set: set_id.clone(),
                    model: model_artifact.clone(),
                });
                update_avatar_visual(&mut avatar, spec, model, &model_artifact, &texture_artifact)?;
            }
            members.sort_by(|left, right| left.name.cmp(&right.name));
            let set_document = ResourceSetDocument {
                schema: crate::RESOURCE_SET_SCHEMA.to_owned(),
                id: set_id.clone(),
                name: spec.set_name.to_owned(),
                domain: "player_item".to_owned(),
                category: spec.category.to_owned(),
                prefix: "PLAYER".to_owned(),
                family: spec.category.to_ascii_uppercase(),
                textures: vec![texture_artifact.clone()],
                members,
            };
            let set_bytes = pretty_json(&set_document)?;
            write_new(&staged_set.join("set.json"), &set_bytes)?;
            let set_relative = format!("{target_relative_root}/set.json");
            catalog.sets.push(ResourceSetCatalogEntry {
                id: set_id.clone(),
                name: spec.set_name.to_owned(),
                category: spec.category.to_owned(),
                prefix: "PLAYER".to_owned(),
                family: spec.category.to_ascii_uppercase(),
                definition: artifact(set_relative, &set_bytes),
                member_count: spec.models.len() as u64,
                texture_count: 1,
            });
            for alias in spec.aliases {
                add_runtime_texture_contract(
                    &mut runtime,
                    alias,
                    spec,
                    &report,
                    &texture_artifact,
                    &levels[0],
                )?;
            }
            extension_sets.push(json!({
                "category": spec.category,
                "patchedItemNumber": spec.item_number,
                "set": spec.set_name,
                "textureTrueName": spec.texture_true_name,
                "textureAliases": spec.aliases,
                "models": spec.models.iter().map(|model| model.true_name).collect::<Vec<_>>(),
            }));
        }

        catalog.sets.sort_by(|left, right| {
            (left.category.as_str(), left.name.as_str())
                .cmp(&(right.category.as_str(), right.name.as_str()))
        });
        catalog.models.sort_by(|left, right| {
            (
                left.category.as_str(),
                left.true_name.as_str(),
                left.source_route.as_str(),
                left.model.path.as_str(),
            )
                .cmp(&(
                    right.category.as_str(),
                    right.true_name.as_str(),
                    right.source_route.as_str(),
                    right.model.path.as_str(),
                ))
        });
        sort_runtime_textures(&mut runtime)?;
        refresh_runtime_coverage(&avatar, &mut runtime)?;
        let catalog_bytes = pretty_json(&catalog)?;
        let catalog_artifact = artifact(
            "characters/player/items/catalog.json".to_owned(),
            &catalog_bytes,
        );
        update_catalog_proof(&mut avatar, &catalog_artifact)?;
        update_catalog_proof(&mut runtime, &catalog_artifact)?;
        let mut appearance = read_json_value(&appearance_path)?;
        let mut name_wheel = read_json_value(&name_wheel_path)?;
        update_catalog_proof(&mut appearance, &catalog_artifact)?;
        update_catalog_proof(&mut name_wheel, &catalog_artifact)?;

        for (spec, destination) in SETS.iter().zip(&destinations) {
            fs::create_dir_all(
                destination
                    .parent()
                    .ok_or_else(|| "extension destination has no parent".to_owned())?,
            )
            .map_err(|error| format!("cannot create destination parent: {error}"))?;
            fs::rename(
                staging_root.join(spec.category).join(spec.set_name),
                destination,
            )
            .map_err(|error| format!("cannot install {}: {error}", destination.display()))?;
        }
        write_replace(&catalog_path, &catalog_bytes)?;
        write_replace(&avatar_path, &pretty_json(&avatar)?)?;
        write_replace(&runtime_path, &pretty_json(&runtime)?)?;
        write_replace(&appearance_path, &pretty_json(&appearance)?)?;
        write_replace(&name_wheel_path, &pretty_json(&name_wheel)?)?;
        update_texture_audit(&audit_path, &runtime, &extension_sets)?;
        verify_installed_extension(&asset_root, &catalog_path, &avatar_path, &runtime_path)?;
        let pre_existing_global_blocker = match verify_player_item_sets(&project_root) {
            Ok(_) => Value::Null,
            Err(error)
                if retired_hnpc_store_preexisted
                    && error.to_string().contains("hnpc-runtime-textures") =>
            {
                json!({
                    "classification": "pre-existing-unrelated-retired-store",
                    "path": relative_slash(&project_root, &retired_hnpc_store)?,
                    "message": error.to_string(),
                    "extensionScopedVerification": "passed"
                })
            }
            Err(error) => return Err(error.to_string()),
        };

        Ok(json!({
            "catalog": catalog_artifact,
            "sets": extension_sets,
            "runtimeTextureCount": runtime["textures"].as_array().map(Vec::len),
            "scopedVerification": "passed",
            "preExistingGlobalBlocker": pre_existing_global_blocker,
        }))
    })();

    let install_summary = match install_result {
        Ok(summary) => summary,
        Err(error) => {
            for (path, bytes) in &originals {
                let _ = write_replace(path, bytes);
            }
            for destination in &destinations {
                let _ = fs::remove_dir_all(destination);
            }
            let _ = fs::remove_dir_all(&staging_root);
            return Err(error);
        }
    };
    let _ = fs::remove_dir_all(&staging_root);

    let evidence_path =
        project_root.join("target/ffone-audits/player-equipment-alternate-extension-audit.json");
    let evidence = build_extension_evidence(
        &alternate_root,
        &source_manifest_path,
        &imported_root,
        &audit_path,
        &gpu_audit_path,
        install_summary,
    )?;
    write_replace(&evidence_path, &pretty_json(&evidence)?)?;
    println!(
        "Installed 6 alternate Ultimate Cannonbolt models, 3 exact textures, and 5 patched aliases; evidence={}",
        evidence_path.display()
    );
    Ok(())
}

pub(super) fn verify_installed_extension(
    asset_root: &Path,
    catalog_path: &Path,
    avatar_path: &Path,
    runtime_path: &Path,
) -> Result<(), String> {
    let catalog: PlayerItemSetCatalog = read_json_typed(catalog_path)?;
    let avatar: CharacterCreationAvatarItems = read_json_typed(avatar_path)?;
    let runtime: CharacterCreationRuntimeTextures = read_json_typed(runtime_path)?;
    let mut verified_models = BTreeSet::new();

    for spec in SETS {
        let entries = catalog
            .sets
            .iter()
            .filter(|entry| entry.category == spec.category && entry.name == spec.set_name)
            .collect::<Vec<_>>();
        let [entry] = entries.as_slice() else {
            return Err(format!(
                "extension catalog must contain exactly one {}/{} set, found {}",
                spec.category,
                spec.set_name,
                entries.len()
            ));
        };
        if entry.member_count != spec.models.len() as u64 || entry.texture_count != 1 {
            return Err(format!(
                "extension set {} has invalid counts",
                spec.set_name
            ));
        }
        verify_artifact(asset_root, &entry.definition)?;
        let set_path = checked_artifact_path(asset_root, &entry.definition.path)?;
        let set: ResourceSetDocument = read_json_typed(&set_path)?;
        if set.id != entry.id
            || set.name != spec.set_name
            || set.category != spec.category
            || set.domain != "player_item"
            || set.members.len() != spec.models.len()
            || set.textures.len() != 1
        {
            return Err(format!("extension set {} identity changed", spec.set_name));
        }
        let texture = &set.textures[0];
        let expected_texture_path = format!(
            "characters/player/items/{}/{}/textures/{}.png",
            spec.category, spec.set_name, spec.texture_true_name
        );
        if texture.path != expected_texture_path {
            return Err(format!(
                "extension texture route changed for {}",
                spec.set_name
            ));
        }
        verify_artifact(asset_root, texture)?;

        for model in spec.models {
            let member = set
                .members
                .iter()
                .find(|member| member.name == model.true_name)
                .ok_or_else(|| format!("extension member {} is absent", model.true_name))?;
            verify_artifact(asset_root, &member.definition)?;
            for artifact in &member.files {
                verify_artifact(asset_root, artifact)?;
            }
            let definition_path = checked_artifact_path(asset_root, &member.definition.path)?;
            let definition: PlayerItemDefinition = read_json_typed(&definition_path)?;
            if definition.id != member.id
                || definition.true_name != model.true_name
                || definition.category != spec.category
                || definition.resource_set != set.id
                || !member.files.contains(&definition.model)
            {
                return Err(format!(
                    "extension item definition changed for {}",
                    model.true_name
                ));
            }
            let catalog_model = catalog
                .models
                .iter()
                .find(|candidate| {
                    candidate.category == spec.category
                        && candidate.true_name == model.true_name
                        && candidate.resource_set == set.id
                })
                .ok_or_else(|| format!("catalog model {} is absent", model.true_name))?;
            if catalog_model.model != definition.model {
                return Err(format!(
                    "catalog model {} does not match its set",
                    model.true_name
                ));
            }
            verified_models.insert(model.true_name);

            let category = match spec.category {
                "pants" => AvatarItemCategory::Pants,
                "shirt" => AvatarItemCategory::Shirt,
                "shoes" => AvatarItemCategory::Shoes,
                _ => return Err("unexpected extension category".to_owned()),
            };
            let avatar_item = avatar
                .items
                .iter()
                .find(|item| {
                    item.category == category && i64::from(item.item_number) == spec.item_number
                })
                .ok_or_else(|| format!("avatar item {} is absent", spec.item_number))?;
            let visual = match model.gender {
                "female" => &avatar_item.female,
                "male" => &avatar_item.male,
                _ => return Err("unexpected extension gender".to_owned()),
            };
            let expected_alias = match (spec.category, model.gender) {
                ("pants", "female") => "f_pants_ultimatecannonbolt",
                ("pants", "male") => "m_pants_ultimatecannonbolt",
                ("shirt", "female") => "f_shirt_ultimatecannonbolt",
                ("shirt", "male") => "m_shirt_ultimatecannonbolt",
                ("shoes", _) => "shoes_ultimatecannonbolt",
                _ => return Err("unexpected extension texture alias".to_owned()),
            };
            let [avatar_model] = visual.models.as_slice() else {
                return Err(format!("avatar model {} is not unique", model.true_name));
            };
            let Some(primary_texture) = visual.primary_texture.as_ref() else {
                return Err(format!("avatar texture {} is absent", expected_alias));
            };
            if visual.model_status != NativeLookupStatus::VerifiedUnique
                || avatar_model.true_name != model.true_name
                || avatar_model.exact_route != model.exact_route
                || !asset_reference_matches(&avatar_model.native_asset, &definition.model)
                || primary_texture.true_name != expected_alias
                || primary_texture.status != NativeLookupStatus::VerifiedUnique
                || primary_texture.candidates.len() != 1
                || !asset_reference_matches(&primary_texture.candidates[0], texture)
            {
                return Err(format!("avatar mapping changed for {}", model.true_name));
            }
        }

        for alias in spec.aliases {
            let contracts = runtime
                .textures
                .iter()
                .filter(|contract| contract.true_name == *alias)
                .collect::<Vec<_>>();
            let [contract] = contracts.as_slice() else {
                return Err(format!(
                    "runtime texture alias {alias} must be unique, found {}",
                    contracts.len()
                ));
            };
            if !asset_reference_matches(&contract.native_asset, texture)
                || contract.source.path_id != spec.texture_path_id
                || contract.source.container_route
                    != format!("texture/{}.dds", spec.texture_true_name)
            {
                return Err(format!("runtime texture alias {alias} identity changed"));
            }
        }
    }

    if verified_models.len() != 6 {
        return Err(format!(
            "extension scoped verifier expected 6 models, found {}",
            verified_models.len()
        ));
    }
    Ok(())
}

pub(super) fn verify_artifact(asset_root: &Path, artifact: &ResourceSetArtifact) -> Result<(), String> {
    let path = checked_artifact_path(asset_root, &artifact.path)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("cannot read artifact {}: {error}", path.display()))?;
    if bytes.len() as u64 != artifact.bytes
        || blake3::hash(&bytes).to_hex().as_str() != artifact.blake3
    {
        return Err(format!(
            "artifact acceptance identity changed: {}",
            artifact.path
        ));
    }
    Ok(())
}

pub(super) fn verify_batch_gate(source_manifest_path: &Path, gpu_audit_path: &Path) -> Result<(), String> {
    let source = read_json_value(source_manifest_path)?;
    if source["schema"] != "ffclient.equipment-model-source-batch.v1"
        || source["status"] != "complete"
        || source
            .pointer("/counts/exportedPhysicalModels")
            .and_then(Value::as_u64)
            != Some(6)
        || source
            .pointer("/counts/totalBlockers")
            .and_then(Value::as_u64)
            != Some(0)
    {
        return Err(
            "alternate model source batch did not pass its exact six-model gate".to_owned(),
        );
    }
    let gpu = read_json_value(gpu_audit_path)?;
    if gpu["schema"] != "ffone.equipment-gpu-batch.v1"
        || gpu
            .pointer("/counts/selectedModels")
            .and_then(Value::as_u64)
            != Some(6)
        || gpu
            .pointer("/counts/standaloneGpuPassedModels")
            .and_then(Value::as_u64)
            != Some(6)
        || gpu
            .pointer("/counts/executionBlockers")
            .and_then(Value::as_u64)
            != Some(0)
    {
        return Err("alternate model GPU audit did not pass 6/6".to_owned());
    }
    Ok(())
}

pub(super) fn build_extension_evidence(
    alternate_root: &Path,
    source_manifest_path: &Path,
    exact_texture_report_root: &Path,
    texture_audit_path: &Path,
    gpu_audit_path: &Path,
    install_summary: Value,
) -> Result<Value, String> {
    let mut texture_sources = Vec::new();
    for spec in SETS {
        let raw_path = canonical_file(
            &alternate_root.join(spec.texture_resource),
            "alternate texture resource",
        )?;
        let raw = fs::read(&raw_path)
            .map_err(|error| format!("cannot read {}: {error}", raw_path.display()))?;
        let report_path =
            exact_texture_report_root.join(format!("{}.texture.json", spec.texture_true_name));
        let report = fs::read(&report_path)
            .map_err(|error| format!("cannot read {}: {error}", report_path.display()))?;
        texture_sources.push(json!({
            "trueName": spec.texture_true_name,
            "patchedAliases": spec.aliases,
            "containerRoute": format!("texture/{}.dds", spec.texture_true_name),
            "pathId": spec.texture_path_id,
            "rawResourceFile": format!("builds/{SOURCE_BUILD}/{}", spec.texture_resource),
            "rawResourceFileBytes": raw.len(),
            "rawResourceFileSha256": sha256_hex(&raw),
            "exactReportBytes": report.len(),
            "exactReportSha256": sha256_hex(&report),
        }));
    }
    let source_manifest = fs::read(source_manifest_path)
        .map_err(|error| format!("cannot read source manifest: {error}"))?;
    let gpu_audit =
        fs::read(gpu_audit_path).map_err(|error| format!("cannot read GPU audit: {error}"))?;
    let unresolved = texture_audit_path
        .is_file()
        .then(|| read_json_value(texture_audit_path))
        .transpose()?
        .and_then(|audit| audit["extensionMissingTrueNames"].as_array().cloned())
        .unwrap_or_default();
    Ok(json!({
        "schema": "ffone.player-equipment-alternate-extension-audit.v1",
        "status": "installed-validated-native-extension",
        "sourceAlias": SOURCE_ALIAS,
        "sourceBuild": SOURCE_BUILD,
        "authority": "explicit_content_donor",
        "intentionalDivergence": "Patched Ultimate Cannonbolt rows use shortened or base-Cannonbolt names. FFOne maps only these proven same-set rows to the complete alternate Ultimate Cannonbolt routes; no other missing patched item is substituted.",
        "conversionCommands": [
            "FFONE_EXTRA_DEPENDENCY_BUNDLES=<alternate>/CharacterCreation.resourceFile fusionforge fusionforge export-equipment-model-sources <bundle-index> <source-plan> <source-root>",
            "ffone-asset-pipeline publish-equipment-logical-model-batch <source-root> <candidate-root>",
            "ffone-asset-pipeline accept-equipment-gpu-batch <candidate-root> <gpu-evidence-root> logical_model_gpu_preview full <gpu-audit>",
            "fusionforge fusionforge export-exact-texture <alternate>/Character_Texture_<category>.resourceFile <pathId> <out.json>",
            "ffone-asset-pipeline install_alternate_ultimate_cannonbolt <project-root> <alternate-root> <exact-texture-report-root> <candidate-root> <source-manifest> <gpu-audit>"
        ],
        "modelSourceManifest": {
            "bytes": source_manifest.len(),
            "sha256": sha256_hex(&source_manifest)
        },
        "gpuAudit": {
            "bytes": gpu_audit.len(),
            "sha256": sha256_hex(&gpu_audit),
            "passed": "6/6"
        },
        "textures": texture_sources,
        "installation": install_summary,
        "remainingUnresolvedTrueNames": unresolved,
        "remainingDisposition": "No exact asset or proven same-item donor exists in primary, patched, or alternate; remain explicit blockers instead of guessed visual substitutions."
    }))
}

pub(super) fn artifact(path: String, bytes: &[u8]) -> ResourceSetArtifact {
    ResourceSetArtifact {
        path,
        bytes: bytes.len() as u64,
        blake3: blake3::hash(bytes).to_hex().to_string(),
    }
}

pub(super) fn pretty_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, String> {
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

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn relative_slash(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| format!("{} is outside {}", path.display(), root.display()))
}
