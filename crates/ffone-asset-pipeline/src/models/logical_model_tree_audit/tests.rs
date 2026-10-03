use std::fs;

use ffone_skinned_model::{
    AnimationChannel, AnimationClip, AnimationMetadata, AnimationTimeRecovery,
    AnimationTimeRecoveryProof, AnimationTimeRecoveryReason, AnimationTimeRecoveryReference,
    Interpolation, ModelMesh, ModelNode, ModelPrimitive, ModelSkin, NativeModel, TrackValues,
    encode_glb, exact_native_coordinate_contract, prove_semantic_roundtrip,
};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;

fn fixture_model(name: &str) -> NativeModel {
    NativeModel {
        schema: LOGICAL_MODEL_SCHEMA.to_owned(),
        name: name.to_owned(),
        native_coordinate_contract: exact_native_coordinate_contract(),
        roots: vec![0],
        nodes: vec![
            ModelNode {
                name: name.to_owned(),
                legacy_name: None,
                legacy_sibling_ordinal: None,
                parent: None,
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
                mesh: Some(0),
                skin: Some(0),
            },
            ModelNode {
                name: "Bip01".to_owned(),
                legacy_name: None,
                legacy_sibling_ordinal: None,
                parent: Some(0),
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
                mesh: None,
                skin: None,
            },
        ],
        meshes: vec![ModelMesh {
            name: "Body".to_owned(),
            renderer_order: 0,
            primitives: vec![ModelPrimitive {
                material: None,
                material_slot: None,
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![],
                uvs: vec![],
                joints: vec![[0, 0, 0, 0]; 3],
                weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
                indices: vec![0, 1, 2],
            }],
        }],
        skins: vec![ModelSkin {
            name: "Body Skin".to_owned(),
            skeleton_root: 1,
            joints: vec![1],
            inverse_bind_matrices: vec![[
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]],
        }],
        materials: vec![],
        textures: vec![],
        samplers: vec![],
        animations: vec![
            AnimationClip {
                name: "idle".to_owned(),
                duration: 1.0,
                declared_duration: Some(1.0),
                keyed_duration: Some(1.0),
                event_duration: None,
                sample_rate: Some(30.0),
                wrap_mode: Some(2),
                looped: true,
                channels: vec![AnimationChannel {
                    target_node: 1,
                    source_index: 0,
                    source_encoding: EmptyTrsSourceEncoding::Plain,
                    source_key_count: 2,
                    source_key_indices: vec![0, 1],
                    duplicate_keys: Vec::new(),
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Translation(vec![[0.0; 3], [0.0, 1.0, 0.0]]),
                    in_tangents: None,
                    out_tangents: None,
                    tangent_modes: vec![0, 0],
                }],
                metadata: AnimationMetadata::default(),
            },
            AnimationClip {
                name: "nif-default".to_owned(),
                duration: 0.0,
                declared_duration: None,
                keyed_duration: None,
                event_duration: None,
                sample_rate: None,
                wrap_mode: None,
                looped: false,
                channels: vec![],
                metadata: AnimationMetadata::default(),
            },
        ],
    }
}

fn publish_fixture(name: &str) -> (TempDir, PathBuf) {
    publish_model_fixture(fixture_model(name))
}

fn publish_time_recovery_fixture(name: &str) -> (TempDir, PathBuf) {
    let mut model = fixture_model(name);
    model.animations[0].channels[0].times[1] = 0.1;
    model.animations[0].keyed_duration = Some(0.1);
    model.animations[0].metadata.time_recoveries = vec![AnimationTimeRecovery {
        kind: EmptyTrsBindingKind::Translation,
        target_node: 1,
        target_path: "Bip01".to_owned(),
        source_encoding: EmptyTrsSourceEncoding::Plain,
        source_index: 0,
        reason: AnimationTimeRecoveryReason::NonStrictSourceTimesRecoveredFromExactSiblingCurve,
        original_times: vec![0.0, 0.0],
        recovered_times: vec![0.0, 0.1],
        source_sample_rate: Some(30.0),
        reference: AnimationTimeRecoveryReference {
            asset: "CustomAssetBundle-Retro_shared".to_owned(),
            path_id: 5245,
            clip_name: "idle_reference".to_owned(),
            kind: EmptyTrsBindingKind::Translation,
            path: "Bip01".to_owned(),
            source_encoding: EmptyTrsSourceEncoding::Plain,
            source_index: 19,
            sample_rate: Some(30.0),
        },
        proof: AnimationTimeRecoveryProof {
            exact_key_payload_excluding_time: true,
            exact_path: true,
            exact_sample_rate: true,
            unique_recovered_time_vector_count: 1,
            matching_reference_count: 1,
        },
    }];
    publish_model_fixture(model)
}

