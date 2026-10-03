use super::*;

#[test]
    fn animation_paths_are_relative_when_root_and_child_names_collide() {
        let paths = BTreeMap::from([
            ("patrol_elena7".to_owned(), 0),
            ("patrol_elena7/patrol_elena7".to_owned(), 1),
        ]);

        assert_eq!(
            animation_node_path(&paths, "patrol_elena7", "patrol_elena7", "animation track")
                .unwrap(),
            1
        );
        assert_eq!(
            animation_node_path(&paths, "patrol_elena7", "", "animation track").unwrap(),
            0
        );
    }

#[test]
    fn preserves_but_does_not_publish_keyed_tracks_for_an_unbound_sibling_rig() {
        let mut value = fixture();
        value["animations"][0]["animationData"]["translations"][0]["path"] =
            json!("SiblingRig/Bip01/Bone");
        value["animations"][0]["animationData"]["translations"][0]["unboundModelTarget"] =
            json!(true);

        let source: SourceDocument = serde_json::from_value(value).unwrap();
        let converted = convert_source(&source).unwrap();
        assert_eq!(converted.model.animations[0].channels.len(), 1);
        assert_eq!(converted.source_counts.animation_channels, 1);
        assert_eq!(converted.source_counts.animation_keyframes, 2);
    }

#[test]
    fn rejects_unproven_or_misaligned_animation_time_recovery() {
        let source = fixture_with_time_recovery();
        let mutations = [
            (
                "/animations/0/animationData/timeRecoveries/0/originalTimes",
                json!([0.0, 0.1]),
                "already strict",
            ),
            (
                "/animations/0/animationData/timeRecoveries/0/recoveredTimes/1",
                json!(0.2),
                "do not align with canonical sampler keys",
            ),
            (
                "/animations/0/animationData/timeRecoveries/0/proof/exactPath",
                json!(false),
                "proof is incomplete",
            ),
            (
                "/animations/0/animationData/timeRecoveries/0/reference/sampleRate",
                json!(60.0),
                "sample rates do not exactly match",
            ),
        ];
        for (pointer, replacement, expected) in mutations {
            let mut mutation = source.clone();
            *mutation.pointer_mut(pointer).unwrap() = replacement;
            let error = publish_error(&mutation);
            assert!(
                error.to_string().contains(expected),
                "{pointer} mutation produced {error}"
            );
        }
    }

