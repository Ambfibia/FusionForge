use super::*;

#[test]
    fn winding_uses_the_persisted_f32_geometry_for_almost_collinear_triangles() {
        // Three primary Ben vertices: f64 export intermediates agree with the
        // normals, but their FLOAT accessor payload has the opposite sign.
        let mut value = fixture();
        value["meshes"][0]["positions"] = json!([
            0.4057001332222266,
            0.8595896311893461,
            0.014777611792634593,
            0.4043018028649883,
            0.858009498544998,
            0.014802754662128548,
            0.4070984635794649,
            0.8611697638336941,
            0.014750534856256547
        ]);
        value["meshes"][0]["normals"] = json!([
            0.0117646161247702,
            0.0117646161247702,
            0.9997231876148736,
            0.0117646161247702,
            0.0117646161247702,
            0.9997231876148736,
            0.09803846770641855,
            0.10588154512293202,
            0.979177557252158
        ]);
        let source: SourceDocument = serde_json::from_value(value).unwrap();
        let converted = convert_source(&source).unwrap();
        let glb = encode_glb(&converted.model).unwrap();
        prove_semantic_roundtrip(&converted.model, &glb).unwrap();
        ffone_skinned_model::gpu_model_facts_from_glb(&glb).unwrap();
    }

#[test]
    fn signed_format7_file_id_is_valid_for_the_exporters_exact_resolved_target() {
        let pointer = SourcePointer {
            source_asset_index: 1,
            file_id: -1_050_476_544,
            path_id: 611_491_380,
            is_null: false,
        };
        let target = SourceObjectReference {
            id: "Asset:611491380".to_string(),
            asset_index: 1,
            path_id: 611_491_380,
            object_type: "Material".to_string(),
        };

        validate_pointer_identity(&pointer, &target, "format-7 local material")
            .expect("the exact exporter resolved this signed fileId to the same source asset");

        let foreign_target = SourceObjectReference {
            asset_index: 2,
            ..target
        };
        validate_pointer_identity(&pointer, &foreign_target, "format-7 external material").expect(
            "a non-zero signed fileId can resolve by exact PathID into a loaded dependency",
        );

        let local_pointer = SourcePointer {
            file_id: 0,
            ..pointer
        };
        assert!(
            validate_pointer_identity(&local_pointer, &foreign_target, "foreign local material")
                .is_err(),
            "fileId zero remains strictly local to the pointer source asset"
        );
    }

pub(super) fn additive_test_channel(source_index: u32, values: TrackValues) -> AnimationChannel {
        AnimationChannel {
            target_node: source_index,
            source_index,
            source_encoding: EmptyTrsSourceEncoding::Plain,
            source_key_count: 2,
            source_key_indices: vec![0, 1],
            duplicate_keys: Vec::new(),
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values,
            in_tangents: None,
            out_tangents: None,
            tangent_modes: Vec::new(),
        }
    }

#[test]
    fn legacy_npc_additive_clips_are_rebased_against_the_first_sample() {
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let mut clips = vec![AnimationClip {
            name: "wound".to_owned(),
            duration: 1.0,
            declared_duration: Some(1.0),
            keyed_duration: Some(1.0),
            event_duration: None,
            sample_rate: Some(30.0),
            wrap_mode: Some(1),
            looped: false,
            channels: vec![
                additive_test_channel(
                    0,
                    TrackValues::Translation(vec![[5.0, 2.0, -1.0], [8.0, 1.0, 3.0]]),
                ),
                additive_test_channel(
                    1,
                    TrackValues::Rotation(vec![[0.0, 0.0, half, half], [0.0, 0.0, 1.0, 0.0]]),
                ),
                additive_test_channel(
                    2,
                    TrackValues::Scale(vec![[1.0, 2.0, 3.0], [2.0, 4.0, 6.0]]),
                ),
            ],
            metadata: AnimationMetadata::default(),
        }];

        rebase_legacy_npc_additive_clips(&mut clips).unwrap();

        assert_eq!(
            clips[0].channels[0].values,
            TrackValues::Translation(vec![[0.0, 0.0, 0.0], [3.0, -1.0, 4.0]])
        );
        let TrackValues::Rotation(rotations) = &clips[0].channels[1].values else {
            panic!("rotation channel changed kind");
        };
        assert!(rotations[0][0..3].iter().all(|value| value.abs() < 1.0e-12));
        assert!((rotations[0][3] - 1.0).abs() < 1.0e-12);
        assert!((rotations[1][2] - half).abs() < 1.0e-12);
        assert!((rotations[1][3] - half).abs() < 1.0e-12);
        assert_eq!(
            clips[0].channels[2].values,
            TrackValues::Scale(vec![[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]])
        );
    }

