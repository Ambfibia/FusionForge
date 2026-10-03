use super::*;

pub(super) const ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD_SCRIPT: &str = concat!(
        "Shader \"normal_blendOneOne_zwriteOff_cullOff_vertexColorAD\" {\n",
        "Properties {\n",
        "\t_Color (\"Main Color\", Color) = (1,1,1,1)\n",
        "\t_SpecColor (\"Spec Color\", Color) = (1,1,1,1)\n",
        "\t_AmbColor (\"Ambient Color\", Color) = (1,1,1,1)\n",
        "\t_Emission (\"Emmisive Color\", Color) = (0,0,0,0)\n",
        "\t_Shininess (\"Shininess\", Range (0.01, 1)) = 0.7\n",
        "\t_MainTex (\"Base (RGB)\", 2D) = \"white\" {}\n",
        "}\n",
        "\n",
        "Category {\n",
        "\tTags {\"Queue\"=\"Transparent+11\"}\n",
        "\n",
        "\tBlend One One \n",
        "\tColorMask RGB\n",
        "\tZWrite Off\n",
        "\tCull Off\n",
        "\tFog {Mode Off}\n",
        "\tColorMaterial AmbientAndDiffuse\n",
        "\t\n",
        "\tSubShader {\n",
        "\t\tPass {\n",
        "\t\t\tMaterial {\n",
        "\t\t\t\tDiffuse [_Color]\n",
        "\t\t\t\tAmbient [_AmbColor]\n",
        "\t\t\t\tShininess [_Shininess]\n",
        "\t\t\t\t//\tSpecular [_SpecColor]\n",
        "\t\t\t\tEmission [_Emission]\n",
        "\t\t\t} \n",
        "\t\t\tLighting On\n",
        "\t\t\tSetTexture [_MainTex] {\n",
        "\t\t\t\tCombine texture * primary DOUBLE , texture\n",
        "\t\t\t} \n",
        "\t\t}\n",
        "\t}\n",
        "}\n",
        "}"
    );

#[test]
    fn es739_vertex_color_additive_program_is_hash_bound_with_exact_legacy_state() {
        assert_eq!(ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD_SCRIPT.len(), 776);
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD_SCRIPT.as_bytes())
            ),
            ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD_SHA256
        );

        let (queue, passes) = resolve_legacy_material(
            ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD,
            ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD_SCRIPT,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(queue, 3_011);
        assert_eq!(passes.len(), 1);
        let pass = &passes[0];
        assert!(pass.blend.enabled);
        assert_eq!(pass.blend.source_color, MaterialBlendFactor::One);
        assert_eq!(pass.blend.destination_color, MaterialBlendFactor::One);
        assert_eq!(pass.blend.source_alpha, MaterialBlendFactor::One);
        assert_eq!(pass.blend.destination_alpha, MaterialBlendFactor::One);
        assert_eq!(pass.cull, MaterialCullMode::Off);
        assert!(!pass.z_write);
        assert_eq!(pass.z_test, MaterialCompareFunction::LessEqual);
        assert_eq!(pass.alpha_test, MaterialAlphaTestState::Disabled);
        assert_eq!(pass.color_mask, 0b0111);

        assert!(
            ShaderFamily::from_exact_name_and_hash(
                ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD,
                "0186fb05f40d4ca77d6c8aa5c40afb615b02c180229788f19c7c08515c429e01",
            )
            .unwrap_err()
            .contains("does not match any audited exact program")
        );
    }
