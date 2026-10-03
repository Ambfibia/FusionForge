use super::*;

pub(super) const NATIVE_VERTEX_FORMULA: &str =
    "[-column * sampleSpacingX, rawU16 / 32767 * heightScale, row * sampleSpacingZ]";

pub(super) const NATIVE_VERTEX_SHIFT_FORMULA: &str =
    "nativeX -= (positiveSourceX - negativeSourceX) * sampleSpacingX; nativeZ += (positiveSourceZ - negativeSourceZ) * sampleSpacingZ";

pub(super) fn exact_vertex_shifts(
    value: Option<&UnityValue>,
    width: u32,
    height: u32,
    label: &str,
) -> Result<Vec<TerrainVertexShift>, String> {
    let mut seen = BTreeSet::new();
    let values = value
        .ok_or_else(|| format!("{label} is missing"))?
        .as_array()
        .ok_or_else(|| format!("{label} is not an array"))?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{label}[{index}] is not an object"))?;
            let exact_u32 = |field: &str| {
                object
                    .get(field)
                    .and_then(exact_i64)
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| format!("{label}[{index}].{field} is not an exact u32"))
            };
            let flags_u32 = exact_u32("flags")?;
            let flags = u8::try_from(flags_u32)
                .map_err(|_| format!("{label}[{index}].flags={flags_u32} is outside u8"))?;
            let column = exact_u32("x")?;
            let row = exact_u32("y")?;
            if flags == 0
                || flags & !0b1111 != 0
                || flags & 0b0011 == 0b0011
                || flags & 0b1100 == 0b1100
            {
                return Err(format!(
                    "{label}[{index}].flags={flags:#06b} is not a valid directional shift mask"
                ));
            }
            if column >= width || row >= height {
                return Err(format!(
                    "{label}[{index}] coordinate ({column}, {row}) is outside {width}x{height}"
                ));
            }
            if !seen.insert((column, row)) {
                return Err(format!(
                    "{label}[{index}] duplicates coordinate ({column}, {row})"
                ));
            }
            Ok(TerrainVertexShift { flags, column, row })
        })
        .collect()
}
