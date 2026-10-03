use super::*;

pub(super) fn apply_plan(asset_root: &Path, mut plan: MigrationPlan) -> Result<AudioTaxonomyMigrationReport> {
    let stamp = transaction_stamp();
    let stage = asset_root.join(format!(".audio-taxonomy-stage-{stamp}"));
    let backup = asset_root.join(format!(".audio-taxonomy-backup-{stamp}"));
    if stage.exists() || backup.exists() {
        return Err(PipelineError::StagingCollision(stage));
    }
    fs::create_dir(&stage).map_err(|source| io_at(&stage, source))?;
    let staged_result = stage_plan(&stage, &plan);
    if let Err(error) = staged_result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    if let Err(error) = commit_plan(asset_root, &stage, &backup, &plan.files) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    plan.report.applied = true;
    Ok(plan.report)
}
