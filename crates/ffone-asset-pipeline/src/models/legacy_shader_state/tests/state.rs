use super::*;

#[test]
    fn primary_time_pod_diffuse_fade_is_hash_bound_with_exact_depth_state() {
        let family =
            ShaderFamily::from_exact_name_and_hash(DIFFUSE_FADE, DIFFUSE_FADE_SHA256).unwrap();
        assert_eq!(family, ShaderFamily::DiffuseFade);
        assert!(
            ShaderFamily::from_exact_name_and_hash(
                DIFFUSE_FADE,
                "1a5bdc216f56d33491301ea9f86aed21a9a476a28343d71c5f4b419154e1cf11",
            )
            .unwrap_err()
            .contains("does not match any audited exact program")
        );

        let expected = expected_program(family);
        assert_eq!(expected.category.queue.as_deref(), Some("transparent-100"));
        assert_eq!(
            expected.category.blend,
            Some(RawBlend::Enabled {
                source: MaterialBlendFactor::SourceAlpha,
                destination: MaterialBlendFactor::OneMinusSourceAlpha,
            })
        );
        assert_eq!(expected.category.color_mask, Some(0b0111));
        assert_eq!(expected.category.cull, None);
        assert_eq!(expected.category.z_write, None);
        assert_eq!(expected.passes[0].name.as_deref(), Some("BASE"));

        let mut resolved = ResolvedPassState::default();
        resolved.apply(&expected.category, &[], "", &[]).unwrap();
        resolved.apply(&expected.passes[0], &[], "", &[]).unwrap();
        let pass = resolved.into_material_pass(
            expected.passes[0].name.clone(),
            MaterialOutlineState::Disabled,
        );
        assert_eq!(pass.cull, MaterialCullMode::Back);
        assert!(pass.z_write);
        assert_eq!(pass.color_mask, 0b0111);
    }

#[test]
    fn primary_hippie_hop_additive_program_is_hash_bound_with_exact_state() {
        let family = ShaderFamily::from_exact_name_and_hash(
            ADDITIVE_ONE_ONE_CULL_OFF,
            ADDITIVE_ONE_ONE_CULL_OFF_HIPPIE_HOP_SHA256,
        )
        .unwrap();
        assert_eq!(family, ShaderFamily::AdditiveOneOneCullOff);
        assert!(
            ShaderFamily::from_exact_name_and_hash(
                ADDITIVE_ONE_ONE_CULL_OFF,
                "0609d98348a9a69435c7b01485eaa12cd939b91153baeb69dfee20d28b80af79",
            )
            .unwrap_err()
            .contains("does not match any audited exact program")
        );

        let expected = expected_program(family);
        assert_eq!(expected.category.queue.as_deref(), Some("transparent+11"));
        assert_eq!(
            expected.category.blend,
            Some(RawBlend::Enabled {
                source: MaterialBlendFactor::One,
                destination: MaterialBlendFactor::One,
            })
        );
        assert_eq!(expected.category.cull, Some(MaterialCullMode::Off));
        assert_eq!(expected.category.z_write, Some(false));
        assert_eq!(expected.category.color_mask, Some(0b0111));
    }

#[test]
    fn primary_nanomachine_additive_program_preserves_default_depth_and_backface_state() {
        let family = ShaderFamily::from_exact_name_and_hash(
            ADDITIVE_ONE_ONE_DEPTH_WRITE,
            ADDITIVE_ONE_ONE_DEPTH_WRITE_SHA256,
        )
        .unwrap();
        assert_eq!(family, ShaderFamily::AdditiveOneOneDepthWrite);
        assert!(
            ShaderFamily::from_exact_name_and_hash(
                ADDITIVE_ONE_ONE_DEPTH_WRITE,
                "04d0ecba4bff63754363c4af568f02d951603cd887009f9c8419fe7a93f942b4",
            )
            .unwrap_err()
            .contains("does not match any audited exact program")
        );

        let expected = expected_program(family);
        assert_eq!(expected.category.queue.as_deref(), Some("transparent"));
        assert_eq!(
            expected.category.blend,
            Some(RawBlend::Enabled {
                source: MaterialBlendFactor::One,
                destination: MaterialBlendFactor::One,
            })
        );
        assert_eq!(expected.category.cull, None);
        assert_eq!(expected.category.z_write, None);
        assert_eq!(expected.category.color_mask, Some(0b0111));

        let mut resolved = ResolvedPassState::default();
        resolved.apply(&expected.category, &[], "", &[]).unwrap();
        resolved.apply(&expected.passes[0], &[], "", &[]).unwrap();
        let pass = resolved.into_material_pass(None, MaterialOutlineState::Disabled);
        assert_eq!(pass.cull, MaterialCullMode::Back);
        assert!(pass.z_write);
    }

