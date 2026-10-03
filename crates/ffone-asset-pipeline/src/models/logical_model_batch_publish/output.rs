use super::*;

pub(super) fn write_batch_report(root: &Path, report: &LogicalModelBatchPublishReport) -> Result<()> {
    let path = root.join(LOGICAL_MODEL_BATCH_REPORT_FILE);
    let mut bytes = serde_json::to_vec_pretty(report).map_err(|error| {
        batch_error_value(format!(
            "could not serialize logical-model batch report: {error}"
        ))
    })?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| io_at(&path, error))?;
    file.write_all(&bytes)
        .map_err(|error| io_at(&path, error))?;
    file.sync_all().map_err(|error| io_at(&path, error))
}
