use super::*;

#[test]
fn npc_augmented_sources_keep_materialized_asset_dirs() {
    let temp = native_build_temp_dir("npc_materialized_assets").expect("temp dir");
    let project = temp.path();
    let mut blueprint = NpcBlueprint {
        npc_id: 3430,
        template_npc_id: Some(3430),
        name: "Rex".to_string(),
        internal_name: "Rex".to_string(),
        comment: None,
        comment1: None,
        greeting_name: None,
        greeting_comment: None,
        greeting_comment1: None,
        greeting_key: None,
        authoring_model_path: None,
        model_bundle: None,
        model_asset: Some("mob/npc_rex.kfm".to_string()),
        texture_bundle: None,
        texture_asset: Some("texture/npc_rex.dds".to_string()),
        icon_bundle: None,
        icon_asset: Some("icons/npcicon_3430.png".to_string()),
        audio_source: None,
        audio_prefix: None,
        animation_set: None,
        generated_icon: None,
        profile: BTreeMap::new(),
        spawn_map: None,
        spawn_position: None,
        spawn_angle: None,
        spawn_json_id: None,
        notes: None,
    };
    let materialized = npc_build_assets_root(project, &blueprint).join("Character_Rex");
    fs::create_dir_all(&materialized).expect("asset dir");
    blueprint.model_bundle = Some(
        path_relative_to_project(project, &materialized).expect("relative materialized path"),
    );
    let external = project
        .join("source-build")
        .join("Character_Rex.resourceFile");
    let context = NpcImportManifestContext {
        source_table_data_bundle: Some(
            project.join("source-build").join("TableData.resourceFile"),
        ),
        source_asset: Some("CustomAssetBundle-TableData".to_string()),
        source_path_id: Some(7),
        source_npc_id: Some(3430),
        table_data_patch_path: None,
        source_paths: vec![materialized.clone()],
    };

    let sources =
        npc_augmented_source_bundles(project, &blueprint, &context, &NpcAssetHints::default());

    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0]
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase(),
        materialized
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase()
    );
    let external_key = external
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    assert!(!sources.iter().any(|path| {
        path.to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase()
            == external_key
    }));
}

