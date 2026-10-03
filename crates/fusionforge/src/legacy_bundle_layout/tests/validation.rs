use super::*;

#[test]
fn strict_roundtrip_and_name_guards_reject_binary_reinterpretation_only() {
    assert!(object_roundtrip_is_lossless(&[1, 2, 3], &[1, 2, 3]));
    assert!(object_roundtrip_is_lossless(&[1, 2, 0, 0], &[1, 2]));
    assert!(object_roundtrip_is_lossless(&[1, 2], &[1, 2, 0, 0]));
    assert!(!object_roundtrip_is_lossless(&[1, 2, 3], &[1, 9, 3]));
    assert!(!object_roundtrip_is_lossless(&[1, 2], &[1, 2, 0, 0, 0, 0]));

    assert!(!invalid_serialized_object_name("Амфибия"));
    assert!(!invalid_serialized_object_name("npc_echoecho-main.dds"));
    assert!(invalid_serialized_object_name("binary\u{0000}name"));
    assert!(invalid_serialized_object_name("lossy\u{fffd}name"));
    assert!(invalid_serialized_object_name(&"x".repeat(4097)));
}
