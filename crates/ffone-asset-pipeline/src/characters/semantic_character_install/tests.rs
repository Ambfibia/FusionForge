use super::*;
use serde_json::json;
use tempfile::tempdir;

fn table_set(npc_rows: Vec<Value>, npc_meshes: Vec<Value>) -> Value {
    json!({
        "schema": TABLE_SET_SCHEMA,
        "tables": [{
            "name": CONSOLIDATED_TABLE,
            "value": {
                "m_pNpcTable": {
                    "m_pNpcData": npc_rows,
                    "m_pNpcMeshData": npc_meshes
                },
                "m_pNanoTable": {
                    "m_pNanoData": [],
                    "m_pNanoMeshData": []
                }
            }
        }]
    })
}

#[test]
fn exact_table_roles_and_authoring_overrides_split_character_taxonomy() {
    let tables = table_set(
        vec![
            json!({"m_iMesh": 1, "m_iNpcNumber": 1, "m_iTeam": 1, "m_iHNpc": 0}),
            json!({"m_iMesh": 2, "m_iNpcNumber": 2, "m_iTeam": 2, "m_iHNpc": 0}),
            json!({"m_iMesh": 3, "m_iNpcNumber": 3, "m_iTeam": 1, "m_iHNpc": 0}),
            json!({"m_iMesh": 3, "m_iNpcNumber": 4, "m_iTeam": 2, "m_iHNpc": 0}),
        ],
        vec![
            json!({}),
            json!({"m_pstrMMeshModelString": "npc_dexter"}),
            json!({"m_pstrMMeshModelString": "mob_bat"}),
            json!({"m_pstrMMeshModelString": "npc_deedee"}),
        ],
    );
    let evidence = exact_character_route_evidence(&tables).unwrap();
    assert_eq!(
        classify_route(
            "mob/npc_dexter.kfm",
            "mob",
            evidence["mob/npc_dexter.kfm"].clone(),
        )
        .category,
        Some(RuntimeCharacterCategory::Npc)
    );
    assert_eq!(
        classify_route(
            "mob/mob_bat.kfm",
            "mob",
            evidence["mob/mob_bat.kfm"].clone(),
        )
        .category,
        Some(RuntimeCharacterCategory::Mob)
    );
    let deedee = classify_route(
        "mob/npc_deedee.kfm",
        "mob",
        evidence["mob/npc_deedee.kfm"].clone(),
    );
    assert_eq!(deedee.category, Some(RuntimeCharacterCategory::Npc));
    assert_eq!(deedee.method, "exact-authoring-npc-route-override");
}

#[test]
fn nano_namespace_is_exact_evidence_without_a_current_table_row() {
    let classification = classify_route("nano/nano_bloo.kfm", "nano", Vec::new());
    assert_eq!(
        classification.category,
        Some(RuntimeCharacterCategory::Nano)
    );
    assert_eq!(classification.method, "exact-nano-kfm-route-namespace");
}

#[test]
fn fusion_namespace_has_its_own_authoring_category() {
    let classification = classify_route("mob/fusion_megawatt.kfm", "mob", Vec::new());
    assert_eq!(
        classification.category,
        Some(RuntimeCharacterCategory::Fusion)
    );
    assert_eq!(classification.method, "exact-fusion-kfm-route-namespace");
}

#[test]
fn unreferenced_mob_namespace_model_remains_unresolved() {
    let classification = classify_route("mob/unreferenced.kfm", "mob", Vec::new());
    assert_eq!(classification.category, None);
    assert_eq!(classification.method, "unresolved-exact-route");
}

#[test]
fn exact_npc_namespace_resolves_only_npc_authoring_routes() {
    let classification = classify_route("mob/npc_bubbles.kfm", "mob", Vec::new());
    assert_eq!(classification.category, Some(RuntimeCharacterCategory::Npc));
    assert_eq!(classification.method, "exact-npc-kfm-route-namespace");

    let hostile = classify_route(
        "mob/npc_hostile.kfm",
        "mob",
        vec![TableReference {
            role: TableRole::Mob,
            row_index: 0,
            entity_number: Some(1),
            mesh_index: 1,
        }],
    );
    assert_eq!(hostile.category, Some(RuntimeCharacterCategory::Mob));
}

