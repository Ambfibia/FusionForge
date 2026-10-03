use super::*;

#[test]
    fn shaderlab_texture_defaults_preserve_exact_order_and_blank_semantics() {
        let script = r#"Shader "typed" {
Properties {
    _MainTex ("Base (RGB)", 2D) = "white" {}
    _SpecMap ("Spec (RGB)", 2D) = "" {}
    _Black ("Black", 2D) = "black" {}
    _Gray ("Gray", 2D) = "gray" {}
    _Bump ("Bump", 2D) = "bump" {}
    _Red ("Red", 2D) = "red" {}
    _Color ("Color", Color) = (1,1,1,1)
}
}"#;
        assert_eq!(
            exact_shader_texture_defaults(script).unwrap(),
            vec![
                ShaderLabTextureDefaultProperty {
                    slot: "_MainTex".into(),
                    value: ShaderLabTextureDefault::BuiltinWhite,
                },
                ShaderLabTextureDefaultProperty {
                    slot: "_SpecMap".into(),
                    value: ShaderLabTextureDefault::Blank,
                },
                ShaderLabTextureDefaultProperty {
                    slot: "_Black".into(),
                    value: ShaderLabTextureDefault::BuiltinBlack,
                },
                ShaderLabTextureDefaultProperty {
                    slot: "_Gray".into(),
                    value: ShaderLabTextureDefault::BuiltinGray,
                },
                ShaderLabTextureDefaultProperty {
                    slot: "_Bump".into(),
                    value: ShaderLabTextureDefault::BuiltinBump,
                },
                ShaderLabTextureDefaultProperty {
                    slot: "_Red".into(),
                    value: ShaderLabTextureDefault::BuiltinRed,
                },
            ]
        );
    }

#[test]
    fn shaderlab_texture_defaults_fail_closed_on_duplicates_unknowns_and_nonexact_rhs() {
        let duplicate = r#"Shader "typed" { Properties {
_MainTex ("A", 2D) = "white" {}
_MainTex ("B", 2D) = "white" {}
} }"#;
        assert!(
            exact_shader_texture_defaults(duplicate)
                .unwrap_err()
                .contains("occurs more than once")
        );

        for rhs in ["\"blue\" {}", "\"white\"{}", "white {}", "\"\" {x}"] {
            let script =
                format!("Shader \"typed\" {{ Properties {{\n_MainTex (\"A\", 2D) = {rhs}\n}} }}");
            assert!(
                exact_shader_texture_defaults(&script)
                    .unwrap_err()
                    .contains("unsupported or non-exact")
            );
        }
    }
