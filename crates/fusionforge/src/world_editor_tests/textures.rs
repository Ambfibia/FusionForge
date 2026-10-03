use super::*;

#[test]
fn live_fusion_eduardo_nif_preview_keeps_uvs_and_texture_materials_when_available() {
    let Some(preview) = live_fusion_model_preview("mob/fusion_eduardo.kfm") else {
        return;
    };
    let meshes = preview
        .get("meshes")
        .and_then(JsonValue::as_array)
        .expect("preview meshes");
    let animation_names = preview["animations"]
        .as_array()
        .expect("animations")
        .iter()
        .filter_map(|clip| clip["name"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        animation_names,
        BTreeSet::from([
            "corruptak",
            "corruptak_readyspell",
            "death",
            "dodge",
            "megatak_readyspell",
            "melee1",
            "melee2",
            "ready",
            "run",
            "skill0",
            "stand1",
            "stand2",
            "stand3",
            "stand4",
            "walk",
            "wound",
        ]),
        "Fusion Eduardo preview must expose only clips bound to its Animation root"
    );
    assert!(
        meshes.iter().any(|mesh| {
            mesh.pointer("/skin/source").and_then(JsonValue::as_str)
                == Some("unity-rigid-mesh-attachment")
        }),
        "Fusion Eduardo eye mesh should follow its nearest skeleton joint"
    );
    let sampler = crate::npc_animation::NpcAnimationSampler::from_preview(&preview)
        .expect("Fusion Eduardo animation sampler");
    assert!(sampler.has_skinned_meshes());
    assert!(sampler.has_sampleable_clip("stand1"));
    assert!(
        meshes.iter().any(|mesh| mesh
            .get("uvs")
            .and_then(JsonValue::as_array)
            .is_some_and(|uvs| !uvs.is_empty())),
        "Fusion Eduardo preview should keep NIF UVs"
    );
    let materials = preview
        .get("materials")
        .and_then(JsonValue::as_object)
        .expect("preview materials");
    assert!(
        materials.values().any(|material| material
            .get("textureDataUrl")
            .and_then(JsonValue::as_str)
            .is_some_and(|data_url| data_url.starts_with("data:image/png;base64,"))),
        "Fusion Eduardo preview should bind decoded material textures"
    );
}