#[test]
fn exact_candidate_mapping_binds_route_and_rejects_name_guessing() {
    let mapping = json!({
        "family": "mob",
        "semanticDirectories": ["fusion_ampfibian"],
        "logicalName": "npc_ampfibian",
        "source": "mob/fusion_ampfibian/npc_ampfibian.source.json",
        "outputGlb": "models/mob/fusion_ampfibian/npc_ampfibian.glb"
    });
    assert_eq!(
        exact_route_from_mapping(&mapping).unwrap(),
        (
            "mob/fusion_ampfibian.kfm".to_owned(),
            "fusion_ampfibian".to_owned()
        )
    );
    let mut invalid_mapping = mapping;
    invalid_mapping["outputGlb"] = json!("models/mob/npc_ampfibian/npc_ampfibian.glb");
    assert!(exact_route_from_mapping(&invalid_mapping).is_err());
}

#[test]
fn managed_paths_exclude_player_and_raw_recovery_models() {
    assert!(is_managed_path("characters/npcs/dexter/dexter.glb"));
    assert!(is_managed_path("characters/catalog.json"));
    assert!(!is_managed_path(
        "characters/player/equipment/back/back_bloo.glb"
    ));
    assert!(!is_managed_path("models/alpha--deadbeef.glb"));
}

#[test]
fn registry_uses_requested_plural_runtime_directories() {
    assert_eq!(
        RuntimeCharacterCategory::Nano.directory(),
        "characters/nanos"
    );
    assert_eq!(RuntimeCharacterCategory::Npc.directory(), "characters/npcs");
    assert_eq!(RuntimeCharacterCategory::Mob.directory(), "characters/mobs");
    assert_eq!(
        RuntimeCharacterCategory::Fusion.directory(),
        "characters/fusions"
    );
    assert_eq!(
        RuntimeCharacterCategory::Shared.directory(),
        "characters/shared"
    );
}

#[test]
fn every_runtime_character_category_is_committed_from_staging() {
    for category in [
        RuntimeCharacterCategory::Nano,
        RuntimeCharacterCategory::Npc,
        RuntimeCharacterCategory::Mob,
        RuntimeCharacterCategory::Fusion,
        RuntimeCharacterCategory::Shared,
    ] {
        assert!(NEW_TARGETS.contains(&category.directory()));
    }
}

#[test]
fn external_promoted_character_packages_are_preserved_fail_closed() {
    let temp = tempdir().unwrap();
    let asset_root = temp.path().join("assets");
    let stage = temp.path().join("stage");
    fs::create_dir_all(&stage).unwrap();

    let glb_path = "characters/fusions/fusion_buttercup/fusion_buttercup.glb";
    let glb_bytes = b"promoted-fusion";
    let glb_blake3 = blake3::hash(glb_bytes).to_hex().to_string();
    let glb_disk = join_relative(&asset_root, glb_path).unwrap();
    create_parent(&glb_disk).unwrap();
    fs::write(&glb_disk, glb_bytes).unwrap();

    let model = RuntimeCharacterModel {
        id: "fusion/fusion_buttercup".to_owned(),
        logical_name: "fusion_buttercup".to_owned(),
        legacy_aliases: Vec::new(),
        category: RuntimeCharacterCategory::Fusion,
        glb: glb_path.to_owned(),
        glb_blake3: glb_blake3.clone(),
        collision: None,
        animations: vec!["stand1".to_owned()],
    };
    let registry = SemanticCharacterRegistry {
        schema: SEMANTIC_CHARACTER_REGISTRY_SCHEMA.to_owned(),
        models: vec![model.clone()],
    };
    let registry_bytes = pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH).unwrap();
    let registry_disk = join_relative(&asset_root, SEMANTIC_CHARACTER_REGISTRY_PATH).unwrap();
    create_parent(&registry_disk).unwrap();
    fs::write(&registry_disk, &registry_bytes).unwrap();

    let manifest: ProjectAssetManifest = serde_json::from_value(json!({
        "schema": PROJECT_ASSET_SCHEMA,
        "protocol": 1,
        "locale": "test",
        "source_pack": {
            "schema": "fixture",
            "manifest_blake3": "0".repeat(64)
        },
        "files": [
            {
                "source_path": "tutorial-character-promotion/fusion_buttercup.glb",
                "path": glb_path,
                "kind": "model",
                "bytes": glb_bytes.len(),
                "blake3": glb_blake3
            },
            {
                "source_path": format!("{SEMANTIC_CHARACTER_SOURCE_PREFIX}registry.json"),
                "path": SEMANTIC_CHARACTER_REGISTRY_PATH,
                "kind": "data",
                "bytes": registry_bytes.len(),
                "blake3": blake3::hash(&registry_bytes).to_hex().to_string()
            }
        ]
    }))
    .unwrap();

    let preserved = plan_preserved_character_content(&asset_root, &manifest).unwrap();
    assert_eq!(preserved.entries.len(), 1);
    assert_eq!(preserved.entries[0].path, glb_path);
    assert_eq!(preserved.models, vec![model]);
    stage_preserved_character_content(&asset_root, &stage, &preserved).unwrap();
    assert_eq!(
        fs::read(join_relative(&stage, glb_path).unwrap()).unwrap(),
        glb_bytes
    );

    fs::write(&glb_disk, b"drifted").unwrap();
    assert!(stage_preserved_character_content(&asset_root, &stage, &preserved).is_err());
}