fn publish_model_fixture(model: NativeModel) -> (TempDir, PathBuf) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("candidate");
    let directory = root.join("models/npc");
    fs::create_dir_all(&directory).unwrap();
    let name = model.name.clone();
    let animation_time_recoveries = model
        .animations
        .iter()
        .map(|clip| clip.metadata.time_recoveries.len())
        .sum::<usize>();
    let recovered_animation_keyframes = model
        .animations
        .iter()
        .flat_map(|clip| &clip.metadata.time_recoveries)
        .map(|recovery| recovery.recovered_times.len())
        .sum::<usize>();
    let glb = encode_glb(&model).unwrap();
    let semantic_proof = prove_semantic_roundtrip(&model, &glb).unwrap();
    let coordinate_audit = crate::logical_model_publish::coordinate_audit(&model).unwrap();
    let glb_relative = format!("models/npc/{name}.glb");
    let glb_path = root.join(&glb_relative);
    fs::write(&glb_path, &glb).unwrap();
    let features = json!({
        "nodes": 2,
        "meshParts": 1,
        "materialSlots": 0,
        "skinnedMeshes": 1,
        "joints": 1,
        "inverseBindMatrices": 1,
        "weightedVertices": 3,
        "animationClips": 2,
        "animationChannels": 1,
        "emptyTrsBindings": 0,
        "duplicateTrsBindings": 0,
        "duplicateTrsKeyframes": 0,
        "duplicateSameTimeKeys": 0,
        "animationTimeRecoveries": animation_time_recoveries,
        "recoveredAnimationKeyframes": recovered_animation_keyframes,
        "animationCurveRecoveries": 0,
        "rejectedConflictingTrsBindings": 0,
        "rejectedConflictingTrsKeyframes": 0,
        "animationKeyframes": 2,
        "cubicSplineKeyframes": 0,
        "animationEvents": 0,
        "animationEventNullObjectPointers": 0,
        "objectReferenceKeys": 0,
        "colliders": 0,
        "lodLevels": 0
    });
    let report_relative = format!("models/npc/{name}.publish.json");
    let report = json!({
        "schema": PUBLISH_REPORT_SCHEMA,
        "status": "staged-incomplete",
        "publishable": false,
        "contract": {
            "schema": PUBLISH_CONTRACT_SCHEMA,
            "legacyName": name,
            "rootNode": name,
            "family": "npc",
            "semanticDirectories": [],
            "outputGlb": glb_relative,
            "glbBlake3": blake3::hash(&glb).to_hex().to_string(),
            "source": features,
            "published": features,
            "unresolvedSourceFeatures": []
        },
        "semanticProof": semantic_proof,
        "coordinateContract": exact_native_coordinate_contract(),
        "coordinateAudit": coordinate_audit,
        "materialPublish": {
            "status": "native-data-complete-runtime-validation-pending",
            "reason": "GPU gate pending",
            "preservedSlotNames": [],
            "explicitNullSlots": 1,
            "materialCount": 0,
            "textureCount": 0,
            "samplerCount": 0,
            "materials": [],
            "textures": []
        },
        "reportPath": report_relative
    });
    fs::write(
        directory.join(format!("{name}.publish.json")),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    (temp, root)
}

