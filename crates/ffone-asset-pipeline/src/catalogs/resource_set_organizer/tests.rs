use super::*;
use serde_json::json;

fn atlas_reference_fixture(second_atlas: Option<&str>, declare: bool) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("characters/player/rendering")).unwrap();
    let write = |path: &str, bytes: &[u8]| {
        let target = root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
        ResourceSetArtifact { path: path.into(), bytes: bytes.len() as u64, blake3: hash_bytes(bytes) }
    };
    let first = "characters/player/items/hat/first/textures/common.png";
    let atlas = write(first, b"accepted atlas");
    let other = second_atlas.map(|path| write(path, b"accepted atlas")).unwrap_or_else(|| atlas.clone());
    let mut catalog = PlayerItemSetCatalog {
        schema: PLAYER_ITEM_SET_CATALOG_SCHEMA.into(), sets: vec![], models: vec![], rendering_textures: vec![],
    };
    for (name, texture) in [("first", atlas), ("second", other)] {
        let package = format!("characters/player/items/hat/{name}");
        let uri = if name == "first" { "textures/common.png".into() }
            else { relative_path(Path::new(&package), Path::new(&texture.path)).unwrap() };
        let mut chunk = serde_json::to_vec(&json!({"images":[{"uri":uri}]})).unwrap();
        while chunk.len() % 4 != 0 {chunk.push(b' ');}
        let mut glb = b"glTF".to_vec();
        glb.extend(2u32.to_le_bytes()); glb.extend(((20+chunk.len()) as u32).to_le_bytes());
        glb.extend((chunk.len() as u32).to_le_bytes()); glb.extend(0x4e4f534au32.to_le_bytes()); glb.extend(chunk);
        let model = write(&format!("{package}/model.glb"), &glb);
        let item = PlayerItemDefinition { schema: PLAYER_ITEM_SCHEMA.into(), id:name.into(), true_name:name.into(),
            category:"hat".into(), source_route:format!("hat/{name}/{name}.glb"), resource_set:name.into(), model:model.clone() };
        let definition = write(&format!("{package}/item.json"), &pretty_json(&item).unwrap());
        let set = ResourceSetDocument { schema:RESOURCE_SET_SCHEMA.into(), id:name.into(), name:name.into(),
            domain:"player_item".into(), category:"hat".into(), prefix:name.into(), family:name.into(),
            textures:if name=="second" && !declare {vec![]}else{vec![texture]},
            members:vec![ResourceSetMember {id:name.into(),name:name.into(),definition,files:vec![model.clone()]}] };
        let definition = write(&format!("{package}/set.json"), &pretty_json(&set).unwrap());
        catalog.sets.push(ResourceSetCatalogEntry {id:name.into(),name:name.into(),category:"hat".into(),prefix:name.into(),family:name.into(),definition,member_count:1,texture_count:set.textures.len() as u64});
        catalog.models.push(PlayerItemCatalogModel {category:item.category,true_name:item.true_name,source_route:item.source_route,resource_set:item.resource_set,model});
    }
    let proof = write("characters/player/items/catalog.json", &pretty_json(&catalog).unwrap());
    for path in player_catalog_provenance_files(root) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, pretty_json(&json!({"provenance":{"playerEquipmentCatalog":proof}})).unwrap()).unwrap();
    }
    temp
}

#[test]
fn player_sets_reuse_one_verified_atlas_path() {
    let fixture = atlas_reference_fixture(None, true);
    verify_player_item_sets_at_asset_root(fixture.path()).unwrap();
    fs::write(fixture.path().join("characters/player/items/hat/first/textures/common.png"), b"tampered").unwrap();
    assert!(verify_player_item_sets_at_asset_root(fixture.path()).is_err());
}

#[test]
fn player_sets_reject_undeclared_cross_package_atlas() {
    let fixture = atlas_reference_fixture(None, false);
    assert!(verify_player_item_sets_at_asset_root(fixture.path()).unwrap_err().to_string().contains("escaped its item set"));
}

#[test]
fn player_sets_reject_duplicate_atlas_files() {
    let fixture = atlas_reference_fixture(Some("characters/player/items/hat/second/textures/common.png"), true);
    assert!(verify_player_item_sets_at_asset_root(fixture.path()).unwrap_err().to_string().contains("duplicated across sets"));
}

#[test]
fn promoted_object_textures_leave_map_shared_ownership_before_routes_change() {
    let source = "map/shared/textures/crate.png";
    let destination = "objects/props/crate/textures/crate.png";
    let mut catalog = json!({
        "sharedFiles": [
            {
                "path": source,
                "bytes": 1,
                "blake3": "source"
            },
            {
                "path": "map/shared/terrain/layers/ground.png",
                "bytes": 2,
                "blake3": "terrain"
            }
        ],
        "objects": [
            {
                "id": "crate",
                "model": {
                    "path": source
                }
            }
        ],
        "compositeObjects": []
    });
    let path_map = BTreeMap::from([(source.to_owned(), destination.to_owned())]);

    update_catalog(
        Path::new("missing-asset-root"),
        &mut catalog,
        &path_map,
        &BTreeMap::new(),
        &[],
    )
    .unwrap();

    assert_eq!(
        catalog
            .pointer("/objects/0/model/path")
            .and_then(JsonValue::as_str),
        Some(destination)
    );
    let shared = catalog["sharedFiles"].as_array().unwrap();
    assert_eq!(shared.len(), 1);
    assert_eq!(
        shared[0]["path"].as_str(),
        Some("map/shared/terrain/layers/ground.png")
    );
}
