use super::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportManifest {
    pub(super) schema: &'static str,
    pub(super) source: SourceBundle,
    pub(super) selection: &'static str,
    pub(super) terrains: Vec<ExportedTerrain>,
}

pub(super) fn fresh_staging_path(output_dir: &Path) -> Result<PathBuf, String> {
    let parent = output_dir.parent().ok_or_else(|| {
        format!(
            "native terrain output has no parent directory: {}",
            output_dir.display()
        )
    })?;
    let name = output_dir
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            format!(
                "native terrain output has no UTF-8 directory name: {}",
                output_dir.display()
            )
        })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    Ok(parent.join(format!(".{name}.staging-{}-{nonce:x}", std::process::id())))
}
