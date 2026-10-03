use super::*;

pub(crate) fn validate_completed_migration(
    asset_root: &Path,
    archive_root: &Path,
    source_build: &str,
) -> Result<RuntimeWorldMigrationReport> {
    reject_stale_transactions(asset_root, archive_root.parent())?;
    let archive_metadata =
        fs::symlink_metadata(archive_root).map_err(|error| io_at(archive_root, error))?;
    if archive_metadata.file_type().is_symlink() || !archive_metadata.is_dir() {
        return invalid("completed world conversion archive is not a regular directory");
    }

    let report_path = archive_root.join("report.json");
    let mut report: RuntimeWorldMigrationReport = read_json_file(&report_path)?;
    let expected_archive_root = format!(
        "../FusionForge/work/ffone/migration-archive/{source_build}/{WORLD_ARCHIVE_DIRECTORY}"
    );
    if report.schema != RUNTIME_WORLD_MIGRATION_REPORT_SCHEMA
        || report.source_build != source_build
        || report.mode != RuntimeWorldMigrationMode::Apply
        || report.asset_root != "assets/game"
        || report.archive_root != expected_archive_root
        || !report.apply_ready
        || !report.blockers.is_empty()
    {
        return invalid("completed world migration report identity or state is invalid");
    }

    let index_path = archive_root.join("index.json");
    let index: CompletedRuntimeWorldArchiveIndex = read_json_file(&index_path)?;
    if index.schema != RUNTIME_WORLD_ARCHIVE_INDEX_SCHEMA
        || index.source_build != source_build
        || index.technical_files != report.archived
        || index.rewritten_originals != report.rewritten
        || index.legacy_catalog_hash_drifts != report.legacy_catalog_hash_drifts
        || index.runtime_registry != report.registry
    {
        return invalid("completed world archive index does not match its apply report");
    }

    let counts = &report.counts;
    let archived_bytes: u64 = report.archived.iter().map(|entry| entry.bytes).sum();
    let rewritten_original_bytes: u64 = report
        .rewritten
        .iter()
        .map(|entry| entry.original_bytes)
        .sum();
    let rewritten_runtime_bytes: u64 = report
        .rewritten
        .iter()
        .map(|entry| entry.runtime_bytes)
        .sum();
    let removed_evidence_fields: u64 = report
        .rewritten
        .iter()
        .map(|entry| entry.removed_evidence_fields)
        .sum();
    let expected_manifest_after = counts
        .manifest_files_before
        .checked_sub(report.archived.len() as u64)
        .and_then(|count| count.checked_add(1));
    if counts.excluded_blocked_entries != report.excluded_blocked_entries.len() as u64
        || counts.legacy_catalog_hash_drifts != report.legacy_catalog_hash_drifts.len() as u64
        || counts.archived_technical_files != report.archived.len() as u64
        || counts.archived_technical_bytes != archived_bytes
        || counts.rewritten_runtime_files != report.rewritten.len() as u64
        || counts.rewritten_original_bytes != rewritten_original_bytes
        || counts.rewritten_runtime_bytes != rewritten_runtime_bytes
        || counts.removed_evidence_fields != removed_evidence_fields
        || counts.removed_manifest_entries != report.archived.len() as u64
        || counts.updated_manifest_entries != report.rewritten.len() as u64
        || counts.added_manifest_entries != 1
        || expected_manifest_after != Some(counts.manifest_files_after)
    {
        return invalid("completed world migration report counts are inconsistent");
    }

    let mut expected_archive_files =
        BTreeSet::from(["index.json".to_owned(), "report.json".to_owned()]);
    let mut archived_source_paths = BTreeSet::new();
    for entry in &report.archived {
        validate_blake3(&entry.blake3)?;
        let expected_archive_path = format!("{ARCHIVE_FILES_DIRECTORY}/{}", entry.source_path);
        if entry.archive_path != expected_archive_path
            || entry.reason.trim().is_empty()
            || !archived_source_paths.insert(entry.source_path.clone())
            || !expected_archive_files.insert(entry.archive_path.clone())
        {
            return invalid("completed world technical archive has invalid or duplicate paths");
        }
        verify_file_identity(
            &archive_root.join(native_path(&entry.archive_path)?),
            entry.bytes,
            &entry.blake3,
        )?;
    }

    let mut rewrite_paths = BTreeSet::new();
    let mut original_hashes = BTreeMap::new();
    let mut runtime_hashes = BTreeMap::new();
    for entry in &report.rewritten {
        validate_blake3(&entry.original_blake3)?;
        validate_blake3(&entry.runtime_blake3)?;
        let expected_original_path = format!("{ARCHIVE_ORIGINALS_DIRECTORY}/{}", entry.path);
        let next_path = format!("{ARCHIVE_NEXT_DIRECTORY}/{}", entry.path);
        if entry.original_archive_path != expected_original_path
            || !rewrite_paths.insert(entry.path.clone())
            || !expected_archive_files.insert(entry.original_archive_path.clone())
            || !expected_archive_files.insert(next_path.clone())
            || original_hashes
                .insert(entry.path.clone(), entry.original_blake3.clone())
                .is_some()
            || runtime_hashes
                .insert(entry.path.clone(), entry.runtime_blake3.clone())
                .is_some()
        {
            return invalid("completed world runtime archive has invalid or duplicate paths");
        }
        verify_file_identity(
            &archive_root.join(native_path(&entry.original_archive_path)?),
            entry.original_bytes,
            &entry.original_blake3,
        )?;
        verify_file_identity(
            &archive_root.join(native_path(&next_path)?),
            entry.runtime_bytes,
            &entry.runtime_blake3,
        )?;
    }

    if report.registry.path != RUNTIME_WORLD_REGISTRY_PATH {
        return invalid("completed world report has an unexpected registry path");
    }
    validate_blake3(&report.registry.blake3)?;
    let registry_next_path = format!("{ARCHIVE_NEXT_DIRECTORY}/{RUNTIME_WORLD_REGISTRY_PATH}");
    if !expected_archive_files.insert(registry_next_path.clone()) {
        return invalid("completed world archive repeats its runtime registry");
    }
    verify_file_identity(
        &archive_root.join(native_path(&registry_next_path)?),
        report.registry.bytes,
        &report.registry.blake3,
    )?;
    for drift in &report.legacy_catalog_hash_drifts {
        validate_blake3(&drift.catalog_blake3)?;
        validate_blake3(&drift.verified_source_blake3)?;
        if !matches!(drift.payload.as_str(), "scene" | "terrain" | "environment") {
            return invalid("completed world report has an invalid catalog drift payload");
        }
    }

    let actual_archive_files = collect_regular_files(archive_root)?;
    let actual_archive_files = actual_archive_files.into_iter().collect::<BTreeSet<_>>();
    if actual_archive_files != expected_archive_files {
        let missing = expected_archive_files
            .difference(&actual_archive_files)
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        let extra = actual_archive_files
            .difference(&expected_archive_files)
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        return invalid(format!(
            "completed world archive coverage mismatch: missing={missing:?}, extra={extra:?}"
        ));
    }

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest: ProjectAssetManifest = read_json_file(&manifest_path)?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid("completed world runtime manifest identity or count is invalid");
    }
    validate_completed_manifest_count(
        asset_root,
        source_build,
        &manifest,
        counts.manifest_files_after,
    )?;
    let manifest_by_path = manifest_index(&manifest)?;
    let current_world_paths = collect_world_json_paths(asset_root)?;
    let current_world_paths = current_world_paths.into_iter().collect::<BTreeSet<_>>();
    let remaining_technical = current_world_paths
        .iter()
        .filter(|path| technical_metadata_reason(path).is_some())
        .take(8)
        .cloned()
        .collect::<Vec<_>>();
    if !remaining_technical.is_empty()
        || report.archived.iter().any(|entry| {
            current_world_paths.contains(&entry.source_path)
                || manifest_by_path.contains_key(entry.source_path.as_str())
        })
    {
        return invalid(format!(
            "completed world runtime retains technical metadata: {remaining_technical:?}"
        ));
    }

    for entry in &report.rewritten {
        let bytes = verified_manifest_bytes(
            asset_root,
            &manifest_by_path,
            &entry.path,
            Some(&entry.runtime_blake3),
        )?;
        if bytes.len() as u64 != entry.runtime_bytes {
            return invalid(format!(
                "completed runtime payload byte count mismatch for {:?}",
                entry.path
            ));
        }
        audit_sanitized_json(&entry.path, &bytes)?;
    }
    let registry_bytes = verified_manifest_bytes(
        asset_root,
        &manifest_by_path,
        RUNTIME_WORLD_REGISTRY_PATH,
        Some(&report.registry.blake3),
    )?;
    if registry_bytes.len() as u64 != report.registry.bytes {
        return invalid("completed runtime registry byte count mismatch");
    }
    audit_sanitized_json(RUNTIME_WORLD_REGISTRY_PATH, &registry_bytes)?;
    let registry: RuntimeWorldRegistry =
        serde_json::from_slice(&registry_bytes).map_err(|source| PipelineError::Json {
            path: asset_root
                .join(RUNTIME_WORLD_REGISTRY_PATH)
                .display()
                .to_string(),
            source,
        })?;
    if registry.schema != RUNTIME_WORLD_REGISTRY_SCHEMA
        || registry.entries.len() as u64 != counts.registry_entries
    {
        return invalid("completed runtime registry schema or entry count is invalid");
    }

    let mut registry_by_identity = BTreeMap::new();
    for entry in &registry.entries {
        let identity = (entry.scope.clone(), entry.id.clone(), entry.tile);
        if registry_by_identity.insert(identity, entry).is_some() {
            return invalid("completed runtime registry repeats a stable identity");
        }
        for reference in std::iter::once(&entry.scene)
            .chain(std::iter::once(&entry.terrain))
            .chain(entry.environment.iter())
        {
            let expected = runtime_hashes.get(&reference.path).ok_or_else(|| {
                invalid_error(format!(
                    "runtime registry references untracked payload {:?}",
                    reference.path
                ))
            })?;
            if expected != &reference.blake3 {
                return invalid(format!(
                    "runtime registry hash differs from rewritten payload {:?}",
                    reference.path
                ));
            }
            verified_manifest_bytes(
                asset_root,
                &manifest_by_path,
                &reference.path,
                Some(&reference.blake3),
            )?;
        }
    }

    let archived_catalog_path = archive_root.join(format!(
        "{ARCHIVE_FILES_DIRECTORY}/{LEGACY_WORLD_CATALOG_PATH}"
    ));
    let catalog: LegacyWorldCatalog = read_json_file(&archived_catalog_path)?;
    if catalog.schema != LEGACY_WORLD_CATALOG_SCHEMA {
        return invalid("completed world archive contains an unsupported legacy catalog");
    }
    let mut catalog_identities = BTreeSet::new();
    let mut expected_excluded = Vec::new();
    let mut expected_drifts = Vec::new();
    let mut expected_rewrite_paths = BTreeSet::new();
    for entry in &catalog.entries {
        validate_catalog_identity(entry, &mut catalog_identities)?;
        expected_rewrite_paths.insert(entry.environment.path.clone());
        expected_rewrite_paths.insert(entry.terrain_descriptor.clone());

        let environment_source_hash =
            required_original_hash(&original_hashes, &entry.environment.path)?;
        push_expected_hash_drift(
            &mut expected_drifts,
            entry,
            "environment",
            &entry.environment.path,
            &entry.environment.blake3,
            environment_source_hash,
        );
        let terrain_source_hash =
            required_original_hash(&original_hashes, &entry.terrain_descriptor)?;
        push_expected_hash_drift(
            &mut expected_drifts,
            entry,
            "terrain",
            &entry.terrain_descriptor,
            &entry.terrain_descriptor_blake3,
            terrain_source_hash,
        );

        let identity = (entry.scope.clone(), entry.instance_id.clone(), entry.tile);
        match entry.placement_status.as_str() {
            "linked" => {
                let scene_path = entry.scene.as_deref().ok_or_else(|| {
                    invalid_error("archived linked world catalog entry has no scene")
                })?;
                let scene_catalog_hash = entry.scene_blake3.as_deref().ok_or_else(|| {
                    invalid_error("archived linked world catalog entry has no scene hash")
                })?;
                expected_rewrite_paths.insert(scene_path.to_owned());
                let scene_source_hash = required_original_hash(&original_hashes, scene_path)?;
                push_expected_hash_drift(
                    &mut expected_drifts,
                    entry,
                    "scene",
                    scene_path,
                    scene_catalog_hash,
                    scene_source_hash,
                );

                let runtime = registry_by_identity.remove(&identity).ok_or_else(|| {
                    invalid_error(format!(
                        "completed runtime registry omits linked identity {:?}",
                        entry.instance_id
                    ))
                })?;
                let runtime_environment = runtime.environment.as_ref().ok_or_else(|| {
                    invalid_error(format!(
                        "completed runtime registry omits environment for {:?}",
                        entry.instance_id
                    ))
                })?;
                if runtime.scene.path != scene_path
                    || runtime.terrain.path != entry.terrain_descriptor
                    || runtime_environment.path != entry.environment.path
                {
                    return invalid(format!(
                        "completed runtime registry path linkage differs for {:?}",
                        entry.instance_id
                    ));
                }
            }
            "blocked" => {
                if registry_by_identity.contains_key(&identity) {
                    return invalid(format!(
                        "completed runtime registry includes blocked identity {:?}",
                        entry.instance_id
                    ));
                }
                expected_excluded.push(entry.instance_id.clone());
            }
            status => {
                return invalid(format!(
                    "archived world catalog retains unsupported placementStatus {status:?}"
                ));
            }
        }
    }
    if !registry_by_identity.is_empty() || expected_rewrite_paths != rewrite_paths {
        return invalid(
            "completed runtime registry or rewrite coverage differs from legacy linkage",
        );
    }
    expected_excluded.sort();
    sort_hash_drifts(&mut expected_drifts);
    if expected_excluded != report.excluded_blocked_entries
        || expected_drifts != report.legacy_catalog_hash_drifts
    {
        return invalid("completed world report does not reproduce archived catalog decisions");
    }

    report.mode = RuntimeWorldMigrationMode::DryRun;
    Ok(report)
}

