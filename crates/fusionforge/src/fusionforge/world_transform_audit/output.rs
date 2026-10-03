use super::*;

pub(super) fn write_report_exclusive(path: &Path, report: &WorldTransformAuditReport) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "could not create world audit report directory {}: {err}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|err| format!("could not serialize world audit report: {err}"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| {
            format!(
                "could not create immutable report {}: {err}",
                path.display()
            )
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|err| format!("could not write report {}: {err}", path.display()))
}
