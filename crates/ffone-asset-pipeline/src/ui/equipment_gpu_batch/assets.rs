use super::*;

pub(super) fn absolute_new_report_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Err(PipelineError::OutputExists(path.to_path_buf()));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let parent = canonical_directory(parent, "report parent")?;
    let name = path
        .file_name()
        .ok_or_else(|| gpu_batch_error_value("report path has no filename"))?;
    Ok(parent.join(name))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn is_exact_equipment_output_path(
    category: &str,
    exact_route: &str,
    true_name: &str,
    output_glb: &str,
) -> bool {
    let unqualified = format!("characters/player/equipment/{category}/{true_name}/{true_name}.glb");
    if output_glb == unqualified {
        return true;
    }
    let Some(route_qualifier) = exact_route
        .strip_prefix("wear/")
        .and_then(|route| route.strip_suffix(".nif"))
    else {
        return false;
    };
    output_glb
        == format!(
            "characters/player/equipment/{category}/{route_qualifier}/{true_name}/{true_name}.glb"
        )
}
