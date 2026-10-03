use super::*;

pub(super) fn write_report_new(path: &Path, report: &EquipmentGpuBatchReport) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(report)
        .map_err(|error| gpu_batch_error_value(format!("could not serialize report: {error}")))?;
    bytes.push(b'\n');
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| gpu_batch_error_value("report filename is not Unicode"))?;
    let temporary = path.with_file_name(format!(
        ".{file_name}.{}.ffone-gpu-batch.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| io_at(&temporary, error))?;
    file.write_all(&bytes)
        .map_err(|error| io_at(&temporary, error))?;
    file.sync_all().map_err(|error| io_at(&temporary, error))?;
    if let Err(error) = fs::hard_link(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(io_at(path, error));
    }
    fs::remove_file(&temporary).map_err(|error| io_at(&temporary, error))
}