#[test]
fn accepts_one_true_name_root_with_skin_and_metadata_only_clip() {
    let (_temp, root) = publish_fixture("True Hero");
    let report = audit_logical_model_tree(&root).unwrap();
    assert!(report.passed, "{:#?}", report.violations);
    assert!(report.gpu_gate_pending);
    assert_eq!(report.counts.logical_roots, 1);
    assert_eq!(report.counts.features.joints, 1);
    assert_eq!(report.counts.features.weighted_vertices, 3);
    assert_eq!(report.counts.standard_animation_clips, 1);
    assert_eq!(report.counts.metadata_only_animation_clips, 1);
    assert_eq!(report.counts.features.animation_clips, 2);
}

#[test]
fn audits_recovered_times_against_actual_f32_sampler_inputs_at_rest() {
    let (_temp, root) = publish_time_recovery_fixture("True Hero");
    let valid = audit_logical_model_tree(&root).unwrap();
    assert!(valid.passed, "{:#?}", valid.violations);
    assert_eq!(valid.counts.features.animation_time_recoveries, 1);
    assert_eq!(valid.counts.features.recovered_animation_keyframes, 2);

    let glb_path = root.join("models/npc/True Hero.glb");
    rewrite_glb_json(&glb_path, |document| {
        document["animations"][0]["extras"]["nonTrs"]["timeRecoveries"][0]["recoveredTimes"]
            [1] = json!(0.2);
    });
    update_contract_glb_hash(&root, "True Hero");
    let tampered = audit_logical_model_tree(&root).unwrap();
    assert!(!tampered.passed);
    assert!(has_code(
        &tampered,
        "animation_time_recovery_sampler_mismatch"
    ));
    assert!(has_code(&tampered, "semantic_digest_mismatch"));
}

#[test]
fn rejects_same_count_glb_value_mutation_even_when_file_hash_is_updated() {
    let (_temp, root) = publish_fixture("True Hero");
    let glb_path = root.join("models/npc/True Hero.glb");
    let mut bytes = fs::read(&glb_path).unwrap();
    let parsed = parse_glb(&bytes).unwrap();
    let weight_accessor =
        parsed.document["meshes"][0]["primitives"][0]["attributes"]["WEIGHTS_0"]
            .as_u64()
            .unwrap() as usize;
    let accessor = &parsed.document["accessors"][weight_accessor];
    let view =
        &parsed.document["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
    let binary_start = 20 + read_u32(&bytes, 12).unwrap() as usize + 8;
    let first_weight = binary_start
        + view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    bytes[first_weight..first_weight + 4].copy_from_slice(&0.75f32.to_le_bytes());
    bytes[first_weight + 4..first_weight + 8].copy_from_slice(&0.25f32.to_le_bytes());
    fs::write(&glb_path, bytes).unwrap();
    update_contract_glb_hash(&root, "True Hero");

    let report = audit_logical_model_tree(&root).unwrap();
    assert!(!report.passed);
    assert!(has_code(&report, "semantic_digest_mismatch"));
    assert!(!has_code(&report, "contract_glb_hash_mismatch"));
    assert!(!has_code(&report, "contract_actual_count_mismatch"));
}

#[test]
fn rejects_hash_identity_and_scattered_node() {
    let (_temp, root) = publish_fixture("True Hero");
    fs::rename(
        root.join("models/npc/True Hero.glb"),
        root.join("models/npc/Hero--deadbeef.glb"),
    )
    .unwrap();
    fs::rename(
        root.join("models/npc/True Hero.publish.json"),
        root.join("models/npc/Hero--deadbeef.publish.json"),
    )
    .unwrap();
    let glb_path = root.join("models/npc/Hero--deadbeef.glb");
    rewrite_glb_json(&glb_path, |document| {
        document["extras"]["logicalModelName"] = json!("Hero--deadbeef");
        document["nodes"][0]["name"] = json!("Hero--deadbeef");
        document["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name": "LoosePart"}));
    });
    let report = audit_logical_model_tree(&root).unwrap();
    assert!(!report.passed);
    assert!(has_code(&report, "generated_or_unsafe_name"));
    assert!(has_code(&report, "unreachable_nodes"));
    assert!(has_code(&report, "contract_glb_hash_mismatch"));
}

