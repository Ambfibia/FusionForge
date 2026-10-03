use super::*;

pub(super) fn float(name: &str, value: f64) -> MaterialFloatProperty {
        MaterialFloatProperty {
            name: name.to_owned(),
            value,
        }
    }

pub(super) fn color(name: &str, value: [f64; 4]) -> MaterialColorProperty {
        MaterialColorProperty {
            name: name.to_owned(),
            value,
        }
    }

pub(super) fn simple_script(name: &str, category: &str, passes: &str) -> String {
        format!("Shader \"{name}\" {{\nCategory {{\n{category}\nSubShader {{\n{passes}\n}}\n}}\n}}")
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
            } else if path.extension().and_then(|value| value.to_str()) == Some("json")
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.ends_with(".source.json"))
            {
                output.push(path);
            }
        }
    }

pub(super) fn toon_script(name: &str) -> String {
        format!(
            r#"Shader "{name}" {{
Category {{
Tags {{"Queue"="Transparent-100"}}
Blend SrcAlpha OneMinusSrcAlpha
ColorMask RGBA
SubShader {{
Pass {{
Name "BASE"
Program "" {{
Local 1, ([_Outline],0,0,0)
Local 2, [_OutlineColor]
Local 3, ([_FatFactor],0,0,0)
}}
Cull Back
}}
Pass {{
Name "OUTLINE"
Cull Front
ZWrite On
ColorMask RGBA
Blend SrcAlpha OneMinusSrcAlpha
}}
}}
SubShader {{
Pass {{ Name "BASE"
Cull Off
}}
}}
}}
}}"#
        )
    }

#[test]
    fn fusion_zak_toon_program_preserves_implicit_backface_culling() {
        let family = ShaderFamily::from_exact_name_and_hash(
            SKINNED_TOON,
            SKINNED_TOON_IMPLICIT_BACKFACE_SHA256,
        )
        .unwrap();
        assert_eq!(family, ShaderFamily::ToonImplicitBackface);
        let expected = expected_program(family);
        assert_eq!(expected.passes[0].cull, None);
        assert_eq!(ResolvedPassState::default().cull, MaterialCullMode::Back);
        assert_eq!(expected.passes[1].cull, Some(MaterialCullMode::Front));
    }

#[test]
    fn alternate_rigid_toon_program_preserves_implicit_backface_culling() {
        let family =
            ShaderFamily::from_exact_name_and_hash(RIGID_TOON, RIGID_TOON_IMPLICIT_BACKFACE_SHA256)
                .unwrap();
        assert_eq!(family, ShaderFamily::ToonImplicitBackface);
        let expected = expected_program(family);
        assert_eq!(expected.passes[0].cull, None);
        assert_eq!(ResolvedPassState::default().cull, MaterialCullMode::Back);
        assert_eq!(expected.passes[1].cull, Some(MaterialCullMode::Front));
    }

pub(super) fn toon_script_with_defaults(name: &str, width: &str, color: &str) -> String {
        toon_script(name).replacen(
            &format!("Shader \"{name}\" {{\nCategory"),
            &format!(
                "Shader \"{name}\" {{\nProperties {{\n_Outline (\"Outline\", Float) = {width}\n_OutlineColor (\"Outline Color\", Color) = {color}\n}}\nCategory"
            ),
            1,
        )
    }

#[test]
    fn toon_outline_defaults_and_saved_overrides_are_fail_closed() {
        let exact = toon_script_with_defaults(SKINNED_TOON_E1, ".005", "(0,0,0,1)");
        assert!(
            resolve_legacy_material(
                SKINNED_TOON_E1,
                &exact,
                None,
                &[float("_Outline", 0.005), float("_Outline", 0.006)],
                &[],
            )
            .unwrap_err()
            .contains("occurs more than once")
        );
        assert!(
            resolve_legacy_material(
                SKINNED_TOON_E1,
                &exact,
                None,
                &[float("_Outline", f64::NAN)],
                &[],
            )
            .unwrap_err()
            .contains("not finite")
        );

        for script in [
            toon_script_with_defaults(SKINNED_TOON_E1, "NaN", "(0,0,0,1)"),
            toon_script_with_defaults(SKINNED_TOON_E1, ".005 extra", "(0,0,0,1)"),
            toon_script_with_defaults(SKINNED_TOON_E1, ".005", "(0,0,0)"),
            toon_script_with_defaults(SKINNED_TOON_E1, ".005", "(0,0,0,NaN)"),
            toon_script_with_defaults(SKINNED_TOON_E1, ".005", "(0,0,0,1)")
                .replacen(
                    "_Outline (\"Outline\", Float) = .005",
                    "_Outline (\"Outline\", Float) = .005\n_Outline (\"Outline duplicate\", Float) = .005",
                    1,
                ),
        ] {
            assert!(
                resolve_legacy_material(SKINNED_TOON_E1, &script, None, &[], &[]).is_err(),
                "ambiguous/non-finite defaults unexpectedly resolved"
            );
        }
    }

