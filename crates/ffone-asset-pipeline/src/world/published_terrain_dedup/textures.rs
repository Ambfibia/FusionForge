use super::*;

pub(super) fn collapse_base_mip_zero(
    terrain_root: &Path,
    terrain_prefix: &str,
    texture: &mut JsonValue,
    removals: &mut BTreeSet<String>,
    context: &str,
) -> Result<bool> {
    let base_path = required_string(texture, "path", &format!("{context} base path"))?.to_owned();
    let base_hash =
        required_string(texture, "pngBlake3", &format!("{context} base hash"))?.to_owned();
    let (mip_index, mip_path, mip_hash) = texture
        .get("mips")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{context} has no mips")))?
        .iter()
        .enumerate()
        .find(|(_, mip)| mip.get("level").and_then(JsonValue::as_u64) == Some(0))
        .map(|(index, mip)| {
            Ok::<_, PipelineError>((
                index,
                required_string(mip, "path", &format!("{context} mip-zero path"))?.to_owned(),
                required_string(mip, "pngBlake3", &format!("{context} mip-zero hash"))?.to_owned(),
            ))
        })
        .transpose()?
        .ok_or_else(|| invalid_error(format!("{context} has no mip zero")))?;
    if base_path == mip_path
        || !exact_hashed_files_match(
            terrain_root,
            &base_path,
            &base_hash,
            &mip_path,
            &mip_hash,
            context,
        )?
    {
        return Ok(false);
    }
    texture
        .get_mut("mips")
        .and_then(JsonValue::as_array_mut)
        .and_then(|mips| mips.get_mut(mip_index))
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| invalid_error(format!("{context} mip zero disappeared")))?
        .insert("path".to_owned(), JsonValue::String(base_path));
    removals.insert(format!("{terrain_prefix}{mip_path}"));
    Ok(true)
}