pub(super) fn external_source_files(directory: &Path, output: &mut Vec<PathBuf>) {
        let mut entries = fs::read_dir(directory)
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                external_source_files(&path, output);
            } else if path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.ends_with(".source.json"))
            {
                output.push(path);
            }
        }
    }

pub(super) fn identity_matrix() -> Value {
        json!([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]
        ])
    }

pub(super) fn node(name: &str, path: &str, parent: Option<&str>, id: i64) -> Value {
        json!({
            "name": name,
            "path": path,
            "parent": parent,
            "sourceAssetIndex": 0,
            "transformPathId": id,
            "gameObjectPathId": id + 1000,
            "translation": [0.0, 0.0, 0.0],
            "rotation": [0.0, 0.0, 0.0, 1.0],
            "scale": [1.0, 1.0, 1.0]
        })
    }

pub(super) fn pointer(path_id: i64) -> Value {
        json!({
            "sourceAssetIndex": 0,
            "fileId": 0,
            "pathId": path_id,
            "isNull": path_id == 0
        })
    }

pub(in super::super) fn fixture() -> Value {
        let identity = identity_matrix();
        let mut value = json!({
            "schema": LOGICAL_MODEL_SOURCE_SCHEMA,
            "selectionMode": "exact-container-route",
            "status": "ready",
            "logicalName": "True Hero",
            "exactContainerRoute": "mob/true_hero.kfm",
            "containerPaths": ["mob/true_hero.kfm"],
            "matchedPaths": ["mob/true_hero.kfm"],
            "nativeCoordinateContract": {
                "schema": "ffone.native-coordinate-contract.v1",
                "publishedSpace": "gltf-right-handed-y-up",
                "position": "[-unity.x,unity.y,unity.z]",
                "normal": "[-unity.x,unity.y,unity.z]",
                "translation": "[-unity.x,unity.y,unity.z]",
                "rotation": "[unity.x,-unity.y,-unity.z,unity.w]",
                "scale": "unchanged",
                "uv": "[unity.u,1-unity.v]",
                "inverseBindMatrix": "H*unity*H where H=diag(-1,1,1,1)",
                "sourceTriangleWinding": "unity-clockwise-preserved",
                "publishedTriangleWinding": "swap-index-1-and-2-for-gltf-counter-clockwise",
                "windingConversionOwner": "ffone-asset-pipeline",
                "unitScale": "1-unity-unit-equals-1-bevy-unit",
                "originPolicy": "source-root-trs-unchanged-no-auto-centering",
                "autoCentered": false,
                "autoScaled": false
            },
            "warnings": [],
            "kfm": [{
                "source": "kfm",
                "path": "mob/true_hero.kfm",
                "clips": [],
                "references": [],
                "previewSupport": "metadata-only"
            }],
            "modelHierarchy": {
                "source": "unity-transform-hierarchy",
                "roots": [{
                    "name": "True Hero",
                    "path": "True Hero",
                    "sourceAssetIndex": 0,
                    "transformPathId": 1,
                    "gameObjectPathId": 1001
                }],
                "nodes": [
                    node("True Hero", "True Hero", None, 1),
                    node("Bip01", "True Hero/Bip01", Some("True Hero"), 2),
                    node("Bone", "True Hero/Bip01/Bone", Some("True Hero/Bip01"), 3),
                    node("Body", "True Hero/Body", Some("True Hero"), 4),
                    node("Sword", "True Hero/Sword", Some("True Hero"), 5)
                ]
            },
            "skeleton": {
                "source": "unity-transform-hierarchy",
                "space": "npc-root",
                "joints": [{
                    "path": "Bip01",
                    "parent": null,
                    "sourceAssetIndex": 0,
                    "transformPathId": 2,
                    "translation": [0.0, 0.0, 0.0],
                    "rotation": [0.0, 0.0, 0.0, 1.0],
                    "scale": [1.0, 1.0, 1.0]
                }, {
                    "path": "Bip01/Bone",
                    "parent": "Bip01",
                    "sourceAssetIndex": 0,
                    "transformPathId": 3,
                    "translation": [0.0, 0.0, 0.0],
                    "rotation": [0.0, 0.0, 0.0, 1.0],
                    "scale": [1.0, 1.0, 1.0]
                }]
            },
            "materials": {
                "mat-body": {"name": "Body Material", "shaderName": "Legacy/Toon"},
                "mat-sword": {"name": "Sword Material", "shaderName": "Legacy/Metal"}
            },
            "meshes": [{
                "name": "Body",
                "kind": "mesh",
                "positions": [0.0,0.0,0.0, 1.0,0.0,0.0, 0.0,1.0,0.0],
                "normals": [0.0,0.0,1.0, 0.0,0.0,1.0, 0.0,0.0,1.0],
                "uvs": [0.0,0.0, 1.0,0.0, 0.0,1.0],
                "indices": [0,1,2],
                "groups": [{"start":0,"count":3,"materialIndex":0}],
                "materialIds": ["mat-body"],
                "sourceBindings": [{
                    "componentType": "SkinnedMeshRenderer",
                    "rendererAssetIndex": 0,
                    "rendererPathId": 100,
                    "rootTransformName": "True Hero",
                    "rootTransformPathId": 1,
                    "transformAssetIndex": 0,
                    "transformPathId": 4,
                    "transformPath": "True Hero/Body"
                }],
                "skin": {
                    "source": "unity-skinned-mesh-renderer",
                    "jointPaths": ["Bip01", "Bip01/Bone"],
                    "inverseBindMatrices": [identity.clone(), identity.clone()],
                    "boneIndices": [0,0,0,0, 0,1,0,0, 1,0,0,0],
                    "weights": [1.0,0.0,0.0,0.0, 0.5,0.5,0.0,0.0, 1.0,0.0,0.0,0.0],
                    "rendererAssetIndex": 0,
                    "rendererPathId": 100,
                    "rendererTransformPathId": 4
                }
            }, {
                "name": "Sword",
                "kind": "mesh",
                "positions": [0.0,0.0,0.0, 0.0,1.0,0.0, 0.0,0.0,1.0],
                "normals": [],
                "uvs": [],
                "indices": [0,1,2],
                "groups": [{"start":0,"count":3,"materialIndex":0}],
                "materialIds": ["mat-sword"],
                "sourceBindings": [{
                    "componentType": "MeshFilter",
                    "componentAssetIndex": 0,
                    "componentPathId": 200,
                    "rootTransformName": "True Hero",
                    "rootTransformPathId": 1,
                    "transformAssetIndex": 0,
                    "transformPathId": 5,
                    "transformPath": "True Hero/Sword"
                }],
                "skin": {
                    "source": "unity-rigid-mesh-attachment",
                    "jointPaths": ["Bip01/Bone"],
                    "boneIndices": [0,0,0,0, 0,0,0,0, 0,0,0,0],
                    "weights": [1.0,0.0,0.0,0.0, 1.0,0.0,0.0,0.0, 1.0,0.0,0.0,0.0],
                    "meshFilterAssetIndex": 0,
                    "meshFilterPathId": 200,
                    "attachmentTransformPathId": 5,
                    "restVertexTransform": identity
                }
            }],
            "animations": [{
                "name": "stand",
                "duration": 1.0,
                "declaredDuration": 1.0,
                "keyedDuration": 1.0,
                "eventDuration": 0.5,
                "sampleRate": 30.0,
                "wrapMode": 2,
                "loop": true,
                "curveCounts": {
                    "compressedRotation": 1,
                    "rotation": 0,
                    "position": 1,
                    "scale": 0,
                    "euler": 0,
                    "float": 0,
                    "pptr": 0,
                    "events": 1
                },
                "animationData": {
                    "translations": [{
                        "path": "Bip01/Bone",
                        "interpolation": "CUBICSPLINE",
                        "keys": [{
                            "sourceKeyIndex": 0,
                            "time": 0.0,
                            "value": [0.0,0.0,0.0],
                            "inTangent": [0.0,0.0,0.0],
                            "outTangent": [0.0,0.0,0.0]
                        }, {
                            "sourceKeyIndex": 1,
                            "time": 1.0,
                            "value": [0.0,1.0,0.0],
                            "inTangent": [0.0,0.0,0.0],
                            "outTangent": [0.0,0.0,0.0]
                        }],
                        "duplicateKeys": [],
                        "sourceKeyCount": 2,
                        "sourceIndex": 0,
                        "sourceEncoding": "plain"
                    }],
                    "rotations": [{
                        "path": "Bip01/Bone",
                        "interpolation": "LINEAR",
                        "keys": [{"sourceKeyIndex":0,"time":0.0,"value":[0.0,0.0,0.0,1.0]},
                                 {"sourceKeyIndex":1,"time":1.0,"value":[0.0,0.0,0.0,1.0]}],
                        "duplicateKeys": [],
                        "sourceKeyCount": 2,
                        "sourceIndex": 0,
                        "sourceEncoding": "compressed"
                    }],
                    "scales": [],
                    "emptyTrsBindings": [],
                    "duplicateTrsBindings": [],
                    "timeRecoveries": [],
                    "curveRecoveries": []
                },
                "events": [{
                    "time": 0.5,
                    "functionName": "particle",
                    "stringParameter": "608",
                    "floatParameter": 0.0,
                    "intParameter": 0,
                    "objectParameter": null,
                    "objectParameterProvenance": {
                        "presence": "missing",
                        "interpretation": "missing"
                    },
                    "messageOptions": 1
                }]
            }]
        });
        add_exact_material_fixture(&mut value);
        value
    }

