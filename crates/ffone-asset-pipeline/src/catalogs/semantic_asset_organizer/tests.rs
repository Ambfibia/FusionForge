use super::*;
use serde_json::json;

fn entity(id: &str, category: SemanticCategory, routes: &[&str]) -> EntityProposal {
    EntityProposal {
        id: id.to_owned(),
        category,
        semantic_directory: category.directory().to_owned(),
        table_owner: TableOwner {
            table: "test".to_owned(),
            row_index: 0,
            entity_number: None,
            mesh_index: None,
        },
        spawn_root_policy: None,
        model_routes: routes.iter().map(|route| (*route).to_owned()).collect(),
        model_proposal_ids: Vec::new(),
        dependencies: Vec::new(),
        blocker_ids: Vec::new(),
    }
}

fn ready_root(route: &str, true_name: &str) -> ReadyRoot {
    ReadyRoot {
        route: route.to_owned(),
        true_name: true_name.to_owned(),
        proof: "exact-test-proof".to_owned(),
        source_root: SerializedObjectIdentity {
            asset_name: "test.asset".to_owned(),
            path_id: 42,
            object_type: "GameObject".to_owned(),
        },
        feature_closure: ModelFeatureClosure {
            status: ModelFeatureClosureStatus::PreserveExactLogicalRootClosure,
            source_mesh_count: 2,
            source_skinned_mesh_renderer_count: 1,
            source_bone_count: 12,
            source_animation_component_count: 1,
            source_animation_clip_count: 4,
            detail: "test closure".to_owned(),
        },
        material_count: 3,
    }
}

fn model(id: &str, route: &str, output_glb: &str) -> ModelProposal {
    ModelProposal {
        id: id.to_owned(),
        legacy_route: route.to_owned(),
        true_root_m_name: output_glb
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".glb")
            .to_owned(),
        category: SemanticCategory::Npc,
        semantic_directory: "characters/npc/test".to_owned(),
        output_glb: output_glb.to_owned(),
        proof: "test".to_owned(),
        source_root: SerializedObjectIdentity {
            asset_name: "test.asset".to_owned(),
            path_id: 1,
            object_type: "GameObject".to_owned(),
        },
        feature_closure: ModelFeatureClosure {
            status: ModelFeatureClosureStatus::PreserveExactLogicalRootClosure,
            source_mesh_count: 0,
            source_skinned_mesh_renderer_count: 0,
            source_bone_count: 0,
            source_animation_component_count: 0,
            source_animation_clip_count: 0,
            detail: "test".to_owned(),
        },
        materials: ModelMaterialPlan {
            status: ModelMaterialStatus::EmittedWithLogicalModelSidecarPending,
            source_material_count: 0,
            detail: "test".to_owned(),
        },
        entity_ids: Vec::new(),
        eligible: true,
        blocker_ids: Vec::new(),
    }
}

#[test]
fn npc_f_fields_are_hurt_audio_but_equipment_f_fields_are_female_wear() {
    let tables = json!({
        "m_pNpcTable": {
            "m_pNpcData": [{
                "m_iHNpc": 0,
                "m_iTeam": 1,
                "m_iMesh": 1,
                "m_iNpcNumber": 7,
                "m_iNpcType": 0,
                "m_fScale": 1.25
            }],
            "m_pNpcMeshData": [{}, {
                "m_pstrMMeshModelString": "npc_dexter",
                "m_pstrMTextureString": "npc_dexter",
                "m_pstrMTextureString2": "npc_dexter_glass",
                "m_pstrFMeshModelString": "Dexter_Hurt_1",
                "m_pstrFTextureString": "Dexter_Hurt_2.wav",
                "m_pstrFTextureString2": "Dexter_Hurt_3"
            }]
        },
        "m_pHatItemTable": {
            "m_pItemData": [{"m_iItemNumber": 11, "m_iMesh": 1}],
            "m_pItemMeshData": [{}, {
                "m_pstrFMeshModelString": "f_hat_001",
                "m_pstrMMeshModelString": "m_hat_001",
                "m_pstrFTextureString": "f_hat_001",
                "m_pstrFTextureString2": "f_hat_001_detail",
                "m_pstrMTextureString": "m_hat_001",
                "m_pstrMTextureString2": "m_hat_001_detail"
            }],
            "m_pItemSoundData": []
        }
    });
    let mut builder = Builder::default();
    add_npc_entities(&mut builder, &tables);
    let npc = builder
        .entities
        .iter()
        .find(|entry| entry.category == SemanticCategory::Npc)
        .unwrap();
    assert_eq!(npc.model_routes, ["mob/npc_dexter.kfm"]);
    for (field, route) in [
        ("m_pstrFMeshModelString", "sound/dexter_hurt_1.wav"),
        ("m_pstrFTextureString", "sound/dexter_hurt_2.wav"),
        ("m_pstrFTextureString2", "sound/dexter_hurt_3.wav"),
    ] {
        let dependency = npc
            .dependencies
            .iter()
            .find(|dependency| dependency.source_field == field)
            .unwrap();
        assert_eq!(dependency.kind, DependencyKind::Audio);
        assert_eq!(dependency.legacy_route, route);
    }

    add_equipment_entities(&mut builder, &tables);
    let hat = builder
        .entities
        .iter()
        .find(|entry| entry.category == SemanticCategory::EquipmentHat)
        .unwrap();
    assert!(hat.model_routes.contains(&"wear/f_hat_001.nif".to_owned()));
    assert!(hat.model_routes.contains(&"wear/m_hat_001.nif".to_owned()));
    assert_eq!(
        hat.spawn_root_policy.as_ref().unwrap().kind,
        SpawnRootPolicyKind::EquipmentRuntimeAttachmentNotIndependent
    );
    assert!(hat.dependencies.iter().all(|dependency| {
        dependency.kind == DependencyKind::Texture
            && dependency.legacy_route.starts_with("texture/")
            && dependency.legacy_route.ends_with(".dds")
    }));
    assert!(hat.dependencies.iter().any(|dependency| {
        dependency.source_field == "m_pstrFTextureString"
            && dependency.legacy_route == "texture/f_hat_001.dds"
    }));
}

