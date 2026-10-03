use super::*;

#[test]
    #[ignore = "requires FFONE_LOGICAL_SOURCE_ROOT pointing at an external exact-source batch"]
    fn preflights_every_external_material_shader_before_full_publish() {
        let root = PathBuf::from(
            std::env::var_os("FFONE_LOGICAL_SOURCE_ROOT")
                .expect("FFONE_LOGICAL_SOURCE_ROOT must be set"),
        );
        let mut files = Vec::new();
        external_source_files(&root, &mut files);
        assert!(!files.is_empty(), "external source root is empty");

        let mut programs = BTreeMap::<(String, String), (u64, BTreeSet<String>)>::new();
        let mut failures = Vec::new();
        let mut material_count = 0_u64;
        for file in &files {
            let document: serde_json::Value =
                serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
            let logical_name = document["logicalName"].as_str().unwrap().to_owned();
            let materials = document["materials"].as_object().unwrap();
            for material in materials.values() {
                material_count += 1;
                let material_name = material["name"].as_str().unwrap();
                let declared_name = material["shader"]["declaredName"].as_str().unwrap();
                let script = material["shader"]["script"]["text"].as_str().unwrap();
                let script_sha256 = material["shader"]["scriptSha256"].as_str().unwrap();
                let render_queue = material["renderQueue"]
                    .as_i64()
                    .and_then(|value| i32::try_from(value).ok())
                    .filter(|value| *value != -1);
                let floats = material["savedProperties"]["floats"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|property| MaterialFloatProperty {
                        name: property["name"].as_str().unwrap().to_owned(),
                        value: property["value"].as_f64().unwrap(),
                    })
                    .collect::<Vec<_>>();
                let colors = material["savedProperties"]["colors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|property| {
                        let value = &property["value"];
                        MaterialColorProperty {
                            name: property["name"].as_str().unwrap().to_owned(),
                            value: [
                                value["r"].as_f64().unwrap(),
                                value["g"].as_f64().unwrap(),
                                value["b"].as_f64().unwrap(),
                                value["a"].as_f64().unwrap(),
                            ],
                        }
                    })
                    .collect::<Vec<_>>();

                let key = (declared_name.to_owned(), script_sha256.to_owned());
                let program = programs.entry(key).or_default();
                program.0 += 1;
                program.1.insert(logical_name.clone());

                if let Err(error) = exact_shader_texture_defaults(script).and_then(|_| {
                    resolve_legacy_material(declared_name, script, render_queue, &floats, &colors)
                        .map(|_| ())
                }) {
                    failures.push(format!(
                        "{} | logicalName={logical_name:?} | material={material_name:?} | declaredName={declared_name:?} | sha256={script_sha256} | {error}",
                        file.display()
                    ));
                }
            }
        }

        eprintln!(
            "shader preflight: sources={}, materialBindings={}, distinctPrograms={}",
            files.len(),
            material_count,
            programs.len()
        );
        for ((declared_name, sha256), (materials, models)) in programs {
            eprintln!(
                "{declared_name:?} sha256={sha256} materials={materials} models={}",
                models.len()
            );
        }
        assert!(
            failures.is_empty(),
            "external shader preflight failed:\n{}",
            failures.join("\n")
        );
    }

#[test]
    fn resolves_the_original_six_exact_dexter_shader_names() {
        let normal = simple_script(
            TRANSPARENT_NORMAL,
            "Tags {\"Queue\"=\"Transparent+10\"}\nBlend SrcAlpha OneMinusSrcAlpha\nColorMask RGBA\nZWrite Off",
            "Pass {\n}",
        );
        let (queue, passes) =
            resolve_legacy_material(TRANSPARENT_NORMAL, &normal, None, &[], &[]).unwrap();
        assert_eq!(queue, 3_010);
        assert_eq!(passes.len(), 1);
        assert!(passes[0].blend.enabled);
        assert_eq!(passes[0].cull, MaterialCullMode::Back);
        assert!(!passes[0].z_write);

        for name in [SKINNED_TOON, RIGID_TOON] {
            let (queue, passes) = resolve_legacy_material(
                name,
                &toon_script(name),
                None,
                &[float("_Outline", 0.005)],
                &[color("_OutlineColor", [0.0, 0.0, 0.0, 1.0])],
            )
            .unwrap();
            assert_eq!(queue, 2_900);
            assert_eq!(passes.len(), 2);
            assert_eq!(passes[0].name.as_deref(), Some("BASE"));
            assert_eq!(passes[1].name.as_deref(), Some("OUTLINE"));
            assert_eq!(passes[1].cull, MaterialCullMode::Front);
            assert_eq!(
                passes[1].outline,
                MaterialOutlineState::WorldSpace {
                    width: 0.005,
                    color: [0.0, 0.0, 0.0, 1.0]
                }
            );
        }

        let fusion = simple_script(
            FUSION_EFFECT,
            "Tags {\"Queue\"=\"Transparent-100\"}\nBlend SrcAlpha OneMinusSrcAlpha\nColorMask RGB",
            "Pass {\nName \"BASE\"\n}",
        );
        let (queue, passes) =
            resolve_legacy_material(FUSION_EFFECT, &fusion, None, &[], &[]).unwrap();
        assert_eq!(queue, 2_900);
        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0].color_mask, 0b0111);
        assert!(passes[0].z_write);

        let additive = simple_script(
            ADDITIVE_TEST,
            "Tags {\"Queue\"=\"Transparent\"}\nBlend One One\nColorMask RGB\nZWrite Off\nAlphaTest Greater [_Cutoff]\nCull Off",
            "Pass {\n}",
        );
        let (queue, passes) = resolve_legacy_material(
            ADDITIVE_TEST,
            &additive,
            None,
            &[float("_Cutoff", 0.0)],
            &[],
        )
        .unwrap();
        assert_eq!(queue, 3_000);
        assert_eq!(passes[0].cull, MaterialCullMode::Off);
        assert_eq!(
            passes[0].alpha_test,
            MaterialAlphaTestState::Enabled {
                compare: MaterialCompareFunction::Greater,
                reference: MaterialAlphaReference::FloatProperty {
                    name: "_Cutoff".to_owned(),
                    resolved_value: 0.0,
                }
            }
        );

        let cutout = simple_script(
            CUTOUT_TWO_SIDED,
            "Tags {\"Queue\"=\"Transparent-100\"}\nCull Off",
            "Pass {\nAlphaTest GEqual 0.9\nColorMask RGBA\n}\nPass {\nBlend SrcAlpha OneMinusSrcAlpha\nColorMask RGB\nZWrite Off\n}",
        );
        let (queue, passes) =
            resolve_legacy_material(CUTOUT_TWO_SIDED, &cutout, None, &[], &[]).unwrap();
        assert_eq!(queue, 2_900);
        assert_eq!(passes.len(), 2);
        assert!(passes[0].z_write);
        assert!(!passes[0].blend.enabled);
        assert_eq!(
            passes[0].alpha_test,
            MaterialAlphaTestState::Enabled {
                compare: MaterialCompareFunction::GreaterEqual,
                reference: MaterialAlphaReference::Literal { value: 0.9 }
            }
        );
        assert!(!passes[1].z_write);
        assert!(passes[1].blend.enabled);
        assert_eq!(passes[1].color_mask, 0b0111);
    }