pub(super) fn fixture_with_biped_display_helper() -> Value {
        let mut value = fixture();
        let mut helper_material = value["materials"]["fixture:mat-sword"].clone();
        helper_material["id"] = json!("fixture:mat-biped-helper");
        helper_material["source"] = source_object("fixture:mat-biped-helper", 302, "Material");
        helper_material["name"] = json!("Biped Helper Material");
        helper_material["savedProperties"]["textureEnvs"][0]["textureId"] =
            json!("fixture:tex-biped-helper");
        helper_material["savedProperties"]["textureEnvs"][0]["texturePointer"] = pointer(402);
        value["materials"]
            .as_object_mut()
            .unwrap()
            .insert("fixture:mat-biped-helper".to_owned(), helper_material);
        value["textures"].as_object_mut().unwrap().insert(
            "fixture:tex-biped-helper".to_owned(),
            texture("fixture:tex-biped-helper", 402, "Biped Helper Diffuse.dds"),
        );
        value["modelHierarchy"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(node(
                "Biped Object@#17",
                "True Hero/Bip01/Bone/Biped Object@#17",
                Some("True Hero/Bip01/Bone"),
                6,
            ));
        value["meshes"].as_array_mut().unwrap().push(json!({
            "id": "fixture:mesh-biped-helper",
            "name": "Biped Object@#17",
            "kind": "mesh",
            "positions": [0.0,0.0,0.0, 0.1,0.0,0.0, 0.0,0.1,0.0],
            "normals": [],
            "uvs": [],
            "indices": [0,1,2],
            "groups": [{"start":0,"count":3,"materialIndex":0}],
            "materialIds": ["fixture:mat-biped-helper"],
            "sourceBindings": [{
                "componentType": "MeshFilter",
                "componentAssetIndex": 0,
                "componentPathId": 202,
                "rootTransformName": "True Hero",
                "rootTransformPathId": 1,
                "transformAssetIndex": 0,
                "transformPathId": 6,
                "transformPath": "True Hero/Bip01/Bone/Biped Object@#17"
            }],
            "skin": null
        }));
        value["rendererMaterialBindings"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "renderer": source_object(
                    "fixture:renderer-biped-helper",
                    203,
                    "MeshRenderer"
                ),
                "rendererType": "MeshRenderer",
                "gameObject": source_object(
                    "fixture:go-biped-helper",
                    1006,
                    "GameObject"
                ),
                "mesh": source_object(
                    "fixture:mesh-biped-helper",
                    502,
                    "Mesh"
                ),
                "materialSlots": [{
                    "slot": 0,
                    "materialId": "fixture:mat-biped-helper",
                    "pointer": pointer(302)
                }]
            }));
        value
    }

