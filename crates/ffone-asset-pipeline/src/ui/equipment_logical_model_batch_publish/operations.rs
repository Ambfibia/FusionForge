use super::*;

pub(super) fn preflight_batch(source_root: &Path) -> Result<PreflightResult> {
    let metadata = fs::symlink_metadata(source_root).map_err(|error| io_at(source_root, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return equipment_error("equipment source root must be a real directory");
    }
    let source_manifest_path = sibling_manifest_path(source_root)?;
    let manifest_metadata = fs::symlink_metadata(&source_manifest_path)
        .map_err(|error| io_at(&source_manifest_path, error))?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return equipment_error("equipment source manifest must be a real regular file");
    }
    let source_manifest_bytes =
        fs::read(&source_manifest_path).map_err(|error| io_at(&source_manifest_path, error))?;
    let source_manifest: EquipmentSourceManifest = serde_json::from_slice(&source_manifest_bytes)
        .map_err(|source| PipelineError::Json {
        path: source_manifest_path.display().to_string(),
        source,
    })?;
    validate_source_manifest(&source_manifest)?;

    let source_files = walk_source_files(source_root)?;
    let mut manifest_exports = BTreeMap::new();
    for export in &source_manifest.exported {
        let key = portable_key(&export.source_relative_path);
        if manifest_exports.insert(key, export.clone()).is_some() {
            return equipment_error("source manifest has a portable source path collision");
        }
    }
    let discovered = source_files
        .iter()
        .map(|(relative, _)| portable_key(relative))
        .collect::<BTreeSet<_>>();
    let declared = manifest_exports.keys().cloned().collect::<BTreeSet<_>>();
    if discovered != declared {
        return equipment_error(format!(
            "source tree differs from source manifest: discovered={}, declared={}",
            discovered.len(),
            declared.len()
        ));
    }

    let mut plans = Vec::with_capacity(source_files.len());
    for (relative, source) in source_files {
        let export = manifest_exports
            .remove(&portable_key(&relative))
            .ok_or_else(|| equipment_error_value("internal source manifest lookup failed"))?;
        if relative != export.source_relative_path {
            return equipment_error(format!(
                "source path spelling differs from manifest: {relative:?} vs {:?}",
                export.source_relative_path
            ));
        }
        plans.push(preflight_source(source, export)?);
    }
    plans.sort_by(|left, right| {
        left.export
            .source_relative_path
            .cmp(&right.export.source_relative_path)
    });
    let collision_blockers = global_collision_blockers(&plans)?;
    let upstream_blockers = source_manifest
        .blockers
        .iter()
        .map(normalize_upstream_blocker)
        .collect();

    Ok(PreflightResult {
        source_manifest_path,
        source_manifest_bytes,
        source_manifest,
        plans,
        collision_blockers,
        upstream_blockers,
    })
}

