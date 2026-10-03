use super::*;

pub(super) fn write_minimal_animation_gltf() -> PathBuf {
    let mut bytes = Vec::new();

    let times_offset = bytes.len();
    for value in [0.0_f32, 1.0] {
        push_f32(&mut bytes, value);
    }
    let times_length = bytes.len() - times_offset;

    let translation_offset = bytes.len();
    for value in [0.0_f32, 0.0, 0.0, 1.0, 2.0, 3.0] {
        push_f32(&mut bytes, value);
    }
    let translation_length = bytes.len() - translation_offset;

    let rotation_offset = bytes.len();
    for value in [0.0_f32, 0.0, 0.0, 1.0, 0.1, 0.2, 0.3, 0.9] {
        push_f32(&mut bytes, value);
    }
    let rotation_length = bytes.len() - rotation_offset;

    let scale_offset = bytes.len();
    for value in [1.0_f32, 1.0, 1.0, 2.0, 3.0, 4.0] {
        push_f32(&mut bytes, value);
    }
    let scale_length = bytes.len() - scale_offset;

    let data_uri = format!(
        "data:application/octet-stream;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    let document = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [
            { "name": "Root", "children": [1] },
            { "name": "Bone" }
        ],
        "buffers": [{
            "uri": data_uri,
            "byteLength": bytes.len()
        }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": times_offset, "byteLength": times_length },
            { "buffer": 0, "byteOffset": translation_offset, "byteLength": translation_length },
            { "buffer": 0, "byteOffset": rotation_offset, "byteLength": rotation_length },
            { "buffer": 0, "byteOffset": scale_offset, "byteLength": scale_length }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0.0], "max": [1.0] },
            { "bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5126, "count": 2, "type": "VEC4" },
            { "bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3" }
        ],
        "animations": [{
            "name": "wave",
            "samplers": [
                { "input": 0, "output": 1, "interpolation": "LINEAR" },
                { "input": 0, "output": 2, "interpolation": "LINEAR" },
                { "input": 0, "output": 3, "interpolation": "LINEAR" }
            ],
            "channels": [
                { "sampler": 0, "target": { "node": 1, "path": "translation" } },
                { "sampler": 1, "target": { "node": 1, "path": "rotation" } },
                { "sampler": 2, "target": { "node": 1, "path": "scale" } }
            ]
        }]
    });
    let path = temp_gltf_path();
    fs::write(&path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    path
}

#[test]
fn imports_gltf_animation_channels_as_unity_curves() {
    let path = write_minimal_animation_gltf();
    let clips = ImportedAnimationClip::from_gltf_path(&path, 30.0).unwrap();
    let _ = fs::remove_file(&path);

    assert_eq!(clips.len(), 1);
    let clip = &clips[0];
    assert_eq!(clip.name, "wave");
    assert_eq!(clip.sample_rate, 30.0);
    assert_eq!(clip.duration, 1.0);
    assert_eq!(clip.translations.len(), 1);
    assert_eq!(clip.rotations.len(), 1);
    assert_eq!(clip.scales.len(), 1);
    assert_eq!(clip.translations[0].path, "Root/Bone");
    assert_eq!(clip.translations[0].keys[1].value, (-1.0, 3.0, 2.0));
    assert_eq!(clip.scales[0].keys[1].value, (2.0, 4.0, 3.0));

    let rotation = clip.rotations[0].keys[1].value;
    assert_close(rotation.0, -0.1);
    assert_close(rotation.1, 0.3);
    assert_close(rotation.2, 0.2);
    assert_close(rotation.3, 0.9);

    let mut unity_value = UnityValue::Object(BTreeMap::new());
    apply_animation_clip_import(&mut unity_value, clip.clone()).unwrap();
    let object = unity_value.as_object().unwrap();
    assert_eq!(
        object.get("m_Name"),
        Some(&UnityValue::String("wave".to_string()))
    );
    assert_eq!(
        object.get("m_UseCompression"),
        Some(&UnityValue::Bool(false))
    );
    assert_eq!(
        object
            .get("m_PositionCurves")
            .and_then(UnityValue::as_array)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        object
            .get("m_CompressedRotationCurves")
            .and_then(UnityValue::as_array)
            .unwrap()
            .len(),
        0
    );
    let rotations = object
        .get("m_RotationCurves")
        .and_then(UnityValue::as_array)
        .unwrap();
    assert_eq!(rotations.len(), 1);
    let rotation_curve = rotations[0].as_object().unwrap();
    assert_eq!(
        rotation_curve.get("path"),
        Some(&UnityValue::String("Root/Bone".to_string()))
    );
    let keys = rotation_curve
        .get("curve")
        .and_then(UnityValue::as_object)
        .and_then(|curve| curve.get("m_Curve"))
        .and_then(UnityValue::as_array)
        .unwrap();
    assert_eq!(keys.len(), 2);
    let second_value = keys[1]
        .as_object()
        .and_then(|key| key.get("value"))
        .and_then(UnityValue::as_object)
        .unwrap();
    let component = |name: &str| second_value.get(name).and_then(UnityValue::as_f64).unwrap();
    assert_close(component("x"), -0.1);
    assert_close(component("y"), 0.3);
    assert_close(component("z"), 0.2);
    assert_close(component("w"), 0.9);
}

#[test]
fn gltf_bind_pose_conversion_matches_runtime_trs_basis() {
    let translation = [1.25_f32, -2.5, 3.75];
    let rotation = [
        0.0_f32,
        0.0,
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ];
    let scale = [1.0_f32, 1.0, 1.0];
    let path = write_skin_bindpose_gltf(translation, rotation, scale);

    let (document, buffers, _) = gltf::import(&path).unwrap();
    let skin = document.skins().next().unwrap();
    let joint = skin.joints().next().unwrap();
    let raw_bind_pose = skin
        .reader(|buffer| buffers.get(buffer.index()).map(|data| data.0.as_slice()))
        .read_inverse_bind_matrices()
        .unwrap()
        .next()
        .unwrap();
    let (translation, rotation, scale) = joint.transform().decomposed();
    let global = trs_matrix4(
        gltf_vec3_to_fusionfall(translation[0], translation[1], translation[2]),
        gltf_quat_to_fusionfall(rotation[0], rotation[1], rotation[2], rotation[3]),
        gltf_scale_to_fusionfall(scale[0], scale[1], scale[2]),
    );
    let production = imported_matrix4(&gltf_bind_pose_to_fusionfall(raw_bind_pose));
    let error = identity_error(multiply_matrix4(global, production));

    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(path.with_extension("bin"));
    assert!(
        error < 1e-4,
        "production bind-pose error global*bind={error}",
    );
}
