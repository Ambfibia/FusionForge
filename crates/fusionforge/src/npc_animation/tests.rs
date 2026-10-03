use serde_json::json;

use super::*;

fn translation_x(matrix: Matrix4) -> f64 {
    matrix[0][3]
}

#[test]
fn samples_split_curves_and_accumulates_parent_transform() {
    let preview = json!({
        "skeleton": { "joints": [
            { "path": "Root", "translation": [1, 0, 0] },
            { "path": "Root/Hand", "parent": 0, "translation": [2, 0, 0] }
        ]},
        "animations": [{
            "name": "Wave",
            "duration": 2.0,
            "loop": false,
            "animationData": {
                "translations": [{
                    "path": "Root/Hand",
                    "keys": [
                        { "time": 0.0, "value": [2, 0, 0] },
                        { "time": 2.0, "value": [6, 0, 0] }
                    ]
                }]
            }
        }]
    });

    let pose = sample_preview_animation(&preview, "wave", 1.0).expect("sample pose");
    assert_eq!(pose.joints.len(), 2);
    assert!((translation_x(pose.joints[1].local_matrix) - 4.0).abs() < 1.0e-6);
    assert!((translation_x(pose.joints[1].global_matrix) - 5.0).abs() < 1.0e-6);
}

#[test]
fn loops_time_and_uses_shortest_quaternion_arc() {
    let preview = json!({
        "animations": [{
            "name": "Turn",
            "duration": 2.0,
            "animationData": { "tracks": [{
                "path": "Root",
                "rotationKeys": [
                    { "time": 0.0, "value": [0, 0, 0, 1] },
                    { "time": 2.0, "value": [0, 0, 0, -1] }
                ]
            }]}
        }]
    });

    let pose = sample_preview_animation(&preview, "Turn", 2.5).expect("sample pose");
    assert!((pose.time_seconds - 0.5).abs() < 1.0e-6);
    let rotation = pose.joints[0].local_matrix;
    assert!((rotation[0][0] - 1.0).abs() < 1.0e-6);
    assert!((rotation[1][1] - 1.0).abs() < 1.0e-6);
    assert!((rotation[2][2] - 1.0).abs() < 1.0e-6);
}

#[test]
fn skins_positions_with_derived_rest_bind_pose() {
    let preview = json!({
        "skeleton": { "joints": [
            { "path": "Root", "translation": [10, 0, 0] }
        ]},
        "meshes": [{
            "id": "body",
            "positions": [1, 2, 3],
            "skin": {
                "jointPaths": ["Root"],
                "boneIndices": [0, 0, 0, 0],
                "weights": [1, 0, 0, 0]
            }
        }],
        "animations": [{
            "name": "Move",
            "duration": 1.0,
            "loop": false,
            "animationData": { "translations": [{
                "path": "Root",
                "keys": [
                    { "time": 0.0, "value": [10, 0, 0] },
                    { "time": 1.0, "value": [12, 0, 0] }
                ]
            }]}
        }]
    });

    let sampler = NpcAnimationSampler::from_preview(&preview).expect("prepare sampler");
    assert!(sampler.has_skinned_meshes());
    let rest = sampler.sample("Move", 0.0).expect("rest pose");
    assert_eq!(rest.meshes[0].positions[0], [1.0, 2.0, 3.0]);
    let moved = sampler.sample("Move", 1.0).expect("moved pose");
    assert_eq!(moved.meshes[0].positions[0], [3.0, 2.0, 3.0]);
}

#[test]
fn explicit_unity_bind_pose_is_normalized_back_to_mesh_space() {
    let preview = json!({
        "skeleton": { "joints": [
            { "path": "Root", "translation": [10, 0, 0] }
        ]},
        "meshes": [{
            "id": "body",
            "positions": [1, 2, 3],
            "skin": {
                "jointPaths": ["Root"],
                "boneIndices": [0, 0, 0, 0],
                "weights": [1, 0, 0, 0],
                "inverseBindMatrices": [[
                    1, 0, 0, 0,
                    0, 1, 0, 0,
                    0, 0, 1, 0,
                    0, 0, 0, 1
                ]]
            }
        }],
        "animations": [{
            "name": "Move",
            "duration": 1.0,
            "loop": false,
            "animationData": { "translations": [{
                "path": "Root",
                "keys": [
                    { "time": 0.0, "value": [10, 0, 0] },
                    { "time": 1.0, "value": [12, 0, 0] }
                ]
            }]}
        }]
    });

    let sampler = NpcAnimationSampler::from_preview(&preview).expect("prepare sampler");
    let rest = sampler.sample("Move", 0.0).expect("rest pose");
    assert_eq!(rest.meshes[0].positions[0], [1.0, 2.0, 3.0]);
    let moved = sampler.sample("Move", 1.0).expect("moved pose");
    assert_eq!(moved.meshes[0].positions[0], [3.0, 2.0, 3.0]);
}

#[test]
fn metadata_only_clip_reports_missing_tracks() {
    let preview = json!({
        "animations": [{
            "name": "Idle",
            "duration": 1.0,
            "curveCounts": { "rotation": 12 },
            "canPreviewPose": true
        }]
    });
    let error = sample_preview_animation(&preview, "Idle", 0.0).unwrap_err();
    assert_eq!(
        error,
        NpcAnimationError::MissingTrackData("Idle".to_string())
    );
}

#[test]
fn rejects_skeleton_cycles_during_preparation() {
    let preview = json!({
        "skeleton": { "joints": [
            { "path": "A", "parent": 1 },
            { "path": "B", "parent": 0 }
        ]},
        "animations": []
    });
    let error = NpcAnimationSampler::from_preview(&preview).unwrap_err();
    assert!(
        matches!(error, NpcAnimationError::InvalidPreview(message) if message.contains("cycle"))
    );
}
