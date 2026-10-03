use super::*;

#[test]
    fn fusion_renderer_order_is_independent_from_mesh_object_order() {
        let main =
            legacy_material_compositor_rank("SkinnedToonShading_blendSrcalphaInvsrcalpha", false);
        let effect = legacy_material_compositor_rank(FUSION_EFFECT_SHADER, false);
        let eye = legacy_material_compositor_rank("normal_blendOneOneTest_cullOff", true);
        assert_eq!((main, effect, eye), (0, 1, 2));

        // Edd's Unity Mesh objects arrive eye/effect/main, while Gwen arrives
        // eye/main/effect. Both must publish the same main/effect/eye order.
        for mut renderers in [
            vec![(0, 3_000, eye), (1, 2_900, effect), (2, 2_900, main)],
            vec![(0, 3_000, eye), (1, 2_900, main), (2, 2_900, effect)],
        ] {
            renderers.sort_by_key(|&(mesh, queue, compositor)| {
                legacy_renderer_sort_key(queue, compositor, mesh, mesh)
            });
            assert_eq!(
                renderers
                    .iter()
                    .map(|(_, queue, compositor)| (*queue, *compositor))
                    .collect::<Vec<_>>(),
                vec![(2_900, main), (2_900, effect), (3_000, eye)]
            );
        }
    }

pub(super) fn glb_json(bytes: &[u8]) -> Value {
        assert_eq!(&bytes[0..4], b"glTF");
        let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        serde_json::from_slice(&bytes[20..20 + json_len]).unwrap()
    }

#[test]
    fn publishes_sibling_joint_chains_under_their_proven_common_model_root() {
        let mut value = fixture();
        value["modelHierarchy"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(node(
                "OtherRoot",
                "True Hero/OtherRoot",
                Some("True Hero"),
                6,
            ));
        value["skeleton"]["joints"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "path": "OtherRoot",
                "parent": null,
                "sourceAssetIndex": 0,
                "transformPathId": 6,
                "translation": [0.0, 0.0, 0.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0]
            }));
        value["meshes"][0]["skin"]["jointPaths"]
            .as_array_mut()
            .unwrap()
            .push(json!("OtherRoot"));
        value["meshes"][0]["skin"]["inverseBindMatrices"]
            .as_array_mut()
            .unwrap()
            .push(json!(identity_matrix()));
        value["meshes"][0]["skin"]["boneIndices"][8] = json!(2);

        let source: SourceDocument = serde_json::from_value(value).unwrap();
        let converted = convert_source(&source).unwrap();
        assert_eq!(converted.model.skins.len(), 1);
        assert_eq!(converted.model.skins[0].skeleton_root, 0);
        assert_eq!(converted.model.skins[0].joints.len(), 3);
    }

#[test]
    fn self_skinned_legacy_renderer_uses_the_unique_mesh_named_output_node() {
        let mut value = fixture();
        value["meshes"][0]["sourceBindings"][0]["transformPath"] = json!("True Hero/Bip01/Bone");
        value["meshes"][0]["sourceBindings"][0]["transformPathId"] = json!(3);
        value["meshes"][0]["skin"]["rendererTransformPathId"] = json!(3);
        value["rendererMaterialBindings"][0]["gameObject"] =
            source_object("fixture:go-bone", 1003, "GameObject");

        let source: SourceDocument = serde_json::from_value(value).unwrap();
        let converted = convert_source(&source).unwrap();
        let bone = converted
            .model
            .nodes
            .iter()
            .position(|node| node.name == "Bone")
            .unwrap();
        let body = converted
            .model
            .nodes
            .iter()
            .position(|node| node.name == "Body")
            .unwrap();

        assert!(converted.model.nodes[bone].mesh.is_none());
        assert!(converted.model.nodes[bone].skin.is_none());
        assert_eq!(converted.model.nodes[body].mesh, Some(0));
        assert_eq!(converted.model.nodes[body].skin, Some(0));
        assert!(converted.model.skins[0].joints.contains(&(bone as u32)));
    }

