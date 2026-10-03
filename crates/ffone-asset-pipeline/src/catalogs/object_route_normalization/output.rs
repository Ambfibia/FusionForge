use super::*;

pub(super) fn write_report(
    project_root: &Path,
    asset_root: &Path,
    report_path: &Path,
    report: &ObjectRouteNormalizationReport,
) -> Result<()> {
    let path = if report_path.is_absolute() {
        report_path.to_owned()
    } else {
        project_root.join(report_path)
    };
    let path = checked_absolute_path(&path)?;
    if path.starts_with(asset_root) {
        return invalid("object route report must remain outside assets/game");
    }
    write_replace(&path, &pretty_json(report)?)
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    create_parent(path)?;
    if path.exists() {
        return invalid(format!(
            "refusing to overwrite staged file: {}",
            path.display()
        ));
    }
    fs::write(path, bytes).map_err(|error| io_at(path, error))
}

pub(super) fn write_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    create_parent(path)?;
    let temporary = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("file"),
        std::process::id()
    ));
    fs::write(&temporary, bytes).map_err(|error| io_at(&temporary, error))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| io_at(path, error))?;
    }
    fs::rename(&temporary, path).map_err(|error| io_at(path, error))
}
