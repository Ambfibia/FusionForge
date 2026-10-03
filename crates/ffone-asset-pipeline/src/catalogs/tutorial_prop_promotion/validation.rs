use super::*;

pub(super) fn reject_stale_staging(project_root: &Path) -> Result<()> {
    let entries = fs::read_dir(project_root).map_err(|source| io_at(project_root, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| io_at(project_root, source))?;
        let name = entry.file_name();
        if name
            .to_string_lossy()
            .starts_with(".tutorial-prop-promotion-")
        {
            return invalid(format!(
                "stale tutorial-prop recovery staging must be inspected before retrying: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_plan_unchanged(plan: &PromotionPlan) -> Result<()> {
    if read_regular(&plan.manifest_path)? != plan.manifest_before {
        return invalid("project manifest changed after prop-promotion planning");
    }
    for promotion in &plan.promotions {
        if read_regular(&promotion.runtime_source_path)? != promotion.runtime_before {
            return invalid(format!(
                "runtime source changed after planning: {}",
                promotion.runtime_source_path.display()
            ));
        }
        let source = plan.asset_root.join(native_path(&promotion.source_root));
        if collect_tree(&source, false)? != promotion.source_tree {
            return invalid(format!(
                "source package changed after planning for {:?}",
                promotion.candidate.id
            ));
        }
        let target = plan.asset_root.join(native_path(&promotion.target_root));
        if regular_directory_exists(&target)? {
            return invalid(format!(
                "target appeared after planning for {:?}",
                promotion.candidate.id
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_committed_plan(plan: &PromotionPlan) -> Result<()> {
    if read_regular(&plan.manifest_path)? != plan.manifest_after {
        return invalid("committed project manifest does not match the staged identity");
    }
    for promotion in &plan.promotions {
        if read_regular(&promotion.runtime_source_path)? != promotion.runtime_after {
            return invalid(format!(
                "committed runtime source differs for {:?}",
                promotion.candidate.id
            ));
        }
        let source = plan.asset_root.join(native_path(&promotion.source_root));
        if regular_directory_exists(&source)? {
            return invalid(format!(
                "legacy source remains after commit for {:?}",
                promotion.candidate.id
            ));
        }
        let target = plan.asset_root.join(native_path(&promotion.target_root));
        if collect_tree(&target, false)? != promotion.source_tree {
            return invalid(format!(
                "committed target identity differs for {:?}",
                promotion.candidate.id
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_tree_manifest(
    root: &str,
    tree: &BTreeMap<String, FileIdentity>,
    manifest: &ProjectAssetManifest,
    index: &BTreeMap<String, ProjectAssetFile>,
) -> Result<()> {
    let manifest_paths = manifest_paths_below(manifest, root);
    let disk_paths = tree
        .keys()
        .map(|relative| format!("{root}/{relative}"))
        .collect::<BTreeSet<_>>();
    if manifest_paths != disk_paths {
        return invalid(format!("disk/manifest closure differs for {root:?}"));
    }
    for (relative, identity) in tree {
        let path = format!("{root}/{relative}");
        let entry = &index[&path];
        let expected_kind = match Path::new(relative)
            .extension()
            .and_then(|value| value.to_str())
        {
            Some("glb") => ProjectAssetKind::Model,
            Some("png") => ProjectAssetKind::Texture,
            _ => return invalid(format!("unsupported promoted package file {path:?}")),
        };
        if entry.kind != expected_kind
            || entry.bytes != identity.bytes
            || entry.blake3 != identity.blake3
        {
            return invalid(format!("manifest identity differs for {path:?}"));
        }
    }
    Ok(())
}

pub(super) fn reject_overlaps(asset: &Path, candidate: &Path, source: &Path, evidence: &Path) -> Result<()> {
    let roots = [asset, candidate, source, evidence];
    for (index, left) in roots.iter().enumerate() {
        for right in roots.iter().skip(index + 1) {
            if left.starts_with(right) || right.starts_with(left) {
                return invalid(format!(
                    "promotion input roots must not overlap: {} and {}",
                    left.display(),
                    right.display()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe relative path {path:?}"));
    }
    Ok(())
}

pub(super) fn require_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(format!("{label} is not a SHA-256 digest"));
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(format!(
        "tutorial-prop promotion failed: {}",
        message.into()
    ))
}
