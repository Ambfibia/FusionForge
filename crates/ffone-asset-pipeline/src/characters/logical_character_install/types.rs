use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticCharacterProofs {
    pub batch_report_source: String,
    pub batch_report_sha256: String,
    pub structural_audit_schema: String,
    pub structural_audit_passed: bool,
    pub structural_violation_count: u64,
    pub structural_upgrade_report: String,
    pub gpu_audit_source: String,
    pub gpu_audit_sha256: String,
    pub gpu_audit_schema: String,
    pub automated_gpu_passed: bool,
    pub archived_gpu_evidence_verified_at_rest: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAuthoredRoot {
    pub translation: [f64; 3],
    pub rotation_xyzw: [f64; 4],
    pub scale: [f64; 3],
    pub unit_scale: f64,
    pub auto_centered: bool,
    pub auto_scaled: bool,
}

impl Eq for SemanticAuthoredRoot {}
