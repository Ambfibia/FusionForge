use super::*;

pub(super) fn verify_semantic_roundtrip_hashes(
    publish: &Value,
    selection: &str,
) -> Result<BTreeMap<String, String>> {
    let source = publish
        .pointer("/semanticProof/source")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_error("semantic roundtrip proof has no source hashes"))?;
    let emitted = publish
        .pointer("/semanticProof/emitted")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_error("semantic roundtrip proof has no emitted hashes"))?;
    if source.is_empty() || source.len() != emitted.len() {
        return invalid(format!(
            "semantic roundtrip hash sets are incomplete for {selection:?}"
        ));
    }
    let mut hashes = BTreeMap::new();
    for (scope, value) in source {
        let hash = value.as_str().ok_or_else(|| {
            invalid_error(format!("semantic source hash {scope:?} is not a string"))
        })?;
        require_sha256(hash, "semantic source hash")?;
        if emitted.get(scope).and_then(Value::as_str) != Some(hash) {
            return invalid(format!(
                "semantic source/emitted hashes differ for {selection:?} scope {scope:?}"
            ));
        }
        hashes.insert(scope.clone(), hash.to_owned());
    }
    Ok(hashes)
}

pub(super) fn reported_external_files(
    publish: &Value,
    candidate_root: &Path,
    model_parent: &str,
) -> Result<BTreeMap<String, ExpectedExternalFile>> {
    let textures = publish
        .pointer("/materialPublish/textures")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_error("publish report has no typed texture list"))?;
    let mut files = BTreeMap::<String, ExpectedExternalFile>::new();
    let mut folds = BTreeMap::<String, String>::new();
    for texture in textures {
        let uri = required_string(texture, "uri")?;
        let bytes = required_u64(texture, "byteLength")?;
        let sha = required_string(texture, "sha256")?;
        insert_expected_external(&mut files, &mut folds, uri, bytes, sha)?;
        let mip_levels = texture
            .get("mipLevels")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_error("published texture has no mipLevels"))?;
        if texture
            .get("sourceMipCount")
            .and_then(Value::as_u64)
            .is_none_or(|count| count != mip_levels.len() as u64)
        {
            return invalid(format!(
                "published texture mip count disagrees for {model_parent:?}/{uri}"
            ));
        }
        for mip in mip_levels {
            insert_expected_external(
                &mut files,
                &mut folds,
                required_string(mip, "uri")?,
                required_u64(mip, "pngByteLength")?,
                required_string(mip, "pngSha256")?,
            )?;
        }
    }
    if files.is_empty() {
        return invalid("tutorial logical model has no external texture closure");
    }
    for expected in files.values() {
        let relative = format!("{model_parent}/{}", expected.path);
        let bytes = read_regular_relative(candidate_root, &relative)?;
        if bytes.len() as u64 != expected.bytes || sha256(&bytes) != expected.sha256 {
            return invalid(format!(
                "publish texture hash/length mismatch at {relative:?}"
            ));
        }
    }
    Ok(files)
}

pub(super) fn insert_expected_external(
    files: &mut BTreeMap<String, ExpectedExternalFile>,
    folds: &mut BTreeMap<String, String>,
    path: &str,
    bytes: u64,
    sha: &str,
) -> Result<()> {
    validate_uri(path)?;
    require_sha256(sha, "published external URI hash")?;
    if bytes == 0 {
        return invalid(format!("published external URI {path:?} is empty"));
    }
    let folded = casefold(path);
    if let Some(previous) = folds.get(&folded) {
        if previous != path {
            return invalid(format!(
                "published external URI has a case-fold collision: {previous:?} and {path:?}"
            ));
        }
    } else {
        folds.insert(folded, path.to_owned());
    }
    let value = ExpectedExternalFile {
        path: path.to_owned(),
        bytes,
        sha256: sha.to_owned(),
    };
    if let Some(previous) = files.get(path) {
        if previous.bytes != bytes || previous.sha256 != sha {
            return invalid(format!(
                "published external URI {path:?} has conflicting hash evidence"
            ));
        }
    } else {
        files.insert(path.to_owned(), value);
    }
    Ok(())
}

