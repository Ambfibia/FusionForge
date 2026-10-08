use super::*;

fn model() -> Value {
    json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
        "nodes":[{"name":"root","scale":[1.0,1.0,1.0],"children":[1]},{"name":"bone"}],
        "buffers":[{"byteLength":4}],"bufferViews":[],"accessors":[],"animations":[]})
}

#[test]
fn static_native_size_override_preserves_original_nodes_and_binary() {
    let donor = model();
    let mut target = model();
    target["nodes"][0]["scale"] = json!([1.3, 1.3, 1.3]);
    assert!(append(target.clone(), vec![1, 2, 3, 4], &donor, &[], &[], false).is_err());
    let bytes = append(target.clone(), vec![1, 2, 3, 4], &donor, &[], &[], true).unwrap();
    let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let output: Value = serde_json::from_slice(&bytes[20..20 + n]).unwrap();
    assert_eq!(output["nodes"], target["nodes"]);
    assert_eq!(&bytes[28 + n..], &[1, 2, 3, 4]);
}

#[test]
fn root_scale_exception_never_accepts_animated_root_or_different_bones() {
    let mut target = model();
    let mut donor = model();
    target["nodes"][0]["scale"] = json!([1.3, 1.3, 1.3]);
    donor["animations"] =
        json!([{"name":"skill2","channels":[{"target":{"node":0,"path":"scale"}}]}]);
    assert!(
        append(target, vec![], &donor, &[], &["skill2".into()], true)
            .unwrap_err()
            .contains("rest pose")
    );
    let mut donor = model();
    donor["nodes"][1]["translation"] = json!([1.0, 0.0, 0.0]);
    assert!(
        append(model(), vec![], &donor, &[], &[], true)
            .unwrap_err()
            .contains("rest pose")
    );
}

#[test]
fn existing_clips_cannot_be_replaced() {
    let mut target = model();
    target["animations"] = json!([{"name":"skill2"}]);
    assert!(
        append(target, vec![], &model(), &[], &["skill2".into()], false)
            .unwrap_err()
            .contains("refusing to replace")
    );
}

#[test]
fn replay_verifies_source_curves_after_buffer_relocation_and_rejects_edits() {
    let mut donor = model();
    donor["bufferViews"] = json!([{"buffer":0,"byteOffset":0,"byteLength":4},
        {"buffer":0,"byteOffset":4,"byteLength":12}]);
    donor["accessors"] = json!([{"bufferView":0,"componentType":5126,"count":1,"type":"SCALAR"},
        {"bufferView":1,"componentType":5126,"count":1,"type":"VEC3"}]);
    donor["animations"] = json!([{"name":"luncher","channels":[{"sampler":0,"target":{"node":1,"path":"translation"}}],
        "samplers":[{"input":0,"output":1,"interpolation":"LINEAR"}]}]);
    let source = append(donor.clone(),vec![0;16],&model(),&[],&[],false).unwrap();
    let mut native = append(model(),vec![1;4],&donor,&[0;16],&["luncher".into()],false).unwrap();
    selected_clips_match(&native,&source,&["luncher".into()]).unwrap();
    *native.last_mut().unwrap() = 1;
    assert!(selected_clips_match(&native,&source,&["luncher".into()]).unwrap_err().contains("differs"));
}
