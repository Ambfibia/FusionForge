use super::*;

pub const REQUEST_SCHEMA: &str = "fusionforge.ui-interaction-analysis-request.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiInteractionAnalysisRequest {
    pub schema: String,
    pub schema_version: u32,
    pub managed: ManagedAssemblyInput,
    pub roots: Vec<ManagedMethodSelector>,
    #[serde(default)]
    pub traversal: TraversalLimits,
}

impl UiInteractionAnalysisRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != REQUEST_SCHEMA || self.schema_version != 1 {
            return Err(format!(
                "request must be {REQUEST_SCHEMA} with schemaVersion 1"
            ));
        }
        if self.roots.is_empty() {
            return Err("at least one exact managed root method is required".to_string());
        }
        if self.traversal.max_direct_call_depth > 8 {
            return Err("maxDirectCallDepth may not exceed 8".to_string());
        }
        if self.traversal.max_methods == 0 || self.traversal.max_methods > 4096 {
            return Err("maxMethods must be between 1 and 4096".to_string());
        }
        for root in &self.roots {
            if root.declaring_type.is_empty() || root.name.is_empty() {
                return Err("root declaringType and name must be non-empty".to_string());
            }
        }
        Ok(())
    }
}

pub(crate) fn analyze_ui_interactions_with_request_bytes(
    request: &UiInteractionAnalysisRequest,
    request_bytes: &[u8],
) -> Result<UiInteractionAnalysis, String> {
    analyze_ui_interactions_bound(request, request_bytes, "exact-request-json-bytes-v1")
}
