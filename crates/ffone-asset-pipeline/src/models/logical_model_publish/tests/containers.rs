use super::*;

pub(super) fn source_object(id: &str, path_id: i64, object_type: &str) -> Value {
        json!({
            "id": id,
            "asset": "fixture",
            "assetIndex": 0,
            "pathId": path_id,
            "type": object_type,
            "classId": 0,
            "typeId": 0
        })
    }

pub(super) fn fixture_with_null_event_object_pointer() -> Value {
        let mut value = fixture();
        value["animations"][0]["events"][0]["objectParameter"] = Value::Null;
        value["animations"][0]["events"][0]["objectParameterProvenance"] = json!({
            "presence": "serialized-pointer",
            "sourceAssetIndex": 3,
            "fileId": 7,
            "pathId": 0,
            "interpretation": "null-path-id"
        });
        value
    }

#[test]
    fn h_reflection_converts_unity_clockwise_triangles_without_a_second_swap() {
        let source: SourceDocument = serde_json::from_value(fixture()).unwrap();
        let converted = convert_source(&source).unwrap();
        assert_eq!(converted.model.meshes[0].primitives[0].indices, [0, 1, 2]);
        assert_eq!(converted.model.meshes[1].primitives[0].indices, [0, 1, 2]);
    }

#[test]
    fn h_conjugation_preserves_nontrivial_unity_skinning_without_requiring_bind_identity() {
        let h = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
        let unity_joint_a = Mat4::from_scale_rotation_translation(
            Vec3::new(1.2, 0.8, 1.1),
            Quat::from_euler(glam::EulerRot::XYZ, 0.41, -0.73, 1.17),
            Vec3::new(3.25, -1.5, 0.75),
        );
        let unity_joint_b = Mat4::from_scale_rotation_translation(
            Vec3::new(0.9, 1.3, 0.7),
            Quat::from_euler(glam::EulerRot::ZYX, -0.28, 0.92, -0.36),
            Vec3::new(-0.5, 2.0, 1.25),
        );
        let unity_bind_a = Mat4::from_scale_rotation_translation(
            Vec3::new(0.95, 1.05, 1.0),
            Quat::from_euler(glam::EulerRot::YXZ, -0.66, 0.23, 0.51),
            Vec3::new(-1.0, 0.4, 2.2),
        );
        let unity_bind_b = Mat4::from_scale_rotation_translation(
            Vec3::new(1.15, 0.85, 1.0),
            Quat::from_euler(glam::EulerRot::XZY, 0.37, -0.19, 0.84),
            Vec3::new(0.3, -1.1, 0.6),
        );
        let unity_position = Vec4::new(0.4, -1.2, 2.5, 1.0);
        let native_position = h * unity_position;

        let native_joint_a = h * unity_joint_a * h;
        let native_joint_b = h * unity_joint_b * h;
        let native_bind_a = h * unity_bind_a * h;
        let native_bind_b = h * unity_bind_b * h;
        let native_weighted = (native_joint_a * native_bind_a * native_position) * 0.35
            + (native_joint_b * native_bind_b * native_position) * 0.65;
        let unity_weighted = (unity_joint_a * unity_bind_a * unity_position) * 0.35
            + (unity_joint_b * unity_bind_b * unity_position) * 0.65;

        assert!((native_weighted - h * unity_weighted).abs().max_element() <= 1.0e-5);
        assert!(
            skinning_basis_parity_error(native_joint_a, native_bind_a)
                <= SKINNING_BASIS_PARITY_TOLERANCE
        );
        assert!(
            skinning_basis_parity_error(native_joint_b, native_bind_b)
                <= SKINNING_BASIS_PARITY_TOLERANCE
        );
        assert!(
            matrix_identity_deviation(native_joint_a * native_bind_a).max_abs > 0.1,
            "basis parity must not be conflated with current-pose bind identity"
        );
    }

#[test]
    fn preserves_serialized_null_event_pointer_provenance_without_legacy_resolution() {
        let source = fixture_with_null_event_object_pointer();
        let (temp, report) = publish_fixture(&source);
        assert_eq!(report.contract.source.animation_events, 1);
        assert_eq!(report.contract.published.animation_events, 1);
        assert_eq!(
            report.contract.source.animation_event_null_object_pointers,
            1
        );
        assert_eq!(
            report
                .contract
                .published
                .animation_event_null_object_pointers,
            1
        );
        assert_eq!(
            report.semantic_proof.source.animation_metadata_sha256,
            report.semantic_proof.emitted.animation_metadata_sha256
        );

        let output = temp.path().join("output");
        let glb = fs::read(output.join(&report.contract.output_glb)).unwrap();
        let document = glb_json(&glb);
        let event = &document["animations"][0]["extras"]["nonTrs"]["events"][0];
        assert!(event["objectParameter"].is_null());
        assert_eq!(
            event["objectParameterProvenance"],
            json!({
                "presence": "serialized-pointer",
                "sourceAssetIndex": 3,
                "fileId": 7,
                "pathId": 0,
                "interpretation": "null-path-id"
            })
        );
        let audit = crate::audit_logical_model_tree(&output).unwrap();
        assert!(audit.passed, "{:#?}", audit.violations);
        assert_eq!(
            audit.counts.features.animation_event_null_object_pointers,
            1
        );
    }