pub(super) fn preflight_source(source: PathBuf, export: EquipmentSourceExport) -> Result<EquipmentPlan> {
    if !ALLOWED_CATEGORIES.contains(&export.category.as_str())
        || export.safe_true_name != export.true_name
        || export.normalized_route != export.canonical_route.to_ascii_lowercase()
        || !export.normalized_route.starts_with("wear/")
        || !export.normalized_route.ends_with(".nif")
        || !export.proven_alias_routes.is_empty()
        || export.table_rows.is_empty()
        || export.owner.is_null()
        || export.physical_target.is_null()
        || export.facts.is_null()
    {
        return equipment_error(format!(
            "source manifest export {:?} violates exact equipment identity",
            export.source_relative_path
        ));
    }
    validate_true_name(&export.true_name)?;
    let expected_route_qualifier = route_stem_qualifier(&export.canonical_route)?;
    if export
        .route_qualifier
        .as_deref()
        .is_some_and(|qualifier| qualifier != expected_route_qualifier)
    {
        return equipment_error(format!(
            "equipment source route qualifier differs from exact XDT route stem: {:?}",
            export.source_relative_path
        ));
    }
    let expected = if let Some(route_qualifier) = &export.route_qualifier {
        format!(
            "characters/player/equipment/{0}/{1}/{2}/{2}.source.json",
            export.category, route_qualifier, export.true_name
        )
    } else {
        format!(
            "characters/player/equipment/{0}/{1}/{1}.source.json",
            export.category, export.true_name
        )
    };
    if export.source_relative_path != expected {
        return equipment_error(format!(
            "equipment source taxonomy mismatch: expected {expected:?}, got {:?}",
            export.source_relative_path
        ));
    }

    let bytes = fs::read(&source).map_err(|error| io_at(&source, error))?;
    if u64_count(bytes.len(), "equipment source byte length")? != export.source_byte_length
        || sha256_hex(&bytes) != export.source_sha256
    {
        return equipment_error(format!(
            "equipment source length/SHA differs from manifest: {:?}",
            source
        ));
    }
    let document: SourcePreflightDocument =
        serde_json::from_slice(&bytes).map_err(|source_error| PipelineError::Json {
            path: source.display().to_string(),
            source: source_error,
        })?;
    if document.schema != LOGICAL_MODEL_SOURCE_SCHEMA
        || document.logical_name != export.true_name
        || document.model_hierarchy.roots.len() != 1
    {
        return equipment_error(format!(
            "equipment source root identity mismatch: {:?}",
            source
        ));
    }
    let parentless = document
        .model_hierarchy
        .nodes
        .iter()
        .filter(|node| node.parent.is_none())
        .collect::<Vec<_>>();
    let root = &document.model_hierarchy.roots[0];
    if parentless.len() != 1
        || root.name != export.true_name
        || root.path != export.true_name
        || parentless[0].name != export.true_name
        || parentless[0].path != export.true_name
    {
        return equipment_error(format!(
            "equipment source does not have one true-name hierarchy root: {:?}",
            source
        ));
    }

    let glb_name = minimal_windows_glb_filename(&export.true_name)
        .map_err(|error| equipment_error_value(error.to_string()))?;
    if glb_name != format!("{}.glb", export.true_name) {
        return equipment_error("true equipment name required filename transformation");
    }
    let output_glb = if let Some(route_qualifier) = &export.route_qualifier {
        PathBuf::from(format!(
            "characters/player/equipment/{0}/{1}/{2}/{2}.glb",
            export.category, route_qualifier, export.true_name
        ))
    } else {
        PathBuf::from(format!(
            "characters/player/equipment/{0}/{1}/{1}.glb",
            export.category, export.true_name
        ))
    };
    let output_parent = output_glb
        .parent()
        .ok_or_else(|| equipment_error_value("equipment GLB has no output parent"))?;
    let mut output_files = vec![
        output_glb.clone(),
        output_glb.with_file_name(format!("{}.publish.json", export.true_name)),
    ];
    let texture_directory = format!("{}.textures", export.true_name);
    let mut preferred_name_counts = BTreeMap::<String, usize>::new();
    for texture in document.textures.values() {
        let preferred = minimal_windows_png_filename(&texture.name)
            .map_err(|error| equipment_error_value(error.to_string()))?;
        *preferred_name_counts
            .entry(preferred.to_ascii_lowercase())
            .or_default() += 1;
    }
    for (texture_id, texture) in document.textures {
        if texture.mip_levels.is_empty() {
            return equipment_error(format!(
                "texture {texture_id:?} in {:?} has no exact mip levels",
                export.source_relative_path
            ));
        }
        let preferred_png_name = minimal_windows_png_filename(&texture.name)
            .map_err(|error| equipment_error_value(error.to_string()))?;
        let png_name = if preferred_name_counts
            .get(&preferred_png_name.to_ascii_lowercase())
            .copied()
            .unwrap_or_default()
            > 1
        {
            windows_png_filename_preserving_legacy_extension(&texture.name)
                .map_err(|error| equipment_error_value(error.to_string()))?
        } else {
            preferred_png_name
        };
        let stem = png_name
            .strip_suffix(".png")
            .ok_or_else(|| equipment_error_value("texture filename has no .png suffix"))?;
        for level in 0..texture.mip_levels.len() {
            let relative = if level == 0 {
                PathBuf::from(&texture_directory).join(&png_name)
            } else {
                PathBuf::from(&texture_directory)
                    .join(format!("{stem}.mips"))
                    .join(format!("mip-{level:02}.png"))
            };
            output_files.push(output_parent.join(relative));
        }
    }
    output_files.sort();
    Ok(EquipmentPlan {
        export,
        source,
        output_glb,
        output_files,
    })
}

pub(super) fn register_output_claims(
    claims: &mut Vec<(String, OutputClaim)>,
    owner: Option<usize>,
    path: &Path,
) -> Result<()> {
    let components = clean_relative_components(path)?;
    let mut parent = PathBuf::new();
    for component in &components[..components.len() - 1] {
        parent.push(component);
        let exact = slash_path(&parent);
        claims.push((
            portable_path_key(&parent)?,
            OutputClaim {
                owner,
                exact,
                kind: "directory",
            },
        ));
    }
    claims.push((
        portable_path_key(path)?,
        OutputClaim {
            owner,
            exact: slash_path(path),
            kind: "file",
        },
    ));
    Ok(())
}

