use super::*;

/// The organizer hashed the pre-organization rooted GLB route. This is the
/// installed `back_alien` package, so the derivation is pinned to a real
/// published identity rather than to this module's own arithmetic.
#[test]
fn identity_derivation_reproduces_the_installed_back_alien_ids() {
    let route = "characters/player/equipment/back/back_alien/back_alien.glb";
    let identity = blake3::hash(route.as_bytes()).to_hex().to_string();
    assert_eq!(
        identity,
        "fc2d2f7c765af95c9d3cddbee4e45b8320ebbd7dd23031e57c4bbf634f09d86f"
    );
    assert_eq!(
        format!("player-item-{identity}"),
        "player-item-fc2d2f7c765af95c9d3cddbee4e45b8320ebbd7dd23031e57c4bbf634f09d86f"
    );
    assert_eq!(
        format!("player-item-set-{identity}"),
        "player-item-set-fc2d2f7c765af95c9d3cddbee4e45b8320ebbd7dd23031e57c4bbf634f09d86f"
    );
}

struct Fixture {
    temp: tempfile::TempDir,
}

impl Fixture {
    fn new(glb: &[u8], toon: &[u8]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let assets = temp.path().join("assets");
        let items = assets.join("characters/player/items");
        fs::create_dir_all(&items).unwrap();
        fs::create_dir_all(assets.join("characters/player/rendering/textures")).unwrap();
        fs::write(
            assets.join("characters/player/rendering/textures/toonramp9.png"),
            toon,
        )
        .unwrap();
        let catalog = PlayerItemSetCatalog {
            schema: PLAYER_ITEM_SET_CATALOG_SCHEMA.to_owned(),
            sets: Vec::new(),
            models: Vec::new(),
            rendering_textures: vec![ResourceSetArtifact {
                path: "characters/player/rendering/textures/toonramp9.png".to_owned(),
                bytes: toon.len() as u64,
                blake3: blake3::hash(toon).to_hex().to_string(),
            }],
        };
        fs::write(
            items.join("catalog.json"),
            serde_json::to_vec_pretty(&catalog).unwrap(),
        )
        .unwrap();

        let candidate = temp.path().join("candidate");
        let model_dir = candidate.join("characters/player/equipment/weapon/melee_razor");
        fs::create_dir_all(model_dir.join("melee_razor.textures")).unwrap();
        fs::write(model_dir.join("melee_razor.glb"), glb).unwrap();
        fs::write(model_dir.join("melee_razor.textures/ToonRamp9.png"), toon).unwrap();

        let batch = serde_json::json!({
            "schema": EQUIPMENT_BATCH_SCHEMA,
            "productionAssetsMutated": false,
            "blockers": [],
            "models": [{
                "category": "weapon",
                "trueName": "melee_razor",
                "outputGlb": "characters/player/equipment/weapon/melee_razor/melee_razor.glb",
                "glbBlake3": blake3::hash(glb).to_hex().to_string(),
            }],
        });
        fs::write(
            candidate.join(EQUIPMENT_BATCH_REPORT_FILE),
            serde_json::to_vec_pretty(&batch).unwrap(),
        )
        .unwrap();

        let gpu = serde_json::json!({
            "schema": EQUIPMENT_GPU_BATCH_SCHEMA,
            "counts": {"executionBlockers": 0},
            "models": [{
                "relativeGlb": "characters/player/equipment/weapon/melee_razor/melee_razor.glb",
                "disposition": "passed-new-evidence",
            }],
        });
        fs::write(
            temp.path().join("gpu.json"),
            serde_json::to_vec_pretty(&gpu).unwrap(),
        )
        .unwrap();
        Self { temp }
    }

    fn options(&self, apply: bool) -> PlayerItemModelInstallOptions {
        PlayerItemModelInstallOptions::new(
            self.temp.path().join("candidate"),
            self.temp.path().join("gpu.json"),
            self.temp.path().join("assets"),
            "retrobution-20260821",
            apply,
        )
    }

    fn catalog(&self) -> PlayerItemSetCatalog {
        serde_json::from_slice(
            &fs::read(
                self.temp
                    .path()
                    .join("assets/characters/player/items/catalog.json"),
            )
            .unwrap(),
        )
        .unwrap()
    }
}