#[test]
    fn skin_directional_face_program_is_hash_bound_with_exact_legacy_state() {
        let family = ShaderFamily::from_exact_name_and_hash(
            SKIN_DIRECTIONAL_ALPHA_BLEND,
            SKIN_DIRECTIONAL_ALPHA_BLEND_SHA256,
        )
        .unwrap();
        assert_eq!(family, ShaderFamily::SkinDirectionalAlphaBlend);
        assert!(
            ShaderFamily::from_exact_name_and_hash(
                SKIN_DIRECTIONAL_ALPHA_BLEND,
                "03e643aa02a6119c0f0e54a4cc77a480b2b55d6f230ede32e382472bf8b70b2c",
            )
            .unwrap_err()
            .contains("does not match any audited exact program")
        );

        let expected = expected_program(family);
        assert_eq!(expected.category.queue.as_deref(), Some("transparent-100"));
        assert_eq!(
            expected.category.blend,
            Some(RawBlend::Enabled {
                source: MaterialBlendFactor::SourceAlpha,
                destination: MaterialBlendFactor::OneMinusSourceAlpha,
            })
        );
        assert_eq!(expected.category.color_mask, Some(0b1111));
        assert_eq!(expected.category.cull, None);
        assert_eq!(expected.category.z_write, None);
        assert_eq!(expected.passes.len(), 1);
        assert_eq!(expected.passes[0].name.as_deref(), Some("BASE"));

        let mut resolved = ResolvedPassState::default();
        resolved.apply(&expected.category, &[], "", &[]).unwrap();
        resolved.apply(&expected.passes[0], &[], "", &[]).unwrap();
        let pass = resolved.into_material_pass(
            expected.passes[0].name.clone(),
            MaterialOutlineState::Disabled,
        );
        assert_eq!(
            queue_from_tag(expected.category.queue.as_deref().unwrap()).unwrap(),
            2_900
        );
        assert_eq!(pass.name.as_deref(), Some("BASE"));
        assert!(pass.blend.enabled);
        assert_eq!(pass.blend.source_color, MaterialBlendFactor::SourceAlpha);
        assert_eq!(
            pass.blend.destination_color,
            MaterialBlendFactor::OneMinusSourceAlpha
        );
        assert_eq!(pass.blend.source_alpha, MaterialBlendFactor::SourceAlpha);
        assert_eq!(
            pass.blend.destination_alpha,
            MaterialBlendFactor::OneMinusSourceAlpha
        );
        assert_eq!(pass.cull, MaterialCullMode::Back);
        assert!(pass.z_write);
        assert_eq!(pass.z_test, MaterialCompareFunction::LessEqual);
        assert_eq!(pass.alpha_test, MaterialAlphaTestState::Disabled);
        assert_eq!(pass.color_mask, 0b1111);
        assert_eq!(pass.outline, MaterialOutlineState::Disabled);
    }

#[test]
    fn es740_additive_programs_are_hash_bound_with_distinct_depth_state() {
        let tested = ShaderFamily::from_exact_name_and_hash(
            ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF,
            ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF_SHA256,
        )
        .unwrap();
        let tested_expected = expected_program(tested);
        assert_eq!(
            tested_expected.category.queue.as_deref(),
            Some("transparent+11")
        );
        assert_eq!(tested_expected.category.cull, Some(MaterialCullMode::Off));
        assert_eq!(tested_expected.category.z_write, Some(false));
        assert!(matches!(
            tested_expected.category.alpha_test,
            Some(RawAlphaTest::Enabled {
                compare: MaterialCompareFunction::Greater,
                ..
            })
        ));

        let depth_writing = ShaderFamily::from_exact_name_and_hash(
            ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE,
            ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE_SHA256,
        )
        .unwrap();
        let depth_expected = expected_program(depth_writing);
        assert_eq!(
            depth_expected.category.queue.as_deref(),
            Some("transparent")
        );
        assert_eq!(depth_expected.category.cull, Some(MaterialCullMode::Off));
        assert_eq!(depth_expected.category.z_write, None);

        assert!(
            ShaderFamily::from_exact_name_and_hash(
                ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE,
                "04916e5c653b2a55839e2365555b4d7b924d6dfba10f0c6d8f7043f695eec84e",
            )
            .unwrap_err()
            .contains("does not match any audited exact program")
        );
    }
