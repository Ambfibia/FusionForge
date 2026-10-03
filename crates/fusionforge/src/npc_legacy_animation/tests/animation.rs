use super::*;

#[test]
fn exact_animation_path_prefers_same_named_descendant_over_true_root() {
    let paths = vec![
        "f_pants_gothgirl".to_string(),
        "f_pants_gothgirl/f_pants_gothgirl".to_string(),
        "f_pants_gothgirl/f_pants_gothgirl/Bip01".to_string(),
    ];

    assert_eq!(
        resolve_exact_animation_path(&paths, "f_pants_gothgirl", "nif-default")
            .expect("non-empty Unity animation path names the direct child"),
        "f_pants_gothgirl/f_pants_gothgirl"
    );
}

#[test]
fn empty_animation_path_resolves_to_the_sole_true_root() {
    let paths = vec![
        "thrown_draculasdentures".to_string(),
        "thrown_draculasdentures/Bip01".to_string(),
    ];

    assert_eq!(
        resolve_exact_animation_path(&paths, "", "idle")
            .expect("Unity empty animation paths bind the owning root"),
        "thrown_draculasdentures"
    );
}

pub(super) fn constant_test_clip(
    name: &str,
    positions: Vec<fusionforge::UnityValue>,
) -> fusionforge::UnityValue {
    use fusionforge::UnityValue::{Array, Float, Object, String as UnityString};

    Object(BTreeMap::from([
        ("m_Name".to_string(), UnityString(name.to_string())),
        ("m_SampleRate".to_string(), Float(30.0)),
        ("m_PositionCurves".to_string(), Array(positions)),
        ("m_RotationCurves".to_string(), Array(Vec::new())),
        ("m_CompressedRotationCurves".to_string(), Array(Vec::new())),
        ("m_EulerCurves".to_string(), Array(Vec::new())),
        ("m_ScaleCurves".to_string(), Array(Vec::new())),
        ("m_FloatCurves".to_string(), Array(Vec::new())),
        ("m_PPtrCurves".to_string(), Array(Vec::new())),
        ("m_Events".to_string(), Array(Vec::new())),
    ]))
}