pub(super) fn validate_completed_manifest_count(
    asset_root: &Path,
    source_build: &str,
    manifest: &ProjectAssetManifest,
    migration_manifest_files: u64,
) -> Result<()> {
    let current_manifest_files = manifest.files.len() as u64;
    if asset_root.file_name().and_then(|name| name.to_str()) != Some("game") {
        if current_manifest_files == migration_manifest_files {
            return Ok(());
        }
        return invalid("runtime world asset root is not assets/game");
    }
    let assets_root = asset_root
        .parent()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("assets"))
        .ok_or_else(|| invalid_error("runtime world asset root is not assets/game"))?;
    let project_root = assets_root
        .parent()
        .ok_or_else(|| invalid_error("runtime world asset root has no project parent"))?;
    let cleanup_root = project_root
        .join("../FusionForge/work/ffone/migration-archive")
        .join(source_build)
        .join("conversion-metadata");
    if !cleanup_root.exists() && current_manifest_files == migration_manifest_files {
        return Ok(());
    }
    let revisions = validate_completed_cleanup_archive(&cleanup_root, source_build)?;
    let mut revision_paths = BTreeSet::new();
    let mut removed_paths = BTreeSet::new();
    for entry in revisions.into_iter().filter(|entry| entry.manifested) {
        if !revision_paths.insert(entry.source_path.clone()) {
            return invalid(format!(
                "cleanup revisions repeat manifested path {:?}",
                entry.source_path
            ));
        }
        let mut current_matches = manifest
            .files
            .iter()
            .filter(|current| current.path == entry.source_path);
        let Some(current) = current_matches.next() else {
            removed_paths.insert(entry.source_path);
            continue;
        };
        if current_matches.next().is_some() {
            return invalid(format!(
                "current manifest repeats cleanup revision path {:?}",
                entry.source_path
            ));
        }
        let archived_source_path = entry.manifest_source_path.as_deref().ok_or_else(|| {
            invalid_error(format!(
                "manifested cleanup revision lacks source identity at {:?}",
                entry.source_path
            ))
        })?;
        let archived_kind = entry.manifest_kind.ok_or_else(|| {
            invalid_error(format!(
                "manifested cleanup revision lacks kind identity at {:?}",
                entry.source_path
            ))
        })?;
        if current.kind != archived_kind {
            return invalid(format!(
                "republished cleanup revision kind differs at {:?}",
                entry.source_path
            ));
        }
        let current_path = asset_root.join(native_path(&current.path)?);
        let metadata = fs::metadata(&current_path).map_err(|error| io_at(&current_path, error))?;
        if !metadata.is_file() {
            return invalid(format!(
                "republished cleanup revision payload is not a regular file at {:?}",
                entry.source_path
            ));
        }
        let current_bytes = fs::read(&current_path).map_err(|error| io_at(&current_path, error))?;
        if current.bytes != current_bytes.len() as u64
            || current.blake3 != blake3::hash(&current_bytes).to_hex().as_str()
        {
            return invalid(format!(
                "republished cleanup revision payload differs from current manifest at {:?}",
                entry.source_path
            ));
        }
        if current.source_path == archived_source_path
            && current.kind == archived_kind
            && current.bytes == entry.bytes
            && current.blake3 == entry.blake3
        {
            return invalid(format!(
                "cleanup revision path retains its archived manifest identity at {:?}",
                entry.source_path
            ));
        }
    }
    let expected_current = migration_manifest_files
        .checked_sub(removed_paths.len() as u64)
        .ok_or_else(|| invalid_error("cleanup revision count exceeds migration manifest count"))?;
    if current_manifest_files != expected_current {
        return invalid(format!(
            "completed world runtime manifest count is {}, expected {} after {} validated cleanup revision removals",
            current_manifest_files,
            expected_current,
            removed_paths.len()
        ));
    }
    Ok(())
}

