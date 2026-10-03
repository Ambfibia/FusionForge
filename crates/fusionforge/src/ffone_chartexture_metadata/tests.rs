use super::validate_complete_image_size;

#[test]
fn complete_image_size_accepts_full_payload_base_mip_or_decoded_footprint() {
    assert!(validate_complete_image_size("full", 43_704, 43_704).is_ok());
    assert!(validate_complete_image_size("base", 32_768, 43_704).is_ok());
    assert!(validate_complete_image_size("decoded-footprint", 32_768, 21_864).is_ok());
}

#[test]
fn complete_image_size_rejects_empty_size_or_payload() {
    assert!(validate_complete_image_size("empty", 0, 43_704).is_err());
    assert!(validate_complete_image_size("empty-payload", 32_768, 0).is_err());
}