pub(super) fn normalize_upstream_blocker(blocker: &EquipmentSourceBlocker) -> EquipmentLogicalModelBlocker {
    let (code, typed_evidence) = if blocker.code == "equipmentRouteMissingFromBundleIndex" {
        ("sourceExactRouteMissing", Value::Null)
    } else if blocker.code == "distinctPhysicalEquipmentTrueNameCollision" {
        ("sourceTrueNamePhysicalCollision", Value::Null)
    } else if let Some(format) = texture_format(&blocker.detail) {
        (
            "sourceTextureFormatUnsupported",
            json!({ "textureFormat": format }),
        )
    } else if blocker.detail.contains("MovieTexture") {
        ("sourceMovieTextureUnsupported", Value::Null)
    } else if blocker.detail.contains("AnimationClip") {
        ("sourceAnimationClosureUnresolved", Value::Null)
    } else {
        ("sourceExactClosureUnresolved", Value::Null)
    };
    EquipmentLogicalModelBlocker {
        stage: "upstream-exact-source".to_owned(),
        code: code.to_owned(),
        upstream_code: Some(blocker.code.clone()),
        category: blocker.category.clone(),
        exact_route: blocker.exact_route.clone(),
        true_name: blocker
            .evidence
            .get("portableDestination")
            .and_then(Value::as_str)
            .and_then(|path| Path::new(path).parent())
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(str::to_owned),
        source: None,
        source_sha256: None,
        detail: blocker.detail.clone(),
        evidence: json!({
            "typed": typed_evidence,
            "upstreamEvidence": blocker.evidence,
            "tableRows": blocker.table_rows,
            "owners": blocker.owners,
            "normalizedRoute": blocker.normalized_route,
            "fallbackApplied": false,
        }),
        required_evidence: blocker.required_evidence.clone(),
        disposition: blocker.disposition.clone(),
    }
}

pub(super) fn native_preparation_blocker(
    plan: &EquipmentPlan,
    detail: String,
) -> EquipmentLogicalModelBlocker {
    EquipmentLogicalModelBlocker {
        stage: "native-glb-preparation".to_owned(),
        code: "nativeLogicalModelPreparationFailed".to_owned(),
        upstream_code: None,
        category: plan.export.category.clone(),
        exact_route: plan.export.canonical_route.clone(),
        true_name: Some(plan.export.true_name.clone()),
        source: Some(plan.export.source_relative_path.clone()),
        source_sha256: Some(plan.export.source_sha256.clone()),
        detail,
        evidence: json!({
            "sourceFacts": plan.export.facts,
            "physicalTarget": plan.export.physical_target,
            "fallbackApplied": false,
        }),
        required_evidence: vec![
            "a lossless native conversion satisfying the exact logical-model, coordinate, rig, animation and material contracts".to_owned(),
        ],
        disposition: "blocked-no-lossy-or-placeholder-glb".to_owned(),
    }
}

pub(super) fn native_contract_blocker(
    plan: &EquipmentPlan,
    detail: &str,
    evidence: Value,
) -> EquipmentLogicalModelBlocker {
    EquipmentLogicalModelBlocker {
        stage: "native-glb-preparation".to_owned(),
        code: "nativePreparedOutputContractMismatch".to_owned(),
        upstream_code: None,
        category: plan.export.category.clone(),
        exact_route: plan.export.canonical_route.clone(),
        true_name: Some(plan.export.true_name.clone()),
        source: Some(plan.export.source_relative_path.clone()),
        source_sha256: Some(plan.export.source_sha256.clone()),
        detail: detail.to_owned(),
        evidence,
        required_evidence: vec![
            "prepared output identity and every sidecar path must equal global preflight"
                .to_owned(),
        ],
        disposition: "blocked-no-unpreflighted-output".to_owned(),
    }
}

pub(super) fn published_file_counts(published: &[PublishedModel]) -> Result<(u64, u64, u64, u64)> {
    let mut files = 1usize;
    let mut glbs = 0usize;
    let mut pngs = 0usize;
    let mut reports = 0usize;
    for model in published {
        files = files
            .checked_add(model.output_files.len())
            .ok_or_else(|| equipment_error_value("published file count overflow"))?;
        for path in &model.output_files {
            match path.extension().and_then(|value| value.to_str()) {
                Some("glb") => glbs += 1,
                Some("png") => pngs += 1,
                Some("json")
                    if path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|name| name.ends_with(".publish.json")) =>
                {
                    reports += 1;
                }
                _ => {}
            }
        }
    }
    Ok((
        u64_count(files, "published file count")?,
        u64_count(glbs, "published GLB count")?,
        u64_count(pngs, "published PNG count")?,
        u64_count(reports, "published report count")?,
    ))
}

