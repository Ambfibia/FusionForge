use super::*;

pub(super) fn strip_mip_source_evidence(object: &mut Map<String, Value>) -> u64 {
    let Some(mips) = object.get_mut("mips").and_then(Value::as_array_mut) else {
        return 0;
    };
    mips.iter_mut()
        .filter_map(Value::as_object_mut)
        .map(|mip| remove_optional(mip, "sourceEncoded"))
        .sum()
}