fn transaction_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let temp = tempdir().unwrap();
    let asset_root = temp.path().join("assets");
    let stage = temp.path().join("stage");
    let report = temp.path().join("generated").join("report.json");
    fs::create_dir_all(asset_root.join("characters/mob/old")).unwrap();
    fs::create_dir_all(asset_root.join("characters/player")).unwrap();
    fs::create_dir_all(asset_root.join("models")).unwrap();
    fs::create_dir_all(report.parent().unwrap()).unwrap();
    fs::write(asset_root.join("characters/mob/old/old.glb"), b"old").unwrap();
    fs::write(asset_root.join("characters/player/player.glb"), b"player").unwrap();
    fs::write(asset_root.join("models/raw--hash.glb"), b"raw").unwrap();
    fs::write(asset_root.join(ASSET_MANIFEST_FILE), b"old-manifest").unwrap();
    for target in NEW_TARGETS {
        let path = join_relative(&stage, target).unwrap();
        if *target == SEMANTIC_CHARACTER_REGISTRY_PATH {
            create_parent(&path).unwrap();
            fs::write(path, b"registry").unwrap();
        } else {
            fs::create_dir_all(path).unwrap();
        }
    }
    fs::create_dir_all(stage.join("characters/mobs/new")).unwrap();
    fs::write(stage.join("characters/mobs/new/new.glb"), b"new").unwrap();
    fs::create_dir_all(stage.join("characters/fusions/fusion_megawatt")).unwrap();
    fs::write(
        stage.join("characters/fusions/fusion_megawatt/npc_megawhatt.glb"),
        b"fusion",
    )
    .unwrap();
    (temp, asset_root, stage, report)
}

#[test]
fn transaction_commits_semantic_targets_without_touching_player_or_raw_models() {
    let (_temp, asset_root, stage, report) = transaction_fixture();
    commit_transaction(&asset_root, &stage, b"new-manifest", &report, b"report").unwrap();
    assert!(!asset_root.join("characters/mob").exists());
    assert_eq!(
        fs::read(asset_root.join("characters/mobs/new/new.glb")).unwrap(),
        b"new"
    );
    assert_eq!(
        fs::read(asset_root.join("characters/fusions/fusion_megawatt/npc_megawhatt.glb"))
            .unwrap(),
        b"fusion"
    );
    assert_eq!(
        fs::read(asset_root.join("characters/player/player.glb")).unwrap(),
        b"player"
    );
    assert_eq!(
        fs::read(asset_root.join("models/raw--hash.glb")).unwrap(),
        b"raw"
    );
    assert_eq!(
        fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap(),
        b"new-manifest"
    );
    assert_eq!(fs::read(report).unwrap(), b"report");
}

#[test]
fn late_report_failure_rolls_back_manifest_and_semantic_targets() {
    let (_temp, asset_root, stage, report) = transaction_fixture();
    fs::create_dir(&report).unwrap();
    assert!(
        commit_transaction(&asset_root, &stage, b"new-manifest", &report, b"report").is_err()
    );
    assert_eq!(
        fs::read(asset_root.join("characters/mob/old/old.glb")).unwrap(),
        b"old"
    );
    assert!(!asset_root.join("characters/mobs").exists());
    assert!(!asset_root.join("characters/fusions").exists());
    assert_eq!(
        fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap(),
        b"old-manifest"
    );
    assert_eq!(
        fs::read(asset_root.join("models/raw--hash.glb")).unwrap(),
        b"raw"
    );
}

