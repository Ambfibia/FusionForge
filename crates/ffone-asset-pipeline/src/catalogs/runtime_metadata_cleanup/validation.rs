use super::*;

pub(super) fn reject_archived_asset_references(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    cohort_paths: &BTreeSet<String>,
    other_archived_paths: &BTreeSet<String>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for entry in manifest.files.iter().filter(|entry| {
        entry.kind == ProjectAssetKind::Data
            && entry.path.ends_with(".json")
            && !cohort_paths.contains(&entry.path)
            && !other_archived_paths.contains(&entry.path)
    }) {
        if !seen.insert(entry.path.as_str()) {
            return invalid(format!(
                "manifest repeats active JSON while checking archived asset references: {:?}",
                entry.path
            ));
        }
        let path = asset_root.join(native_path(&entry.path)?);
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        let actual_blake3 = blake3::hash(&bytes).to_hex().to_string();
        if entry.bytes != bytes.len() as u64 || entry.blake3 != actual_blake3 {
            return invalid(format!(
                "active JSON manifest identity mismatch while checking archived asset \
                 references: {:?}",
                entry.path
            ));
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
                path: path.display().to_string(),
                source,
            })?;
        if let Some((reference, target)) =
            find_archived_asset_reference(&entry.path, &value, cohort_paths)
        {
            return invalid(format!(
                "active JSON {:?} retains relative reference {reference:?} to archived asset \
                 {target:?}",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_proven_offline_content_index_contract(
    path: &Path,
    bytes: &[u8],
    proof: &ProvenOfflineContentIndexProof,
) -> Result<()> {
    let value: Value = serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    let matches_contract = value.get("schema").and_then(Value::as_str)
        == Some(proof.schema.as_str())
        && value.get("nativeOnly").and_then(Value::as_bool) == Some(true)
        && value.get("complete").and_then(Value::as_bool) == Some(false)
        && value.get("profile").and_then(Value::as_str) == Some(proof.profile.as_str());
    if !matches_contract {
        return invalid(format!(
            "proven offline content index contract drift for {}",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn validate_orphan_anchor(
    files: &BTreeMap<String, PayloadIdentity>,
    path: &str,
    expected_blake3: &str,
) -> Result<()> {
    let Some(identity) = files.get(path) else {
        return invalid(format!("orphan world proof anchor is missing: {path:?}"));
    };
    if identity.blake3 != expected_blake3 {
        return invalid(format!(
            "orphan world proof anchor mismatch for {path:?}: actual={}, expected={expected_blake3}",
            identity.blake3
        ));
    }
    Ok(())
}

pub(super) fn reject_orphan_world_references(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    roots: &[String],
    blockers: &mut Vec<String>,
) -> Result<()> {
    for entry in manifest.files.iter().filter(|entry| {
        entry.kind == ProjectAssetKind::Data
            && entry.path.ends_with(".json")
            && roots
                .iter()
                .all(|root| !entry.path.starts_with(&format!("{root}/")))
    }) {
        let path = asset_root.join(native_path(&entry.path)?);
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        for root in roots {
            if text.contains(&format!("{root}/")) {
                blockers.push(format!(
                    "active data asset {:?} still references orphan world root {:?}",
                    entry.path, root
                ));
            }
        }
    }
    blockers.sort();
    blockers.dedup();
    Ok(())
}

pub(super) fn validate_tutorial_static_gate(
    asset_root: &Path,
    manifest_by_path: &BTreeMap<&str, Vec<&ProjectAssetFile>>,
    static_paths: &BTreeSet<String>,
) -> Result<TutorialStaticGate> {
    if static_paths.is_empty() {
        return Ok(TutorialStaticGate {
            ready: true,
            reason: String::new(),
        });
    }
    let blocked = |reason: String| TutorialStaticGate {
        ready: false,
        reason: format!(
            "tutorial static metadata is deferred until a manifested {RUNTIME_WORLD_REGISTRY_PATH} \
             schema v1 pins every merged scene: {reason}"
        ),
    };
    if !static_paths.contains(TUTORIAL_STATIC_INSTALL_MANIFEST) {
        return Ok(blocked(format!(
            "{TUTORIAL_STATIC_INSTALL_MANIFEST} is absent"
        )));
    }
    for relative in static_paths {
        if !manifested_file_is_valid(asset_root, manifest_by_path, relative)? {
            return Ok(blocked(format!(
                "{relative} is absent or has no single matching manifest identity"
            )));
        }
    }

    let mut tile_files = BTreeMap::<String, BTreeSet<String>>::new();
    for relative in static_paths {
        if relative == TUTORIAL_STATIC_INSTALL_MANIFEST {
            continue;
        }
        let Some(tile_id) = tutorial_static_tile_id(relative) else {
            return Ok(blocked(format!(
                "unsupported tutorial static metadata path {relative:?}"
            )));
        };
        tile_files
            .entry(tile_id.to_owned())
            .or_default()
            .insert(relative.rsplit('/').next().unwrap_or(relative).to_owned());
    }
    let expected = ["catalog.json", "hierarchy.json", "materials.json"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if tile_files.is_empty() {
        return Ok(blocked(
            "no complete static tile metadata set exists".to_owned(),
        ));
    }
    for (tile_id, files) in &tile_files {
        if files != &expected {
            return Ok(blocked(format!(
                "{tile_id} static metadata set is incomplete: {files:?}"
            )));
        }
    }

    if !manifested_file_is_valid(asset_root, manifest_by_path, RUNTIME_WORLD_REGISTRY_PATH)? {
        return Ok(blocked(format!(
            "{RUNTIME_WORLD_REGISTRY_PATH} is absent or has no single matching manifest identity"
        )));
    }
    let registry_path = asset_root.join(native_path(RUNTIME_WORLD_REGISTRY_PATH)?);
    let registry_bytes = fs::read(&registry_path).map_err(|error| io_at(&registry_path, error))?;
    let registry: RuntimeWorldRegistry = match serde_json::from_slice(&registry_bytes) {
        Ok(registry) => registry,
        Err(error) => {
            return Ok(blocked(format!(
                "{RUNTIME_WORLD_REGISTRY_PATH} is not a valid runtime registry: {error}"
            )));
        }
    };
    if registry.schema != RUNTIME_WORLD_REGISTRY_SCHEMA {
        return Ok(blocked(format!(
            "{RUNTIME_WORLD_REGISTRY_PATH} schema is {:?}, expected {:?}",
            registry.schema, RUNTIME_WORLD_REGISTRY_SCHEMA
        )));
    }

    for tile_id in tile_files.keys() {
        let Some(tile) = tutorial_tile_coordinates(tile_id) else {
            return Ok(blocked(format!(
                "invalid tutorial static tile id {tile_id:?}"
            )));
        };
        let matches = registry
            .entries
            .iter()
            .filter(|entry| entry.id == *tile_id && entry.scope == "tutorial" && entry.tile == tile)
            .collect::<Vec<_>>();
        let [entry] = matches.as_slice() else {
            return Ok(blocked(format!(
                "{tile_id} has {} matching runtime-world entries, expected one",
                matches.len()
            )));
        };
        let expected_scene = format!("world/tutorial/terrain/tiles/{tile_id}/scene.json");
        if entry.scene.path != expected_scene {
            return Ok(blocked(format!(
                "{tile_id} runtime scene is {:?}, expected {expected_scene:?}",
                entry.scene.path
            )));
        }
        if !manifested_file_is_valid(asset_root, manifest_by_path, &entry.scene.path)? {
            return Ok(blocked(format!(
                "{} is absent or has no single matching manifest identity",
                entry.scene.path
            )));
        }
        let scene_path = asset_root.join(native_path(&entry.scene.path)?);
        let scene_hash = hash_file(&scene_path)?;
        if scene_hash != entry.scene.blake3 {
            return Ok(blocked(format!(
                "{} BLAKE3 is {scene_hash}, registry pins {}",
                entry.scene.path, entry.scene.blake3
            )));
        }
    }
    Ok(TutorialStaticGate {
        ready: true,
        reason: String::new(),
    })
}

pub(crate) fn validate_completed_cleanup_archive(
    archive_root: &Path,
    source_build: &str,
) -> Result<Vec<ArchivedRuntimeMetadata>> {
    validate_completed_archive(archive_root, source_build)?;
    let revisions_root = archive_root.join(ARCHIVE_REVISIONS_DIRECTORY);
    if !revisions_root.exists() {
        return Ok(Vec::new());
    }
    let mut revisions = fs::read_dir(&revisions_root)
        .map_err(|error| io_at(&revisions_root, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(&revisions_root, error))?;
    revisions.sort_by_key(|entry| entry.file_name());
    let mut files = Vec::new();
    for revision in revisions {
        let index_path = revision.path().join(CLEAN_RUNTIME_METADATA_INDEX_FILE);
        let index: ConversionMetadataRevisionIndex = serde_json::from_slice(
            &fs::read(&index_path).map_err(|error| io_at(&index_path, error))?,
        )
        .map_err(|source| PipelineError::Json {
            path: index_path.display().to_string(),
            source,
        })?;
        files.extend(index.files);
    }
    Ok(files)
}

pub(super) fn validate_completed_archive(archive_root: &Path, source_build: &str) -> Result<ArchiveHistory> {
    if !archive_root.is_dir() {
        return invalid(format!(
            "completed archive path is not a directory: {}",
            archive_root.display()
        ));
    }
    validate_archive_root_layout(archive_root)?;
    let index_path = archive_root.join(CLEAN_RUNTIME_METADATA_INDEX_FILE);
    let report_path = archive_root.join(CLEAN_RUNTIME_METADATA_REPORT_FILE);
    let index_bytes = fs::read(&index_path).map_err(|error| io_at(&index_path, error))?;
    let report_bytes = fs::read(&report_path).map_err(|error| io_at(&report_path, error))?;
    let index: CompletedConversionMetadataArchiveIndex = serde_json::from_slice(&index_bytes)
        .map_err(|source| PipelineError::Json {
            path: index_path.display().to_string(),
            source,
        })?;
    let report: CleanRuntimeMetadataReport =
        serde_json::from_slice(&report_bytes).map_err(|source| PipelineError::Json {
            path: report_path.display().to_string(),
            source,
        })?;
    if index.schema != CLEAN_RUNTIME_METADATA_INDEX_SCHEMA
        || index.source_build != source_build
        || report.schema != CLEAN_RUNTIME_METADATA_REPORT_SCHEMA
        || report.source_build != source_build
        || report.mode != CleanRuntimeMetadataMode::Apply
        || !report.apply_ready
        || !report.blockers.is_empty()
        || index.files != report.archived
        || !report.revision_archived.is_empty()
        || !report.removal_only.is_empty()
        || report.revision_plan_blake3.is_some()
    {
        return invalid("existing conversion-metadata archive index/report contract mismatch");
    }
    let archived_bytes = index.files.iter().map(|entry| entry.bytes).sum::<u64>();
    let manifested = index.files.iter().filter(|entry| entry.manifested).count() as u64;
    let unmanifested = index.files.len() as u64 - manifested;
    let proven_conversion_payload_files = index
        .files
        .iter()
        .filter(|entry| entry.reason == PROVEN_CONVERSION_PAYLOAD_REASON)
        .collect::<Vec<_>>();
    let proven_conversion_payload_bytes = proven_conversion_payload_files
        .iter()
        .map(|entry| entry.bytes)
        .sum::<u64>();
    let proven_offline_content_index_files = index
        .files
        .iter()
        .filter(|entry| entry.reason == PROVEN_OFFLINE_CONTENT_INDEX_REASON)
        .count() as u64;
    let proven_offline_content_index_bytes = index
        .files
        .iter()
        .filter(|entry| entry.reason == PROVEN_OFFLINE_CONTENT_INDEX_REASON)
        .map(|entry| entry.bytes)
        .sum::<u64>();
    if report.counts.archived_files != index.files.len() as u64
        || report.counts.archived_bytes != archived_bytes
        || report.counts.removed_manifest_entries != manifested
        || report.counts.unmanifested_archived_files != unmanifested
        || report.counts.runtime_registry_files != report.runtime_registries.len() as u64
        || report.counts.revision_archived_files != 0
        || report.counts.revision_archived_bytes != 0
        || report.counts.removal_only_files != 0
        || report.counts.removal_only_bytes != 0
        || report.counts.proven_conversion_payload_files
            != proven_conversion_payload_files.len() as u64
        || report.counts.proven_conversion_payload_bytes != proven_conversion_payload_bytes
        || report
            .counts
            .already_archived_proven_conversion_payload_files
            != 0
        || report.counts.proven_offline_content_index_files != proven_offline_content_index_files
        || report.counts.proven_offline_content_index_bytes != proven_offline_content_index_bytes
        || report
            .counts
            .already_archived_proven_offline_content_index_files
            != 0
    {
        return invalid("existing conversion-metadata archive counts do not match its index");
    }
    validate_archive_payload(archive_root, &index.files, "primary")?;
    for copy in &report.runtime_registries {
        let source = index
            .files
            .iter()
            .find(|entry| entry.source_path == copy.source_path)
            .ok_or_else(|| {
                invalid_error(format!(
                    "runtime registry source {:?} is absent from archive index",
                    copy.source_path
                ))
            })?;
        if source.bytes != copy.bytes || source.blake3 != copy.blake3 {
            return invalid(format!(
                "primary archive runtime registry source identity mismatch for {:?}",
                copy.source_path
            ));
        }
    }
    let mut history = ArchiveHistory::default();
    history.add(&index.files);
    validate_revision_archives(archive_root, source_build, &mut history)?;
    Ok(history)
}

pub(super) fn validate_archive_root_layout(archive_root: &Path) -> Result<()> {
    let allowed = [
        ARCHIVE_PAYLOAD_DIRECTORY,
        ARCHIVE_REVISIONS_DIRECTORY,
        CLEAN_RUNTIME_METADATA_INDEX_FILE,
        CLEAN_RUNTIME_METADATA_REPORT_FILE,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for entry in fs::read_dir(archive_root)
        .map_err(|error| io_at(archive_root, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(archive_root, error))?
    {
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(entry.path(), error))?;
        if file_type.is_symlink() {
            return invalid(format!(
                "conversion-metadata archive contains symlink {}",
                entry.path().display()
            ));
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !allowed.contains(name.as_str()) {
            return invalid(format!(
                "conversion-metadata archive contains unexpected entry {}",
                entry.path().display()
            ));
        }
        let expected_directory = matches!(
            name.as_str(),
            ARCHIVE_PAYLOAD_DIRECTORY | ARCHIVE_REVISIONS_DIRECTORY
        );
        if expected_directory != file_type.is_dir() {
            return invalid(format!(
                "conversion-metadata archive entry has wrong type: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_revision_archives(
    archive_root: &Path,
    source_build: &str,
    history: &mut ArchiveHistory,
) -> Result<()> {
    let revisions_root = archive_root.join(ARCHIVE_REVISIONS_DIRECTORY);
    if !revisions_root.exists() {
        return Ok(());
    }
    if !revisions_root.is_dir() {
        return invalid("conversion-metadata revisions path is not a directory");
    }
    let entries = fs::read_dir(&revisions_root)
        .map_err(|error| io_at(&revisions_root, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(&revisions_root, error))?;
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(entry.path(), error))?;
        if file_type.is_symlink() || !file_type.is_dir() {
            return invalid(format!(
                "conversion-metadata revision is not a regular directory: {}",
                entry.path().display()
            ));
        }
        let plan_blake3 = entry.file_name().to_string_lossy().into_owned();
        validate_blake3(&plan_blake3, "conversion-metadata revision directory")?;
        let revision_root = entry.path();
        validate_revision_root_layout(&revision_root)?;
        let index_path = revision_root.join(CLEAN_RUNTIME_METADATA_INDEX_FILE);
        let report_path = revision_root.join(CLEAN_RUNTIME_METADATA_REPORT_FILE);
        let index: ConversionMetadataRevisionIndex = serde_json::from_slice(
            &fs::read(&index_path).map_err(|error| io_at(&index_path, error))?,
        )
        .map_err(|source| PipelineError::Json {
            path: index_path.display().to_string(),
            source,
        })?;
        let report: ConversionMetadataRevisionReport = serde_json::from_slice(
            &fs::read(&report_path).map_err(|error| io_at(&report_path, error))?,
        )
        .map_err(|source| PipelineError::Json {
            path: report_path.display().to_string(),
            source,
        })?;
        let archived_bytes = index.files.iter().map(|file| file.bytes).sum::<u64>();
        let calculated_plan = revision_plan_blake3(source_build, &index.files)?;
        if index.schema != CLEAN_RUNTIME_METADATA_REVISION_INDEX_SCHEMA
            || report.schema != CLEAN_RUNTIME_METADATA_REVISION_REPORT_SCHEMA
            || index.source_build != source_build
            || report.source_build != source_build
            || index.plan_blake3 != plan_blake3
            || report.plan_blake3 != plan_blake3
            || calculated_plan != plan_blake3
            || report.files != index.files
            || report.archived_files != index.files.len() as u64
            || report.archived_bytes != archived_bytes
        {
            return invalid(format!(
                "conversion-metadata revision {plan_blake3} index/report contract mismatch"
            ));
        }
        validate_archive_payload(&revision_root, &index.files, "revision")?;
        history.add(&index.files);
    }
    Ok(())
}

pub(super) fn validate_revision_root_layout(revision_root: &Path) -> Result<()> {
    let allowed = [
        ARCHIVE_PAYLOAD_DIRECTORY,
        CLEAN_RUNTIME_METADATA_INDEX_FILE,
        CLEAN_RUNTIME_METADATA_REPORT_FILE,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for entry in fs::read_dir(revision_root)
        .map_err(|error| io_at(revision_root, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(revision_root, error))?
    {
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(entry.path(), error))?;
        if file_type.is_symlink() {
            return invalid(format!(
                "conversion-metadata revision contains symlink {}",
                entry.path().display()
            ));
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !allowed.contains(name.as_str()) {
            return invalid(format!(
                "conversion-metadata revision contains unexpected entry {}",
                entry.path().display()
            ));
        }
        let expected_directory = name == ARCHIVE_PAYLOAD_DIRECTORY;
        if expected_directory != file_type.is_dir() {
            return invalid(format!(
                "conversion-metadata revision entry has wrong type: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_plan_manifest_identity(
    plan: &CleanupPlan,
    expected_blake3: Option<&str>,
) -> Result<String> {
    let bytes = fs::read(&plan.manifest_path).map_err(|error| io_at(&plan.manifest_path, error))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: plan.manifest_path.display().to_string(),
            source,
        })?;
    if manifest != plan.manifest {
        return invalid("asset manifest changed after cleanup planning");
    }
    let actual_blake3 = blake3::hash(&bytes).to_hex().to_string();
    if expected_blake3.is_some_and(|expected| expected != actual_blake3) {
        return invalid("asset manifest byte identity changed during cleanup transaction");
    }
    Ok(actual_blake3)
}

pub(super) fn validate_file_identity(
    path: &Path,
    expected_bytes: u64,
    expected_blake3: &str,
    label: &str,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return invalid(format!("{label} is not a regular file: {}", path.display()));
    }
    if metadata.len() != expected_bytes {
        return invalid(format!(
            "{label} byte mismatch at {}: expected {expected_bytes}, found {}",
            path.display(),
            metadata.len()
        ));
    }
    let actual_blake3 = hash_file(path)?;
    if actual_blake3 != expected_blake3 {
        return invalid(format!(
            "{label} BLAKE3 mismatch at {}: expected {expected_blake3}, found {actual_blake3}",
            path.display()
        ));
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
            if name.starts_with(".clean-runtime-metadata-")
                || name.starts_with(".conversion-metadata-stage-")
                || name.starts_with(".conversion-metadata-revision-stage-")
            {
                return invalid(format!(
                    "stale clean-runtime-metadata transaction artifact exists at {}",
                    entry.path().display()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_blake3(value: &str, label: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(format!("{label} BLAKE3 is not 64 hexadecimal characters"));
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
    PipelineError::ProjectAssetManifest(format!(
        "clean-runtime-metadata failed: {}",
        message.into()
    ))
}