pub(super) fn external_uri_set(glb: &[u8]) -> Result<BTreeSet<String>> {
    let document = glb_document(glb)?;
    let mut uris = Vec::new();
    collect_uris(&document, &mut uris)?;
    let mut exact = BTreeSet::new();
    let mut folds = BTreeMap::<String, String>::new();
    for uri in uris {
        validate_uri(&uri)?;
        let folded = casefold(&uri);
        if let Some(previous) = folds.get(&folded) {
            if previous != &uri {
                return invalid(format!(
                    "GLB external URIs have a case-fold collision: {previous:?} and {uri:?}"
                ));
            }
        } else {
            folds.insert(folded, uri.clone());
        }
        exact.insert(uri);
    }
    if exact.is_empty() {
        return invalid("tutorial logical model GLB has no external URI closure");
    }
    Ok(exact)
}

pub(super) fn verify_evidence(
    proof: &EvidenceProof,
    selection: &str,
    glb: &[u8],
    facts: &LogicalModelGpuFacts,
) -> Result<TutorialModelGpuProof> {
    let evidence: LogicalModelGpuEvidence =
        serde_json::from_slice(&proof.json_bytes).map_err(|source| PipelineError::Json {
            path: proof.root.join(&proof.json_relative).display().to_string(),
            source,
        })?;
    if evidence.schema != GPU_EVIDENCE_SCHEMA
        || evidence.status != AutomatedGpuStatus::Passed
        || evidence.render_profile != GPU_RENDER_PROFILE
        || evidence.visual_parity != VisualParityClaim::NotAsserted
        || evidence.model.relative_glb != selection
        || evidence.model.true_name != facts.true_name
        || evidence.model.glb_byte_length != glb.len() as u64
        || evidence.model.glb_sha256 != sha256(glb)
    {
        return invalid(format!(
            "strict GPU evidence identity failed for {selection:?}"
        ));
    }
    verify_animation_evidence(&evidence, facts, selection)?;
    verify_runtime_evidence(&evidence, facts, selection)?;
    if evidence.screenshot.relative_png != proof.png_relative
        || evidence.screenshot.byte_length != proof.png_bytes.len() as u64
        || evidence.screenshot.sha256 != sha256(&proof.png_bytes)
    {
        return invalid(format!(
            "GPU screenshot hash/length/path failed for {selection:?}"
        ));
    }
    let image = image::load_from_memory_with_format(&proof.png_bytes, image::ImageFormat::Png)
        .map_err(|error| {
            invalid_error(format!(
                "GPU screenshot for {selection:?} is not a valid PNG: {error}"
            ))
        })?
        .to_rgb8();
    if image.width() != evidence.screenshot.width || image.height() != evidence.screenshot.height {
        return invalid(format!(
            "GPU screenshot dimensions changed for {selection:?}"
        ));
    }
    let pixels = u64::from(image.width()) * u64::from(image.height());
    let minimum = (pixels / 2_000).max(128).min(pixels);
    let foreground = screenshot_foreground(image.as_raw(), image.width(), image.height())
        .ok_or_else(|| invalid_error("GPU screenshot foreground could not be decoded"))?;
    if foreground != evidence.screenshot.foreground_pixels
        || foreground < minimum
        || foreground > pixels
    {
        return invalid(format!(
            "GPU screenshot foreground evidence failed for {selection:?}"
        ));
    }
    Ok(TutorialModelGpuProof {
        evidence_root: slash_path(&proof.root),
        evidence_json: proof.json_relative.clone(),
        evidence_json_sha256: sha256(&proof.json_bytes),
        screenshot_png: proof.png_relative.clone(),
        screenshot_sha256: evidence.screenshot.sha256,
        render_profile: evidence.render_profile,
        visual_parity: "not-asserted".to_owned(),
    })
}