#[test]
fn rejects_orphan_png_and_publishable_before_gpu_gate() {
    let (_temp, root) = publish_fixture("True Hero");
    fs::write(root.join("orphan.png"), minimal_png()).unwrap();
    let report_path = root.join("models/npc/True Hero.publish.json");
    let mut report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    report["publishable"] = Value::Bool(true);
    fs::write(report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    let audit = audit_logical_model_tree(&root).unwrap();
    assert!(!audit.passed);
    assert!(has_code(&audit, "orphan_png"));
    assert!(has_code(&audit, "gpu_gate_bypassed"));
}

#[test]
fn verifies_external_png_and_rejects_a_missing_reference() {
    let (_temp, root) = publish_fixture("True Hero");
    add_external_texture(&root, "True Hero");
    let audit = audit_logical_model_tree(&root).unwrap();
    assert!(audit.passed, "{:#?}", audit.violations);
    assert_eq!(audit.counts.pngs, 1);
    assert_eq!(audit.counts.referenced_pngs, 1);
    assert_eq!(audit.counts.materials, 1);

    fs::remove_file(root.join("models/npc/True Hero.textures/body.png")).unwrap();
    let missing = audit_logical_model_tree(&root).unwrap();
    assert!(!missing.passed);
    assert!(has_code(&missing, "missing_png"));
}

#[test]
fn rejects_missing_required_post_glb_true_names() {
    let (_temp, root) = publish_fixture("True Hero");
    add_external_texture(&root, "True Hero");
    let glb_path = root.join("models/npc/True Hero.glb");
    rewrite_glb_json(&glb_path, |document| {
        document["skins"][0].as_object_mut().unwrap().remove("name");
        document["meshes"][0]
            .as_object_mut()
            .unwrap()
            .remove("name");
        document["animations"][0]
            .as_object_mut()
            .unwrap()
            .remove("name");
        document["images"][0]
            .as_object_mut()
            .unwrap()
            .remove("name");
        document["images"][0]["extras"]["ffone"]
            .as_object_mut()
            .unwrap()
            .remove("sourceName");
        document["textures"][0]
            .as_object_mut()
            .unwrap()
            .remove("name");
        document["samplers"][0]
            .as_object_mut()
            .unwrap()
            .remove("name");
    });
    update_contract_glb_hash(&root, "True Hero");

    let audit = audit_logical_model_tree(&root).unwrap();
    assert!(!audit.passed);
    for code in [
        "missing_skin_name",
        "missing_mesh_name",
        "missing_standard_animation_name",
        "missing_image_name",
        "missing_image_source_name",
        "missing_texture_name",
        "missing_sampler_name",
    ] {
        assert!(
            has_code(&audit, code),
            "missing violation {code}: {audit:#?}"
        );
    }
}

#[test]
fn rejects_generated_identities_in_every_required_post_glb_name_class() {
    let (_temp, root) = publish_fixture("True Hero");
    add_external_texture(&root, "True Hero");
    let glb_path = root.join("models/npc/True Hero.glb");
    rewrite_glb_json(&glb_path, |document| {
        document["skins"][0]["name"] = json!("Skin--deadbeef");
        document["meshes"][0]["name"] = json!("mesh#42");
        document["animations"][0]["name"] = json!("PathID_99");
        document["images"][0]["name"] = json!("texture#42");
        document["images"][0]["extras"]["ffone"]["sourceName"] = json!("texture#42");
        document["textures"][0]["name"] = json!("texture#42");
        document["samplers"][0]["name"] = json!("texture#42");
    });
    update_contract_glb_hash(&root, "True Hero");

    let audit = audit_logical_model_tree(&root).unwrap();
    let generated_count = audit
        .violations
        .iter()
        .filter(|violation| violation.code == "generated_or_unsafe_name")
        .count();
    assert!(
        generated_count >= 7,
        "all seven required name classes must be validated: {audit:#?}"
    );
}

#[test]
fn rejects_contradictory_image_texture_and_sampler_true_names() {
    let (_temp, root) = publish_fixture("True Hero");
    add_external_texture(&root, "True Hero");
    let glb_path = root.join("models/npc/True Hero.glb");
    rewrite_glb_json(&glb_path, |document| {
        document["images"][0]["name"] = json!("other image.dds");
        document["textures"][0]["name"] = json!("other texture.dds");
        document["samplers"][0]["name"] = json!("other sampler.dds");
    });
    update_contract_glb_hash(&root, "True Hero");

    let audit = audit_logical_model_tree(&root).unwrap();
    assert!(!audit.passed);
    for code in [
        "image_name_source_name_mismatch",
        "texture_image_name_mismatch",
        "sampler_texture_name_mismatch",
    ] {
        assert!(
            has_code(&audit, code),
            "missing violation {code}: {audit:#?}"
        );
    }
}

#[test]
fn shared_texture_variant_keeps_exact_names_and_byte_validation() {
    let (_temp, root) = publish_fixture("True Hero");
    add_external_texture(&root, "True Hero");
    fn redirect(value: &mut Value) {
        match value {
            Value::String(text) if text == "True Hero.textures/body.png" => {
                *text = "../shared/textures/body_variant_02.png".into();
            }
            Value::Array(values) => values.iter_mut().for_each(redirect),
            Value::Object(values) => values.values_mut().for_each(redirect),
            _ => (),
        }
    }
    let shared = root.join("models/shared/textures/body_variant_02.png");
    fs::create_dir_all(shared.parent().unwrap()).unwrap();
    fs::rename(root.join("models/npc/True Hero.textures/body.png"), &shared).unwrap();
    rewrite_glb_json(&root.join("models/npc/True Hero.glb"), redirect);
    let report_path = root.join("models/npc/True Hero.publish.json");
    let mut report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    redirect(&mut report);
    let semantic = semantic_digests_from_glb(
        &fs::read(root.join("models/npc/True Hero.glb")).unwrap(),
    ).unwrap();
    report["semanticProof"]["source"] = serde_json::to_value(&semantic).unwrap();
    report["semanticProof"]["emitted"] = serde_json::to_value(&semantic).unwrap();
    fs::write(&report_path, serde_json::to_vec(&report).unwrap()).unwrap();
    update_contract_glb_hash(&root, "True Hero");
    let audit = audit_logical_model_tree(&root).unwrap();
    assert!(audit.passed, "{:?}", audit.violations);
    fs::remove_file(shared).unwrap();
    let audit = audit_logical_model_tree(&root).unwrap();
    assert!(has_code(&audit, "missing_png"));
    assert!(safe_image_relative("models/npc/True Hero.glb", "../../../outside.png").is_none());
}

#[test]
fn exact_lower_mip_is_owned_and_missing_or_corrupt_bytes_are_rejected() {
    let (_temp, root) = publish_fixture("True Hero");
    add_external_texture(&root, "True Hero");
    let lower = add_exact_lower_mip(&root, "True Hero");

    let valid = audit_logical_model_tree(&root).unwrap();
    assert!(valid.passed, "{:#?}", valid.violations);
    assert_eq!(valid.counts.pngs, 2);
    assert_eq!(valid.counts.referenced_pngs, 2);
    assert_eq!(valid.counts.orphan_pngs, 0);

    fs::remove_file(&lower).unwrap();
    let missing = audit_logical_model_tree(&root).unwrap();
    assert!(!missing.passed);
    assert!(has_code(&missing, "missing_exact_mip_png"));
    assert!(has_code(&missing, "missing_report_mip_png"));

    let mut corrupt = minimal_png();
    corrupt.push(0xff);
    fs::write(&lower, corrupt).unwrap();
    let corrupt = audit_logical_model_tree(&root).unwrap();
    assert!(!corrupt.passed);
    assert!(has_code(&corrupt, "exact_mip_png_hash_mismatch"));
    assert!(has_code(&corrupt, "report_mip_png_hash_mismatch"));
}

fn add_external_texture(root: &Path, name: &str) {
    let texture_relative = format!("models/npc/{name}.textures/body.png");
    let texture_path = root.join(&texture_relative);
    fs::create_dir_all(texture_path.parent().unwrap()).unwrap();
    let png = minimal_png();
    fs::write(&texture_path, &png).unwrap();
    let png_sha256 = format!("{:x}", Sha256::digest(&png));
    let source_sha256 = "1".repeat(64);
    let decoded_sha256 = "2".repeat(64);

    let glb_path = root.join(format!("models/npc/{name}.glb"));
    rewrite_glb_json(&glb_path, |document| {
        document["materials"] = json!([{
            "name": "Body Material",
            "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}},
            "extras": {"ffone": {
                "name": "Body Material",
                "serializedShaderName": "skinntoone2",
                "declaredShaderName": "SkinnedToonShading_blendSrcalphaInvsrcalpha e1",
                "legacyShaderName": "SkinnedToonShading_blendSrcalphaInvsrcalpha e1",
                "renderQueue": 2900,
                "colors": [],
                "floats": [],
                "shaderTextureDefaults": [{
                    "slot": "_MainTex",
                    "value": "builtinWhite"
                }],
                "textureBindings": [{
                    "slot": "_MainTex",
                    "texture": 0,
                    "sourceName": "body.dds",
                    "uri": format!("{name}.textures/body.png"),
                    "sampler": {"index": 0, "descriptor": {
                        "name": "body.dds",
                        "magFilter": "linear",
                        "minFilter": "linear",
                        "wrapS": "repeat",
                        "wrapT": "repeat",
                        "legacyFilterMode": 1,
                        "legacyWrapMode": 0,
                        "anisotropyLevel": 1,
                        "mipMapBias": 0.0
                    }},
                    "mipProvenance": {
                        "sourceTextureFormat": 4,
                        "sourceTextureFormatName": "RGBA32",
                        "sourceMipCount": 1,
                        "sourceChainByteLength": 4,
                        "sourceChainSha256": source_sha256,
                        "sourceChainComplete": true,
                        "sourceLayout": "largestToSmallestContiguous",
                        "publishedPixelTransform": "vertical-flip-only-for-png-top-left-origin",
                        "publishedPolicy": "baseLevelOnly"
                    },
                    "mipLevels": [{
                        "level": 0,
                        "width": 1,
                        "height": 1,
                        "uri": format!("{name}.textures/body.png"),
                        "sourceByteOffset": 0,
                        "sourceByteLength": 4,
                        "sourceByteSha256": source_sha256,
                        "decodedRgba8ByteLength": 4,
                        "decodedRgba8Sha256": decoded_sha256,
                        "pngByteLength": png.len(),
                        "pngSha256": png_sha256
                    }],
                    "scale": [1.0, 1.0],
                    "offset": [0.0, 0.0],
                    "pivot": null,
                    "rotation": null,
                    "colorSpace": "srgb"
                }],
                "passes": [],
                "standardTextureRefsAreLoaderHints": true
            }}
        }]);
        document["images"] = json!([{
            "name": "body.dds",
            "uri": format!("{name}.textures/body.png"),
            "mimeType": "image/png",
            "extras": {"ffone": {
                "sourceName": "body.dds",
                "uri": format!("{name}.textures/body.png"),
                "width": 1,
                "height": 1,
                "sampler": 0,
                "mipProvenance": {
                    "sourceTextureFormat": 4,
                    "sourceTextureFormatName": "RGBA32",
                    "sourceMipCount": 1,
                    "sourceChainByteLength": 4,
                    "sourceChainSha256": source_sha256,
                    "sourceChainComplete": true,
                    "sourceLayout": "largestToSmallestContiguous",
                    "publishedPixelTransform": "vertical-flip-only-for-png-top-left-origin",
                    "publishedPolicy": "baseLevelOnly"
                },
                "mipLevels": [{
                    "level": 0,
                    "width": 1,
                    "height": 1,
                    "uri": format!("{name}.textures/body.png"),
                    "sourceByteOffset": 0,
                    "sourceByteLength": 4,
                    "sourceByteSha256": source_sha256,
                    "decodedRgba8ByteLength": 4,
                    "decodedRgba8Sha256": decoded_sha256,
                    "pngByteLength": png.len(),
                    "pngSha256": png_sha256
                }]
            }}
        }]);
        document["textures"] = json!([{"name": "body.dds", "source": 0, "sampler": 0}]);
        document["samplers"] = json!([{"name": "body.dds"}]);
        document["meshes"][0]["primitives"][0]["material"] = json!(0);
        document["meshes"][0]["primitives"][0]["extras"]["materialSlot"] =
            json!("Body Material");
    });

    let glb = fs::read(&glb_path).unwrap();
    let report_path = root.join(format!("models/npc/{name}.publish.json"));
    let mut report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    report["contract"]["glbBlake3"] = json!(blake3::hash(&glb).to_hex().to_string());
    let semantic = semantic_digests_from_glb(&glb).unwrap();
    report["semanticProof"]["source"] = serde_json::to_value(&semantic).unwrap();
    report["semanticProof"]["emitted"] = serde_json::to_value(&semantic).unwrap();
    report["contract"]["source"]["materialSlots"] = json!(1);
    report["contract"]["published"]["materialSlots"] = json!(1);
    report["materialPublish"]["explicitNullSlots"] = json!(0);
    report["materialPublish"]["materialCount"] = json!(1);
    report["materialPublish"]["textureCount"] = json!(1);
    report["materialPublish"]["samplerCount"] = json!(1);
    report["materialPublish"]["materials"] = json!([{
        "name": "Body Material",
        "serializedShaderName": "skinntoone2",
        "declaredShaderName": "SkinnedToonShading_blendSrcalphaInvsrcalpha e1",
        "legacyShaderName": "SkinnedToonShading_blendSrcalphaInvsrcalpha e1",
        "shaderSha256": "00",
        "effectiveRenderQueue": 2900,
        "renderPassCount": 1
    }]);
    report["materialPublish"]["textures"] = json!([{
        "sourceName": "body.dds",
        "uri": format!("{name}.textures/body.png"),
        "width": 1,
        "height": 1,
        "sourceMipCount": 1,
        "publishedPolicy": "baseLevelOnly",
        "byteLength": png.len(),
        "sha256": png_sha256,
        "mipLevels": [{
            "level": 0,
            "uri": format!("{name}.textures/body.png"),
            "width": 1,
            "height": 1,
            "sourceByteOffset": 0,
            "sourceByteLength": 4,
            "sourceByteSha256": source_sha256,
            "decodedRgba8ByteLength": 4,
            "decodedRgba8Sha256": decoded_sha256,
            "pngByteLength": png.len(),
            "pngSha256": png_sha256
        }]
    }]);
    fs::write(report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

fn add_exact_lower_mip(root: &Path, name: &str) -> PathBuf {
    let base_relative = format!("models/npc/{name}.textures/body.png");
    let lower_relative = format!("models/npc/{name}.textures/body.mips/mip-01.png");
    let base_path = root.join(&base_relative);
    let lower_path = root.join(&lower_relative);
    fs::create_dir_all(lower_path.parent().unwrap()).unwrap();
    let base_png = minimal_png_dimensions(2, 2);
    let lower_png = minimal_png_dimensions(1, 1);
    fs::write(&base_path, &base_png).unwrap();
    fs::write(&lower_path, &lower_png).unwrap();
    let base_hash = format!("{:x}", Sha256::digest(&base_png));
    let lower_hash = format!("{:x}", Sha256::digest(&lower_png));
    let source_hash = "3".repeat(64);
    let base_source_hash = "4".repeat(64);
    let lower_source_hash = "5".repeat(64);
    let base_decoded_hash = "6".repeat(64);
    let lower_decoded_hash = "7".repeat(64);
    let base_uri = format!("{name}.textures/body.png");
    let lower_uri = format!("{name}.textures/body.mips/mip-01.png");

    let glb_path = root.join(format!("models/npc/{name}.glb"));
    rewrite_glb_json(&glb_path, |document| {
        let exact = &mut document["images"][0]["extras"]["ffone"];
        exact["width"] = json!(2);
        exact["height"] = json!(2);
        exact["mipProvenance"] = json!({
            "sourceTextureFormat": 4,
            "sourceTextureFormatName": "RGBA32",
            "sourceMipCount": 2,
            "sourceChainByteLength": 20,
            "sourceChainSha256": source_hash,
            "sourceChainComplete": true,
            "sourceLayout": "largestToSmallestContiguous",
            "publishedPixelTransform": "vertical-flip-only-for-png-top-left-origin",
            "publishedPolicy": "exactSourceLevels"
        });
        exact["mipLevels"] = json!([{
            "level": 0,
            "width": 2,
            "height": 2,
            "uri": base_uri,
            "sourceByteOffset": 0,
            "sourceByteLength": 16,
            "sourceByteSha256": base_source_hash,
            "decodedRgba8ByteLength": 16,
            "decodedRgba8Sha256": base_decoded_hash,
            "pngByteLength": base_png.len(),
            "pngSha256": base_hash
        }, {
            "level": 1,
            "width": 1,
            "height": 1,
            "uri": lower_uri,
            "sourceByteOffset": 16,
            "sourceByteLength": 4,
            "sourceByteSha256": lower_source_hash,
            "decodedRgba8ByteLength": 4,
            "decodedRgba8Sha256": lower_decoded_hash,
            "pngByteLength": lower_png.len(),
            "pngSha256": lower_hash
        }]);
    });

    let glb = fs::read(&glb_path).unwrap();
    let report_path = root.join(format!("models/npc/{name}.publish.json"));
    let mut report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    report["contract"]["glbBlake3"] = json!(blake3::hash(&glb).to_hex().to_string());
    let texture = &mut report["materialPublish"]["textures"][0];
    texture["width"] = json!(2);
    texture["height"] = json!(2);
    texture["sourceMipCount"] = json!(2);
    texture["publishedPolicy"] = json!("exactSourceLevels");
    texture["byteLength"] = json!(base_png.len());
    texture["sha256"] = json!(base_hash);
    texture["mipLevels"] = json!([{
        "level": 0,
        "uri": base_uri,
        "width": 2,
        "height": 2,
        "sourceByteOffset": 0,
        "sourceByteLength": 16,
        "sourceByteSha256": base_source_hash,
        "decodedRgba8ByteLength": 16,
        "decodedRgba8Sha256": base_decoded_hash,
        "pngByteLength": base_png.len(),
        "pngSha256": base_hash
    }, {
        "level": 1,
        "uri": lower_uri,
        "width": 1,
        "height": 1,
        "sourceByteOffset": 16,
        "sourceByteLength": 4,
        "sourceByteSha256": lower_source_hash,
        "decodedRgba8ByteLength": 4,
        "decodedRgba8Sha256": lower_decoded_hash,
        "pngByteLength": lower_png.len(),
        "pngSha256": lower_hash
    }]);
    fs::write(report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    lower_path
}

fn rewrite_glb_json(path: &Path, mutate: impl FnOnce(&mut Value)) {
    let bytes = fs::read(path).unwrap();
    let parsed = parse_glb(&bytes).unwrap();
    let mut document = parsed.document;
    let binary = parsed.binary.to_vec();
    mutate(&mut document);
    let mut json = serde_json::to_vec(&document).unwrap();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut output = Vec::new();
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2u32.to_le_bytes());
    let total = 12 + 8 + json.len() + usize::from(!binary.is_empty()) * (8 + binary.len());
    output.extend_from_slice(&(total as u32).to_le_bytes());
    output.extend_from_slice(&(json.len() as u32).to_le_bytes());
    output.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
    output.extend_from_slice(&json);
    if !binary.is_empty() {
        output.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        output.extend_from_slice(&0x004e_4942u32.to_le_bytes());
        output.extend_from_slice(&binary);
    }
    fs::write(path, output).unwrap();
}

fn update_contract_glb_hash(root: &Path, name: &str) {
    let glb = fs::read(root.join(format!("models/npc/{name}.glb"))).unwrap();
    let report_path = root.join(format!("models/npc/{name}.publish.json"));
    let mut report: Value = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    report["contract"]["glbBlake3"] = json!(blake3::hash(&glb).to_hex().to_string());
    fs::write(report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

fn minimal_png() -> Vec<u8> {
    minimal_png_dimensions(1, 1)
}

fn minimal_png_dimensions(width: u32, height: u32) -> Vec<u8> {
    // The structural auditor only needs the standard signature and IHDR.
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes
}

fn has_code(report: &LogicalModelTreeAuditReport, code: &str) -> bool {
    report.violations.iter().any(|issue| issue.code == code)
}
