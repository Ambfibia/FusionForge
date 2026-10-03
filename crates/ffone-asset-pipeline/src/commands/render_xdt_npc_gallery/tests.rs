use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn package_texture_scan_is_recursive_and_png_only() {
    let root = tempfile::tempdir().expect("create isolated asset root");
    let package = root
        .path()
        .join("characters/mobs/mob_sneakyspawn/mob_sneakyspawn.textures");
    fs::create_dir_all(&package).expect("create character package");
    let png = package.join("spawn11_green.png");
    fs::write(&png, b"png bytes").expect("write package PNG");
    fs::write(package.join("ignored.json"), b"{}").expect("write non-PNG");

    assert_eq!(
        collect_runtime_texture_files(root.path()).expect("scan package textures"),
        vec![png]
    );
}

#[test]
fn null_model_and_texture_names_are_not_routes() {
    assert_eq!(model_name(Some(&json!("null"))), None);
    assert_eq!(model_name(Some(&json!("  "))), None);
    assert_eq!(
        model_name(Some(&json!("npc_Dexter"))),
        Some("npc_Dexter".to_owned())
    );
}

#[test]
fn animation_selection_prefers_stand_then_nif_default() {
    assert_eq!(
        select_animation(&["walk".to_owned(), "stand1".to_owned()]),
        Some("stand1".to_owned())
    );
    assert_eq!(
        select_animation(&["nif-default".to_owned(), "walk".to_owned()]),
        Some("nif-default".to_owned())
    );
    assert_eq!(select_animation(&[]), None);
}

#[test]
fn registry_output_paths_reject_parent_components() {
    assert_eq!(
        checked_registry_output_base("npc/npc_dexter").unwrap(),
        PathBuf::from("npc/npc_dexter")
    );
    assert!(checked_registry_output_base("npc/../escape").is_err());
}

#[test]
fn legacy_routes_are_case_normalized() {
    assert_eq!(
        normalized_route("mob_QueenSpider"),
        "mob/mob_queenspider.kfm"
    );
}

#[test]
fn later_source_and_batch_blockers_override_plan_ready_routes() {
    let mut evidence = LegacyEvidence::default();
    evidence.ready.insert("mob/npc_ed.kfm".to_owned());
    evidence.ready.insert("mob/npc_spidermonkey.kfm".to_owned());
    apply_supplemental_blockers(
        &json!({
            "blocked": [{
                "exactRoute": "mob/npc_ed.kfm",
                "code": "sourceStageExportFailed"
            }],
            "blockers": [{
                "logicalName": "npc_spidermonkey",
                "code": "invalidSerializedTextureSlotName"
            }]
        }),
        &mut evidence,
    );

    assert!(evidence.ready.is_empty());
    assert!(evidence.blockers["mob/npc_ed.kfm"].contains("sourceStageExportFailed"));
    assert!(
        evidence.blockers["mob/npc_spidermonkey.kfm"]
            .contains("invalidSerializedTextureSlotName")
    );
}

#[test]
fn objectnpc1_is_not_presented_as_the_named_world_visual() {
    assert!(is_primary_interaction_placeholder("ObjectNPC1"));
    assert!(is_primary_interaction_placeholder("objectnpc1"));
    // Clean primary resolves mob/rxcom.kfm to the inactive Recall Point
    // controller GameObject `RXcom`. Its child `Box` has a MeshFilter but
    // no MeshRenderer, so the shared preload's three character meshes are
    // unrelated dependencies rather than the Recall Point visual.
    assert!(is_primary_interaction_placeholder("RXcom"));
    assert!(!is_primary_interaction_placeholder("npc_dexter"));
}

#[test]
fn appearance_identity_includes_both_xdt_texture_slots() {
    let model = RuntimeModel {
        id: "mob/mob_spawn".to_owned(),
        logical_name: "mob_spawn".to_owned(),
        legacy_aliases: Vec::new(),
        category: "mob".to_owned(),
        glb: "characters/mobs/mob_spawn/mob_spawn.glb".to_owned(),
        glb_blake3: "f8ead7cef9e25441e9a81d65fa3bb286148007e1cd9873ab86cd82176d4f3cad"
            .to_owned(),
        animations: vec!["stand1".to_owned()],
    };
    assert_ne!(
        appearance_key(
            &model,
            Some("map/shared/effects/textures/spawn11_green/primary.png"),
            Some("characters/mobs/mob_gasspawn/mob_gasspawn.textures/mob_gasspawn.png"),
            None,
            None,
        )
        .unwrap(),
        appearance_key(&model, None, None, None, None).unwrap()
    );
}

