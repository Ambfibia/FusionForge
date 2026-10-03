use super::*;

pub(super) fn validate_replacement_target(target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(target).map_err(|error| io_at(target, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return invalid("map replacement target is not a regular directory");
    }
    let catalog_path = target.join("catalog.json");
    let catalog: WorldPrefabCatalog =
        serde_json::from_slice(&read_regular_file(&catalog_path, "existing map catalog")?)
            .map_err(|source| PipelineError::Json {
                path: catalog_path.display().to_string(),
                source,
            })?;
    if catalog.schema != WORLD_PREFAB_CATALOG_SCHEMA
        || catalog.source_build != SOURCE_BUILD
        || catalog.generated_by != WORLD_PREFAB_TOOL
    {
        return invalid("existing map root is not owned by this exact organizer contract");
    }
    Ok(())
}

pub(super) fn validate_relative(value: &str) -> Result<()> {
    validate_relative_path(Path::new(value))
}

pub(super) fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("path is not safe relative: {}", path.display()));
    }
    Ok(())
}

pub(super) fn generated_json_error(source: serde_json::Error) -> PipelineError {
    PipelineError::Json {
        path: "generated map JSON".to_owned(),
        source,
    }
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(message.into())
}
