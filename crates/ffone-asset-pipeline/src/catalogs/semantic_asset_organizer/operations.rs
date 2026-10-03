use super::*;

pub fn plan_semantic_assets(
    options: &SemanticAssetOrganizerOptions,
) -> Result<SemanticAssetOrganizationReport> {
    guard_output(options)?;
    let manifest = read_evidence(
        "assetManifest",
        &options.asset_manifest,
        ASSET_MANIFEST_SCHEMA,
    )?;
    let content = read_evidence("contentIndex", &options.content_index, CONTENT_INDEX_SCHEMA)?;
    let cook = read_evidence("cookReport", &options.cook_report, COOK_REPORT_SCHEMA)?;
    let tables = read_evidence("tableSet", &options.table_set, TABLE_SET_SCHEMA)?;
    let logical = read_evidence(
        "logicalModelPlan",
        &options.logical_model_plan,
        LOGICAL_MODEL_PLAN_SCHEMA,
    )?;
    require_bool(&content.value, "nativeOnly", true, "content index")?;
    require_bool(&cook.value, "nativeOnly", true, "cook report")?;
    require_bool(&logical.value, "emitsGlb", false, "logical-model plan")?;

    let manifest_files = value_array(manifest.value.get("files")).len() as u64;
    let content_assets = value_array(content.value.get("assets")).len() as u64;
    let cook_mappings = value_array(cook.value.get("mappings")).len() as u64;
    let native_index = build_native_index(&manifest.value, &content.value, &cook.value);
    let (ready_roots, plan_route_blockers, global_plan_blockers) =
        read_logical_plan(&logical.value);
    let logical_plan_ready_roots = ready_roots.values().map(Vec::len).sum::<usize>() as u64;
    let mut builder = Builder {
        ready_roots,
        plan_route_blockers,
        native_index,
        ..Builder::default()
    };
    for issue in builder.native_index.evidence_issues.clone() {
        builder.add_blocker(
            BlockerCode::NativeEvidenceMismatch,
            "native-evidence",
            None,
            issue,
        );
    }
    for detail in global_plan_blockers {
        builder.add_blocker(
            BlockerCode::LogicalPlanGlobalBlocker,
            "logical-model-plan",
            None,
            detail,
        );
    }
    let table_root = consolidated_table_root(&tables.value)?;
    add_npc_entities(&mut builder, table_root);
    add_nano_entities(&mut builder, table_root);
    add_player_entities(&mut builder);
    add_equipment_entities(&mut builder, table_root);
    builder.resolve_models();
    let shared_assets = builder.resolve_shared_assets();
    let route_scale_usage = builder.route_scale_report();
    let unresolved_routes = builder.finish_unresolved();
    let mut counts = count_report(
        &builder,
        manifest_files,
        content_assets,
        cook_mappings,
        logical_plan_ready_roots,
        &shared_assets,
        &route_scale_usage,
        &unresolved_routes,
    );
    counts.blockers = builder.blockers.len() as u64;
    for blocker in &builder.blockers {
        *counts
            .blockers_by_code
            .entry(blocker.code.as_str().to_owned())
            .or_default() += 1;
    }
    let report = SemanticAssetOrganizationReport {
        schema: SEMANTIC_ASSET_ORGANIZATION_SCHEMA.to_owned(),
        mode: "plan-only".to_owned(),
        status: if builder.blockers.is_empty() {
            "ready".to_owned()
        } else {
            "blocked".to_owned()
        },
        production_assets_mutated: false,
        taxonomy: taxonomy(),
        inputs: vec![
            manifest.evidence,
            content.evidence,
            cook.evidence,
            tables.evidence,
            logical.evidence,
        ],
        counts,
        entities: builder.entities,
        models: builder.models,
        route_scale_usage,
        shared_assets,
        unresolved_routes,
        blockers: builder.blockers,
    };
    write_atomic_create_new(&options.output, &report)?;
    Ok(report)
}