pub(super) fn fixture_with_empty_trs_binding() -> Value {
        let mut value = fixture();
        value["animations"][0]["curveCounts"]["position"] = json!(2);
        value["animations"][0]["animationData"]["emptyTrsBindings"] = json!([{
            "kind": "translation",
            "path": "Bip01",
            "sourceIndex": 1,
            "sourceEncoding": "plain"
        }]);
        value
    }

pub(super) fn fixture_with_duplicate_trs_binding() -> Value {
        let mut value = fixture();
        value["animations"][0]["curveCounts"]["position"] = json!(2);
        let canonical = value["animations"][0]["animationData"]["translations"][0].clone();
        value["animations"][0]["animationData"]["duplicateTrsBindings"] = json!([{
            "kind": "translation",
            "path": canonical["path"],
            "sourceIndex": 1,
            "sourceEncoding": "plain",
            "relation": "identical",
            "canonicalTrackIndex": 0,
            "canonicalSourceIndex": 0,
            "canonicalSourceEncoding": "plain",
            "keys": canonical["keys"],
            "duplicateKeys": canonical["duplicateKeys"],
            "sourceKeyCount": canonical["sourceKeyCount"]
        }]);
        value
    }

pub(super) fn fixture_with_duplicate_same_time_key() -> Value {
        let mut value = fixture();
        let track = &mut value["animations"][0]["animationData"]["translations"][0];
        track["keys"][1]["sourceKeyIndex"] = json!(2);
        track["sourceKeyCount"] = json!(3);
        let mut payload = track["keys"][0].clone();
        payload.as_object_mut().unwrap().remove("sourceKeyIndex");
        track["duplicateKeys"] = json!([{
            "sourceKeyIndex": 1,
            "canonicalKeyIndex": 0,
            "relation": "identical-same-time",
            "key": payload
        }]);
        value
    }

