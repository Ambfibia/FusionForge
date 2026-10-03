use super::*;

pub(super) fn validate_publish_identity(
    publish: &Value,
    contract: &ModelPublishContract,
    mapping: &LogicalModelBatchMapping,
    selection: &str,
    publish_relative: &str,
    glb_bytes: &[u8],
) -> Result<()> {
    if contract.family != mapping.family
        || contract.legacy_name != mapping.logical_name
        || contract.root_node != mapping.logical_name
        || contract.semantic_directories != mapping.semantic_directories
        || contract.output_glb != selection
    {
        return invalid(format!(
            "batch mapping and publish contract identity differ for {selection:?}"
        ));
    }
    if publish.get("reportPath").and_then(Value::as_str) != Some(publish_relative) {
        return invalid(format!(
            "publish report path does not bind exact sidecar {publish_relative:?}"
        ));
    }
    contract.validate(glb_bytes).map_err(|error| {
        invalid_error(format!(
            "publish contract validation failed for {selection:?}: {error}"
        ))
    })
}

pub(super) fn validate_batch(batch: &LogicalModelBatchPublishReport) -> Result<()> {
    if batch.schema != LOGICAL_MODEL_BATCH_REPORT_SCHEMA
        || !batch.structural_audit_passed
        || batch.models.is_empty()
    {
        return invalid("candidate batch report is not a structurally accepted v4 mapping");
    }
    Ok(())
}

pub(super) fn validate_mapping(mapping: &LogicalModelBatchMapping) -> Result<()> {
    validate_relative(&mapping.output_glb)?;
    validate_relative(&mapping.source)?;
    validate_safe_component(&mapping.family, "source family")?;
    validate_safe_component(&mapping.logical_name, "logical name")?;
    require_sha256(&mapping.source_sha256, "batch source SHA-256")?;
    if !mapping.output_glb.ends_with(".glb") {
        return invalid(format!(
            "batch output is not a lowercase GLB: {:?}",
            mapping.output_glb
        ));
    }
    Ok(())
}

pub(super) fn validate_selections(models: &[String]) -> Result<Vec<String>> {
    let mut exact = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut selections = Vec::with_capacity(models.len());
    for model in models {
        validate_relative(model)?;
        if !model.starts_with("models/") || !model.ends_with(".glb") {
            return invalid(format!(
                "tutorial model selection must be models/.../*.glb: {model:?}"
            ));
        }
        if !exact.insert(model.clone()) {
            return invalid(format!("duplicate tutorial model selection {model:?}"));
        }
        if !folded.insert(casefold(model)) {
            return invalid(format!(
                "case-folded tutorial model selection collision at {model:?}"
            ));
        }
        selections.push(model.clone());
    }
    selections.sort();
    Ok(selections)
}