pub(super) fn taxonomy() -> Vec<String> {
    [
        "characters/npc",
        "characters/mob",
        "characters/shared",
        "characters/hnpc",
        "characters/nano",
        "characters/player",
        "characters/player/equipment/hat",
        "characters/player/equipment/mask",
        "characters/player/equipment/glasses",
        "characters/player/equipment/back",
        "characters/player/equipment/head",
        "characters/player/equipment/shirt",
        "characters/player/equipment/pants",
        "characters/player/equipment/shoes",
        "characters/player/equipment/weapon",
        "characters/player/equipment/vehicle",
        "characters/unresolved",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub(super) fn consolidated_table_root(table_set: &Value) -> Result<&Value> {
    let mut matches = value_array(table_set.get("tables")).iter().filter(|table| {
        table.get("name").and_then(Value::as_str) == Some("npc_imports_consolidated")
    });
    let Some(table) = matches.next() else {
        return semantic_error("table set has no npc_imports_consolidated table");
    };
    if matches.next().is_some() {
        return semantic_error("table set has duplicate npc_imports_consolidated tables");
    }
    table.get("value").ok_or_else(|| {
        PipelineError::SemanticAssetOrganizer("consolidated table has no value".into())
    })
}

pub(super) fn add_npc_entities(builder: &mut Builder, tables: &Value) {
    let table = tables.get("m_pNpcTable").unwrap_or(&Value::Null);
    let rows = value_array(table.get("m_pNpcData"));
    let meshes = value_array(table.get("m_pNpcMeshData"));
    for (row_index, row) in rows.iter().enumerate() {
        let hnpc = int_field(row, "m_iHNpc").unwrap_or_default() != 0;
        let category = if hnpc {
            SemanticCategory::Hnpc
        } else if int_field(row, "m_iTeam") == Some(2) {
            SemanticCategory::Mob
        } else {
            SemanticCategory::Npc
        };
        let mesh_index = positive_index(row, "m_iMesh");
        let entity_number = int_field(row, "m_iNpcNumber");
        let id = format!(
            "{}:row:{row_index}:number:{}",
            if hnpc { "hnpc" } else { "npc" },
            entity_number.unwrap_or(-1)
        );
        let mut entity = EntityProposal {
            id: id.clone(),
            category,
            semantic_directory: category.directory().to_owned(),
            table_owner: TableOwner {
                table: "m_pNpcTable.m_pNpcData".to_owned(),
                row_index,
                entity_number,
                mesh_index,
            },
            spawn_root_policy: None,
            model_routes: Vec::new(),
            model_proposal_ids: Vec::new(),
            dependencies: Vec::new(),
            blocker_ids: Vec::new(),
        };
        if hnpc {
            let blocker = builder.add_blocker(
                BlockerCode::HnpcPhysicalMappingUnproven,
                &id,
                None,
                "m_iHNpc != 0 requires All_HNpc.TableElement wearable closure; that physical mapping is absent from the supplied native table-set evidence",
            );
            entity.blocker_ids.push(blocker);
            builder.entities.push(entity);
            continue;
        }
        let scale = float_field(row, "m_fScale");
        entity.spawn_root_policy = Some(SpawnRootPolicy {
            kind: SpawnRootPolicyKind::NpcSetupNpcReplaceRootTrs,
            authority: NPC_SETUP_AUTHORITY.to_owned(),
            position: RootTrsAction::ReplaceWithSpawnWorldValue,
            rotation: RootTrsAction::ReplaceWithSpawnWorldValue,
            scale: RootTrsAction::ReplaceWithNpcRowMFScale,
            row_m_f_scale: scale,
        });
        if scale.is_none() {
            let blocker = builder.add_blocker(
                BlockerCode::NpcScaleMissing,
                &id,
                None,
                "direct NPC row has no numeric m_fScale",
            );
            entity.blocker_ids.push(blocker);
        }
        let Some(mesh_index) = mesh_index else {
            let blocker = builder.add_blocker(
                BlockerCode::TableRowHasNoModel,
                &id,
                None,
                "direct NPC row has no positive m_iMesh",
            );
            entity.blocker_ids.push(blocker);
            builder.entities.push(entity);
            continue;
        };
        let Some(mesh) = meshes.get(mesh_index) else {
            let blocker = builder.add_blocker(
                BlockerCode::TableReferenceInvalid,
                &id,
                None,
                format!("m_iMesh {mesh_index} is outside m_pNpcMeshData"),
            );
            entity.blocker_ids.push(blocker);
            builder.entities.push(entity);
            continue;
        };
        if let Some(route) = route_from_field(mesh, "m_pstrMMeshModelString", "mob", "kfm") {
            push_unique(&mut entity.model_routes, route.legacy_route.clone());
            if let Some(scale) = scale {
                builder
                    .route_scales
                    .entry(route.legacy_route)
                    .or_default()
                    .push(scale);
            }
        } else {
            let blocker = builder.add_blocker(
                BlockerCode::TableRowHasNoModel,
                &id,
                None,
                "NPC mesh row has no m_pstrMMeshModelString",
            );
            entity.blocker_ids.push(blocker);
        }
        if int_field(row, "m_iNpcType").is_some_and(|value| value < 100) {
            for field in ["m_pstrMTextureString", "m_pstrMTextureString2"] {
                builder.add_dependency(
                    &mut entity,
                    mesh,
                    field,
                    "texture",
                    "dds",
                    DependencyKind::Texture,
                );
            }
        }
        // Despite their names, the NPC-table F-fields are hurt SFX. This is the exact
        // runtime contract in NpcMoveController.cs:811-823; item-table F-fields below
        // remain female wear models/textures and must not share this interpretation.
        for field in [
            "m_pstrFMeshModelString",
            "m_pstrFTextureString",
            "m_pstrFTextureString2",
        ] {
            builder.add_dependency(
                &mut entity,
                mesh,
                field,
                "sound",
                "wav",
                DependencyKind::Audio,
            );
        }
        builder.entities.push(entity);
    }
}

pub(super) fn add_nano_entities(builder: &mut Builder, tables: &Value) {
    let table = tables.get("m_pNanoTable").unwrap_or(&Value::Null);
    let rows = value_array(table.get("m_pNanoData"));
    let meshes = value_array(table.get("m_pNanoMeshData"));
    for (row_index, row) in rows.iter().enumerate() {
        let mesh_index = positive_index(row, "m_iMesh");
        let entity_number = int_field(row, "m_iNanoNumber").or(Some(row_index as i64));
        let id = format!(
            "nano:row:{row_index}:number:{}",
            entity_number.unwrap_or(-1)
        );
        let mut entity = EntityProposal {
            id: id.clone(),
            category: SemanticCategory::Nano,
            semantic_directory: SemanticCategory::Nano.directory().to_owned(),
            table_owner: TableOwner {
                table: "m_pNanoTable.m_pNanoData".to_owned(),
                row_index,
                entity_number,
                mesh_index,
            },
            spawn_root_policy: Some(SpawnRootPolicy {
                kind: SpawnRootPolicyKind::NanoPreserveAuthoredRootScale,
                authority: NANO_SETUP_AUTHORITY.to_owned(),
                position: RootTrsAction::ReplaceWithSpawnWorldValue,
                rotation: RootTrsAction::ReplaceWithSpawnWorldValue,
                scale: RootTrsAction::PreserveAuthored,
                row_m_f_scale: None,
            }),
            model_routes: Vec::new(),
            model_proposal_ids: Vec::new(),
            dependencies: Vec::new(),
            blocker_ids: Vec::new(),
        };
        let Some(mesh_index) = mesh_index else {
            let blocker = builder.add_blocker(
                BlockerCode::TableRowHasNoModel,
                &id,
                None,
                "Nano row has no positive m_iMesh",
            );
            entity.blocker_ids.push(blocker);
            builder.entities.push(entity);
            continue;
        };
        let Some(mesh) = meshes.get(mesh_index) else {
            let blocker = builder.add_blocker(
                BlockerCode::TableReferenceInvalid,
                &id,
                None,
                format!("m_iMesh {mesh_index} is outside m_pNanoMeshData"),
            );
            entity.blocker_ids.push(blocker);
            builder.entities.push(entity);
            continue;
        };
        if let Some(route) = route_from_field(mesh, "m_pstrMMeshModelString", "nano", "kfm") {
            entity.model_routes.push(route.legacy_route);
        } else {
            let blocker = builder.add_blocker(
                BlockerCode::TableRowHasNoModel,
                &id,
                None,
                "Nano mesh row has no m_pstrMMeshModelString",
            );
            entity.blocker_ids.push(blocker);
        }
        for field in ["m_pstrMTextureString", "m_pstrMTextureString2"] {
            builder.add_dependency(
                &mut entity,
                mesh,
                field,
                "texture",
                "dds",
                DependencyKind::Texture,
            );
        }
        if positive_index(row, "m_iSound").is_some() {
            let blocker = builder.add_blocker(
                BlockerCode::NanoSoundTableUnproven,
                &id,
                None,
                "m_iSound is positive but no exact Nano sound string table is present in the supplied table-set",
            );
            entity.blocker_ids.push(blocker);
        }
        builder.entities.push(entity);
    }
}

pub(super) fn add_player_entities(builder: &mut Builder) {
    for (sex, route, skin) in [
        ("male", "actor/m.kfm", "m_skin"),
        ("female", "actor/w.kfm", "f_skin"),
    ] {
        let mut entity = EntityProposal {
            id: format!("player:{sex}:base"),
            category: SemanticCategory::Player,
            semantic_directory: SemanticCategory::Player.directory().to_owned(),
            table_owner: TableOwner {
                table: "runtime.player-base".to_owned(),
                row_index: usize::from(sex == "female"),
                entity_number: None,
                mesh_index: None,
            },
            spawn_root_policy: Some(SpawnRootPolicy {
                kind: SpawnRootPolicyKind::PlayerReplaceRootTrsWithIdentityScale,
                authority: PLAYER_SETUP_AUTHORITY.to_owned(),
                position: RootTrsAction::ReplaceWithIdentity,
                rotation: RootTrsAction::ReplaceWithIdentity,
                scale: RootTrsAction::ReplaceWithIdentity,
                row_m_f_scale: None,
            }),
            model_routes: vec![route.to_owned()],
            model_proposal_ids: Vec::new(),
            dependencies: Vec::new(),
            blocker_ids: Vec::new(),
        };
        if let Some(route) = make_route("texture", skin, "dds") {
            builder.add_dependency_route(
                &mut entity,
                "runtime.player-base-skin".to_owned(),
                route,
                DependencyKind::Texture,
            );
        }
        builder.entities.push(entity);
    }
}

pub(super) fn add_equipment_entities(builder: &mut Builder, tables: &Value) {
    const FAMILIES: &[(&str, SemanticCategory)] = &[
        ("m_pHatItemTable", SemanticCategory::EquipmentHat),
        ("m_pFaceItemTable", SemanticCategory::EquipmentMask),
        ("m_pGlassItemTable", SemanticCategory::EquipmentGlasses),
        ("m_pBackItemTable", SemanticCategory::EquipmentBack),
        ("m_pHeadItemTable", SemanticCategory::EquipmentHead),
        ("m_pShirtsItemTable", SemanticCategory::EquipmentShirt),
        ("m_pPantsItemTable", SemanticCategory::EquipmentPants),
        ("m_pShoesItemTable", SemanticCategory::EquipmentShoes),
        ("m_pWeaponItemTable", SemanticCategory::EquipmentWeapon),
        ("m_pVehicleItemTable", SemanticCategory::EquipmentVehicle),
    ];
    for (table_name, category) in FAMILIES {
        let table = tables.get(*table_name).unwrap_or(&Value::Null);
        let rows = value_array(table.get("m_pItemData"));
        let meshes = value_array(table.get("m_pItemMeshData"));
        let sounds = value_array(table.get("m_pItemSoundData"));
        if rows.is_empty() {
            continue;
        }
        let special_aliases = matches!(
            category,
            SemanticCategory::EquipmentMask | SemanticCategory::EquipmentHead
        );
        for (row_index, row) in rows.iter().enumerate() {
            let mesh_index = positive_index(row, "m_iMesh");
            let entity_number = int_field(row, "m_iItemNumber").or(Some(row_index as i64));
            let id = format!(
                "equipment:{}:row:{row_index}:number:{}",
                category.directory().rsplit('/').next().unwrap_or("unknown"),
                entity_number.unwrap_or(-1)
            );
            let mut entity = EntityProposal {
                id: id.clone(),
                category: *category,
                semantic_directory: category.directory().to_owned(),
                table_owner: TableOwner {
                    table: format!("{table_name}.m_pItemData"),
                    row_index,
                    entity_number,
                    mesh_index,
                },
                spawn_root_policy: Some(SpawnRootPolicy {
                    kind: SpawnRootPolicyKind::EquipmentRuntimeAttachmentNotIndependent,
                    authority: EQUIPMENT_ATTACHMENT_AUTHORITY.to_owned(),
                    position: RootTrsAction::ControlledByRuntimeAttachment,
                    rotation: RootTrsAction::ControlledByRuntimeAttachment,
                    scale: RootTrsAction::ControlledByRuntimeAttachment,
                    row_m_f_scale: None,
                }),
                model_routes: Vec::new(),
                model_proposal_ids: Vec::new(),
                dependencies: Vec::new(),
                blocker_ids: Vec::new(),
            };
            if let Some(mesh_index) = mesh_index {
                if let Some(mesh) = meshes.get(mesh_index) {
                    for field in ["m_pstrFMeshModelString", "m_pstrMMeshModelString"] {
                        if let Some(route) = route_from_field(mesh, field, "wear", "nif") {
                            push_unique(&mut entity.model_routes, route.legacy_route);
                        }
                        if special_aliases {
                            for alias in face_head_model_aliases(mesh, field) {
                                push_unique(&mut entity.model_routes, alias.legacy_route);
                            }
                        }
                    }
                    for field in [
                        "m_pstrFTextureString",
                        "m_pstrFTextureString2",
                        "m_pstrMTextureString",
                        "m_pstrMTextureString2",
                    ] {
                        builder.add_dependency(
                            &mut entity,
                            mesh,
                            field,
                            "texture",
                            "dds",
                            DependencyKind::Texture,
                        );
                        if special_aliases {
                            for alias in face_head_texture_aliases(mesh, field) {
                                builder.add_dependency_route(
                                    &mut entity,
                                    format!("runtime-alias:{field}"),
                                    alias,
                                    DependencyKind::Texture,
                                );
                            }
                        }
                    }
                } else {
                    let blocker = builder.add_blocker(
                        BlockerCode::TableReferenceInvalid,
                        &id,
                        None,
                        format!("m_iMesh {mesh_index} is outside {table_name}.m_pItemMeshData"),
                    );
                    entity.blocker_ids.push(blocker);
                }
            } else {
                let blocker = builder.add_blocker(
                    BlockerCode::TableRowHasNoModel,
                    &id,
                    None,
                    "equipment row has no positive m_iMesh",
                );
                entity.blocker_ids.push(blocker);
            }
            for sound_index in [
                positive_index(row, "m_iSound1"),
                positive_index(row, "m_iSound2"),
            ]
            .into_iter()
            .flatten()
            .collect::<BTreeSet<_>>()
            {
                let Some(sound) = sounds.get(sound_index) else {
                    let blocker = builder.add_blocker(
                        BlockerCode::TableReferenceInvalid,
                        &id,
                        None,
                        format!(
                            "sound index {sound_index} is outside {table_name}.m_pItemSoundData"
                        ),
                    );
                    entity.blocker_ids.push(blocker);
                    continue;
                };
                for field in [
                    "m_pstrSoundString1",
                    "m_pstrSoundString2",
                    "m_pstrSoundString3",
                ] {
                    builder.add_dependency(
                        &mut entity,
                        sound,
                        field,
                        "sound",
                        "wav",
                        DependencyKind::Audio,
                    );
                }
            }
            builder.entities.push(entity);
        }
    }
}

pub(super) fn count_report(
    builder: &Builder,
    manifest_files: u64,
    content_index_assets: u64,
    cook_mappings: u64,
    logical_plan_ready_roots: u64,
    shared_assets: &[SharedNativeAsset],
    route_scale_usage: &[RouteScaleUsage],
    unresolved_routes: &[UnresolvedRoute],
) -> OrganizerCounts {
    let mut counts = OrganizerCounts {
        manifest_files,
        content_index_assets,
        cook_mappings,
        logical_plan_ready_roots,
        entities: builder.entities.len() as u64,
        model_proposals: builder.models.len() as u64,
        eligible_model_proposals: builder.models.iter().filter(|model| model.eligible).count()
            as u64,
        blocked_model_proposals: builder
            .models
            .iter()
            .filter(|model| !model.eligible)
            .count() as u64,
        shared_native_assets: shared_assets.len() as u64,
        unresolved_routes: unresolved_routes.len() as u64,
        npc_routes_with_multiple_scales: route_scale_usage
            .iter()
            .filter(|usage| usage.multiple_values)
            .count() as u64,
        ..OrganizerCounts::default()
    };
    for entity in &builder.entities {
        match entity.category {
            SemanticCategory::Npc => counts.npc_entities += 1,
            SemanticCategory::Mob => counts.mob_entities += 1,
            SemanticCategory::Hnpc => counts.hnpc_entities += 1,
            SemanticCategory::Nano => counts.nano_entities += 1,
            SemanticCategory::Player => counts.player_entities += 1,
            category if category.is_equipment() => {
                *counts
                    .equipment_entities
                    .entry(category.directory().rsplit('/').next().unwrap().to_owned())
                    .or_default() += 1;
            }
            _ => {}
        }
        for dependency in &entity.dependencies {
            counts.dependencies += 1;
            match dependency.resolution {
                DependencyResolution::Exact => counts.resolved_dependencies += 1,
                DependencyResolution::UniqueNameCandidateUnproven => {
                    counts.name_only_candidates += 1;
                    counts.route_proof_pending += 1;
                }
                DependencyResolution::Ambiguous => counts.ambiguous_dependencies += 1,
                DependencyResolution::Missing | DependencyResolution::EvidenceMismatch => {
                    counts.missing_dependencies += 1;
                }
            }
        }
    }
    counts
}

pub(super) fn alias_base(value: &str, suffixes: &[&str]) -> Option<String> {
    let mut stem = asset_stem(value)?.to_ascii_lowercase();
    for suffix in suffixes {
        if stem.ends_with(suffix) {
            stem.truncate(stem.len().saturating_sub(suffix.len()));
            break;
        }
    }
    for shape in 1..=5 {
        let suffix = format!("_type0{shape}");
        if stem.ends_with(&suffix) {
            stem.truncate(stem.len().saturating_sub(suffix.len()));
            break;
        }
    }
    (!stem.is_empty()).then_some(stem)
}

pub(super) fn clean_value(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"').trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") || value.eq_ignore_ascii_case("none")
    {
        None
    } else {
        Some(value.to_owned())
    }
}

pub(super) fn portable_true_name(name: &str) -> bool {
    if name.trim().is_empty()
        || name != name.trim()
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name);
    !matches!(
        stem.to_ascii_lowercase().as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    )
}