#[test]
    fn accepts_typed_exact_character_root_mesh_selection_proof() {
        let mut value = fixture();
        value["exactMeshSelectionProof"] = json!({
            "schema": "ffone.exact-character-root-mesh-selection.v1",
            "policy": "retain-only-meshes-under-proven-selected-character-roots",
            "excludedCandidateMeshes": 4,
            "selectedMeshes": 2,
            "sourceWarning": "Ignored 4 mesh(es) from unrelated character roots in the shared preload range.",
            "warningDisposition": "resolved-as-positive-character-root-ownership-proof"
        });

        let source: SourceDocument = serde_json::from_value(value).unwrap();
        convert_source(&source).expect("typed ownership proof resolves the source warning");
    }

#[test]
    fn preserves_empty_trs_binding_as_typed_digest_bound_metadata_without_a_gltf_channel() {
        let (temp, report) = publish_fixture(&fixture_with_empty_trs_binding());
        assert_eq!(report.contract.source.empty_trs_bindings, 1);
        assert_eq!(report.contract.published.empty_trs_bindings, 1);
        assert_eq!(report.contract.source.animation_channels, 2);
        assert_eq!(report.contract.published.animation_channels, 2);
        assert_eq!(
            report.semantic_proof.source.animation_metadata_sha256,
            report.semantic_proof.emitted.animation_metadata_sha256
        );

        let output = temp.path().join("output");
        let glb = fs::read(output.join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let animation = &document["animations"][0];
        assert_eq!(animation["channels"].as_array().unwrap().len(), 2);
        assert_eq!(
            animation["extras"]["nonTrs"]["emptyTrsBindings"],
            json!([{
                "kind": "translation",
                "targetNode": 1,
                "targetPath": "Bip01",
                "sourceIndex": 1,
                "sourceEncoding": "plain"
            }])
        );
        let audit = crate::audit_logical_model_tree(&output).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
        assert_eq!(audit.counts.features.empty_trs_bindings, 1);
    }

#[test]
    fn biped_display_name_filter_is_exact_and_rejects_legitimate_mesh_names() {
        assert!(is_biped_display_name("Biped Object"));
        assert!(is_biped_display_name("Biped Object@#0"));
        assert!(is_biped_display_name("Biped Object@#517"));
        for legitimate in [
            "Biped Object Costume",
            "Biped Object@#",
            "Biped Object@#17_LOD0",
            "Biped Object@#hair",
            "biped object",
            "BipedObject",
        ] {
            assert!(
                !is_biped_display_name(legitimate),
                "{legitimate:?} must remain renderable"
            );
        }
    }

#[test]
    fn renderer_binding_is_authoritative_for_shared_mesh_with_different_materials() {
        let mut value = fixture();
        value["meshes"][1]["id"] = json!("fixture:mesh-body");
        value["meshes"][0]["materialIds"] = json!(["fixture:mat-sword"]);
        value["meshes"][1]["materialIds"] = json!(["fixture:mat-body"]);
        value["rendererMaterialBindings"][1]["mesh"] =
            source_object("fixture:mesh-body", 500, "Mesh");
        let (temp, report) = publish_fixture(&value);
        let glb_path = temp.path().join("output").join(report.contract.output_glb);
        let document = glb_json(&fs::read(glb_path).unwrap());
        let materials = document["materials"].as_array().unwrap();
        let meshes = document["meshes"].as_array().unwrap();
        let material_name = |mesh_name: &str| {
            let mesh = meshes
                .iter()
                .find(|mesh| mesh["name"] == mesh_name)
                .unwrap();
            let index = mesh["primitives"][0]["material"].as_u64().unwrap() as usize;
            materials[index]["name"].as_str().unwrap()
        };
        assert_eq!(material_name("Body"), "Body Material");
        assert_eq!(material_name("Sword"), "Sword Material");
    }
