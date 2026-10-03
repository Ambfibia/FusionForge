use super::*;

pub(super) fn validate_registry(registry: &SemanticCharacterRegistry) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut glbs = BTreeSet::new();
    for model in &registry.models {
        validate_relative(&model.glb)?;
        if !ids.insert(model.id.to_ascii_lowercase()) {
            return invalid(format!("duplicate character registry id {:?}", model.id));
        }
        if !glbs.insert(model.glb.to_ascii_lowercase()) {
            return invalid(format!("duplicate character registry GLB {:?}", model.glb));
        }
    }
    Ok(())
}

pub(super) fn validate_registry_manifest_identity(
    manifest: &BTreeMap<String, ProjectAssetFile>,
    bytes: &[u8],
) -> Result<()> {
    let entry = manifest
        .get(SEMANTIC_CHARACTER_REGISTRY_PATH)
        .ok_or_else(|| invalid_error("manifest has no semantic character registry"))?;
    if entry.kind != ProjectAssetKind::Data
        || entry.bytes != bytes.len() as u64
        || entry.blake3 != blake3::hash(bytes).to_hex().to_string()
    {
        return invalid("semantic character registry manifest identity changed");
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
        "tutorial-character promotion failed: {}",
        message.into()
    ))
}