pub(super) fn fixture_with_time_recovery() -> Value {
        let mut value = fixture();
        let track = &mut value["animations"][0]["animationData"]["translations"][0];
        track["keys"][1]["time"] = json!(0.1);
        value["animations"][0]["animationData"]["timeRecoveries"] = json!([{
            "kind": "translation",
            "path": "Bip01/Bone",
            "sourceEncoding": "plain",
            "sourceIndex": 0,
            "reason": "non-strict-source-times-recovered-from-exact-sibling-curve",
            "originalTimes": [0.0, 0.0],
            "recoveredTimes": [0.0, 0.1],
            "sourceSampleRate": 30.0,
            "reference": {
                "asset": "CustomAssetBundle-Retro_shared",
                "pathId": 5245,
                "clipName": "stand_reference",
                "kind": "translation",
                "path": "Bip01/Bone",
                "sourceEncoding": "plain",
                "sourceIndex": 19,
                "sampleRate": 30.0
            },
            "proof": {
                "exactKeyPayloadExcludingTime": true,
                "exactPath": true,
                "exactSampleRate": true,
                "uniqueRecoveredTimeVectorCount": 1,
                "matchingReferenceCount": 1
            }
        }]);
        value
    }

pub(super) fn fixture_with_curve_recovery() -> Value {
        let mut value = fixture();
        value["animations"][0]["asset"] = json!("CustomAssetBundle-Retro_shared");
        value["animations"][0]["pathId"] = json!(100);
        value["animations"][0]["duration"] = json!(1.25);
        value["animations"][0]["declaredDuration"] = json!(1.0);
        value["animations"][0]["keyedDuration"] = json!(1.25);
        value["animations"][0]["curveCounts"]["position"] = json!(2);
        for key in value["animations"][0]["animationData"]["translations"][0]["keys"]
            .as_array_mut()
            .unwrap()
        {
            key["value"] = json!([0.0, -0.75, 0.0]);
        }
        let canonical_track = value["animations"][0]["animationData"]["translations"][0].clone();
        let mut canonical = canonical_track.clone();
        canonical["trackIndex"] = json!(0);
        let mut rejected = canonical_track.clone();
        rejected["sourceIndex"] = json!(1);
        rejected["keys"][0]["value"] = json!([0.0, 0.0, 0.0]);
        rejected["keys"][1]["value"] = json!([0.0, 0.0, 0.0]);
        rejected["keys"][1]["time"] = json!(1.25);
        value["animations"][0]["animationData"]["curveRecoveries"] = json!([{
            "kind": "translation",
            "path": "Bip01/Bone",
            "sourceEncoding": "plain",
            "reason": "conflicting-duplicate-constant-curve-resolved-from-exact-sibling-curve",
            "sourceSampleRate": 30.0,
            "source": {
                "field": "m_PositionCurves",
                "sourceEncoding": "plain",
                "sourceIndices": [0, 1],
                "sourceTargetCurveCount": 2
            },
            "canonical": canonical,
            "rejected": [rejected],
            "reference": {
                "asset": "CustomAssetBundle-Retro_shared",
                "pathId": 101,
                "clipName": "stand_reference",
                "field": "m_PositionCurves",
                "kind": "translation",
                "path": "Bip01/Bone",
                "sourceEncoding": "plain",
                "sourceIndex": 0,
                "sampleRate": 30.0
            },
            "proof": {
                "exactPath": true,
                "exactSampleRate": true,
                "allSourceCurvesConstant": true,
                "canonicalMatchesReference": true,
                "uniqueCanonicalCandidateCount": 1,
                "matchingReferenceCount": 1,
                "sourceTargetCurveCount": 2
            }
        }]);

        let mut sibling = value["animations"][0].clone();
        sibling["asset"] = json!("CustomAssetBundle-Retro_shared");
        sibling["pathId"] = json!(101);
        sibling["name"] = json!("stand_reference");
        sibling["duration"] = json!(1.0);
        sibling["declaredDuration"] = json!(1.0);
        sibling["keyedDuration"] = json!(1.0);
        sibling["eventDuration"] = Value::Null;
        sibling["curveCounts"]["position"] = json!(1);
        sibling["curveCounts"]["compressedRotation"] = json!(0);
        sibling["curveCounts"]["events"] = json!(0);
        sibling["animationData"]["translations"] = json!([canonical_track]);
        sibling["animationData"]["rotations"] = json!([]);
        sibling["animationData"]["emptyTrsBindings"] = json!([]);
        sibling["animationData"]["duplicateTrsBindings"] = json!([]);
        sibling["animationData"]["timeRecoveries"] = json!([]);
        sibling["animationData"]["curveRecoveries"] = json!([]);
        sibling["events"] = json!([]);
        value["animations"].as_array_mut().unwrap().push(sibling);
        value
    }

