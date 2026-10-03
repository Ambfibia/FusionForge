use super::*;

#[test]
fn retrobution_fusion_eye_report_requires_the_new_primary_texture() {
    let report = serde_json::json!({
        "materials": [{
            "name": "spwaneye",
            "serializedShaderName": "normal_blendOneOneTest_cullOff",
            "declaredShaderName": "normal_blendOneOneTest_cullOff",
            "legacyShaderName": "normal_blendOneOneTest_cullOff",
            "shaderSha256": "cb4b27442bbd396742347760f7610d17d767e2d4fa226ec8cd3ec863cdebe0ed",
            "effectiveRenderQueue": 3000,
            "renderPassCount": 1
        }],
        "textures": [{
            "sourceName": "spwaneye.dds",
            "uri": "fusion_test.textures/spwaneye.png",
            "width": 128,
            "height": 512,
            "sourceMipCount": 1,
            "publishedPolicy": "baseLevelOnly",
            "byteLength": 17046,
            "sha256": "b895006b292f14784a902625761cf75b642c2103d3d845605f09bc47851de214",
            "mipLevels": [{
                "level": 0,
                "uri": "fusion_test.textures/spwaneye.png",
                "width": 128,
                "height": 512,
                "pngByteLength": 17046,
                "pngSha256": "b895006b292f14784a902625761cf75b642c2103d3d845605f09bc47851de214"
            }]
        }]
    });
    validate_retrobution_fusion_eye_report(&report).expect("Retrobution eye contract");

    let mut old_resolution = report;
    old_resolution["textures"][0]["width"] = serde_json::json!(64);
    assert!(validate_retrobution_fusion_eye_report(&old_resolution).is_err());
}
#[test]
fn character_destination_keeps_true_semantic_names() {
    let routes = [CharacterRoute {
        source_directory: "models/mob/npc_dexter".to_owned(),
        destination_directory: "characters/npc/npc_dexter".to_owned(),
    }];
    assert_eq!(
        destination_file_path("models/mob/npc_dexter/npc_dexter.glb", &routes).unwrap(),
        "characters/npc/npc_dexter/npc_dexter.glb"
    );
    assert!(destination_file_path("../npc_dexter.glb", &routes).is_err());
    assert!(destination_file_path("models/mob/other/other.glb", &routes).is_err());
}

#[test]
fn glb_animation_names_preserve_exact_gltf_order() {
    let json =
        br#"{"asset":{"version":"2.0"},"animations":[{"name":"stand1"},{"name":"run"}]}"#;
    let padded = (json.len() + 3) & !3;
    let mut glb = Vec::new();
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(20_u32 + padded as u32).to_le_bytes());
    glb.extend_from_slice(&(padded as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(json);
    glb.resize(20 + padded, b' ');
    assert_eq!(
        glb_animation_names(&glb).unwrap(),
        vec!["stand1".to_owned(), "run".to_owned()]
    );
}
