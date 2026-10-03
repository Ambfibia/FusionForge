use std::{fs, path::PathBuf};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

fn asset_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../FFOneClient/assets/game")
}

fn assert_contract_compatible<Offline, Runtime>(relative: &str)
where
    Offline: DeserializeOwned + Serialize,
    Runtime: DeserializeOwned + Serialize,
{
    let path = asset_root().join(relative);
    let bytes = fs::read(&path)
        .unwrap_or_else(|error| panic!("cannot read runtime contract {}: {error}", path.display()));
    let offline: Offline = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        panic!(
            "offline publisher contract cannot parse {}: {error}",
            path.display()
        )
    });
    let runtime: Runtime = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        panic!("runtime contract cannot parse {}: {error}", path.display())
    });
    assert_eq!(
        serde_json::to_value(offline).expect("serialize offline contract"),
        serde_json::to_value(runtime).expect("serialize runtime contract"),
        "offline and runtime contract shapes diverged for {relative}"
    );
}

#[test]
fn published_runtime_documents_match_offline_publisher_contracts() {
    assert_contract_compatible::<
        ffone_asset_pipeline::CharacterCreationNameWheel,
        ffone_runtime_contracts::CharacterCreationNameWheel,
    >("data/character_creation/name_wheel.json");
    assert_contract_compatible::<
        ffone_asset_pipeline::CharacterCreationAppearance,
        ffone_runtime_contracts::CharacterCreationAppearance,
    >("data/character_creation/appearance.json");
    assert_contract_compatible::<
        ffone_asset_pipeline::CharacterCreationAvatarItems,
        ffone_runtime_contracts::CharacterCreationAvatarItems,
    >("data/character_creation/avatar_items.json");
    assert_contract_compatible::<
        ffone_asset_pipeline::CharacterCreationRuntimeTextures,
        ffone_runtime_contracts::CharacterCreationRuntimeTextures,
    >("data/character_creation/runtime_textures.json");
    assert_contract_compatible::<
        ffone_asset_pipeline::PlayerSharedRigContract,
        ffone_runtime_contracts::PlayerSharedRigContract,
    >("characters/player/shared/player_rig_contract.json");
    assert_contract_compatible::<
        ffone_asset_pipeline::TutorialEffectCatalog,
        ffone_runtime_contracts::TutorialEffectCatalog,
    >("map/shared/effects/catalog.json");
    assert_contract_compatible::<
        ffone_asset_pipeline::TutorialProjectileCatalog,
        ffone_runtime_contracts::TutorialProjectileCatalog,
    >("map/shared/projectiles/catalog.json");
}

#[test]
fn published_runtime_tree_has_no_flat_texture_store_or_hashed_map_package_names() {
    let root = asset_root();
    assert!(
        !root.join("textures").exists(),
        "the retired flat Unity texture dump must not be published"
    );
    assert!(
        root.join("objects/attractions/a_merry_go_round_01_01_amusment/models/dt_cpap_a_merry_go_round_01_01_amusment_a_merry_go/object.json")
            .is_file(),
        "the readable merry-go-round package route is missing"
    );

    for relative in ["objects", "map/shared/terrain/layers"] {
        let mut pending = vec![root.join(relative)];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("cannot enumerate {}: {error}", directory.display()))
            {
                let entry = entry.expect("read runtime asset entry");
                if !entry.file_type().expect("read runtime file type").is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                let bytes = name.as_bytes();
                let has_hash_suffix = bytes.len() >= 18
                    && &bytes[bytes.len() - 18..bytes.len() - 16] == b"--"
                    && bytes[bytes.len() - 16..].iter().all(u8::is_ascii_hexdigit);
                assert!(
                    !has_hash_suffix,
                    "published map directory still exposes a content hash: {}",
                    entry.path().display()
                );
                pending.push(entry.path());
            }
        }
    }

    for relative in [
        "data/tables/npc_texture_overrides.json",
        "data/character_creation/runtime_textures.json",
        "data/character_creation/avatar_items.json",
        "data/character_creation/appearance.json",
    ] {
        let value: Value = serde_json::from_slice(&fs::read(root.join(relative)).unwrap()).unwrap();
        assert_no_flat_texture_route(&value, relative);
    }
}

fn assert_no_flat_texture_route(value: &Value, owner: &str) {
    match value {
        Value::String(text) => assert!(
            !text.starts_with("textures/"),
            "{owner} still references the retired flat texture store: {text}"
        ),
        Value::Array(values) => {
            for value in values {
                assert_no_flat_texture_route(value, owner);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                assert_no_flat_texture_route(value, owner);
            }
        }
        _ => {}
    }
}