#[test]
    fn toon_outline_uses_exact_shader_defaults_only_when_saved_values_are_absent() {
        let script = toon_script_with_defaults(SKINNED_TOON_E1, ".005", "(0,0,0,1)");
        let (_, default_passes) =
            resolve_legacy_material(SKINNED_TOON_E1, &script, None, &[], &[]).unwrap();
        assert_eq!(
            default_passes[1].outline,
            MaterialOutlineState::WorldSpace {
                width: 0.005,
                color: [0.0, 0.0, 0.0, 1.0]
            }
        );

        let (_, overridden) = resolve_legacy_material(
            SKINNED_TOON_E1,
            &script,
            None,
            &[float("_Outline", 0.025)],
            &[color("_OutlineColor", [0.25, 0.5, 0.75, 1.0])],
        )
        .unwrap();
        assert_eq!(
            overridden[1].outline,
            MaterialOutlineState::WorldSpace {
                width: 0.025,
                color: [0.25, 0.5, 0.75, 1.0]
            }
        );
    }

#[test]
    fn alpha_test_uses_exact_shader_default_when_material_omits_cutoff() {
        let script = simple_script(
            ADDITIVE_TEST,
            "Tags {\"Queue\"=\"Transparent\"}\nBlend One One\nColorMask RGB\nZWrite Off\nAlphaTest Greater [_Cutoff]\nCull Off",
            "Pass {\n}",
        )
        .replace(
            "Category {",
            "Properties {\n_Cutoff (\"Cutoff\", Range (0, 1)) = 0.0\n}\nCategory {",
        );

        let (_, passes) = resolve_legacy_material(ADDITIVE_TEST, &script, None, &[], &[]).unwrap();
        assert_eq!(
            passes[0].alpha_test,
            MaterialAlphaTestState::Enabled {
                compare: MaterialCompareFunction::Greater,
                reference: MaterialAlphaReference::FloatProperty {
                    name: "_Cutoff".to_owned(),
                    resolved_value: 0.0,
                },
            }
        );
    }

#[test]
    fn rejects_hash_or_render_state_contradictions_for_additive_transparent_cull_off() {
        let hash_mismatch = ADDITIVE_TRANSPARENT_CULL_OFF_SCRIPT.replace(
            "Combine texture * primary DOUBLE , texture",
            "Combine texture * primary, texture",
        );
        let error = resolve_legacy_material(
            ADDITIVE_TRANSPARENT_CULL_OFF,
            &hash_mismatch,
            None,
            &[],
            &[],
        )
        .unwrap_err();
        assert!(error.contains("script SHA-256"));

        let state_mismatch = ADDITIVE_TRANSPARENT_CULL_OFF_SCRIPT
            .replace("Blend SrcAlpha One ", "Blend SrcAlpha OneMinusSrcAlpha ");
        let error = resolve_legacy_material(
            ADDITIVE_TRANSPARENT_CULL_OFF,
            &state_mismatch,
            None,
            &[],
            &[],
        )
        .unwrap_err();
        // The byte identity is checked before state parsing, so even a
        // plausible state mutation fails at the exact-program gate.
        assert!(error.contains("script SHA-256"));
    }

#[test]
    fn preserves_a_real_material_custom_queue_override() {
        let script = simple_script(
            TRANSPARENT_NORMAL,
            "Tags {\"Queue\"=\"Transparent+10\"}\nBlend SrcAlpha OneMinusSrcAlpha\nColorMask RGBA\nZWrite Off",
            "Pass {\n}",
        );
        let (queue, _) =
            resolve_legacy_material(TRANSPARENT_NORMAL, &script, Some(3_111), &[], &[]).unwrap();
        assert_eq!(queue, 3_111);
    }