pub(super) fn guard_output(options: &SemanticAssetOrganizerOptions) -> Result<()> {
    if options.output.exists() {
        return Err(PipelineError::OutputExists(options.output.clone()));
    }
    for input in [
        &options.asset_manifest,
        &options.content_index,
        &options.cook_report,
        &options.table_set,
        &options.logical_model_plan,
    ] {
        if same_absolute_path(input, &options.output)? {
            return semantic_error("output report must not alias an input evidence file");
        }
    }
    let Some(asset_root) = options.asset_manifest.parent() else {
        return semantic_error("asset manifest has no parent asset directory");
    };
    let asset_root = absolute_path(asset_root)?;
    let output = absolute_path(&options.output)?;
    if output.starts_with(&asset_root) {
        return semantic_error(format!(
            "plan output must be outside immutable production asset tree {}",
            asset_root.display()
        ));
    }
    Ok(())
}

pub(super) fn value_array(value: Option<&Value>) -> &[Value] {
    value
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(super) fn int_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

pub(super) fn float_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

pub(super) fn u64_pointer(value: &Value, pointer: &str) -> u64 {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

pub(super) fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    let value = value.get(key)?.as_str()?;
    clean_value(value).map(|_| value)
}

pub(super) fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

pub(super) fn normalize_slashes(value: &str) -> String {
    value.replace('\\', "/")
}
