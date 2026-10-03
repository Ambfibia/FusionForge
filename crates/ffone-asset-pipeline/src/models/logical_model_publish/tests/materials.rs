use super::*;

#[test]
    fn packed_shader_masks_publish_as_linear_data() {
        assert_eq!(texture_color_space("_MaskTex"), TextureColorSpace::Linear);
        assert_eq!(texture_color_space("_MainTex"), TextureColorSpace::Srgb);
    }

pub(super) fn transparent_shader_script() -> String {
        r#"Shader "normal_blendSrcalphaInvsrcalpha_zwriteOff" {
Properties {
_MainTex ("Base (RGB)", 2D) = "white" {}
}
Category {
Tags {"Queue"="Transparent+10"}
Blend SrcAlpha OneMinusSrcAlpha
ColorMask RGBA
ZWrite Off
SubShader {
Pass {
}
}
}
}"#
        .to_string()
    }

pub(super) fn toon_shader_script() -> String {
        r#"Shader "SkinnedToonShading_blendSrcalphaInvsrcalpha" {
Properties {
_MainTex ("Base (RGB)", 2D) = "white" {}
}
Category {
Tags {"Queue"="Transparent-100"}
Blend SrcAlpha OneMinusSrcAlpha
ColorMask RGBA
SubShader {
Pass {
Name "BASE"
Program "" {
Local 1, ([_Outline],0,0,0)
Local 2, [_OutlineColor]
Local 3, ([_FatFactor],0,0,0)
}
Cull Back
}
Pass {
Name "OUTLINE"
Cull Front
ZWrite On
ColorMask RGBA
Blend SrcAlpha OneMinusSrcAlpha
}
}
}
}"#
        .to_string()
    }

pub(super) fn shader(name: &str, script: String) -> Value {
        json!({
            "name": name,
            "declaredName": name,
            "script": {"encoding": "utf-8", "text": script},
            "scriptByteLength": script.len(),
            "scriptSha256": sha256_hex(script.as_bytes())
        })
    }

pub(super) fn add_exact_material_fixture(value: &mut Value) {
        let body_script = toon_shader_script();
        let sword_script = transparent_shader_script();
        value["meshes"][0]["id"] = json!("fixture:mesh-body");
        value["meshes"][1]["id"] = json!("fixture:mesh-sword");
        value["materials"] = json!({
            "fixture:mat-body": {
                "id": "fixture:mat-body",
                "source": source_object("fixture:mat-body", 300, "Material"),
                "name": "Body Material",
                "shaderName": "SkinnedToonShading_blendSrcalphaInvsrcalpha",
                "shader": shader("SkinnedToonShading_blendSrcalphaInvsrcalpha", body_script),
                "renderQueue": null,
                "renderState": {"shaderEvidence": "fixture"},
                "savedProperties": {
                    "colors": [{"slot": 0, "name": "_OutlineColor", "value": {"r":0.0,"g":0.0,"b":0.0,"a":1.0}}],
                    "floats": [{"slot": 0, "name": "_Outline", "value": 0.005}],
                    "textureEnvs": [{
                        "slot": 0,
                        "name": "_MainTex",
                        "textureId": "fixture:tex-body",
                        "texturePointer": pointer(400),
                        "scale": {"x":1.0,"y":1.0},
                        "offset": {"x":0.0,"y":0.0},
                        "pivot": {"x":0.0,"y":0.0},
                        "rotation": 0.0
                    }]
                }
            },
            "fixture:mat-sword": {
                "id": "fixture:mat-sword",
                "source": source_object("fixture:mat-sword", 301, "Material"),
                "name": "Sword Material",
                "shaderName": "normal_blendSrcalphaInvsrcalpha_zwriteOff",
                "shader": shader("normal_blendSrcalphaInvsrcalpha_zwriteOff", sword_script),
                "renderQueue": null,
                "renderState": {"shaderEvidence": "fixture"},
                "savedProperties": {
                    "colors": [],
                    "floats": [],
                    "textureEnvs": [{
                        "slot": 0,
                        "name": "_MainTex",
                        "textureId": "fixture:tex-sword",
                        "texturePointer": pointer(401),
                        "scale": {"x":1.0,"y":1.0},
                        "offset": {"x":0.0,"y":0.0},
                        "pivot": {"x":0.0,"y":0.0},
                        "rotation": 0.0
                    }]
                }
            }
        });
        value["textures"] = json!({
            "fixture:tex-body": texture("fixture:tex-body", 400, "Body Diffuse.dds"),
            "fixture:tex-sword": texture("fixture:tex-sword", 401, "Sword Diffuse.dds")
        });
        value["rendererMaterialBindings"] = json!([{
            "renderer": source_object("fixture:renderer-body", 100, "SkinnedMeshRenderer"),
            "rendererType": "SkinnedMeshRenderer",
            "gameObject": source_object("fixture:go-body", 1004, "GameObject"),
            "mesh": source_object("fixture:mesh-body", 500, "Mesh"),
            "materialSlots": [{"slot":0,"materialId":"fixture:mat-body","pointer":pointer(300)}]
        }, {
            "renderer": source_object("fixture:renderer-sword", 201, "MeshRenderer"),
            "rendererType": "MeshRenderer",
            "gameObject": source_object("fixture:go-sword", 1005, "GameObject"),
            "mesh": source_object("fixture:mesh-sword", 501, "Mesh"),
            "materialSlots": [{"slot":0,"materialId":"fixture:mat-sword","pointer":pointer(301)}]
        }]);
        value["meshes"][0]["materialIds"] = json!(["fixture:mat-body"]);
        value["meshes"][1]["materialIds"] = json!(["fixture:mat-sword"]);
    }