#[test]
fn exact_animation_validation_rejects_duplicate_sampleable_target() {
    let (mut clip, hierarchy) = simon_empty_binding_regression_fixture();
    let mut duplicate = clip["animationData"]["translations"][0].clone();
    duplicate["sourceIndex"] = json!(94);
    clip["animationData"]["translations"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    clip["curveCounts"]["position"] = json!(95);

    let error = validate_exact_animation_source(&hierarchy, &[clip]).unwrap_err();
    assert!(
        error.contains("duplicate sampleable translation target"),
        "{error}"
    );
}

#[test]
fn exact_animation_validation_rejects_decoder_warning() {
    let (mut clip, hierarchy) = simon_empty_binding_regression_fixture();
    clip["decodeWarnings"] = json!(["m_PositionCurves[7]: source value could not be decoded"]);

    let error = validate_exact_animation_source(&hierarchy, &[clip]).unwrap_err();
    assert!(error.contains("undecoded source curves"), "{error}");
    assert!(error.contains("m_PositionCurves[7]"), "{error}");
}

#[test]
fn exact_animation_validation_preserves_unmatched_empty_binding_metadata() {
    let (mut clip, hierarchy) = simon_empty_binding_regression_fixture();
    clip["animationData"]["emptyTrsBindings"][0]["path"] = json!("SiblingRig/Bip01/Head");

    mark_unbound_animation_bindings(&hierarchy, std::slice::from_mut(&mut clip))
        .expect("zero-key sibling metadata is classified");
    assert_eq!(
        clip["animationData"]["emptyTrsBindings"][0]["unboundModelTarget"],
        json!(true)
    );
    validate_exact_animation_source(&hierarchy, &[clip])
        .expect("zero-key metadata does not require a model target");
}

#[test]
fn unbound_classification_accepts_zero_curve_metadata_only_clip() {
    let hierarchy = json!({ "nodes": [{ "path": "npc_arghost" }] });
    let mut clip = json!({
        "asset": "CustomAssetBundle-DongResources_12_15",
        "pathId": 1,
        "name": "nif-default",
        "duration": 0.0,
        "declaredDuration": null,
        "keyedDuration": null,
        "eventDuration": null,
        "sampleRate": 30.0,
        "curveCounts": {
            "rotation": 0,
            "compressedRotation": 0,
            "position": 0,
            "scale": 0,
            "euler": 0,
            "float": 0,
            "pptr": 0,
            "events": 0
        },
        "events": []
    });

    mark_unbound_animation_bindings(&hierarchy, std::slice::from_mut(&mut clip))
        .expect("metadata-only clip has no keyed targets to classify");
    validate_exact_animation_source(&hierarchy, &[clip])
        .expect("zero-curve metadata-only clip remains an exact source record");
}

#[test]
fn exact_animation_classifies_keyed_tracks_for_a_sibling_rig_as_unbound() {
    let (mut clip, hierarchy) = simon_empty_binding_regression_fixture();
    clip["animationData"]["translations"][0]["path"] = json!("SiblingRig/Bip01/Head");

    let unmarked_error =
        validate_exact_animation_source(&hierarchy, &[clip.clone()]).unwrap_err();
    assert!(
        unmarked_error.contains("has no suffix match under the true root"),
        "{unmarked_error}"
    );

    mark_unbound_animation_bindings(&hierarchy, std::slice::from_mut(&mut clip))
        .expect("shared Unity clip can classify its sibling-rig curves");
    assert_eq!(
        clip["animationData"]["translations"][0]["unboundModelTarget"],
        json!(true)
    );
    validate_exact_animation_source(&hierarchy, &[clip])
        .expect("explicit unbound keyed metadata remains lossless");
}

#[test]
fn legacy_animation_event_is_preserved_as_typed_metadata() {
    use fusionforge::UnityValue::{Array, Float, Int, Object, String as UnityString};

    let body = Object(BTreeMap::from([(
        "m_Events".to_string(),
        Array(vec![Object(BTreeMap::from([
            ("time".to_string(), Float(0.5)),
            ("functionName".to_string(), UnityString("end".to_string())),
            ("data".to_string(), UnityString("payload".to_string())),
            ("floatParameter".to_string(), Float(1.25)),
            ("intParameter".to_string(), Int(7)),
            ("messageOptions".to_string(), Int(1)),
        ]))]),
    )]));

    let mut errors = Vec::new();
    let events = decode_animation_events(&body, &mut errors);
    assert!(errors.is_empty());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["time"], json!(0.5));
    assert_eq!(events[0]["functionName"], json!("end"));
    assert_eq!(events[0]["stringParameter"], json!("payload"));
    assert_eq!(events[0]["floatParameter"], json!(1.25));
    assert_eq!(events[0]["intParameter"], json!(7));
    assert_eq!(events[0]["messageOptions"], json!(1));
    assert!(events[0]["objectParameter"].is_null());
    assert_eq!(
        events[0]["objectParameterProvenance"],
        json!({
            "presence": "missing",
            "interpretation": "missing",
        })
    );
}

