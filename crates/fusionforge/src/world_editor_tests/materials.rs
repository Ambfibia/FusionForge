use super::*;

#[test]
fn live_fusion_mandy_preview_material_bindings_when_available() {
    let Some(preview) = live_fusion_model_preview("mob/fusion_mandy.kfm") else {
        return;
    };
    let materials = preview
        .get("materials")
        .and_then(JsonValue::as_object)
        .expect("preview materials");
    let textured_count = materials
        .values()
        .filter(|material| {
            material
                .get("textureDataUrl")
                .and_then(JsonValue::as_str)
                .is_some_and(|value| value.starts_with("data:image/png;base64,"))
        })
        .count();
    assert!(
        textured_count >= 2,
        "Fusion Mandy preview should bind decoded textures for main/sub materials"
    );
    assert!(
        materials.values().filter(|material| material
            .get("textureDataUrl")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| value.starts_with("data:image/png;base64,"))).all(|material| {
            material
                .get("textureTint")
                .and_then(JsonValue::as_bool)
                .is_some_and(|value| !value)
        }),
        "Fusion Mandy textured materials should not darken diffuse textures with material color"
    );
    let meshes = preview
        .get("meshes")
        .and_then(JsonValue::as_array)
        .expect("preview meshes");
    let eye_material_ids = materials
        .iter()
        .filter(|(_, material)| {
            material
                .get("name")
                .and_then(JsonValue::as_str)
                .is_some_and(|name| {
                    let name = name.to_ascii_lowercase();
                    name.contains("spwaneye") || name.contains("fusion_eye")
                })
        })
        .map(|(id, _)| id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(
        eye_material_ids.iter().any(|id| materials[*id]
            .get("textureDataUrl")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| value.starts_with("data:image/png;base64,"))),
        "Fusion Mandy preview should include the shared Fusion eye texture material"
    );
    assert!(
        eye_material_ids.iter().any(|id| materials[*id]
            .get("blendMode")
            .and_then(JsonValue::as_str)
            .is_some_and(|value| value == "additive")),
        "Fusion Mandy eye material should preserve the Unity Blend One One mode"
    );
    let eye_material_summary = eye_material_ids
        .iter()
        .map(|id| {
            let material = &materials[*id];
            format!(
                "{}: name={:?} shader={:?} alphaMode={:?} blendMode={:?} hasAlpha={:?} hasPartialAlpha={:?} texture={}x{}",
                id,
                material.get("name").and_then(JsonValue::as_str),
                material.get("shaderName").and_then(JsonValue::as_str),
                material.get("alphaMode").and_then(JsonValue::as_str),
                material.get("blendMode").and_then(JsonValue::as_str),
                material.get("hasAlpha").and_then(JsonValue::as_bool),
                material.get("hasPartialAlpha").and_then(JsonValue::as_bool),
                material.get("textureWidth").and_then(JsonValue::as_u64).unwrap_or_default(),
                material.get("textureHeight").and_then(JsonValue::as_u64).unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(
        eye_material_ids.iter().any(|id| materials[*id]
            .get("hasAlpha")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false)),
        "Fusion Mandy eye texture should be converted to transparent preview alpha: {eye_material_summary}"
    );
    assert!(
        meshes.iter().any(|mesh| mesh
            .get("materialId")
            .and_then(JsonValue::as_str)
            .is_some_and(|material_id| eye_material_ids.contains(material_id))),
        "Fusion Mandy preview should keep a mesh using the shared Fusion eye material"
    );
    let textured_mandy_mesh_count = meshes
        .iter()
        .filter(|mesh| {
            mesh.get("name")
                .and_then(JsonValue::as_str)
                .is_some_and(|name| name.contains("MandyFusion"))
        })
        .filter(|mesh| {
            let Some(material_id) = mesh.get("materialId").and_then(JsonValue::as_str) else {
                return false;
            };
            materials.get(material_id).is_some_and(|material| {
                material
                    .get("textureDataUrl")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|value| value.starts_with("data:image/png;base64,"))
            })
        })
        .count();
    assert!(
        textured_mandy_mesh_count >= 2,
        "Fusion Mandy meshes should reference decoded exact material textures"
    );
}

