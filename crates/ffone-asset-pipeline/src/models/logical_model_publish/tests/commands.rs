use super::*;

#[test]
    fn legacy_npc_upper_and_event_precedence_matches_make_uper_layer() {
        assert!(legacy_npc_clip_is_additive("melee1"));
        assert!(legacy_npc_clip_is_additive("wound"));
        assert!(!legacy_npc_clip_is_additive("melee1upper"));
        assert!(!legacy_npc_clip_is_additive("woundupper"));
        assert!(!legacy_npc_clip_is_additive("melee1event"));
        assert!(logical_model_uses_legacy_npc_additive_deltas(
            "mob",
            "mob_oilmonster"
        ));
        assert!(logical_model_uses_legacy_npc_additive_deltas(
            "mob",
            "mob_sneakyspawn"
        ));
        assert!(logical_model_uses_legacy_npc_additive_deltas(
            "mob",
            "mob_cerberus"
        ));
    }

#[test]
    fn event_pointer_provenance_is_required_typed_and_non_null_unresolved_stays_blocked() {
        let source = fixture_with_null_event_object_pointer();

        let mut inconsistent = source.clone();
        inconsistent["animations"][0]["events"][0]["objectParameterProvenance"]["pathId"] =
            json!(1);
        assert!(
            publish_error(&inconsistent)
                .to_string()
                .contains("null-path-id object provenance is inconsistent")
        );

        let mut unresolved = source.clone();
        unresolved["animations"][0]["events"][0]["objectParameter"] = json!({
            "sourceAssetIndex": 3,
            "fileId": 7,
            "pathId": 99
        });
        unresolved["animations"][0]["events"][0]["objectParameterProvenance"] = json!({
            "presence": "serialized-pointer",
            "sourceAssetIndex": 3,
            "fileId": 7,
            "pathId": 99,
            "interpretation": "non-null-unresolved"
        });
        assert!(
            publish_error(&unresolved)
                .to_string()
                .contains("unresolved non-null Unity object pointer")
        );

        let mut metadata_only = unresolved;
        metadata_only["animations"][0]["events"][0]["functionName"] = json!("sound");
        metadata_only["animations"][0]["events"][0]["stringParameter"] =
            json!("FusionSamJack_Death.wav");
        metadata_only["animations"][0]["events"][0]["objectParameterProvenance"]["interpretation"] =
            json!("non-null-metadata-only");
        let metadata_source: SourceDocument = serde_json::from_value(metadata_only).unwrap();
        let converted = convert_source(&metadata_source).unwrap();
        assert!(
            converted.model.animations[0].metadata.events[0]
                .object_parameter
                .is_none()
        );
        assert_eq!(
            converted.model.animations[0].metadata.events[0].object_parameter_provenance,
            AnimationEventObjectParameterProvenance::SerializedPointer {
                source_asset_index: 3,
                file_id: 7,
                path_id: 99,
                interpretation: SerializedEventObjectParameterInterpretation::NonNullMetadataOnly,
            }
        );

        let mut missing = source.clone();
        missing["animations"][0]["events"][0]
            .as_object_mut()
            .unwrap()
            .remove("objectParameterProvenance");
        assert!(
            publish_error(&missing)
                .to_string()
                .contains("objectParameterProvenance")
        );

        let mut unknown = source;
        unknown["animations"][0]["events"][0]["objectParameterProvenance"]["invented"] =
            json!(true);
        assert!(
            publish_error(&unknown)
                .to_string()
                .contains("unknown field")
        );
    }
