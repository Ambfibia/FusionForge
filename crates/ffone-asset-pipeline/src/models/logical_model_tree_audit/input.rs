use super::*;

pub(super) fn read_unsigned(
    accessor: AccessorInfo,
    binary: &[u8],
    element: usize,
    component: usize,
) -> Option<u32> {
    let offset = accessor.component_offset(element, component)?;
    match accessor.component_type {
        5_121 => binary.get(offset).copied().map(u32::from),
        5_123 => binary
            .get(offset..offset + 2)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u16::from_le_bytes)
            .map(u32::from),
        5_125 => binary
            .get(offset..offset + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes),
        _ => None,
    }
}

pub(super) fn read_f32(
    accessor: AccessorInfo,
    binary: &[u8],
    element: usize,
    component: usize,
) -> Option<f32> {
    if accessor.component_type != 5_126 {
        return None;
    }
    let offset = accessor.component_offset(element, component)?;
    binary
        .get(offset..offset + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(f32::from_le_bytes)
}

pub(super) fn read_weight(
    accessor: AccessorInfo,
    binary: &[u8],
    element: usize,
    component: usize,
) -> Option<f64> {
    match accessor.component_type {
        5_126 => read_f32(accessor, binary, element, component).map(f64::from),
        5_121 if accessor.normalized => {
            read_unsigned(accessor, binary, element, component).map(|value| value as f64 / 255.0)
        }
        5_123 if accessor.normalized => {
            read_unsigned(accessor, binary, element, component).map(|value| value as f64 / 65_535.0)
        }
        _ => None,
    }
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
}

pub(super) fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_be_bytes)
}
