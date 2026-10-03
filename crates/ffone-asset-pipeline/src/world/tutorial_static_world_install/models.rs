use super::*;

pub(super) fn validate_glb_dependencies(selected: &[SourcePublication]) -> Result<()> {
    let index = selected
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    for model in selected
        .iter()
        .filter(|file| file.kind == ProjectAssetKind::Model)
    {
        let bytes = read_regular_file(&model.source, "static world GLB")?;
        let document = parse_glb_json(&bytes, &model.path)?;
        let Some(images) = document.get("images") else {
            continue;
        };
        for image in images.as_array().ok_or_else(|| {
            invalid_error(format!("GLB images is not an array in {:?}", model.path))
        })? {
            let uri = required_string(image, "uri", "GLB image")?;
            validate_relative(uri)?;
            if uri.contains('?') || uri.contains('#') || !uri.ends_with(".png") {
                return invalid(format!(
                    "unsafe or unsupported external GLB image URI {uri:?} in {:?}",
                    model.path
                ));
            }
            let parent = model.path.rsplit_once('/').map_or("", |(parent, _)| parent);
            let resolved = format!("{parent}/{uri}");
            let dependency = index.get(resolved.as_str()).ok_or_else(|| {
                invalid_error(format!(
                    "GLB {:?} references missing selected texture {:?}",
                    model.path, resolved
                ))
            })?;
            if dependency.kind != ProjectAssetKind::Texture {
                return invalid(format!(
                    "GLB {:?} image dependency {:?} is not a texture",
                    model.path, resolved
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn parse_glb_json(bytes: &[u8], path: &str) -> Result<JsonValue> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return invalid(format!("{path:?} is not GLB 2.0"));
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().expect("fixed slice"));
    let total = u32::from_le_bytes(bytes[8..12].try_into().expect("fixed slice")) as usize;
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().expect("fixed slice")) as usize;
    let json_kind = u32::from_le_bytes(bytes[16..20].try_into().expect("fixed slice"));
    let json_end = 20_usize
        .checked_add(json_len)
        .ok_or_else(|| invalid_error(format!("{path:?} GLB JSON length overflow")))?;
    if version != 2 || total != bytes.len() || json_kind != 0x4e4f_534a || json_end > bytes.len() {
        return invalid(format!("{path:?} has an invalid GLB header"));
    }
    serde_json::from_slice(&bytes[20..json_end]).map_err(|source| PipelineError::Json {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn model_tile_root(tile_id: &str) -> String {
    match StaticWorldScope::of_tile(tile_id) {
        Ok(StaticWorldScope::WorldMap) => format!("models/world/maps/{tile_id}"),
        _ => format!("models/world/tutorial/{tile_id}"),
    }
}
