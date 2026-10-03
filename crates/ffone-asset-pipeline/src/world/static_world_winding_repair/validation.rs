use super::*;

pub(super) fn validate_winding_revision(
    revision_root: &Path,
    plan_blake3: &str,
    metadata: &ArchivedRuntimeMetadata,
) -> Result<()> {
    if plan_blake3.len() != 64 || !plan_blake3.bytes().all(|value| value.is_ascii_hexdigit()) {
        return invalid("conversion-metadata revision name is not a BLAKE3 hash");
    }
    let revision_metadata =
        fs::symlink_metadata(revision_root).map_err(|error| io_at(revision_root, error))?;
    if revision_metadata.file_type().is_symlink() || !revision_metadata.is_dir() {
        return invalid("conversion-metadata winding revision is invalid");
    }
    let files = vec![metadata.clone()];
    let identity = RevisionPlanIdentity {
        schema: REVISION_PLAN_SCHEMA,
        source_build: TUTORIAL_STATIC_WORLD_SOURCE_BUILD,
        files: &files,
    };
    let identity_bytes = serde_json::to_vec(&identity).map_err(generated_json_error)?;
    if hash_bytes(&identity_bytes) != plan_blake3 {
        return invalid("conversion-metadata winding revision plan hash mismatch");
    }
    let index = read_json::<RevisionIndex>(&revision_root.join("index.json"), "revision index")?;
    let report =
        read_json::<RevisionReport>(&revision_root.join("report.json"), "revision report")?;
    if index.schema != REVISION_INDEX_SCHEMA
        || report.schema != REVISION_REPORT_SCHEMA
        || index.source_build != TUTORIAL_STATIC_WORLD_SOURCE_BUILD
        || report.source_build != TUTORIAL_STATIC_WORLD_SOURCE_BUILD
        || index.plan_blake3 != plan_blake3
        || report.plan_blake3 != plan_blake3
        || index.files != files
        || report.files != files
        || report.archived_files != 1
        || report.archived_bytes != metadata.bytes
    {
        return invalid("conversion-metadata winding revision index/report mismatch");
    }
    let archived = read_regular_file(
        &revision_root.join(&metadata.archive_path),
        "archived repaired tutorial ownership",
    )?;
    if archived.len() as u64 != metadata.bytes || hash_bytes(&archived) != metadata.blake3 {
        return invalid("conversion-metadata winding revision payload mismatch");
    }
    Ok(())
}

pub(super) fn require_repair_improves(path: &str, before: GeometryStats, after: GeometryStats) -> Result<()> {
    if before.normal_bearing
        && (after.aligned <= after.opposed
            || after.aligned != before.opposed
            || after.opposed != before.aligned)
    {
        return invalid(format!(
            "{path:?} winding repair did not exchange aligned/opposed triangle evidence: before={before:?}, after={after:?}"
        ));
    }
    if before.triangles != after.triangles
        || before.index_accessors != after.index_accessors
        || before.degenerate != after.degenerate
    {
        return invalid(format!(
            "{path:?} winding repair changed geometry cardinality"
        ));
    }
    Ok(())
}

pub(super) fn generated_json_error(source: serde_json::Error) -> PipelineError {
    PipelineError::Json {
        path: "generated static-world winding repair JSON".to_owned(),
        source,
    }
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(format!("static-world winding repair: {}", message.into()))
}
