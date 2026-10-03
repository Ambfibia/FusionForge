use super::*;

pub(super) fn validate_options(options: &EquipmentGpuBatchOptions) -> Result<()> {
    if options.max_frames == 0
        || !options.timeout_seconds.is_finite()
        || options.timeout_seconds <= 0.0
    {
        return gpu_batch_error("frames and timeout must be finite positive values");
    }
    if options.mode == EquipmentGpuBatchMode::Smoke && options.shard.is_some() {
        return gpu_batch_error("deterministic sharding is supported only for full mode");
    }
    Ok(())
}

pub(super) fn reject_overlapping_paths(candidate: &Path, evidence: &Path, report: &Path) -> Result<()> {
    if candidate.starts_with(evidence)
        || evidence.starts_with(candidate)
        || report.starts_with(candidate)
        || report.starts_with(evidence)
    {
        return gpu_batch_error(
            "candidate, evidence root and sibling report must be mutually non-overlapping",
        );
    }
    Ok(())
}

pub(super) fn gpu_batch_error<T>(message: impl Into<String>) -> Result<T> {
    Err(gpu_batch_error_value(message))
}

pub(super) fn gpu_batch_error_value(message: impl Into<String>) -> PipelineError {
    PipelineError::ModelAudit(format!("equipment GPU batch: {}", message.into()))
}
