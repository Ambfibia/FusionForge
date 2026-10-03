use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentSourceExport {
    pub(super) category: String,
    pub(super) true_name: String,
    pub(super) safe_true_name: String,
    #[serde(default)]
    pub(super) route_qualifier: Option<String>,
    pub(super) source_relative_path: String,
    pub(super) source_byte_length: u64,
    pub(super) source_sha256: String,
    pub(super) canonical_route: String,
    pub(super) normalized_route: String,
    #[serde(default)]
    pub(super) proven_alias_routes: Vec<String>,
    #[serde(default)]
    pub(super) table_rows: Vec<Value>,
    pub(super) owner: Value,
    pub(super) physical_target: Value,
    pub(super) facts: Value,
}

pub(super) fn write_report(root: &Path, report: &EquipmentLogicalModelBatchReport) -> Result<()> {
    let path = root.join(EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE);
    let bytes = report_bytes(report)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| io_at(&path, error))?;
    file.write_all(&bytes)
        .map_err(|error| io_at(&path, error))?;
    file.sync_all().map_err(|error| io_at(&path, error))
}
