use super::*;
use crate::ResourceSetArtifact;

fn fixture(missing_true_name: Option<&str>) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("characters/player/items")).unwrap();
    fs::create_dir_all(root.join("data/character_creation")).unwrap();

    let catalog = PlayerItemSetCatalog {
        schema: PLAYER_ITEM_SET_CATALOG_SCHEMA.to_owned(),
        sets: Vec::new(),
        models: vec![crate::PlayerItemCatalogModel {
            category: "weapon".to_owned(),
            true_name: "melee_razor".to_owned(),
            source_route: "weapon/melee_razor/melee_razor.glb".to_owned(),
            resource_set: "player-item-set-x".to_owned(),
            model: ResourceSetArtifact {
                path: "characters/player/items/weapon/melee_razor/models/melee_razor/model.glb"
                    .to_owned(),
                bytes: 4242,
                blake3: "a".repeat(64),
            },
        }],
        rendering_textures: Vec::new(),
    };
    fs::write(
        root.join("characters/player/items/catalog.json"),
        serde_json::to_vec_pretty(&catalog).unwrap(),
    )
    .unwrap();

    let mut male = serde_json::json!({"modelStatus": "missing", "models": []});
    if let Some(name) = missing_true_name {
        male["sourceModelTrueName"] = JsonValue::from(name);
    }
    let document = serde_json::json!({
        "schema": AVATAR_ITEMS_SCHEMA,
        "lookupComplete": false,
        "counts": {"modelReferences": 10, "resolvedModels": 10},
        "items": [
            {
                "category": "weapon",
                "itemNumber": 764,
                "name": "Razor Mantis Scythe",
                "male": male,
                // A boys-only slot: the table names no model, so it must
                // stay missing.
                "female": {"modelStatus": "missing", "models": []},
            },
            {
                "category": "weapon",
                "itemNumber": 1,
                "name": "Disco Bomb",
                "male": {
                    "modelStatus": "verified_unique",
                    "sourceModelTrueName": "theown_discobomb",
                    "models": [{"trueName": "theown_discobomb"}],
                },
            },
        ],
    });
    fs::write(
        root.join("data/character_creation/avatar_items.json"),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    temp
}

fn document(root: &Path) -> JsonValue {
    serde_json::from_slice(
        &fs::read(root.join("data/character_creation/avatar_items.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn resolves_only_missing_slots_whose_model_the_catalog_publishes() {
    let temp = fixture(Some("melee_razor"));
    let root = temp.path();

    let plan =
        refresh_avatar_item_models(&AvatarItemModelRefreshOptions::new(root, false)).unwrap();
    assert!(!plan.applied);
    // Item 764 male (resolvable) and 764 female (boys-only, no model named).
    assert_eq!(plan.missing_slots_before, 2);
    assert_eq!(plan.slots_without_a_source_model, 1);
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].true_name, "melee_razor");
    assert_eq!(plan.changes[0].exact_route, "wear/melee_razor.nif");
    assert_eq!(plan.changes[0].gender, "male");
    assert_eq!(plan.resolved_models_after, 11);
    // A plan run must not touch the document.
    assert_eq!(document(root)["items"][0]["male"]["modelStatus"], "missing");

    let applied =
        refresh_avatar_item_models(&AvatarItemModelRefreshOptions::new(root, true)).unwrap();
    assert!(applied.applied);
    let after = document(root);
    let male = &after["items"][0]["male"];
    assert_eq!(male["modelStatus"], "verified_unique");
    assert_eq!(male["models"][0]["trueName"], "melee_razor");
    assert_eq!(male["models"][0]["exactRoute"], "wear/melee_razor.nif");
    assert_eq!(male["models"][0]["nativeAsset"]["bytes"], 4242);
    // The boys-only slot and the already-resolved item are untouched.
    assert_eq!(after["items"][0]["female"]["modelStatus"], "missing");
    assert_eq!(
        after["items"][1]["male"]["models"][0]["trueName"],
        "theown_discobomb"
    );
    assert_eq!(after["counts"]["resolvedModels"], 11);
    // One slot still legitimately has no model, so the lookup is not complete.
    assert_eq!(after["lookupComplete"], false);

    let again =
        refresh_avatar_item_models(&AvatarItemModelRefreshOptions::new(root, true)).unwrap();
    assert!(again.changes.is_empty());
    assert!(!again.applied);
}

#[test]
fn leaves_a_missing_slot_alone_when_the_catalog_has_no_such_model() {
    let temp = fixture(Some("pistol_musket"));
    let root = temp.path();
    let report =
        refresh_avatar_item_models(&AvatarItemModelRefreshOptions::new(root, true)).unwrap();
    assert!(report.changes.is_empty());
    assert_eq!(document(root)["items"][0]["male"]["modelStatus"], "missing");
}

#[test]
fn rejects_a_catalog_model_from_another_category() {
    let temp = fixture(Some("melee_razor"));
    let root = temp.path();
    let path = root.join("data/character_creation/avatar_items.json");
    let mut doc: JsonValue = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    doc["items"][0]["category"] = JsonValue::from("hat");
    fs::write(&path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();

    let error = refresh_avatar_item_models(&AvatarItemModelRefreshOptions::new(root, true))
        .unwrap_err();
    assert!(error.to_string().contains("category"), "{error}");
}