#[test]
    fn publishes_one_hierarchy_with_rigid_and_skinned_bindings_and_exact_animation_data() {
        let (temp, report) = publish_fixture(&fixture());
        assert_eq!(report.status, "staged-incomplete");
        assert!(!report.publishable);
        assert_eq!(
            report.material_publish.status,
            "native-data-complete-runtime-validation-pending"
        );
        assert_eq!(report.material_publish.material_count, 2);
        assert_eq!(report.material_publish.texture_count, 2);
        assert_eq!(report.material_publish.sampler_count, 2);
        assert_eq!(report.material_publish.explicit_null_slots, 0);
        assert_eq!(report.contract.legacy_name, "True Hero");
        assert_eq!(report.contract.source.nodes, 5);
        assert_eq!(report.contract.source.mesh_parts, 2);
        assert_eq!(report.contract.source.skinned_meshes, 1);
        assert_eq!(report.contract.source.joints, 2);
        assert_eq!(report.contract.source.weighted_vertices, 3);
        assert_eq!(report.contract.source.animation_channels, 2);
        assert_eq!(report.contract.source.animation_keyframes, 4);
        assert_eq!(report.contract.source.cubic_spline_keyframes, 2);
        assert_eq!(report.contract.source.animation_events, 1);

        let glb_path = temp.path().join("output").join(&report.contract.output_glb);
        assert_eq!(glb_path.file_name().unwrap(), "True Hero.glb");
        let document = glb_json(&fs::read(&glb_path).unwrap());
        let nodes = document["nodes"].as_array().unwrap();
        let root = document["scenes"][0]["nodes"][0].as_u64().unwrap() as usize;
        assert_eq!(nodes[root]["name"], "True Hero");
        let body = nodes.iter().find(|node| node["name"] == "Body").unwrap();
        let sword = nodes.iter().find(|node| node["name"] == "Sword").unwrap();
        assert!(body.get("mesh").is_some() && body.get("skin").is_some());
        assert!(sword.get("mesh").is_some() && sword.get("skin").is_none());
        let interpolations = document["animations"][0]["samplers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sampler| sampler["interpolation"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(interpolations, BTreeSet::from(["CUBICSPLINE", "LINEAR"]));
        assert_eq!(
            document["animations"][0]["extras"]["nonTrs"]["events"][0]["functionName"],
            "particle"
        );
        let slots = document["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|mesh| mesh["primitives"].as_array().unwrap())
            .filter_map(|primitive| primitive["extras"]["materialSlot"].as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(slots, BTreeSet::from(["Body Material", "Sword Material"]));
        assert_eq!(document["materials"].as_array().unwrap().len(), 2);
        assert_eq!(document["images"].as_array().unwrap().len(), 2);
        assert_eq!(document["textures"].as_array().unwrap().len(), 2);
        assert_eq!(document["samplers"].as_array().unwrap().len(), 2);
        let image_uris = document["images"]
            .as_array()
            .unwrap()
            .iter()
            .map(|image| image["uri"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            image_uris,
            BTreeSet::from([
                "True Hero.textures/Body Diffuse.png",
                "True Hero.textures/Sword Diffuse.png"
            ])
        );
        for uri in image_uris {
            assert!(glb_path.parent().unwrap().join(uri).is_file());
        }
        let material_indices = document["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|mesh| mesh["primitives"].as_array().unwrap())
            .map(|primitive| primitive["material"].as_u64().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(material_indices, BTreeSet::from([0, 1]));
        let tree_audit =
            crate::logical_model_tree_audit::audit_logical_model_tree(&temp.path().join("output"))
                .unwrap();
        assert!(tree_audit.passed, "{:?}", tree_audit.violations);
        assert!(tree_audit.gpu_gate_pending);
        assert_eq!(tree_audit.counts.glbs, 1);
        assert_eq!(tree_audit.counts.pngs, 2);
        assert_eq!(tree_audit.counts.orphan_pngs, 0);
    }

#[test]
    fn excludes_only_biped_display_mesh_geometry_and_preserves_the_rig() {
        let (temp, report) = publish_fixture(&fixture_with_biped_display_helper());
        assert_eq!(
            report.source_geometry_filter.policy,
            "exclude-3ds-max-biped-display-meshes-v1"
        );
        assert_eq!(
            report.source_geometry_filter.excluded_rigid_mesh_bindings,
            1
        );
        assert_eq!(report.source_geometry_filter.excluded_mesh_parts, 1);
        assert_eq!(
            report.source_geometry_filter.excluded_source_mesh_ids,
            ["fixture:mesh-biped-helper"]
        );
        assert_eq!(report.source_geometry_filter.preserved_transform_nodes, 6);
        assert_eq!(
            report
                .source_geometry_filter
                .preserved_skinned_mesh_bindings,
            1
        );
        assert_eq!(report.contract.source.nodes, 6);
        assert_eq!(report.contract.source.mesh_parts, 2);
        assert_eq!(report.contract.source.skinned_meshes, 1);
        assert_eq!(report.contract.source.joints, 2);
        assert_eq!(report.contract.source.inverse_bind_matrices, 2);
        assert_eq!(report.contract.source.weighted_vertices, 3);
        assert_eq!(report.contract.source.animation_clips, 1);
        assert_eq!(report.contract.source.animation_channels, 2);
        assert_eq!(report.contract.source, report.contract.published);

        let output = temp.path().join("output");
        let document = glb_json(&fs::read(output.join(&report.contract.output_glb)).unwrap());
        let nodes = document["nodes"].as_array().unwrap();
        let helper = nodes
            .iter()
            .find(|node| node["name"] == "Biped Object@#17")
            .expect("helper Transform must remain for hierarchy fidelity");
        assert!(
            helper.get("mesh").is_none(),
            "viewport helper geometry must not survive publication"
        );
        assert_eq!(document["meshes"].as_array().unwrap().len(), 2);
        assert_eq!(document["skins"].as_array().unwrap().len(), 1);
        assert_eq!(document["animations"].as_array().unwrap().len(), 1);
        assert_eq!(document["materials"].as_array().unwrap().len(), 2);
        assert_eq!(document["textures"].as_array().unwrap().len(), 2);
        assert!(
            document["materials"]
                .as_array()
                .unwrap()
                .iter()
                .all(|material| material["name"] != "Biped Helper Material")
        );
    }

#[test]
    fn permits_duplicate_true_animation_names_without_renaming_them() {
        let mut value = fixture();
        let duplicate = value["animations"][0].clone();
        value["animations"].as_array_mut().unwrap().push(duplicate);
        let (temp, report) = publish_fixture(&value);
        let glb_path = temp.path().join("output").join(report.contract.output_glb);
        let document = glb_json(&fs::read(glb_path).unwrap());
        let names = document["animations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|animation| animation["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["stand", "stand"]);
    }

#[test]
    fn relative_skeleton_path_prefers_the_deepest_repeated_root_name() {
        let paths = BTreeMap::from([
            ("mob_sneakyspawn".to_owned(), 0),
            ("mob_sneakyspawn/mob_sneakyspawn".to_owned(), 1),
            ("mob_sneakyspawn/mob_sneakyspawn/Bip01".to_owned(), 2),
        ]);
        assert_eq!(
            suffix_node_path(&paths, "mob_sneakyspawn", "skeleton joint").unwrap(),
            1
        );
        assert_eq!(
            suffix_node_path(&paths, "mob_sneakyspawn/Bip01", "skeleton joint").unwrap(),
            2
        );
    }