#[test]
    fn resolves_hash_bound_additive_transparent_default_backface_culling_program() {
        assert_eq!(ADDITIVE_TRANSPARENT_SCRIPT.len(), 706);
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(ADDITIVE_TRANSPARENT_SCRIPT.as_bytes())
            ),
            ADDITIVE_TRANSPARENT_SHA256
        );
        let (queue, passes) = resolve_legacy_material(
            ADDITIVE_TRANSPARENT,
            ADDITIVE_TRANSPARENT_SCRIPT,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(queue, 3_010);
        assert_eq!(passes.len(), 1);
        let pass = &passes[0];
        assert!(pass.blend.enabled);
        assert_eq!(pass.blend.source_color, MaterialBlendFactor::SourceAlpha);
        assert_eq!(pass.blend.destination_color, MaterialBlendFactor::One);
        assert_eq!(pass.blend.source_alpha, MaterialBlendFactor::SourceAlpha);
        assert_eq!(pass.blend.destination_alpha, MaterialBlendFactor::One);
        assert_eq!(pass.cull, MaterialCullMode::Back);
        assert!(!pass.z_write);
        assert_eq!(pass.z_test, MaterialCompareFunction::LessEqual);
        assert_eq!(pass.alpha_test, MaterialAlphaTestState::Disabled);
        assert_eq!(pass.color_mask, 0b0111);
    }

#[test]
    fn rejects_hash_or_cull_contradictions_for_additive_transparent_default_culling() {
        let hash_mismatch = ADDITIVE_TRANSPARENT_SCRIPT.replace(
            "Combine texture * primary DOUBLE , texture",
            "Combine texture * primary, texture",
        );
        let error = resolve_legacy_material(ADDITIVE_TRANSPARENT, &hash_mismatch, None, &[], &[])
            .unwrap_err();
        assert!(error.contains("script SHA-256"));

        let cull_mismatch =
            ADDITIVE_TRANSPARENT_SCRIPT.replace("\tZWrite Off\n", "\tZWrite Off\n\tCull Off\n");
        let error = resolve_legacy_material(ADDITIVE_TRANSPARENT, &cull_mismatch, None, &[], &[])
            .unwrap_err();
        assert!(error.contains("script SHA-256"));
    }

#[test]
    fn resolves_hash_bound_additive_transparent_cull_off_program() {
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(ADDITIVE_TRANSPARENT_CULL_OFF_SCRIPT.as_bytes())
            ),
            ADDITIVE_TRANSPARENT_CULL_OFF_SHA256
        );
        let (queue, passes) = resolve_legacy_material(
            ADDITIVE_TRANSPARENT_CULL_OFF,
            ADDITIVE_TRANSPARENT_CULL_OFF_SCRIPT,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(queue, 3_010);
        assert_eq!(passes.len(), 1);
        let pass = &passes[0];
        assert!(pass.blend.enabled);
        assert_eq!(pass.blend.source_color, MaterialBlendFactor::SourceAlpha);
        assert_eq!(pass.blend.destination_color, MaterialBlendFactor::One);
        assert_eq!(pass.blend.source_alpha, MaterialBlendFactor::SourceAlpha);
        assert_eq!(pass.blend.destination_alpha, MaterialBlendFactor::One);
        assert_eq!(pass.cull, MaterialCullMode::Off);
        assert!(!pass.z_write);
        assert_eq!(pass.z_test, MaterialCompareFunction::LessEqual);
        assert_eq!(pass.alpha_test, MaterialAlphaTestState::Disabled);
        assert_eq!(pass.color_mask, 0b0111);
    }

#[test]
    fn rejects_unknown_names_header_mismatches_and_contradictory_evidence() {
        let script = simple_script(
            TRANSPARENT_NORMAL,
            "Tags {\"Queue\"=\"Transparent+10\"}\nBlend SrcAlpha OneMinusSrcAlpha\nColorMask RGBA\nZWrite Off",
            "Pass {\n}",
        );
        assert!(resolve_legacy_material("normal_but_not_exact", &script, None, &[], &[]).is_err());
        assert!(resolve_legacy_material(SKINNED_TOON, &script, None, &[], &[]).is_err());

        let contradictory = script.replace("ZWrite Off", "ZWrite On");
        let error = resolve_legacy_material(TRANSPARENT_NORMAL, &contradictory, None, &[], &[])
            .unwrap_err();
        assert!(error.contains("contradicts"));
    }

#[test]
    fn retrobution_fusion_matter_program_is_hash_bound_and_has_an_outline() {
        let family = ShaderFamily::from_exact_name_and_hash(
            SKINNED_FUSION_MATTER_LIGHT_DIR,
            SKINNED_FUSION_MATTER_LIGHT_DIR_SHA256,
        )
        .unwrap();
        assert_eq!(family, ShaderFamily::SkinnedFusionMatterLightDir);
        assert!(family.is_toon());
        assert!(family.requires_fusion_matter_outline_program_evidence());
        assert_eq!(expected_program(family).passes.len(), 2);

        let error = ShaderFamily::from_exact_name_and_hash(
            SKINNED_FUSION_MATTER_LIGHT_DIR,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap_err();
        assert!(error.contains("does not match any audited exact program"));
    }

#[test]
    fn rejects_missing_or_ambiguous_property_references() {
        let script = simple_script(
            ADDITIVE_TEST,
            "Tags {\"Queue\"=\"Transparent\"}\nBlend One One\nColorMask RGB\nZWrite Off\nAlphaTest Greater [_Cutoff]\nCull Off",
            "Pass {\n}",
        );
        assert!(resolve_legacy_material(ADDITIVE_TEST, &script, None, &[], &[]).is_err());
        assert!(
            resolve_legacy_material(
                ADDITIVE_TEST,
                &script,
                None,
                &[float("_Cutoff", 0.0), float("_Cutoff", 0.0)],
                &[],
            )
            .is_err()
        );
    }