#[test]
    fn preserves_unassigned_stale_material_slot_without_inventing_a_shader_default() {
        let mut value = fixture();
        value["materials"]["fixture:mat-body"]["savedProperties"]["textureEnvs"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "slot": 1,
                "name": "_SpecMap",
                "textureId": null,
                "texturePointer": pointer(0),
                "scale": {"x":1.0,"y":1.0},
                "offset": {"x":0.0,"y":0.0},
                "pivot": {"x":0.0,"y":0.0},
                "rotation": 0.0
            }));

        let (temp, report) = publish_fixture(&value);
        let glb = fs::read(temp.path().join("output").join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let ffone = &document["materials"][0]["extras"]["ffone"];
        assert_eq!(ffone["shaderTextureDefaults"].as_array().unwrap().len(), 1);
        assert_eq!(ffone["textureBindings"].as_array().unwrap().len(), 2);
        assert_eq!(ffone["textureBindings"][1]["slot"], "_SpecMap");
        assert!(ffone["textureBindings"][1]["texture"].is_null());
        let audit = crate::audit_logical_model_tree(&temp.path().join("output")).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
    }

#[test]
    fn serialized_shader_name_may_differ_from_exact_proven_shaderlab_declaration() {
        let mut value = fixture();
        let declared = "SkinnedToonShading_blendSrcalphaInvsrcalpha e1";
        let script = toon_shader_script()
            .replacen(
                "SkinnedToonShading_blendSrcalphaInvsrcalpha",
                declared,
                1,
            )
            .replacen(
                "_MainTex (\"Base (RGB)\", 2D) = \"white\" {}",
                "_OutlineColor (\"Outline Color\", Color) = (0,0,0,1)\n_Outline (\"Outline\", Float) = .005\n_MainTex (\"Base (RGB)\", 2D) = \"white\" {}",
                1,
            );
        let material = &mut value["materials"]["fixture:mat-body"];
        material["shaderName"] = json!("skinntoone2");
        material["shader"]["name"] = json!("skinntoone2");
        material["shader"]["declaredName"] = json!(declared);
        material["shader"]["script"]["text"] = json!(script);
        material["shader"]["scriptByteLength"] = json!(script.len());
        material["shader"]["scriptSha256"] = json!(sha256_hex(script.as_bytes()));
        material["savedProperties"]["colors"] = json!([{
            "slot": 0,
            "name": "_Color",
            "value": {"r":1.0,"g":1.0,"b":1.0,"a":1.0}
        }]);
        material["savedProperties"]["floats"] = json!([]);

        let (temp, report) = publish_fixture(&value);
        let published = report
            .material_publish
            .materials
            .iter()
            .find(|material| material.name == "Body Material")
            .unwrap();
        assert_eq!(published.serialized_shader_name, "skinntoone2");
        assert_eq!(published.declared_shader_name, declared);
        assert_eq!(published.legacy_shader_name, declared);
        let document = glb_json(
            &fs::read(temp.path().join("output").join(report.contract.output_glb)).unwrap(),
        );
        let exact = &document["materials"][0]["extras"]["ffone"];
        assert_eq!(exact["serializedShaderName"], "skinntoone2");
        assert_eq!(exact["declaredShaderName"], declared);
        assert_eq!(exact["legacyShaderName"], declared);
        assert_eq!(exact["colors"].as_array().unwrap().len(), 1);
        assert!(exact["floats"].as_array().unwrap().is_empty());
        assert_eq!(exact["passes"][1]["outline"]["width"], 0.005);
        assert_eq!(
            exact["passes"][1]["outline"]["color"],
            json!([0.0, 0.0, 0.0, 1.0])
        );

        let mut tampered = value;
        tampered["materials"]["fixture:mat-body"]["shader"]["declaredName"] =
            json!("SkinnedToonShading_blendSrcalphaInvsrcalpha");
        let error = publish_error(&tampered);
        assert!(
            error
                .to_string()
                .contains("contradicts exact script declaration"),
            "unexpected error: {error}"
        );
    }

