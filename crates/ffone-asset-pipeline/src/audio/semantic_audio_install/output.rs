use super::*;

#[derive(Clone, Debug)]
pub(super) struct PreviousInstall {
    pub(super) present: bool,
}

pub(super) fn inspect_previous_install(
    audio_root: &Path,
    manifest: &ProjectAssetManifest,
) -> Result<PreviousInstall> {
    let catalog_path = audio_root.join("catalog.json");
    let component_presence = OWNED_DIRECTORIES
        .iter()
        .map(|directory| audio_root.join(directory).exists())
        .collect::<Vec<_>>();
    let catalog_present = catalog_path.exists();
    let any_present = catalog_present || component_presence.iter().any(|present| *present);
    let owned_manifest_entries = manifest
        .files
        .iter()
        .filter(|entry| is_owned_entry(entry))
        .count();

    if !any_present {
        if owned_manifest_entries != 0 {
            return invalid(
                "manifest contains semantic-audio entries but the owned filesystem tree is absent",
            );
        }
        return Ok(PreviousInstall { present: false });
    }
    if !catalog_present || component_presence.iter().any(|present| !present) {
        return invalid(
            "partial audio/music|ambient|voice|sfx|catalog.json tree exists; ownership is not provable",
        );
    }
    let bytes = fs::read(&catalog_path).map_err(|error| io_at(&catalog_path, error))?;
    let header: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if header.get("schema").and_then(serde_json::Value::as_str)
        != Some(SEMANTIC_AUDIO_CATALOG_SCHEMA)
    {
        return invalid("existing semantic audio directories are not owned by this installer");
    }
    if owned_manifest_entries == 0 {
        return invalid(
            "semantic audio catalog exists but the project manifest has no owned semantic entries",
        );
    }
    Ok(PreviousInstall { present: true })
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    file.write_all(bytes).map_err(|error| io_at(path, error))?;
    file.sync_all().map_err(|error| io_at(path, error))
}