pub(super) fn prepare_evidence_roots(
    paths: &[PathBuf],
    candidate_root: &Path,
    asset_root: &Path,
) -> Result<Vec<EvidenceRoot>> {
    let mut roots = Vec::with_capacity(paths.len());
    let mut folded = BTreeSet::new();
    for path in paths {
        let canonical = canonical_plain_directory(path, "GPU evidence root")?;
        reject_overlapping_roots(
            &canonical,
            candidate_root,
            "GPU evidence root",
            "candidate root",
        )?;
        reject_overlapping_roots(&canonical, asset_root, "GPU evidence root", "asset root")?;
        let key = casefold(&slash_path(&canonical));
        if !folded.insert(key) {
            return invalid(format!(
                "duplicate case-folded GPU evidence root {}",
                canonical.display()
            ));
        }
        roots.push(EvidenceRoot {
            files: index_regular_tree(&canonical)?,
            canonical,
        });
    }
    for left in 0..roots.len() {
        for right in left + 1..roots.len() {
            reject_overlapping_roots(
                &roots[left].canonical,
                &roots[right].canonical,
                "GPU evidence root",
                "GPU evidence root",
            )?;
        }
    }
    roots.sort_by(|left, right| left.canonical.cmp(&right.canonical));
    Ok(roots)
}

pub(super) fn rollback_empty_parent(parent: &Path, existed: bool) {
    if !existed {
        let _ = fs::remove_dir(parent);
    }
}

pub(super) fn ensure_no_stale_transaction(asset_root: &Path) -> Result<()> {
    for path in [
        asset_root.join(MANIFEST_NEXT),
        asset_root.join(MANIFEST_BACKUP),
        asset_root.join(INSTALL_BACKUP),
    ] {
        if fs::symlink_metadata(&path).is_ok() {
            return invalid(format!(
                "stale tutorial-model transaction artifact exists: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn create_stage(asset_root: &Path) -> Result<PathBuf> {
    for _ in 0..128 {
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let stage = asset_root.join(format!(
            ".tutorial-models-install-stage-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&stage) {
            Ok(()) => return Ok(stage),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_at(&stage, error)),
        }
    }
    Err(PipelineError::StagingCollision(
        asset_root.join(TUTORIAL_MODEL_ROOT),
    ))
}

pub(super) fn canonical_plain_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return invalid(format!("{label} is not a regular directory: {path:?}"));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

pub(super) fn kind_for_external(path: &str) -> Result<ProjectAssetKind> {
    if path.ends_with(".png") {
        Ok(ProjectAssetKind::Texture)
    } else {
        invalid(format!(
            "unsupported tutorial logical-model external URI {path:?}"
        ))
    }
}

pub(super) fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_error(format!("required string field {field:?} is missing")))
}

pub(super) fn required_u64(value: &Value, field: &str) -> Result<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_error(format!("required integer field {field:?} is missing")))
}

pub(super) fn file_name(path: &str) -> Result<&str> {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error(format!("path has no UTF-8 filename: {path:?}")))
}

pub(super) fn parent_slash(path: &str) -> Result<String> {
    Path::new(path)
        .parent()
        .map(slash_path)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_error(format!("path has no parent: {path:?}")))
}

pub(super) fn relative_slash(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid_error("filesystem path escaped its root"))?;
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return invalid("filesystem path has a non-normal component");
    }
    Ok(slash_path(relative))
}

pub(super) fn casefold(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn screenshot_foreground(rgb: &[u8], width: u32, height: u32) -> Option<u64> {
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    if width < 2 || height < 2 || rgb.len() != usize::try_from(pixels).ok()?.checked_mul(3)? {
        return None;
    }
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let corners = [
        0,
        (width - 1) * 3,
        (height - 1) * width * 3,
        (height * width - 1) * 3,
    ];
    let mut background = [0_u32; 3];
    for offset in corners {
        for channel in 0..3 {
            background[channel] += u32::from(rgb[offset + channel]);
        }
    }
    let background = background.map(|value| (value / 4) as i16);
    Some(
        rgb.chunks_exact(3)
            .filter(|pixel| {
                (0..3).any(|channel| (i16::from(pixel[channel]) - background[channel]).abs() > 8)
            })
            .count() as u64,
    )
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