#[test]
fn one_name_candidate_is_fail_closed_without_exact_route_target_proof() {
    let candidate = NativeAssetReference {
        native_key: "texture-key".to_owned(),
        content_path: "native/textures/hero.png".to_owned(),
        project_path: Some("textures/hero.png".to_owned()),
        mapping_occurrences: 1,
    };
    let mut builder = Builder {
        native_index: NativeIndex {
            by_kind_name: BTreeMap::from([(
                ("texture".to_owned(), "hero".to_owned()),
                vec![candidate.clone()],
            )]),
            evidence_issues: Vec::new(),
        },
        ..Builder::default()
    };
    let mut owner = entity("npc:1", SemanticCategory::Npc, &[]);
    builder.add_dependency_route(
        &mut owner,
        "m_pstrMTextureString".to_owned(),
        make_route("texture", "hero", "dds").unwrap(),
        DependencyKind::Texture,
    );
    let dependency = &owner.dependencies[0];
    assert_eq!(
        dependency.resolution,
        DependencyResolution::UniqueNameCandidateUnproven
    );
    assert_eq!(dependency.ownership, NativeOwnership::Unresolved);
    assert_eq!(dependency.candidates, [candidate]);
    assert_eq!(
        builder.blockers[0].code,
        BlockerCode::RouteTargetIdentityUnproven
    );
}

#[test]
fn cook_alias_name_can_join_native_identity_without_becoming_route_proof() {
    let manifest = json!({
        "files": [{
            "source_path": "textures/hero--abcd.png",
            "path": "textures/hero--abcd.png"
        }]
    });
    let content = json!({
        "assets": [{
            "key": "native-key",
            "kind": "texture",
            "name": "CanonicalImportedName",
            "path": "textures/hero--abcd.png"
        }]
    });
    let cook = json!({
        "mappings": [
            {
                "kind": "texture",
                "name": "LegacyTableAlias",
                "nativeKey": "native-key",
                "nativePath": "textures/hero--abcd.png"
            },
            {
                "kind": "localization",
                "name": "Per-source string outside character dependency scope",
                "nativeKey": "intentional-per-source-key",
                "nativePath": "localization/strings.json"
            }
        ]
    });
    let index = build_native_index(&manifest, &content, &cook);
    assert!(index.evidence_issues.is_empty());
    assert!(
        index
            .by_kind_name
            .keys()
            .all(|(kind, _)| matches!(kind.as_str(), "texture" | "audio"))
    );
    assert_eq!(
        index
            .by_kind_name
            .get(&("texture".to_owned(), "LegacyTableAlias".to_owned()))
            .unwrap()
            .len(),
        1
    );

    let mut builder = Builder {
        native_index: index,
        ..Builder::default()
    };
    let mut owner = entity("npc:alias", SemanticCategory::Npc, &[]);
    builder.add_dependency_route(
        &mut owner,
        "m_pstrMTextureString".to_owned(),
        make_route("texture", "LegacyTableAlias", "dds").unwrap(),
        DependencyKind::Texture,
    );
    assert_eq!(
        owner.dependencies[0].resolution,
        DependencyResolution::UniqueNameCandidateUnproven
    );
    assert_eq!(owner.dependencies[0].ownership, NativeOwnership::Unresolved);
}