#[test]
fn live_larry_animation_event_type_tree_is_legacy_format_6() {
    fn find_named<'a>(
        tree: &'a fusionforge::TypeTree,
        name: &str,
    ) -> Option<&'a fusionforge::TypeTree> {
        (tree.name == name).then_some(tree).or_else(|| {
            tree.children
                .iter()
                .find_map(|child| find_named(child, name))
        })
    }
    fn find_type<'a>(
        tree: &'a fusionforge::TypeTree,
        type_name: &str,
    ) -> Option<&'a fusionforge::TypeTree> {
        (tree.type_name == type_name).then_some(tree).or_else(|| {
            tree.children
                .iter()
                .find_map(|child| find_type(child, type_name))
        })
    }

    let repo_root = crate::repository_root()
        .parent()
        .expect("workspace root");
    let asset_path = repo_root
        .join("FFOneClient")
        .join("content")
        .join("generated")
        .join("primary-focused-self-contained.ffclient")
        .join("cache")
        .join("extracted-bundles")
        .join("4e026249c2e3d168")
        .join("CustomAssetBundle-574ca8c2fed89492582a3fbd49965c12");
    if !asset_path.is_file() {
        eprintln!(
            "skipping live Larry event TypeTree fixture: not found at {}",
            asset_path.display()
        );
        return;
    }

    let env = fusionforge::UnityEnvironment::from_paths(&[asset_path]);
    let asset = env.assets.first().expect("Larry source asset");
    assert_eq!(asset.format, 6);
    let info = asset.objects.get(&154).expect("Larry walk AnimationClip");
    assert_eq!(asset.object_type_name(info), "AnimationClip");
    let tree = asset
        .object_type_tree(info)
        .expect("AnimationClip TypeTree");
    let events = find_named(tree, "m_Events").expect("m_Events TypeTree");
    let event = find_type(events, "AnimationEvent").expect("AnimationEvent TypeTree");
    let field_names = event
        .children
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        field_names,
        vec![
            "time",
            "functionName",
            "data",
            "objectReferenceParameter",
            "floatParameter",
            "messageOptions",
        ]
    );
    let body = asset.read_object(0, info).expect("Larry walk body");
    let event = fusionforge::value_array(body.get("m_Events"))
        .first()
        .expect("Larry walk event");
    let float_parameter = event_field(event, &["floatParameter", "m_FloatParameter"])
        .and_then(fusionforge::UnityValue::as_f64)
        .expect("Larry floatParameter");
    assert_eq!(
        event_field(event, &["time", "m_Time"]).and_then(fusionforge::UnityValue::as_f64),
        Some(3.0)
    );
    assert_eq!(
        event_field(event, &["functionName", "m_FunctionName"])
            .and_then(fusionforge::UnityValue::as_str),
        Some("end")
    );
    assert_eq!(
        event_field(event, &["data", "stringParameter", "m_StringParameter"])
            .and_then(fusionforge::UnityValue::as_str),
        Some("")
    );
    assert_eq!(
        event_field(event, &["messageOptions", "m_MessageOptions"])
            .and_then(fusionforge::UnityValue::as_i64),
        Some(1_852_788_223)
    );
    assert_eq!((float_parameter as f32).to_bits(), 0xffff_ff00);
    assert!(float_parameter.is_nan());
}