#[test]
fn appearance_identity_changes_when_the_published_glb_changes() {
    let mut model = RuntimeModel {
        id: "npc/npc_courage".to_owned(),
        logical_name: "npc_courage".to_owned(),
        legacy_aliases: Vec::new(),
        category: "npc".to_owned(),
        glb: "characters/npcs/npc_courage/npc_courage.glb".to_owned(),
        glb_blake3: "8d35f0ed8469b209c20b1307fff0778810c23b5f23833e6e464ce430634bdd60"
            .to_owned(),
        animations: vec!["stand1".to_owned()],
    };
    let stale_key = appearance_key(&model, None, None, None, None).unwrap();
    model.glb_blake3 =
        "58a7c16ffdb7601c5018f862ec2964f385d9c06a777efaf4fc57acdc6b1e18df".to_owned();
    let republished_key = appearance_key(&model, None, None, None, None).unwrap();

    assert_ne!(stale_key, republished_key);
}

#[test]
fn spawn11_green_uses_the_primary_tutorial_selection() {
    let candidates = vec![
        "map/shared/effects/textures/spawn11_green/primary.png".to_owned(),
        "map/shared/effects/textures/spawn11_green/alternate_01.png".to_owned(),
    ];
    let (path, status) =
        resolve_runtime_texture_path("spawn11_green", &candidates, &candidates);
    assert_eq!(status, "primary_selected");
    assert_eq!(
        path.as_deref(),
        Some("map/shared/effects/textures/spawn11_green/primary.png")
    );
}

#[test]
fn source_hash_selection_prefers_the_canonical_true_name_over_identical_aliases() {
    let candidates = vec![
        "textures/npc_3460_spawn11_green--867af776a8398f20.png".to_owned(),
        "map/shared/effects/textures/spawn11_green/primary.png".to_owned(),
        "map/shared/effects/textures/spawn11_green/alternate_01.png".to_owned(),
    ];
    let (path, status) =
        resolve_runtime_texture_path("spawn11_green", &candidates, &candidates);
    assert_eq!(status, "primary_selected");
    assert_eq!(
        path.as_deref(),
        Some("map/shared/effects/textures/spawn11_green/primary.png")
    );
}

#[test]
fn identical_cross_bundle_true_name_sources_are_not_ambiguous() {
    let source = SourceTextureMetadata {
        true_name: "mob_bat".to_owned(),
        path_id: 4869,
        container_routes: vec!["texture/vehicle_bat.dds".to_owned()],
        source_chain_sha256: "d960464c22e196e7664f1461a8c3699955df2d8a9216542fb18f2cbe3db6b1e6"
            .to_owned(),
        native_png_blake3: "db1b7125b00a61fe0edad0958a608161b44476c4ae4dd9e7bb1de6c6861fabc4"
            .to_owned(),
        mip_map: true,
        source_mip_count: 9,
        filter_mode: 1,
        wrap_mode: 0,
        anisotropy_level: 1,
        mip_map_bias: 0.0,
    };
    let mut catalog = RuntimeTextureCatalog::default();
    catalog.by_true_name.insert(
        "mob_bat".to_owned(),
        vec!["characters/mobs/mob_bat/mob_bat.textures/mob_bat.png".to_owned()],
    );
    catalog
        .samplers_by_true_name
        .insert("mob_bat".to_owned(), vec![source.clone(), source]);

    let resolution = catalog.resolve("mob_bat");
    assert_eq!(
        resolution.sampler_status,
        "verified_identical_true_name_sources"
    );
    assert_eq!(
        resolution.path.as_deref(),
        Some("characters/mobs/mob_bat/mob_bat.textures/mob_bat.png")
    );
    assert!(resolution.sampler.is_some());
}

#[test]
fn xdt_texture_route_resolves_when_texture_object_true_name_differs() {
    let source = SourceTextureMetadata {
        true_name: "sheetmusicninja".to_owned(),
        path_id: 489,
        container_routes: vec!["texture/mob_musicsheetninja.dds".to_owned()],
        source_chain_sha256: "07c78c39e42db05948f7539a6fd36b1507d75e3aa7c22f14bd3f591e91f90283"
            .to_owned(),
        native_png_blake3: "d04ef62df8bf08c6b6727cadd2a8e705b90c815dc111d62a32edc1c34e3e4a04"
            .to_owned(),
        mip_map: false,
        source_mip_count: 1,
        filter_mode: 1,
        wrap_mode: 0,
        anisotropy_level: 1,
        mip_map_bias: 0.0,
    };
    let mut catalog = RuntimeTextureCatalog::default();
    catalog.by_blake3_prefix.insert(
        "d04ef62df8bf08c6".to_owned(),
        vec!["characters/shared/runtime-textures/sheetmusicninja.png".to_owned()],
    );
    catalog
        .samplers_by_container_route
        .insert("texture/mob_musicsheetninja.dds".to_owned(), vec![source]);

    let resolution = catalog.resolve("mob_musicsheetninja");
    assert_eq!(resolution.sampler_status, "verified_exact_container_route");
    assert_eq!(
        resolution.path.as_deref(),
        Some("characters/shared/runtime-textures/sheetmusicninja.png")
    );
    assert!(resolution.sampler.is_some());
}