#[test]
fn installs_one_set_and_reuses_the_shared_rendering_texture() {
    let fixture = Fixture::new(b"glb-bytes", b"toon");

    let plan = install_player_item_models(&fixture.options(false)).unwrap();
    assert!(!plan.applied);
    assert_eq!(plan.installed.len(), 1);
    let entry = &plan.installed[0];
    assert_eq!(entry.true_name, "melee_razor");
    assert_eq!(
        entry.identity_route,
        "characters/player/equipment/weapon/melee_razor/melee_razor.glb"
    );
    // ToonRamp9 is byte-identical to the published rendering texture, so it
    // must be reused rather than duplicated into the set.
    assert!(entry.textures.is_empty());
    assert_eq!(
        entry.shared_rendering_textures,
        vec!["characters/player/rendering/textures/toonramp9.png".to_owned()]
    );
    assert_eq!(entry.gpu_disposition, "passed-new-evidence");
    // A plan run must not touch the tree.
    assert!(fixture.catalog().models.is_empty());
    assert!(
        !fixture
            .temp
            .path()
            .join("assets/characters/player/items/weapon")
            .exists()
    );

    let applied = install_player_item_models(&fixture.options(true)).unwrap();
    assert!(applied.applied);
    assert_eq!(applied.catalog_models_after, 1);
    let catalog = fixture.catalog();
    assert_eq!(catalog.models.len(), 1);
    assert_eq!(catalog.sets.len(), 1);
    assert_eq!(
        catalog.models[0].source_route,
        "weapon/melee_razor/melee_razor.glb"
    );
    assert_eq!(catalog.sets[0].texture_count, 0);
    assert_eq!(catalog.sets[0].member_count, 1);
    let set_root = fixture
        .temp
        .path()
        .join("assets/characters/player/items/weapon/melee_razor");
    assert!(set_root.join("set.json").is_file());
    assert!(set_root.join("models/melee_razor/item.json").is_file());
    assert_eq!(
        fs::read(set_root.join("models/melee_razor/model.glb")).unwrap(),
        b"glb-bytes"
    );
    // No conversion report may leak into runtime assets.
    assert!(!set_root.join("melee_razor.publish.json").exists());
}

#[test]
fn refuses_to_install_the_same_model_twice() {
    let fixture = Fixture::new(b"glb-bytes", b"toon");
    install_player_item_models(&fixture.options(true)).unwrap();
    let error = install_player_item_models(&fixture.options(true)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("already publishes a model named"),
        "{error}"
    );
    assert_eq!(fixture.catalog().models.len(), 1);
}

#[test]
fn publishes_a_texture_the_catalog_does_not_already_share() {
    let fixture = Fixture::new(b"glb-bytes", b"a-distinct-texture");
    // Rewrite the shared rendering texture so the candidate PNG no longer
    // matches it; the set must then own its own copy.
    let assets = fixture.temp.path().join("assets");
    fs::write(
        assets.join("characters/player/rendering/textures/toonramp9.png"),
        b"other",
    )
    .unwrap();
    let catalog_path = assets.join("characters/player/items/catalog.json");
    let mut catalog: PlayerItemSetCatalog =
        serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    catalog.rendering_textures[0].blake3 = blake3::hash(b"other").to_hex().to_string();
    catalog.rendering_textures[0].bytes = 5;
    fs::write(&catalog_path, serde_json::to_vec_pretty(&catalog).unwrap()).unwrap();

    let report = install_player_item_models(&fixture.options(true)).unwrap();
    let entry = &report.installed[0];
    assert!(entry.shared_rendering_textures.is_empty());
    assert_eq!(entry.textures.len(), 1);
    assert_eq!(
        entry.textures[0].path,
        "characters/player/items/weapon/melee_razor/textures/ToonRamp9.png"
    );
    assert!(
        fixture
            .temp
            .path()
            .join("assets/characters/player/items/weapon/melee_razor/textures/ToonRamp9.png")
            .is_file()
    );
    assert_eq!(fixture.catalog().sets[0].texture_count, 1);
}

#[test]
fn rejects_a_model_without_a_passing_standalone_gpu_gate() {
    let fixture = Fixture::new(b"glb-bytes", b"toon");
    let gpu_path = fixture.temp.path().join("gpu.json");
    let mut gpu: JsonValue = serde_json::from_slice(&fs::read(&gpu_path).unwrap()).unwrap();
    gpu["models"][0]["disposition"] = JsonValue::String("blocked".to_owned());
    fs::write(&gpu_path, serde_json::to_vec_pretty(&gpu).unwrap()).unwrap();

    let error = install_player_item_models(&fixture.options(true)).unwrap_err();
    assert!(error.to_string().contains("standalone GPU gate"), "{error}");
    assert!(fixture.catalog().models.is_empty());
}

#[test]
fn rejects_a_candidate_glb_that_does_not_match_its_batch_hash() {
    let fixture = Fixture::new(b"glb-bytes", b"toon");
    fs::write(
        fixture
            .temp
            .path()
            .join("candidate/characters/player/equipment/weapon/melee_razor/melee_razor.glb"),
        b"tampered",
    )
    .unwrap();
    let error = install_player_item_models(&fixture.options(true)).unwrap_err();
    assert!(error.to_string().contains("batch report hash"), "{error}");
    assert!(fixture.catalog().models.is_empty());
}