pub(super) fn validate_catalog_identity(
    entry: &LegacyWorldCatalogEntry,
    identities: &mut BTreeSet<(String, String, [i32; 2])>,
) -> Result<()> {
    if entry.instance_id.trim().is_empty()
        || !matches!(entry.scope.as_str(), "tutorial" | "worldMap")
        || !identities.insert((entry.scope.clone(), entry.instance_id.clone(), entry.tile))
    {
        return invalid(format!(
            "world catalog has an invalid or duplicate stable identity {:?}/{:?}/{:?}",
            entry.scope, entry.instance_id, entry.tile
        ));
    }
    for hash in [
        entry.terrain_descriptor_blake3.as_str(),
        entry.environment.blake3.as_str(),
    ]
    .into_iter()
    .chain(entry.scene_blake3.as_deref())
    {
        validate_blake3(hash)?;
    }
    Ok(())
}

pub(super) fn audit_sanitized_json(path: &str, bytes: &[u8]) -> Result<()> {
    let value: Value = serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
        path: path.to_owned(),
        source,
    })?;
    fn visit(path: &str, value: &Value) -> Result<()> {
        match value {
            Value::Array(values) => {
                for value in values {
                    visit(path, value)?;
                }
            }
            Value::Object(values) => {
                for (key, value) in values {
                    if matches!(
                        key.as_str(),
                        "sourceBuild"
                            | "provenance"
                            | "mapScene"
                            | "placementAudit"
                            | "sourceCodeEvidence"
                            | "sourceObject"
                            | "terrainRendererSourceObject"
                            | "assetClosure"
                            | "rawDocument"
                            | "rawParsedDocument"
                            | "parsedData"
                            | "sceneInstance"
                            | "sceneInstancePath"
                            | "sceneInstanceBlake3"
                            | "modeSource"
                            | "modeEvidence"
                            | "runtimeSelection"
                            | "sourceEncoded"
                    ) {
                        return invalid(format!(
                            "sanitized runtime JSON {path:?} retains forbidden field {key:?}"
                        ));
                    }
                    visit(path, value)?;
                }
            }
            Value::String(text) => {
                let folded = text.to_ascii_lowercase();
                if folded.contains("retrobution") || folded.contains("provenance") {
                    return invalid(format!(
                        "sanitized runtime JSON {path:?} retains import-only string {text:?}"
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(path, &value)
}

pub(super) fn validate_archive_coverage(
    archived: &[ArchivedWorldTechnicalMetadata],
    rewrites: &[PreparedRuntimeRewrite],
    registry: &RuntimeWorldRegistry,
) -> Result<()> {
    if archived.is_empty() || registry.entries.is_empty() {
        return invalid("world migration found no technical metadata or linked runtime scenes");
    }
    let archived_paths = archived
        .iter()
        .map(|entry| entry.source_path.as_str())
        .collect::<BTreeSet<_>>();
    if archived_paths.len() != archived.len() || !archived_paths.contains(LEGACY_WORLD_CATALOG_PATH)
    {
        return invalid("world technical archive contains duplicates or no legacy catalog");
    }
    let rewrite_paths = rewrites
        .iter()
        .map(|entry| entry.report.path.as_str())
        .collect::<BTreeSet<_>>();
    if rewrite_paths.len() != rewrites.len()
        || registry.entries.iter().any(|entry| {
            !rewrite_paths.contains(entry.scene.path.as_str())
                || !rewrite_paths.contains(entry.terrain.path.as_str())
                || entry
                    .environment
                    .as_ref()
                    .is_some_and(|environment| !rewrite_paths.contains(environment.path.as_str()))
        })
    {
        return invalid("runtime registry references a missing or duplicate sanitized payload");
    }
    Ok(())
}

pub(super) fn reject_stale_transactions(asset_root: &Path, archive_parent: Option<&Path>) -> Result<()> {
    for root in std::iter::once(asset_root).chain(archive_parent) {
        if !root.exists() {
            continue;
        }
        for entry in fs::read_dir(root)
            .map_err(|error| io_at(root, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(root, error))?
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(".runtime-world-") {
                return invalid(format!(
                    "stale runtime-world transaction artifact exists at {}",
                    entry.path().display()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_blake3(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(format!("invalid BLAKE3 digest {value:?}"));
    }
    Ok(())
}

pub(super) fn validate_source_build(source_build: &str) -> Result<()> {
    if source_build.is_empty()
        || source_build.len() > 128
        || matches!(source_build, "." | "..")
        || !source_build.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return invalid("source build must be one safe ASCII path segment");
    }
    Ok(())
}

pub(super) fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::ProjectAssetManifest(format!("migrate-runtime-world failed: {}", message.into()))
}