pub(super) fn validate_manifest(manifest: &ProjectAssetManifest) -> Result<()> {
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "asset manifest schema must be {PROJECT_ASSET_SCHEMA:?}"
        ));
    }
    let mut folded = BTreeSet::new();
    for entry in &manifest.files {
        validate_relative(&entry.path)?;
        if !folded.insert(casefold(&entry.path)) {
            return invalid(format!(
                "asset manifest has a case-folded path collision at {:?}",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_previous_install(asset_root: &Path, manifest: &ProjectAssetManifest) -> Result<bool> {
    let destination = asset_root.join(TUTORIAL_MODEL_ROOT);
    let owned_entries = manifest
        .files
        .iter()
        .filter(|entry| owned_path(&entry.path))
        .collect::<Vec<_>>();
    let metadata = match fs::symlink_metadata(&destination) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(io_at(&destination, error)),
    };
    let Some(metadata) = metadata else {
        if !owned_entries.is_empty() {
            return invalid(
                "manifest claims tutorial-model ownership but installer prefix is absent",
            );
        }
        return Ok(false);
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return invalid("existing tutorial-model prefix is not a regular directory");
    }
    if owned_entries.is_empty() {
        return invalid("existing tutorial-model prefix has no manifest ownership");
    }
    let disk = index_regular_tree(&destination)?;
    let disk_paths = disk
        .values()
        .map(|relative| format!("{TUTORIAL_MODEL_ROOT}/{relative}"))
        .collect::<BTreeSet<_>>();
    let manifest_paths = owned_entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    if disk_paths != manifest_paths {
        return invalid("existing tutorial-model disk and manifest file sets differ");
    }
    for entry in &owned_entries {
        if !entry
            .source_path
            .starts_with("native-tutorial-model-installer/")
        {
            return invalid(format!(
                "existing tutorial-model entry has foreign ownership: {:?}",
                entry.path
            ));
        }
        let bytes = read_regular_relative(asset_root, &entry.path)?;
        if bytes.len() as u64 != entry.bytes
            || blake3::hash(&bytes).to_hex().as_str() != entry.blake3
        {
            return invalid(format!(
                "existing tutorial-model file disagrees with manifest: {:?}",
                entry.path
            ));
        }
    }

    let catalog_path = asset_root.join(TUTORIAL_MODEL_CATALOG_PATH);
    match fs::symlink_metadata(&catalog_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return invalid("existing tutorial-model catalog is not a regular file");
            }
            let catalog_bytes = read_regular_relative(asset_root, TUTORIAL_MODEL_CATALOG_PATH)?;
            let catalog: TutorialModelCatalog =
                serde_json::from_slice(&catalog_bytes).map_err(|source| PipelineError::Json {
                    path: catalog_path.display().to_string(),
                    source,
                })?;
            if catalog.schema != TUTORIAL_MODEL_CATALOG_SCHEMA
                || catalog.installer != INSTALLER_ID
                || catalog.install_prefix != TUTORIAL_MODEL_ROOT
            {
                return invalid("existing tutorial-model prefix was not created by this installer");
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Early installer revisions wrote the exact ownership prefix and
            // manifest entries before the catalog became mandatory. Accept
            // that one migration shape only after the disk/manifest sets,
            // native-installer source ownership, byte lengths and BLAKE3
            // hashes above all agree. Any foreign or drifted tree still fails
            // closed before the transaction is staged.
        }
        Err(error) => return Err(io_at(&catalog_path, error)),
    }
    Ok(true)
}

pub(super) fn reject_overlapping_roots(
    left: &Path,
    right: &Path,
    left_label: &str,
    right_label: &str,
) -> Result<()> {
    if left.starts_with(right) || right.starts_with(left) {
        return invalid(format!(
            "{left_label} and {right_label} must be disjoint: {left:?}, {right:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || path.contains(':')
        || path.contains('?')
        || path.contains('#')
        || path.contains('%')
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid(format!("unsafe tutorial-model relative path {path:?}"));
    }
    for component in path.split('/') {
        validate_path_component(component)?;
    }
    Ok(())
}

pub(super) fn validate_uri(uri: &str) -> Result<()> {
    validate_relative(uri)?;
    if !uri.ends_with(".png") {
        return invalid(format!(
            "logical-model external URI is not an exact lowercase PNG: {uri:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_safe_component(component: &str, label: &str) -> Result<()> {
    validate_path_component(component)?;
    if !component
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return invalid(format!(
            "{label} must use only ASCII letters, digits, underscore or hyphen: {component:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_path_component(component: &str) -> Result<()> {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.ends_with(' ')
        || component.ends_with('.')
        || component.chars().any(char::is_control)
        || component
            .chars()
            .any(|value| matches!(value, '<' | '>' | '"' | '|' | '*' | '/' | '\\'))
        || component.nfkc().collect::<String>() != component
    {
        return invalid(format!(
            "unsafe tutorial-model path component {component:?}"
        ));
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        return invalid(format!(
            "reserved Windows tutorial-model path component {component:?}"
        ));
    }
    Ok(())
}

pub(super) fn validate_source_build(source_build: &str) -> Result<()> {
    if source_build.trim().is_empty() || source_build.chars().any(char::is_control) {
        return invalid("source build identity may not be empty or contain controls");
    }
    Ok(())
}

pub(super) fn require_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return invalid(format!("{label} is not a lowercase SHA-256 value"));
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::LogicalModelPublish(format!("tutorial-model install failed: {}", message.into()))
}
