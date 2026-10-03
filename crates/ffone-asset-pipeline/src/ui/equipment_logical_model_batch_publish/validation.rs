use super::*;

pub(super) fn validate_source_manifest(manifest: &EquipmentSourceManifest) -> Result<()> {
    if manifest.schema != EQUIPMENT_SOURCE_BATCH_SCHEMA {
        return equipment_error(format!(
            "equipment source manifest schema must be {EQUIPMENT_SOURCE_BATCH_SCHEMA:?}"
        ));
    }
    if manifest.benchmark_route_limit.is_some()
        || !manifest.status.starts_with("complete")
        || manifest.counts.selected_resolved_routes != manifest.counts.resolved_routes
        || manifest.counts.exported_physical_models
            != u64_count(manifest.exported.len(), "source manifest exports")?
        || manifest.counts.total_blockers
            != u64_count(manifest.blockers.len(), "source manifest blockers")?
    {
        return equipment_error("equipment source manifest is not a complete full-batch manifest");
    }
    if manifest.source_root.trim().is_empty() {
        return equipment_error("equipment source manifest has no sourceRoot");
    }
    Ok(())
}

pub(super) fn reject_existing_output(output: &Path) -> Result<()> {
    if fs::symlink_metadata(output).is_ok() {
        return Err(PipelineError::OutputExists(output.to_path_buf()));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return equipment_error(format!(
            "equipment output parent is not a directory: {parent:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_true_name(value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > 1_024
        || value.chars().any(char::is_control)
        || has_generated_identity(value)
    {
        return equipment_error(format!(
            "equipment true name is invalid or generated: {value:?}"
        ));
    }
    validate_windows_component(value)
}

pub(super) fn validate_windows_component(value: &str) -> Result<()> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.trim() != value
        || value.ends_with('.')
        || value
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
        || value.encode_utf16().count() > 255
    {
        return equipment_error(format!("path component is not Windows-safe: {value:?}"));
    }
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        });
    if reserved {
        return equipment_error(format!(
            "path component is a reserved Windows name: {value:?}"
        ));
    }
    Ok(())
}

pub(super) fn equipment_error<T>(message: impl Into<String>) -> Result<T> {
    Err(equipment_error_value(message))
}

pub(super) fn equipment_error_value(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(format!(
        "equipment semantic batch publication: {}",
        message.into()
    ))
}
