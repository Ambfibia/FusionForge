use super::*;

pub(super) fn face_head_texture_aliases(row: &Value, field: &str) -> Vec<FieldRoute> {
    let Some(value) = string_field(row, field) else {
        return Vec::new();
    };
    let Some(base) = alias_base(value, &["_a", "_b", "_c", "_d", "_e"]) else {
        return Vec::new();
    };
    ['a', 'b', 'c', 'd', 'e']
        .into_iter()
        .filter_map(|color| make_route("texture", &format!("{base}_{color}"), "dds"))
        .collect()
}