#[test]
fn ensure_icon_manifest_supports_builtin_npc_without_existing_import() {
    let temp = native_build_temp_dir("npc_builtin_icon_manifest").expect("temp dir");
    let project = &temp.root;
    let tabledata_dir = project.join("tabledata");
    fs::create_dir_all(&tabledata_dir).expect("tabledata dir");
    let bundle = project.join("source").join("TableData.resourceFile");
    let asset = "CustomAssetBundle-TableData";
    let path_id = 7;
    let relative_patch = "TableData/CustomAssetBundle-TableData/7__npc.json";
    let patch_path = tabledata_dir.join(relative_patch.replace('/', "\\"));
    fs::create_dir_all(patch_path.parent().expect("patch parent")).expect("patch parent dir");
    let patch = json!({
        "format": "fftools.tabledata-object.v1",
        "sourceBundle": bundle.to_string_lossy(),
        "asset": asset,
        "pathId": path_id,
        "value": {
            "m_pNpcTable": {
                "m_pNpcData": [
                    {},
                    { "m_iTeam": 1, "m_iIcon1": 1 }
                ],
                "m_pNpcIconData": [
                    {},
                    { "m_iIconType": 4, "m_iIconNumber": 70 }
                ]
            }
        }
    });
    fs::write(
        &patch_path,
        serde_json::to_string_pretty(&patch).expect("patch json"),
    )
    .expect("write patch");
    let tabledata_manifest = json!({
        "format": "fftools.tabledata-patch.v1",
        "entries": [{
            "container": container_name_from_bundle(&bundle),
            "asset": asset,
            "pathId": path_id,
            "name": "npc_icon",
            "file": relative_patch,
        }]
    });
    fs::write(
        tabledata_dir.join("manifest.json"),
        serde_json::to_string_pretty(&tabledata_manifest).expect("manifest json"),
    )
    .expect("write tabledata manifest");
    let blueprint = NpcBlueprint {
        npc_id: 1,
        template_npc_id: None,
        name: "Built In".to_string(),
        internal_name: "built_in".to_string(),
        comment: None,
        comment1: None,
        greeting_name: None,
        greeting_comment: None,
        greeting_comment1: None,
        greeting_key: None,
        authoring_model_path: None,
        model_bundle: Some("Character_BuiltIn.resourceFile".to_string()),
        model_asset: Some("mob/built_in.kfm".to_string()),
        texture_bundle: Some("Character_Texture.resourceFile".to_string()),
        texture_asset: Some("texture/built_in.dds".to_string()),
        icon_bundle: Some("Icons.resourceFile".to_string()),
        icon_asset: Some("icons/npcicon_70.png".to_string()),
        audio_source: None,
        audio_prefix: None,
        animation_set: None,
        generated_icon: None,
        profile: BTreeMap::new(),
        spawn_map: None,
        spawn_position: None,
        spawn_angle: None,
        spawn_json_id: None,
        notes: None,
    };

    let manifest_path = ensure_staged_npc_icon_manifest(
        project.to_string_lossy().to_string(),
        bundle.to_string_lossy().to_string(),
        asset.to_string(),
        path_id,
        1,
        blueprint,
    )
    .expect("ensure icon manifest");
    let manifest: JsonValue =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read icon manifest"))
            .expect("parse icon manifest");
    assert_eq!(
        manifest.get("bundleStrategy"),
        Some(&json!("build-time-icon-resource-bundle"))
    );
    assert_eq!(
        manifest.pointer("/blueprint/modelAsset"),
        Some(&JsonValue::Null)
    );
    assert_eq!(
        manifest.pointer("/resources/iconTexture/asset"),
        Some(&json!("icons/npcicon_70.png"))
    );
    let plan =
        plan_staged_npc_generated_icon(project.to_string_lossy().to_string(), 1, Some(4))
            .expect("plan built-in icon");
    assert_eq!(plan.asset_path, "icons/npcicon_01.png");
    assert_eq!(plan.icon_row_index, 1);
}

#[test]
fn staged_npc_edit_retargets_import_manifest_to_replacement_patch() {
    let temp = native_build_temp_dir("npc_icon_patch_retarget").expect("temp dir");
    let project = &temp.root;
    let npc_dir = project.join("npcs").join("7__Retarget");
    fs::create_dir_all(&npc_dir).expect("npc dir");
    let manifest_path = npc_dir.join("7__Retarget.npc-import.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&json!({
            "format": "fftools.npc-import.v1",
            "blueprint": { "npcId": 7 },
            "tableDataPatch": {
                "file": "tabledata/old.json",
                "copiedSections": ["m_pNpcData"]
            }
        }))
        .expect("manifest json"),
    )
    .expect("write manifest");
    let replacement = project.join("tabledata").join("replacement.json");
    fs::create_dir_all(replacement.parent().expect("replacement parent"))
        .expect("replacement dir");
    fs::write(&replacement, "{}").expect("replacement file");

    retarget_staged_npc_import_table_patch(
        project,
        7,
        &replacement,
        &["m_pNpcIconData".to_string()],
    )
    .expect("retarget manifest");
    let manifest: JsonValue =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    assert_eq!(
        manifest.pointer("/tableDataPatch/file"),
        Some(&json!("tabledata/replacement.json"))
    );
    let sections = manifest
        .pointer("/tableDataPatch/copiedSections")
        .and_then(JsonValue::as_array)
        .expect("copied sections");
    assert!(sections.contains(&json!("m_pNpcData")));
    assert!(sections.contains(&json!("m_pNpcIconData")));
}

pub(super) fn test_clean_asset() -> fusionforge::Asset {
    fusionforge::Asset::empty_with_metadata("test.asset", 6).expect("test asset")
}