#[test]
fn legacy_textures_asset_route_resolves_when_true_name_differs() {
    let source = SourceTextureMetadata {
        true_name: "EX_EXMN_Kevin_Car_01.dds".to_owned(),
        path_id: 1794,
        container_routes: vec!["textures/npc_kevincar.dds.asset".to_owned()],
        source_chain_sha256: "4ed2a4d1b4b14c9554c45028a3d5bb74b0e116143d150b2ee46509d50a36bdbb"
            .to_owned(),
        native_png_blake3: "9cfa57f2f5e5fdbb4aabcfced17e719e514d7edd5fbe6ba5dd1557efb3a53c0c"
            .to_owned(),
        mip_map: true,
        source_mip_count: 10,
        filter_mode: 1,
        wrap_mode: 0,
        anisotropy_level: 1,
        mip_map_bias: 0.0,
    };
    let mut catalog = RuntimeTextureCatalog::default();
    catalog.by_blake3_prefix.insert(
        "9cfa57f2f5e5fdbb".to_owned(),
        vec!["textures/EX_EXMN_Kevin_Car_01.dds--9cfa57f2f5e5fdbb.png".to_owned()],
    );
    catalog
        .samplers_by_container_route
        .insert("textures/npc_kevincar.dds.asset".to_owned(), vec![source]);

    let resolution = catalog.resolve("npc_kevincar");
    assert_eq!(resolution.sampler_status, "verified_exact_container_route");
    assert_eq!(
        resolution.path.as_deref(),
        Some("textures/EX_EXMN_Kevin_Car_01.dds--9cfa57f2f5e5fdbb.png")
    );
}

#[test]
fn exact_true_name_owner_wins_over_a_differing_route_alias() {
    let direct = SourceTextureMetadata {
        true_name: "npc_flapjack".to_owned(),
        path_id: 326,
        container_routes: vec!["texture/npc_flapjack.dds".to_owned()],
        source_chain_sha256: "bafde5cd970db8356b17bbb5147a38d1ced55485e6c09af200ce4b5223706593"
            .to_owned(),
        native_png_blake3: "23e285fbb39a928fa27308081bf080f6fa9ee94736e8a731a10e41dbc08392cd"
            .to_owned(),
        mip_map: false,
        source_mip_count: 1,
        filter_mode: 1,
        wrap_mode: 0,
        anisotropy_level: 1,
        mip_map_bias: 0.0,
    };
    let mut alias = direct.clone();
    alias.true_name = "npc_flapjack.dds".to_owned();
    alias.path_id = 4446;
    alias.source_chain_sha256 =
        "81c0adb8863d48578c911d2b9fc97f429094bea1148802f2d9d665f3edb84f86".to_owned();
    alias.native_png_blake3 =
        "d8257689d09d0522457de5dc2a55f6163ac97f43715dabcabfccd54130696b28".to_owned();

    let mut catalog = RuntimeTextureCatalog::default();
    catalog.by_blake3_prefix.insert(
        "23e285fbb39a928f".to_owned(),
        vec!["characters/shared/runtime-textures/npc_flapjack.png".to_owned()],
    );
    catalog
        .samplers_by_true_name
        .insert("npc_flapjack".to_owned(), vec![direct.clone()]);
    catalog
        .samplers_by_container_route
        .insert("texture/npc_flapjack.dds".to_owned(), vec![direct, alias]);

    let resolution = catalog.resolve("npc_flapjack");
    assert_eq!(resolution.sampler_status, "verified_exact_true_name_source");
    assert_eq!(
        resolution.path.as_deref(),
        Some("characters/shared/runtime-textures/npc_flapjack.png")
    );
}

#[test]
fn resume_replaces_stale_entity_placeholder_links() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "ffone-npc-gallery-resume-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create isolated test directory");
    let source = root.join("appearance.png");
    let destination = root.join("entity.png");
    fs::write(&source, b"new appearance").expect("write source");
    fs::write(&destination, b"stale placeholder").expect("write stale destination");

    link_or_copy(&source, &destination, true).expect("resume should replace destination");
    assert_eq!(fs::read(&destination).unwrap(), b"new appearance");

    fs::remove_dir_all(&root).expect("remove isolated test directory");
}
