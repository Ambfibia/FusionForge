use super::*;
use serde_json::json;

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
