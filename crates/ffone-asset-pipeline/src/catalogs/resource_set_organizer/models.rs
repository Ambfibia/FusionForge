use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerItemCatalogModel {
    pub category: String,
    pub true_name: String,
    pub source_route: String,
    pub resource_set: String,
    pub model: ResourceSetArtifact,
}

#[derive(Clone, Debug)]
pub(super) struct PlayerModel {
    pub(super) old_glb: PathBuf,
    pub(super) old_rooted: String,
    pub(super) directory: PathBuf,
    pub(super) category: String,
    pub(super) true_name: String,
    pub(super) source_route: String,
    pub(super) files: Vec<PathBuf>,
    pub(super) atlas_hashes: BTreeSet<String>,
    pub(super) support_hashes: BTreeSet<String>,
}

pub(super) fn read_glb_json_bytes(bytes: &[u8], label: &str) -> Result<JsonValue> {
    if bytes.len() < 20 || &bytes[..4] != b"glTF" {
        return invalid(format!("file is not GLB 2.0: {label}"));
    }
    let length = read_u32(bytes, 12)? as usize;
    if read_u32(bytes, 16)? != 0x4e4f_534a || 20 + length > bytes.len() {
        return invalid(format!("GLB has no valid JSON chunk: {label}"));
    }
    let mut json = &bytes[20..20 + length];
    while json.last().is_some_and(|byte| *byte == b' ' || *byte == 0) {
        json = &json[..json.len() - 1];
    }
    serde_json::from_slice(json).map_err(|source| PipelineError::Json {
        path: label.to_owned(),
        source,
    })
}

pub(super) fn rewrite_glb_uris(
    source: &Path,
    destination_rooted: &str,
    path_map: &BTreeMap<String, String>,
    asset_root: &Path,
) -> Result<Vec<u8>> {
    let bytes = fs::read(source).map_err(|error| io_at(source, error))?;
    if bytes.len() < 20 || &bytes[..4] != b"glTF" {
        return invalid(format!("map model is not GLB 2.0: {source:?}"));
    }
    let declared = read_u32(&bytes, 8)? as usize;
    if declared != bytes.len() {
        return invalid(format!("GLB length mismatch: {source:?}"));
    }
    let mut cursor = 12usize;
    let mut chunks = Vec::<(u32, Vec<u8>)>::new();
    while cursor < bytes.len() {
        let length = read_u32(&bytes, cursor)? as usize;
        let kind = read_u32(&bytes, cursor + 4)?;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .ok_or_else(|| invalid_error("GLB chunk length overflow"))?;
        if end > bytes.len() {
            return invalid(format!("truncated GLB chunk: {source:?}"));
        }
        chunks.push((kind, bytes[start..end].to_vec()));
        cursor = end;
    }
    let Some((_, json_bytes)) = chunks.first_mut().filter(|(kind, _)| *kind == 0x4e4f_534a) else {
        return invalid(format!("GLB has no leading JSON chunk: {source:?}"));
    };
    while json_bytes
        .last()
        .is_some_and(|byte| *byte == b' ' || *byte == 0)
    {
        json_bytes.pop();
    }
    let mut document: JsonValue =
        serde_json::from_slice(json_bytes).map_err(|source_error| PipelineError::Json {
            path: source.display().to_string(),
            source: source_error,
        })?;
    let old_parent = source
        .parent()
        .ok_or_else(|| invalid_error("GLB has no source parent"))?;
    let new_parent = Path::new(destination_rooted)
        .parent()
        .ok_or_else(|| invalid_error("GLB has no destination parent"))?;
    rewrite_json_file_strings(&mut document, old_parent, new_parent, path_map, asset_root)?;
    *json_bytes = serde_json::to_vec(&document).map_err(generated_json_error)?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12usize
        + chunks
            .iter()
            .map(|(_, chunk)| 8usize + chunk.len())
            .sum::<usize>();
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2u32.to_le_bytes());
    output.extend_from_slice(&(total as u32).to_le_bytes());
    for (kind, chunk) in chunks {
        output.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        output.extend_from_slice(&kind.to_le_bytes());
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

pub(super) fn read_glb_json(path: &Path) -> Result<JsonValue> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    if bytes.len() < 20 || &bytes[..4] != b"glTF" {
        return invalid(format!("file is not GLB 2.0: {path:?}"));
    }
    let length = read_u32(&bytes, 12)? as usize;
    let kind = read_u32(&bytes, 16)?;
    if kind != 0x4e4f_534a || 20 + length > bytes.len() {
        return invalid(format!("GLB has no valid JSON chunk: {path:?}"));
    }
    let mut json = &bytes[20..20 + length];
    while json.last().is_some_and(|byte| *byte == b' ' || *byte == 0) {
        json = &json[..json.len() - 1];
    }
    serde_json::from_slice(json).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })
}