#[test]
fn manifestless_catalog_proves_the_existing_character_package_closure() {
    let temp = tempdir().unwrap();
    let asset_root = temp.path().join("assets");
    let glb = "characters/fusions/fusion_existing/fusion_existing.glb";
    let glb_bytes = b"existing-glb";
    let glb_path = join_relative(&asset_root, glb).unwrap();
    create_parent(&glb_path).unwrap();
    fs::write(&glb_path, glb_bytes).unwrap();
    let texture = join_relative(
        &asset_root,
        "characters/fusions/fusion_existing/fusion_existing.textures/main.png",
    )
    .unwrap();
    create_parent(&texture).unwrap();
    fs::write(&texture, b"png").unwrap();
    let external_texture = join_relative(
        &asset_root,
        "characters/shared/runtime-textures/spawn11_green.png",
    )
    .unwrap();
    create_parent(&external_texture).unwrap();
    fs::write(&external_texture, b"external-png").unwrap();
    let collision_descriptor =
        "characters/fusions/fusion_existing/fusion_existing.collision.json";
    let collider_glb = "characters/fusions/fusion_existing/fusion_existing.collision.glb";
    let collider_bytes = b"collider-glb";
    let collider_path = join_relative(&asset_root, collider_glb).unwrap();
    fs::write(&collider_path, collider_bytes).unwrap();
    let collision_descriptor_path = join_relative(&asset_root, collision_descriptor).unwrap();
    fs::write(
        &collision_descriptor_path,
        serde_json::to_vec_pretty(&json!({
            "schema": "ffone.native-character-collision.v1",
            "glb": glb,
            "glbBlake3": blake3::hash(glb_bytes).to_hex().to_string(),
            "colliderGlb": collider_glb,
            "colliderGlbBlake3": blake3::hash(collider_bytes).to_hex().to_string(),
        }))
        .unwrap(),
    )
    .unwrap();
    let registry = SemanticCharacterRegistry {
        schema: SEMANTIC_CHARACTER_REGISTRY_SCHEMA.to_owned(),
        models: vec![RuntimeCharacterModel {
            id: "fusion/fusion_existing".to_owned(),
            logical_name: "fusion_existing".to_owned(),
            legacy_aliases: Vec::new(),
            category: RuntimeCharacterCategory::Fusion,
            glb: glb.to_owned(),
            glb_blake3: blake3::hash(glb_bytes).to_hex().to_string(),
            collision: Some(collision_descriptor.to_owned()),
            animations: vec!["stand1".to_owned()],
        }],
    };
    let registry_path = join_relative(&asset_root, SEMANTIC_CHARACTER_REGISTRY_PATH).unwrap();
    create_parent(&registry_path).unwrap();
    fs::write(
        &registry_path,
        pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH).unwrap(),
    )
    .unwrap();

    let preserved = plan_manifestless_character_content(&asset_root).unwrap();
    assert_eq!(preserved.models, registry.models);
    assert_eq!(preserved.entries.len(), 4);
    assert!(preserved.entries.iter().any(|entry| entry.path == glb));
    let unsupported = external_texture.with_extension("txt");
    fs::write(&unsupported, b"not-a-texture").unwrap();
    assert!(plan_manifestless_character_content(&asset_root).is_err());
    fs::remove_file(unsupported).unwrap();
    fs::write(&collider_path, b"drifted-collider").unwrap();
    assert!(plan_manifestless_character_content(&asset_root).is_err());
    fs::write(&collider_path, collider_bytes).unwrap();
    fs::remove_file(&collider_path).unwrap();
    fs::write(
        &collision_descriptor_path,
        serde_json::to_vec_pretty(&json!({
            "schema": "ffone.native-character-collision.v1",
            "glb": glb,
            "glbBlake3": blake3::hash(glb_bytes).to_hex().to_string(),
        }))
        .unwrap(),
    )
    .unwrap();
    let embedded_collision = plan_manifestless_character_content(&asset_root).unwrap();
    assert_eq!(embedded_collision.entries.len(), 3);

    fs::write(&glb_path, b"drifted").unwrap();
    assert!(plan_manifestless_character_content(&asset_root).is_err());
}

fn manifestless_transaction_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let temp = tempdir().unwrap();
    let asset_root = temp.path().join("assets");
    let stage = temp.path().join("stage");
    let report = temp.path().join("generated/report.json");
    fs::create_dir_all(asset_root.join("characters/fusions/fusion_existing")).unwrap();
    fs::write(
        asset_root.join("characters/fusions/fusion_existing/existing.glb"),
        b"existing",
    )
    .unwrap();
    fs::create_dir_all(asset_root.join("_runtime")).unwrap();
    fs::write(
        asset_root.join(SEMANTIC_CHARACTER_REGISTRY_PATH),
        b"old-registry",
    )
    .unwrap();
    fs::create_dir_all(stage.join("characters/fusions/fusion_new")).unwrap();
    fs::write(stage.join("characters/fusions/fusion_new/new.glb"), b"new").unwrap();
    fs::create_dir_all(stage.join("_runtime")).unwrap();
    fs::write(
        stage.join(SEMANTIC_CHARACTER_REGISTRY_PATH),
        b"new-registry",
    )
    .unwrap();
    fs::create_dir_all(report.parent().unwrap()).unwrap();
    (temp, asset_root, stage, report)
}