#[test]
    fn preserves_identical_duplicate_trs_curve_as_digest_bound_metadata() {
        let source = fixture_with_duplicate_trs_binding();
        let (temp, report) = publish_fixture(&source);
        assert_eq!(report.contract.source.duplicate_trs_bindings, 1);
        assert_eq!(report.contract.published.duplicate_trs_bindings, 1);
        assert_eq!(report.contract.source.duplicate_trs_keyframes, 2);
        assert_eq!(report.contract.published.duplicate_trs_keyframes, 2);
        assert_eq!(report.contract.source.animation_channels, 2);

        let output = temp.path().join("output");
        let glb = fs::read(output.join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let metadata = &document["animations"][0]["extras"]["nonTrs"];
        assert_eq!(
            metadata["duplicateTrsBindings"][0]["canonicalSourceIndex"],
            json!(0)
        );
        assert_eq!(metadata["duplicateTrsBindings"][0]["sourceIndex"], json!(1));
        assert_eq!(
            metadata["duplicateTrsBindings"][0]["keys"],
            source["animations"][0]["animationData"]["translations"][0]["keys"]
        );
        let audit = crate::audit_logical_model_tree(&output).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
        assert_eq!(audit.counts.features.duplicate_trs_bindings, 1);
        assert_eq!(audit.counts.features.duplicate_trs_keyframes, 2);

        let mut tampered = source;
        tampered["animations"][0]["animationData"]["duplicateTrsBindings"][0]["keys"][0]["value"]
            [0] = json!(42.0);
        assert!(
            publish_error(&tampered)
                .to_string()
                .contains("does not exactly match its canonical track")
        );
    }

#[test]
    fn preserves_identical_same_time_key_without_emitting_duplicate_sampler_time() {
        let source = fixture_with_duplicate_same_time_key();
        let (temp, report) = publish_fixture(&source);
        assert_eq!(report.contract.source.duplicate_same_time_keys, 1);
        assert_eq!(report.contract.published.duplicate_same_time_keys, 1);

        let output = temp.path().join("output");
        let glb = fs::read(output.join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let channel = &document["animations"][0]["channels"][0];
        assert_eq!(channel["extras"]["sourceKeyCount"], json!(3));
        assert_eq!(channel["extras"]["sourceKeyIndices"], json!([0, 2]));
        assert_eq!(
            channel["extras"]["duplicateKeys"].as_array().unwrap().len(),
            1
        );
        let audit = crate::audit_logical_model_tree(&output).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
        assert_eq!(audit.counts.features.duplicate_same_time_keys, 1);

        let mut tampered = source;
        tampered["animations"][0]["animationData"]["translations"][0]["duplicateKeys"][0]["key"]
            ["value"][0] = json!(42.0);
        assert!(
            publish_error(&tampered)
                .to_string()
                .contains("differs from its canonical key")
        );
    }

#[test]
    fn preserves_unique_constant_curve_recovery_and_rejected_keyed_duration() {
        let source = fixture_with_curve_recovery();
        let (temp, report) = publish_fixture(&source);
        assert_eq!(report.contract.source.animation_curve_recoveries, 1);
        assert_eq!(report.contract.published.animation_curve_recoveries, 1);
        assert_eq!(report.contract.source.rejected_conflicting_trs_bindings, 1);
        assert_eq!(
            report.contract.published.rejected_conflicting_trs_bindings,
            1
        );
        assert_eq!(report.contract.source.rejected_conflicting_trs_keyframes, 2);
        assert_eq!(
            report.contract.published.rejected_conflicting_trs_keyframes,
            2
        );
        assert_eq!(
            report.semantic_proof.source.animation_metadata_sha256,
            report.semantic_proof.emitted.animation_metadata_sha256
        );

        let output = temp.path().join("output");
        let glb = fs::read(output.join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let recovery = &document["animations"][0]["extras"]["nonTrs"]["curveRecoveries"][0];
        assert_eq!(
            document["animations"][0]["extras"]["keyedDuration"],
            json!(1.25)
        );
        assert_eq!(recovery["canonical"]["sourceIndex"], json!(0));
        assert_eq!(recovery["rejected"][0]["sourceIndex"], json!(1));
        assert_eq!(recovery["reference"]["clipName"], json!("stand_reference"));
        assert_eq!(recovery["reference"]["path"], json!("Bip01/Bone"));
        // Source identities are validated above but must not escape into the
        // native payload. The semantic clip/bone reference survives publication.
        assert!(recovery["reference"].get("pathId").is_none());
        assert!(recovery["reference"].get("asset").is_none());
        let audit = crate::audit_logical_model_tree(&output).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
        assert_eq!(audit.counts.features.animation_curve_recoveries, 1);
        assert_eq!(audit.counts.features.rejected_conflicting_trs_bindings, 1);
        assert_eq!(audit.counts.features.rejected_conflicting_trs_keyframes, 2);
    }
