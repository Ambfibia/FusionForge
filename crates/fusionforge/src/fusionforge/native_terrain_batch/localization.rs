
pub(super) fn unity_translation_to_native(value: [f64; 3]) -> [f64; 3] {
    [-value[0], value[1], value[2]]
}