#[test]
fn manifestless_transaction_adds_only_new_packages_and_swaps_the_domain_catalog() {
    let (_temp, asset_root, stage, report) = manifestless_transaction_fixture();
    let packages = BTreeSet::from(["characters/fusions/fusion_new".to_owned()]);
    commit_manifestless_transaction(
        &asset_root,
        &stage,
        &packages,
        &BTreeSet::new(),
        &report,
        b"report",
    )
    .unwrap();
    assert_eq!(
        fs::read(asset_root.join("characters/fusions/fusion_existing/existing.glb")).unwrap(),
        b"existing"
    );
    assert_eq!(
        fs::read(asset_root.join("characters/fusions/fusion_new/new.glb")).unwrap(),
        b"new"
    );
    assert_eq!(
        fs::read(asset_root.join(SEMANTIC_CHARACTER_REGISTRY_PATH)).unwrap(),
        b"new-registry"
    );
    assert!(!asset_root.join(ASSET_MANIFEST_FILE).exists());
    assert_eq!(fs::read(report).unwrap(), b"report");
}

#[test]
fn manifestless_transaction_atomically_replaces_an_explicit_package() {
    let (_temp, asset_root, stage, report) = manifestless_transaction_fixture();
    let replacement = stage.join("characters/fusions/fusion_existing");
    fs::create_dir_all(&replacement).unwrap();
    fs::write(replacement.join("existing.glb"), b"replacement").unwrap();
    let packages = BTreeSet::from(["characters/fusions/fusion_existing".to_owned()]);
    commit_manifestless_transaction(
        &asset_root,
        &stage,
        &packages,
        &packages,
        &report,
        b"replacement-report",
    )
    .unwrap();
    assert_eq!(
        fs::read(asset_root.join("characters/fusions/fusion_existing/existing.glb")).unwrap(),
        b"replacement"
    );
    assert_eq!(
        fs::read(asset_root.join(SEMANTIC_CHARACTER_REGISTRY_PATH)).unwrap(),
        b"new-registry"
    );
    assert_eq!(fs::read(report).unwrap(), b"replacement-report");
}

#[test]
fn manifestless_replacement_failure_restores_the_previous_package() {
    let (_temp, asset_root, stage, report) = manifestless_transaction_fixture();
    let replacement = stage.join("characters/fusions/fusion_existing");
    fs::create_dir_all(&replacement).unwrap();
    fs::write(replacement.join("existing.glb"), b"replacement").unwrap();
    fs::create_dir(&report).unwrap();
    let packages = BTreeSet::from(["characters/fusions/fusion_existing".to_owned()]);
    assert!(
        commit_manifestless_transaction(
            &asset_root,
            &stage,
            &packages,
            &packages,
            &report,
            b"replacement-report",
        )
        .is_err()
    );
    assert_eq!(
        fs::read(asset_root.join("characters/fusions/fusion_existing/existing.glb")).unwrap(),
        b"existing"
    );
    assert_eq!(
        fs::read(asset_root.join(SEMANTIC_CHARACTER_REGISTRY_PATH)).unwrap(),
        b"old-registry"
    );
}

#[test]
fn manifestless_late_report_failure_rolls_back_packages_and_catalog() {
    let (_temp, asset_root, stage, report) = manifestless_transaction_fixture();
    fs::create_dir(&report).unwrap();
    let packages = BTreeSet::from(["characters/fusions/fusion_new".to_owned()]);
    assert!(
        commit_manifestless_transaction(
            &asset_root,
            &stage,
            &packages,
            &BTreeSet::new(),
            &report,
            b"report",
        )
        .is_err()
    );
    assert!(!asset_root.join("characters/fusions/fusion_new").exists());
    assert_eq!(
        fs::read(asset_root.join(SEMANTIC_CHARACTER_REGISTRY_PATH)).unwrap(),
        b"old-registry"
    );
    assert_eq!(
        fs::read(asset_root.join("characters/fusions/fusion_existing/existing.glb")).unwrap(),
        b"existing"
    );
}