#[test]
fn shared_route_keeps_exact_true_name_materials_and_each_npc_scale() {
    let route = "mob/npc_shared.kfm";
    let tables = json!({
        "m_pNpcTable": {
            "m_pNpcData": [
                {"m_iHNpc": 0, "m_iTeam": 1, "m_iMesh": 1, "m_iNpcNumber": 1, "m_iNpcType": 100, "m_fScale": 0.75},
                {"m_iHNpc": 0, "m_iTeam": 2, "m_iMesh": 1, "m_iNpcNumber": 2, "m_iNpcType": 100, "m_fScale": 1.25},
                {"m_iHNpc": 1, "m_iTeam": 1, "m_iMesh": 1, "m_iNpcNumber": 3, "m_fScale": 9.0}
            ],
            "m_pNpcMeshData": [{}, {"m_pstrMMeshModelString": "npc_shared"}]
        }
    });
    let mut builder = Builder::default();
    add_npc_entities(&mut builder, &tables);
    builder
        .ready_roots
        .insert(route.to_owned(), vec![ready_root(route, "True Npc Name")]);
    builder.resolve_models();

    let proposal = builder
        .models
        .iter()
        .find(|proposal| proposal.legacy_route == route)
        .unwrap();
    assert_eq!(proposal.category, SemanticCategory::Shared);
    assert_eq!(proposal.true_root_m_name, "True Npc Name");
    assert_eq!(
        proposal.output_glb,
        "characters/shared/npc_shared/True Npc Name.glb"
    );
    assert!(proposal.eligible);
    assert_eq!(proposal.feature_closure.source_bone_count, 12);
    assert_eq!(proposal.feature_closure.source_animation_clip_count, 4);
    assert_eq!(proposal.materials.source_material_count, 3);
    assert_eq!(
        proposal.materials.status,
        ModelMaterialStatus::EmittedWithLogicalModelSidecarPending
    );

    let usage = builder.route_scale_report();
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].values, [0.75, 1.25]);
    assert!(usage[0].multiple_values);
    let hnpc = builder
        .entities
        .iter()
        .find(|entry| entry.category == SemanticCategory::Hnpc)
        .unwrap();
    assert!(hnpc.spawn_root_policy.is_none());
    assert!(hnpc.blocker_ids.iter().any(|id| {
        builder.blockers.iter().any(|blocker| {
            &blocker.id == id && blocker.code == BlockerCode::HnpcPhysicalMappingUnproven
        })
    }));
}

#[test]
fn nfkc_case_collision_blocks_every_conflicting_exact_name() {
    let mut builder = Builder {
        models: vec![
            model("model:a", "mob/a.kfm", "characters/npc/test/Ａ.glb"),
            model("model:b", "mob/b.kfm", "characters/npc/test/A.glb"),
        ],
        ..Builder::default()
    };
    builder.apply_model_path_collision_gate();
    assert!(builder.models.iter().all(|proposal| !proposal.eligible));
    assert_eq!(builder.blockers.len(), 2);
    assert!(
        builder
            .blockers
            .iter()
            .all(|blocker| blocker.code == BlockerCode::NormalizedPathCollision)
    );
}

#[test]
fn unrelated_logical_plan_blockers_are_not_pulled_into_character_scope() {
    let mut builder = Builder {
        entities: vec![entity(
            "npc:referenced",
            SemanticCategory::Npc,
            &["mob/referenced.kfm"],
        )],
        plan_route_blockers: BTreeMap::from([
            (
                "mob/referenced.kfm".to_owned(),
                PlanRouteBlockers {
                    codes: BTreeSet::from(["kfmNotReady".to_owned()]),
                    details: BTreeSet::from(["referenced blocker".to_owned()]),
                },
            ),
            (
                "map/unrelated.nif".to_owned(),
                PlanRouteBlockers {
                    codes: BTreeSet::from(["nifPhysicalTargetConflict".to_owned()]),
                    details: BTreeSet::from(["outside character taxonomy".to_owned()]),
                },
            ),
        ]),
        ..Builder::default()
    };
    builder.resolve_models();
    let unresolved = builder.finish_unresolved();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].legacy_route, "mob/referenced.kfm");
}

#[test]
fn atomic_report_writer_is_create_new_and_never_overwrites() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("semantic-plan.json");
    let report = SemanticAssetOrganizationReport {
        schema: SEMANTIC_ASSET_ORGANIZATION_SCHEMA.to_owned(),
        mode: "plan-only".to_owned(),
        status: "ready".to_owned(),
        production_assets_mutated: false,
        taxonomy: taxonomy(),
        inputs: Vec::new(),
        counts: OrganizerCounts::default(),
        entities: Vec::new(),
        models: Vec::new(),
        route_scale_usage: Vec::new(),
        shared_assets: Vec::new(),
        unresolved_routes: Vec::new(),
        blockers: Vec::new(),
    };
    write_atomic_create_new(&output, &report).unwrap();
    let decoded: SemanticAssetOrganizationReport =
        serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(decoded, report);
    let error = write_atomic_create_new(&output, &report).unwrap_err();
    assert!(matches!(error, PipelineError::OutputExists(path) if path == output));
}