pub(super) fn coordinate_summary(published: &[PublishedModel]) -> (String, u64, Option<f64>, Option<f64>) {
    let mismatches = published
        .iter()
        .filter(|model| {
            model.mapping.coordinate_status != "artifact-space-proven-runtime-spawn-policy-pending"
                || model.mapping.runtime_spawn_policy != "runtime-archetype-dependent-pending"
        })
        .count();
    let skinning = published
        .iter()
        .filter_map(|model| model.mapping.skinning_basis_parity_max_error)
        .reduce(f64::max);
    let current_pose = published
        .iter()
        .filter_map(|model| model.mapping.current_pose_bind_identity_deviation_max)
        .reduce(f64::max);
    (
        if mismatches == 0 {
            "artifact-space-proven-runtime-spawn-policy-pending"
        } else {
            "coordinate-evidence-mismatch"
        }
        .to_owned(),
        mismatches.try_into().unwrap_or(u64::MAX),
        skinning,
        current_pose,
    )
}

pub(super) fn rewrite_report(root: &Path, report: &EquipmentLogicalModelBatchReport) -> Result<()> {
    let path = root.join(EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE);
    let temporary = root.join(format!(
        ".{EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE}.rewrite"
    ));
    let bytes = report_bytes(report)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| io_at(&temporary, error))?;
    file.write_all(&bytes)
        .map_err(|error| io_at(&temporary, error))?;
    file.sync_all().map_err(|error| io_at(&temporary, error))?;
    fs::remove_file(&path).map_err(|error| io_at(&path, error))?;
    fs::rename(&temporary, &path).map_err(|error| io_at(&path, error))
}

pub(super) fn report_bytes(report: &EquipmentLogicalModelBatchReport) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(report).map_err(|error| {
        equipment_error_value(format!(
            "could not serialize equipment batch report: {error}"
        ))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn walk_source_files(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut pending = vec![root.to_path_buf()];
    let mut output = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| io_at(&directory, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(&directory, error))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| io_at(&path, error))?;
            if file_type.is_symlink() {
                return equipment_error(format!(
                    "symlink is forbidden in equipment source root: {path:?}"
                ));
            }
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| equipment_error_value("source escaped source root"))?;
                let relative = slash_path_checked(relative)?;
                if !relative.ends_with(".source.json") {
                    return equipment_error(format!(
                        "only .source.json files are allowed in equipment source root: {relative:?}"
                    ));
                }
                output.push((relative, path));
            } else {
                return equipment_error(format!("non-regular equipment source entry: {path:?}"));
            }
        }
    }
    output.sort_by(|left, right| left.0.cmp(&right.0));
    if output.is_empty() {
        return equipment_error("equipment source root has no source files");
    }
    Ok(output)
}

pub(super) fn clean_relative_components(path: &Path) -> Result<Vec<String>> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| equipment_error_value("output path is not Unicode"))?;
                validate_windows_component(value)?;
                if has_generated_identity(value) {
                    return equipment_error(format!(
                        "generated/hash/PathID output component is forbidden: {value:?}"
                    ));
                }
                components.push(value.to_owned());
            }
            _ => return equipment_error("output path is not a clean relative path"),
        }
    }
    if components.is_empty() {
        return equipment_error("output path is empty");
    }
    Ok(components)
}

pub(super) fn portable_key(value: &str) -> String {
    value.nfkc().flat_map(char::to_lowercase).nfkc().collect()
}

pub(super) fn sort_blockers(blockers: &mut [EquipmentLogicalModelBlocker]) {
    blockers.sort_by(|left, right| {
        left.stage
            .cmp(&right.stage)
            .then_with(|| left.category.cmp(&right.category))
            .then_with(|| left.exact_route.cmp(&right.exact_route))
            .then_with(|| left.code.cmp(&right.code))
    });
}

pub(super) fn elapsed_milliseconds(started: Instant) -> Result<u64> {
    started
        .elapsed()
        .as_millis()
        .try_into()
        .map_err(|_| equipment_error_value("elapsed milliseconds exceed u64"))
}

pub(super) fn u64_count(value: usize, label: &str) -> Result<u64> {
    value
        .try_into()
        .map_err(|_| equipment_error_value(format!("{label} exceeds u64")))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