#[test]
    fn preserves_an_explicit_null_renderer_slot_without_inventing_a_material() {
        let mut value = fixture();
        value["rendererMaterialBindings"][1]["materialSlots"][0]["materialId"] = Value::Null;
        value["rendererMaterialBindings"][1]["materialSlots"][0]["pointer"] = pointer(0);
        value["materials"]
            .as_object_mut()
            .unwrap()
            .remove("fixture:mat-sword");
        value["textures"]
            .as_object_mut()
            .unwrap()
            .remove("fixture:tex-sword");
        let (temp, report) = publish_fixture(&value);
        assert_eq!(report.material_publish.explicit_null_slots, 1);
        assert_eq!(report.material_publish.material_count, 1);
        assert_eq!(report.contract.source.material_slots, 2);
        let glb_path = temp.path().join("output").join(report.contract.output_glb);
        let document = glb_json(&fs::read(glb_path).unwrap());
        let sword = document["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mesh| mesh["name"] == "Sword")
            .unwrap();
        assert!(sword["primitives"][0].get("material").is_none());
        assert_eq!(document["materials"].as_array().unwrap().len(), 1);
    }

#[test]
    fn repeats_the_last_unity_submesh_for_extra_renderer_material_slots() {
        let mut value = fixture();
        value["rendererMaterialBindings"][0]["materialSlots"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "slot": 1,
                "materialId": "fixture:mat-body",
                "pointer": pointer(300)
            }));

        let (temp, report) = publish_fixture(&value);
        assert_eq!(report.contract.source.material_slots, 3);
        let glb_path = temp.path().join("output").join(report.contract.output_glb);
        let document = glb_json(&fs::read(glb_path).unwrap());
        let body = document["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mesh| mesh["name"] == "Body")
            .unwrap();
        let primitives = body["primitives"].as_array().unwrap();
        assert_eq!(primitives.len(), 2);
        assert_eq!(primitives[0]["material"], primitives[1]["material"]);
        let accessors = document["accessors"].as_array().unwrap();
        let first = &accessors[primitives[0]["indices"].as_u64().unwrap() as usize];
        let repeated = &accessors[primitives[1]["indices"].as_u64().unwrap() as usize];
        for field in ["componentType", "count", "type", "min", "max"] {
            assert_eq!(first[field], repeated[field]);
        }
    }
