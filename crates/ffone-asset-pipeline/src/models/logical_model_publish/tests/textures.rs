use super::*;

pub(super) fn texture(id: &str, path_id: i64, name: &str) -> Value {
        let png = BASE64_STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/lIFZfgAAAABJRU5ErkJggg==")
            .unwrap();
        let decoded_rgba = [255_u8, 0, 0, 255];
        let payload = json!({
            "kind": "decoded-rgba8-png",
            "mimeType": "image/png",
            "dataUrl": format!("data:image/png;base64,{}", BASE64_STANDARD.encode(&png)),
            "byteLength": png.len(),
            "sha256": sha256_hex(&png),
            "decodedRgbaSha256": sha256_hex(&decoded_rgba),
            "pixelTransform": "vertical-flip-only-for-png-top-left-origin",
            "resized": false,
            "rgbRepair": false,
            "alphaMaskApplied": false,
            "tintApplied": false
        });
        json!({
            "id": id,
            "source": source_object(id, path_id, "Texture2D"),
            "name": name,
            "width": 1,
            "height": 1,
            "textureFormat": 4,
            "textureFormatName": "RGBA32",
            "mipCount": 1,
            "mipCountEvidence": {"method": "fixture-exact-bytes"},
            "sampler": {
                "filterMode": {"value": 1, "source": "m_TextureSettings.m_FilterMode", "serialized": true},
                "wrapMode": {"value": 0, "source": "m_TextureSettings.m_WrapMode", "serialized": true},
                "aniso": {"value": 1, "source": "m_TextureSettings.m_Aniso", "serialized": true},
                "mipBias": {"value": 0.0, "source": "m_TextureSettings.m_MipBias", "serialized": true}
            },
            "sourcePayload": {
                "byteLength": decoded_rgba.len(),
                "sha256": sha256_hex(&decoded_rgba),
                "embeddedInExactSource": false,
                "layout": "largest-to-smallest-contiguous",
                "levelCount": 1
            },
            "mipLevels": [{
                "level": 0,
                "width": 1,
                "height": 1,
                "sourceByteOffset": 0,
                "sourceByteLength": decoded_rgba.len(),
                "sourceByteSha256": sha256_hex(&decoded_rgba),
                "decodedRgbaByteLength": decoded_rgba.len(),
                "decodedRgbaSha256": sha256_hex(&decoded_rgba),
                "payload": payload.clone()
            }],
            "payload": payload
        })
    }

#[test]
    fn preserves_a_named_dangling_texture_property_as_an_unassigned_binding() {
        let mut value = fixture();
        value["materials"]["fixture:mat-body"]["savedProperties"]["textureEnvs"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "slot": 1,
                "name": "_ShaderMap",
                "unassignedSlot": true,
                "textureId": null,
                "texturePointer": {
                    "sourceAssetIndex": 0,
                    "fileId": 163_774_465_i64,
                    "pathId": -621_565_751_i64,
                    "isNull": false
                },
                "scale": {"x":1.0,"y":1.0},
                "offset": {"x":0.0,"y":0.0},
                "pivot": null,
                "rotation": null
            }));

        let (temp, report) = publish_fixture(&value);
        let glb = fs::read(temp.path().join("output").join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let bindings = document["materials"][0]["extras"]["ffone"]["textureBindings"]
            .as_array()
            .unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[1]["slot"], "_ShaderMap");
        assert_eq!(bindings[1]["unassignedStaleNull"], true);
        assert!(bindings[1]["texture"].is_null());
        let audit = crate::audit_logical_model_tree(&temp.path().join("output")).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
    }

#[test]
    fn rejects_a_named_unassigned_texture_property_that_carries_a_null_pointer() {
        let mut value = fixture();
        value["materials"]["fixture:mat-body"]["savedProperties"]["textureEnvs"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "slot": 1,
                "name": "_ShaderMap",
                "unassignedSlot": true,
                "textureId": null,
                "texturePointer": pointer(0),
                "scale": {"x":1.0,"y":1.0},
                "offset": {"x":0.0,"y":0.0},
                "pivot": null,
                "rotation": null
            }));

        let source: SourceDocument = serde_json::from_value(value).unwrap();
        let error = match convert_source(&source) {
            Ok(_) => panic!("a named unassigned slot with a null pointer must be rejected"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("contradictory unassigned texture property"),
            "{error}"
        );
    }

#[test]
    fn publishes_exact_materials_whose_texture_slots_are_all_explicitly_null() {
        let mut value = fixture();
        for material in value["materials"].as_object_mut().unwrap().values_mut() {
            for environment in material["savedProperties"]["textureEnvs"]
                .as_array_mut()
                .unwrap()
            {
                environment["textureId"] = Value::Null;
                environment["texturePointer"] = pointer(0);
            }
        }
        value["textures"] = json!({});

        let (temp, report) = publish_fixture(&value);
        assert_eq!(report.material_publish.material_count, 2);
        assert_eq!(report.material_publish.texture_count, 0);
        let glb_path = temp.path().join("output").join(report.contract.output_glb);
        let document = glb_json(&fs::read(glb_path).unwrap());
        assert_eq!(document["materials"].as_array().unwrap().len(), 2);
        assert!(document.get("textures").is_none());
        let audit = crate::audit_logical_model_tree(&temp.path().join("output")).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
    }

#[test]
    fn rejects_unrelated_material_or_texture_resources_in_the_exact_source() {
        let mut value = fixture();
        value["textures"].as_object_mut().unwrap().insert(
            "fixture:tex-extra".to_string(),
            texture("fixture:tex-extra", 499, "Extra Texture.dds"),
        );
        let error = publish_error(&value);
        assert!(error.to_string().contains("texture resources not used"));
    }

#[test]
    fn disambiguates_distinct_texture_objects_that_share_a_windows_png_name() {
        let mut value = fixture();
        value["textures"]["fixture:tex-body"]["name"] = json!("Collision.dds");
        value["textures"]["fixture:tex-sword"]["name"] = json!("Collision");
        let different_png = BASE64_STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z8S8AAAAASUVORK5CYII=")
            .unwrap();
        value["textures"]["fixture:tex-sword"]["payload"]["dataUrl"] = json!(format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(&different_png)
        ));
        value["textures"]["fixture:tex-sword"]["payload"]["byteLength"] =
            json!(different_png.len());
        value["textures"]["fixture:tex-sword"]["payload"]["sha256"] =
            json!(sha256_hex(&different_png));
        value["textures"]["fixture:tex-sword"]["mipLevels"][0]["payload"] =
            value["textures"]["fixture:tex-sword"]["payload"].clone();
        let (temp, report) = publish_fixture(&value);
        let textures = &report.material_publish.textures;
        assert_eq!(textures.len(), 2);
        assert_eq!(textures[0].source_name, "Collision.dds");
        assert_eq!(textures[1].source_name, "Collision");
        assert_ne!(
            textures[0].uri.to_ascii_lowercase(),
            textures[1].uri.to_ascii_lowercase()
        );
        assert!(
            textures
                .iter()
                .any(|texture| texture.uri.ends_with("Collision.dds.png"))
        );
        let glb_parent = temp
            .path()
            .join("output")
            .join(&report.contract.output_glb)
            .parent()
            .unwrap()
            .to_owned();
        for texture in textures {
            assert!(glb_parent.join(&texture.uri).is_file());
        }
    }