#[test]
fn npc_otto_authoring_preview_preserves_gltf_material_binding() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let preview = preview_authoring_model(path.to_string_lossy().to_string())
        .expect("preview npc_otto GLB");

    let materials = preview["materials"].as_object().expect("materials map");
    assert_eq!(materials.len(), 1);
    let (material_id, material) = materials.iter().next().unwrap();
    assert_eq!(material["name"], json!("npc_otto"));
    assert_eq!(material["color"], json!("#ffffff"));
    assert_eq!(material["cullOff"], json!(true));
    assert!(material["textureDataUrl"]
        .as_str()
        .is_some_and(|value| value.starts_with("data:image/png;base64,")));
    assert!(material["textureWidth"]
        .as_u64()
        .is_some_and(|value| value > 0));
    assert!(material["textureHeight"]
        .as_u64()
        .is_some_and(|value| value > 0));

    let mesh = preview["meshes"]
        .as_array()
        .and_then(|meshes| meshes.first())
        .expect("preview mesh");
    assert_eq!(mesh["materialId"], json!(material_id));
    assert_eq!(mesh.pointer("/material/id"), Some(&json!(material_id)));
    assert_eq!(mesh.pointer("/material/name"), Some(&json!("npc_otto")));
    assert_eq!(
        mesh["submeshMaterialIds"]
            .as_array()
            .and_then(|ids| ids.first())
            .and_then(JsonValue::as_str),
        Some(material_id.as_str())
    );
    assert_eq!(
        mesh["materialIds"]
            .as_array()
            .and_then(|ids| ids.first())
            .and_then(JsonValue::as_str),
        Some(material_id.as_str())
    );
    assert!(mesh["groups"]
        .as_array()
        .is_some_and(|groups| !groups.is_empty()));
    assert_ne!(mesh.pointer("/material/color"), Some(&json!("#7b9a8b")));
}

#[test]
fn clean_material_includes_official_shader_map_layout() {
    let asset = test_clean_asset();
    let material = clean_material(
        &asset,
        "npc_otto_material",
        7,
        1,
        NPC_SHARED_SHADER_PATH_ID,
        None,
        None,
    )
    .expect("material");

    let saved = material
        .as_object()
        .and_then(|object| object.get("m_SavedProperties"))
        .and_then(fusionforge::UnityValue::as_object)
        .expect("saved properties");
    let tex_envs = saved
        .get("m_TexEnvs")
        .and_then(fusionforge::UnityValue::as_array)
        .expect("tex envs");

    let mut names = BTreeSet::new();
    let mut shader_map_is_null = false;
    let mut spec_map_is_null = false;

    for tex_env in tex_envs {
        let (left, right) = pair_values(tex_env);
        let name = left
            .as_object()
            .and_then(|object| object.get("name"))
            .and_then(fusionforge::UnityValue::as_str)
            .expect("tex env name");
        names.insert(name.to_string());
        let texture = right
            .as_object()
            .and_then(|object| object.get("m_Texture"))
            .expect("tex env texture");
        if name == "_ShaderMap" {
            shader_map_is_null = matches!(
                texture,
                fusionforge::UnityValue::Pointer(pointer) if pointer.is_null()
            );
        } else if name == "_SpecMap" {
            spec_map_is_null = matches!(
                texture,
                fusionforge::UnityValue::Pointer(pointer) if pointer.is_null()
            );
        }
    }

    assert_eq!(
        names,
        BTreeSet::from([
            "_MainTex".to_string(),
            "_ShaderMap".to_string(),
            "_SpecMap".to_string(),
        ])
    );
    assert!(shader_map_is_null, "expected null _ShaderMap texture");
    assert!(spec_map_is_null, "expected null _SpecMap texture");
}

#[test]
fn npc_otto_clean_material_uses_authored_gltf_color_and_emission() {
    let repo_root = crate::repository_root().parent().unwrap();
    let path = repo_root.join("Models").join("npc_otto.glb");
    let imported = fusionforge::modding::ImportedMaterial::primary_from_model_path(&path)
        .expect("read npc_otto material")
        .expect("npc_otto primary material");
    assert!(imported.base_color_texture.is_some());
    assert!(imported.double_sided);

    let asset = test_clean_asset();
    let material = clean_material(
        &asset,
        "npc_otto_material",
        7,
        0,
        15,
        Some((0, 16)),
        Some(&imported),
    )
    .expect("clean npc_otto material");
    let saved = material
        .get("m_SavedProperties")
        .and_then(fusionforge::UnityValue::as_object)
        .expect("saved properties");
    let colors = saved
        .get("m_Colors")
        .and_then(fusionforge::UnityValue::as_array)
        .expect("material colors");
    let color = colors
        .iter()
        .find_map(|entry| {
            let (name, value) = pair_values(entry);
            (name.get("name").and_then(fusionforge::UnityValue::as_str) == Some("_Color"))
                .then_some(value)
        })
        .expect("_Color");
    assert_eq!(
        color.get("r").and_then(fusionforge::UnityValue::as_f64),
        Some(1.0)
    );
    assert_eq!(
        color.get("g").and_then(fusionforge::UnityValue::as_f64),
        Some(1.0)
    );
    assert_eq!(
        color.get("b").and_then(fusionforge::UnityValue::as_f64),
        Some(1.0)
    );
    assert_eq!(
        color.get("a").and_then(fusionforge::UnityValue::as_f64),
        Some(1.0)
    );
}