#[test]
fn clean_asset_bundle_preloads_are_fully_local_without_externals() {
    let asset = test_clean_asset();
    let bundle = clean_asset_bundle(
        &asset,
        "Character_Otto",
        "mob/npc_otto.kfm",
        2,
        "texture/npc_otto.dds",
        7,
        &[],
        &[2, 3, 4, 5, 6, 7, 8],
    )
    .expect("bundle");

    let object = bundle.as_object().expect("bundle object");
    let preload = object
        .get("m_PreloadTable")
        .and_then(fusionforge::UnityValue::as_array)
        .expect("preload table");
    assert_eq!(
        preload[0].as_pointer(),
        Some(&fusionforge::Pointer {
            source_asset: 0,
            file_id: 0,
            path_id: 2,
        })
    );
    assert!(
        preload.iter().all(|entry| entry
            .as_pointer()
            .is_some_and(|pointer| pointer.file_id == 0)),
        "expected a fully local preload table"
    );

    let container = object
        .get("m_Container")
        .and_then(fusionforge::UnityValue::as_array)
        .expect("container");
    assert_eq!(container.len(), 3, "expected kfm + nif alias + texture");
    let (model_name, model_entry) = pair_values(&container[0]);
    let (nif_name, nif_entry) = pair_values(&container[1]);
    let (_, texture_entry) = pair_values(&container[2]);

    assert_eq!(
        model_name.as_str().map(str::to_string),
        Some("mob/npc_otto.kfm".to_string())
    );
    assert_eq!(
        nif_name.as_str().map(str::to_string),
        Some("mob/npc_otto.nif".to_string())
    );
    assert_eq!(
        model_entry
            .as_object()
            .and_then(|object| object.get("preloadSize"))
            .and_then(fusionforge::UnityValue::as_i64),
        Some(7)
    );
    assert_eq!(
        nif_entry
            .as_object()
            .and_then(|object| object.get("preloadSize"))
            .and_then(fusionforge::UnityValue::as_i64),
        Some(7)
    );
    assert_eq!(
        texture_entry
            .as_object()
            .and_then(|object| object.get("preloadIndex"))
            .and_then(fusionforge::UnityValue::as_i64),
        Some(7)
    );
}

#[test]
fn resolved_pointer_root_recovers_legacy_format7_local_path_id() {
    let mut asset =
        fusionforge::Asset::empty_with_metadata("legacy.asset", 7).expect("legacy asset");
    asset.objects.insert(
        91_877_802,
        fusionforge::ObjectInfo {
            path_id: 91_877_802,
            data_offset: 0,
            size: 0,
            type_id: 1,
            class_id: 1,
        },
    );
    let env = fusionforge::UnityEnvironment::from_assets(vec![asset]);
    let pointer = fusionforge::Pointer {
        source_asset: 0,
        file_id: -1_015_676_928,
        path_id: 91_877_802,
    };
    let mut selected = BTreeSet::new();
    let mut queue = VecDeque::new();

    add_resolved_pointer_root(&env, &pointer, &mut selected, &mut queue);

    assert_eq!(selected, BTreeSet::from([(0usize, 91_877_802i64)]));
    assert_eq!(queue, VecDeque::from([(0usize, 91_877_802i64)]));
}

#[test]
fn normalized_server_tdata_path_rejects_flag_like_values() {
    assert_eq!(normalized_server_tdata_path(Some("--nocapture")), None);
    assert_eq!(normalized_server_tdata_path(Some("  --help  ")), None);
    assert_eq!(
        normalized_server_tdata_path(Some("OpenFusion\\bin\\tdata")),
        Some("OpenFusion/bin/tdata".to_string())
    );
}

#[test]
fn patch_config_from_json_ignores_invalid_server_tdata_path() {
    let config = patch_config_from_json(&json!({
        "ServerTdataPath": "--nocapture"
    }));

    assert_eq!(config.server_tdata_path, None);
}

#[test]
fn resolve_server_tdata_path_prefers_workspace_root_for_openfusion() {
    let project = Path::new("D:/CodexProject/FusionFallProject/builds/test.ffclient");
    let resolved = resolve_server_tdata_path(project, "OpenFusion/bin/tdata");
    assert_eq!(
        resolved.to_string_lossy().replace('\\', "/"),
        "D:/CodexProject/FusionFallProject/OpenFusion/bin/tdata"
    );
}
