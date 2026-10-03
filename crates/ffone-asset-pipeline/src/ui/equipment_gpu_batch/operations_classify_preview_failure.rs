use super::*;

pub(super) fn classify_preview_failure(stdout: &[u8]) -> ClassifiedPreviewFailure {
    let text = String::from_utf8_lossy(stdout);
    let runtime_error = text
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|value| {
            value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let Some(error) = runtime_error.clone() else {
        return ClassifiedPreviewFailure {
            code: "standaloneGpuPreviewProcessFailed".to_owned(),
            detail: "Bevy standalone GPU preview exited without a typed runtime error".to_owned(),
            runtime_error: None,
            typed_evidence: json!({ "reason": "noTypedRuntimeError" }),
        };
    };
    if error.contains("unsupported exact legacy shader name:") {
        let shader_names = error
            .split("unsupported exact legacy shader name:")
            .skip(1)
            .filter_map(|suffix| suffix.split('|').next())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        return ClassifiedPreviewFailure {
            code: "runtimeLegacyShaderUnsupported".to_owned(),
            detail: error.clone(),
            runtime_error: Some(error),
            typed_evidence: json!({
                "unsupportedExactLegacyShaderNames": shader_names,
                "gateApplicable": true,
                "fallbackApplied": false,
            }),
        };
    }
    if error.contains("clear-only GPU captures")
        || error.contains("GPU screenshot still pending")
        || error.contains("only the clear background")
        || error.contains("only the source outline pass")
    {
        return ClassifiedPreviewFailure {
            code: "gpuVisibleSurfaceEvidenceMissing".to_owned(),
            detail: error.clone(),
            runtime_error: Some(error),
            typed_evidence: json!({
                "gateApplicable": true,
                "visibleSurfaceProven": false,
                "fallbackApplied": false,
            }),
        };
    }
    if error.contains("material metadata error") || error.contains("material") {
        return ClassifiedPreviewFailure {
            code: "runtimeLegacyMaterialRejected".to_owned(),
            detail: error.clone(),
            runtime_error: Some(error),
            typed_evidence: json!({
                "gateApplicable": true,
                "fallbackApplied": false,
            }),
        };
    }
    if error.contains("shader") || error.contains("render error") {
        return ClassifiedPreviewFailure {
            code: "gpuShaderOrRenderError".to_owned(),
            detail: error.clone(),
            runtime_error: Some(error),
            typed_evidence: json!({
                "gateApplicable": true,
                "fallbackApplied": false,
            }),
        };
    }
    if error.contains("frame limit") || error.contains("wall-clock timeout") {
        return ClassifiedPreviewFailure {
            code: "gpuAcceptanceTimeout".to_owned(),
            detail: error.clone(),
            runtime_error: Some(error),
            typed_evidence: json!({
                "gateApplicable": true,
                "fallbackApplied": false,
            }),
        };
    }
    ClassifiedPreviewFailure {
        code: "standaloneGpuRuntimeGateFailed".to_owned(),
        detail: error.clone(),
        runtime_error: Some(error),
        typed_evidence: json!({
            "gateApplicable": true,
            "fallbackApplied": false,
        }),
    }
}

pub(super) fn elapsed_milliseconds(started: Instant) -> Result<u64> {
    started
        .elapsed()
        .as_millis()
        .try_into()
        .map_err(|_| gpu_batch_error_value("elapsed milliseconds exceed u64"))
}

pub(super) fn u64_count(value: usize, label: &str) -> Result<u64> {
    value
        .try_into()
        .map_err(|_| gpu_batch_error_value(format!("{label} exceeds u64")))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
