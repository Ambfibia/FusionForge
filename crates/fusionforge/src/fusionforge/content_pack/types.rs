use super::*;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct LauncherFileInfo {
    pub(super) hash: String,
    pub(super) size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CookReport {
    pub(super) schema: &'static str,
    pub(super) profile: &'static str,
    pub(super) complete: bool,
    pub(super) native_only: bool,
    pub(super) build_uuid: String,
    pub(super) locale: String,
    pub(super) inventory: Vec<InventoryReportEntry>,
    pub(super) bundles: Vec<BundleCookReport>,
    pub(super) overlay_errors: Vec<String>,
    pub(super) overlay_warnings: Vec<String>,
    pub(super) mappings: Vec<CookMapping>,
    pub(super) coverage: CoverageCounters,
    pub(super) counts: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CoverageCounters {
    pub(super) bundle_files_scanned: u64,
    pub(super) bundle_failures: u64,
    pub(super) bundle_objects_scanned: u64,
    pub(super) bundle_supported_objects: u64,
    pub(super) bundle_emitted: u64,
    pub(super) bundle_skipped: u64,
    pub(super) overlay_entries_scanned: u64,
    pub(super) overlay_emitted: u64,
    pub(super) overlay_skipped: u64,
    pub(super) localization_fallbacks: u64,
    pub(super) audio_size_mismatches: u64,
    pub(super) errors: u64,
    pub(super) bundle_types: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CookMapping {
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) native_key: String,
    pub(super) native_path: String,
    pub(super) source: CookSourceDescriptor,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CookSourceDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bundle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) path_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) asset: Option<String>,
}
