use ffone_asset_pipeline::{MODEL_PUBLISH_SCHEMA, ModelFeatureCounts, ModelPublishContract};

fn glb_with_root(name: &str) -> Vec<u8> {
    let mut json = serde_json::to_vec(&serde_json::json!({
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": name}]
    }))
    .unwrap();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let total = 20 + json.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
    glb.extend_from_slice(&json);
    glb
}

fn contract(name: &str, glb: &[u8]) -> ModelPublishContract {
    let counts = ModelFeatureCounts {
        nodes: 1,
        mesh_parts: 1,
        ..Default::default()
    };
    ModelPublishContract {
        schema: MODEL_PUBLISH_SCHEMA.into(),
        legacy_name: name.into(),
        root_node: name.into(),
        family: "npc".into(),
        semantic_directories: vec!["main".into()],
        output_glb: format!("models/npc/main/{name}.glb"),
        glb_blake3: blake3::hash(glb).to_hex().to_string(),
        source: counts.clone(),
        published: counts,
        unresolved_source_features: vec![],
    }
}

#[test]
fn valid_true_name_contract_passes() {
    let name = "Рекс Старший";
    let glb = glb_with_root(name);
    contract(name, &glb).validate(&glb).unwrap();
}

#[test]
fn lost_skin_or_animation_data_is_rejected() {
    let glb = glb_with_root("Rex");
    let mut value = contract("Rex", &glb);
    value.source.skinned_meshes = 1;
    value.source.joints = 20;
    value.source.inverse_bind_matrices = 20;
    value.source.weighted_vertices = 500;
    value.source.animation_clips = 10;
    value.source.animation_channels = 80;
    value.source.animation_keyframes = 400;
    assert!(value.validate(&glb).is_err());
}

#[test]
fn unresolved_curve_semantics_and_hash_fallbacks_are_rejected() {
    let glb = glb_with_root("Rex");
    let mut value = contract("Rex", &glb);
    value.unresolved_source_features = vec!["legacy tangentMode".into()];
    assert!(value.validate(&glb).is_err());

    let bad_glb = glb_with_root("rex--0123456789abcdef");
    assert!(
        contract("rex--0123456789abcdef", &bad_glb)
            .validate(&bad_glb)
            .is_err()
    );
}

#[test]
fn filesystem_sanitization_is_minimal_but_metadata_stays_exact() {
    let name = "Рекс: Старший. ";
    let glb = glb_with_root(name);
    let mut value = contract(name, &glb);
    value.output_glb = "models/npc/main/Рекс_ Старший.glb".into();
    value.validate(&glb).unwrap();
    assert_eq!(value.legacy_name, name);
    assert_eq!(value.root_node, name);
}