#[test]
fn live_rex_bundle_exports_sampleable_animation_and_skin() {
    let repo_root = crate::repository_root()
        .parent()
        .expect("workspace root");
    let bundle = repo_root
        .join("builds")
        .join("FusionFall-ru-0-0-3")
        .join("Character_Rex.resourceFile");
    if !bundle.is_file() {
        eprintln!(
            "skipping live Rex animation test: fixture not found at {}",
            bundle.display()
        );
        return;
    }
    let project = repo_root.join("work").join("ffclienteditor");
    let preview = crate::preview_bundle_container_model(
        bundle.to_string_lossy().to_string(),
        Some(project.to_string_lossy().to_string()),
        vec!["mob/npc_rex.kfm".to_string()],
    )
    .expect("preview Character_Rex");

    let stand3 = preview["animations"]
        .as_array()
        .expect("animations")
        .iter()
        .find(|clip| clip["name"] == "stand3")
        .expect("stand3 clip");
    let animation_names = preview["animations"]
        .as_array()
        .expect("animations")
        .iter()
        .filter_map(|clip| clip["name"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        animation_names,
        BTreeSet::from([
            "death",
            "melee1",
            "melee2",
            "nif-default",
            "run",
            "stand1",
            "stand2",
            "stand3",
            "walk",
            "wound",
        ]),
        "Rex preview must not inherit clips from unrelated Animation components"
    );
    assert_eq!(stand3["canPreviewPose"], json!(true));
    assert_eq!(stand3["previewSupport"], "unity-legacy-trs");
    assert!(stand3["duration"].as_f64().unwrap_or_default() > 2.0);
    assert!(stand3["animationData"]["rotations"]
        .as_array()
        .is_some_and(|curves| !curves.is_empty()));
    assert!(stand3["animationData"]["translations"]
        .as_array()
        .is_some_and(|curves| !curves.is_empty()));
    assert!(preview["skeleton"]["joints"]
        .as_array()
        .is_some_and(|joints| joints.len() >= 50));
    assert!(preview["meshes"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|mesh| mesh.get("skin").is_some()));

    let sampler = crate::npc_animation::NpcAnimationSampler::from_preview(&preview)
        .expect("animation sampler");
    assert!(sampler.has_sampleable_clip("stand3"));
    assert!(sampler.has_skinned_meshes());
    let early_pose = sampler.sample("stand3", 0.05).expect("sample early stand3");
    let later_pose = sampler.sample("stand3", 0.5).expect("sample later stand3");
    let early_mesh = early_pose
        .meshes
        .iter()
        .find(|mesh| mesh.skinned)
        .expect("skinned mesh");
    let later_mesh = later_pose
        .meshes
        .iter()
        .find(|mesh| mesh.id == early_mesh.id)
        .expect("same skinned mesh at later time");
    assert!(!early_mesh.positions.is_empty());
    assert!(early_mesh
        .positions
        .iter()
        .flatten()
        .all(|value| value.is_finite()));
    let max_vertex_delta = early_mesh
        .positions
        .iter()
        .zip(&later_mesh.positions)
        .map(|(early, later)| {
            early
                .iter()
                .zip(later)
                .map(|(left, right)| (*left - *right).abs())
                .fold(0.0_f32, f32::max)
        })
        .fold(0.0_f32, f32::max);
    assert!(
        max_vertex_delta > 1.0e-4,
        "stand3 must deform the Rex mesh over time (max delta {max_vertex_delta})"
    );
}

#[test]
fn live_tutorial_building_rigid_animation_owner_exports_exact_fall_and_stand1() {
    let repo_root = crate::repository_root()
        .parent()
        .expect("workspace root");
    let project = crate::repository_root()
        .join("work/sources/previous");
    let bundle = repo_root
        .join("builds")
        .join("retrobution-20260613")
        .join("Tutorial.resourceFile");
    if !bundle.is_file() {
        eprintln!(
            "skipping live npc_building rigid animation test: fixture not found at {}",
            bundle.display()
        );
        return;
    }

    let preview = crate::preview_bundle_container_model_exact(
        bundle.to_string_lossy().to_string(),
        Some(project.to_string_lossy().to_string()),
        "mob/npc_building.kfm".to_string(),
    )
    .expect("exact npc_building logical-model source");
    assert!(preview["exactContainerTargets"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|target| target["pathId"] == json!(4149) && target["objectType"] == "GameObject"));

    let asset_path = project
        .join("cache")
        .join("extracted-bundles")
        .join("96f743bf345458ed")
        .join("CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a");
    assert!(
        asset_path.is_file(),
        "exact route extraction must retain its serialized Unity asset at {}",
        asset_path.display()
    );
    let env = fusionforge::UnityEnvironment::from_paths(&[asset_path]);
    let (asset_index, asset) = env
        .assets
        .iter()
        .enumerate()
        .find(|(_, asset)| asset.objects.contains_key(&4149))
        .expect("serialized asset containing npc_building GameObject#4149");
    assert_eq!(
        asset.object_type_name(asset.objects.get(&4149).unwrap()),
        "GameObject"
    );
    let animation_info = asset
        .objects
        .get(&7055)
        .expect("npc_building Animation#7055");
    assert_eq!(asset.object_type_name(animation_info), "Animation");
    let animation = asset
        .read_object(asset_index, animation_info)
        .expect("npc_building Animation body");
    assert_eq!(
        resolved_key(&env, animation.get("m_GameObject")),
        Some((asset_index, 4149))
    );
    assert_eq!(
        resolved_key(&env, animation.get("m_Animation")),
        Some((asset_index, 1007))
    );
    let linked_clips = fusionforge::value_array(animation.get("m_Animations"))
        .iter()
        .filter_map(|value| resolved_key(&env, Some(value)))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        linked_clips,
        BTreeSet::from([(asset_index, 1007), (asset_index, 1008)])
    );
    assert_eq!(
        fusionforge::object_name(
            &asset
                .read_object(asset_index, asset.objects.get(&1007).unwrap())
                .expect("fall AnimationClip#1007")
        ),
        "fall"
    );

    let published_clips = preview["animations"]
        .as_array()
        .expect("npc_building animations")
        .iter()
        .map(|clip| {
            (
                clip["name"].as_str().unwrap_or_default(),
                clip["pathId"].as_i64().unwrap_or_default(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        published_clips,
        BTreeSet::from([("fall", 1007), ("stand1", 1008)])
    );
    let fall = preview["animations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|clip| clip["pathId"] == json!(1007))
        .expect("published fall#1007");
    assert_eq!(fall["canPreviewPose"], json!(true));
    assert_eq!(fall["previewSupport"], "unity-legacy-trs");
}
