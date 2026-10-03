use super::*;

fn asset_with_path(name: &str, path_id: i64) -> Asset {
    let mut asset = Asset::empty_with_metadata(name, 7).unwrap();
    asset.objects.insert(
        path_id,
        ObjectInfo {
            path_id,
            data_offset: 0,
            size: 0,
            type_id: 1,
            class_id: 1,
        },
    );
    asset
}

#[test]
fn bare_path_id_collision_requires_exact_serialized_asset() {
    let env = UnityEnvironment::from_assets(vec![
        asset_with_path("left.asset", 7),
        asset_with_path("right.asset", 7),
    ]);

    let error = select_object(&env, None, 7).unwrap_err();
    assert!(error.contains("ambiguous across serialized assets"));
    assert!(error.contains("--serialized-asset"));
}

#[test]
fn exact_serialized_asset_scopes_path_id_selection() {
    let env = UnityEnvironment::from_assets(vec![
        asset_with_path("left.asset", 7),
        asset_with_path("right.asset", 7),
    ]);

    let (asset_index, asset, info) = select_object(&env, Some("right.asset"), 7).unwrap();
    assert_eq!(asset_index, 1);
    assert_eq!(asset.name, "right.asset");
    assert_eq!(info.path_id, 7);
}

#[test]
fn pointer_field_paths_are_stable_json_pointers() {
    let value = UnityValue::Object(std::collections::BTreeMap::from([
        (
            "m_Target".to_string(),
            UnityValue::Pointer(Pointer {
                source_asset: 0,
                file_id: 0,
                path_id: 4,
            }),
        ),
        (
            "m_Items".to_string(),
            UnityValue::Array(vec![UnityValue::Pointer(Pointer {
                source_asset: 0,
                file_id: 1,
                path_id: 8,
            })]),
        ),
    ]));
    let mut pointers = Vec::new();
    collect_pointers(&value, "", &mut pointers);

    assert_eq!(pointers[0].0, "/m_Items/0");
    assert_eq!(pointers[1].0, "/m_Target");
}

#[test]
fn sha256_output_is_lowercase() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
