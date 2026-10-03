use super::*;

pub const CLEAN_RUNTIME_METADATA_INDEX_SCHEMA: &str = "ffone.conversion-metadata-archive-index.v1";

pub const CLEAN_RUNTIME_METADATA_INDEX_FILE: &str = "index.json";

pub(super) const CLEAN_RUNTIME_METADATA_REVISION_INDEX_SCHEMA: &str =
    "ffone.conversion-metadata-revision-index.v1";

pub(super) const TUTORIAL_STATIC_INSTALL_MANIFEST: &str = "world/tutorial/static/install-manifest.json";

pub(super) const PROVEN_OFFLINE_CONTENT_INDEX_PATH: &str = "data/catalog/content-index--818266880751860c.json";

pub(super) const PROVEN_OFFLINE_CONTENT_INDEX_SOURCE_PATH: &str =
    "catalog/content-index--818266880751860c.json";

pub(super) const PROVEN_OFFLINE_CONTENT_INDEX_REASON: &str =
    "native-only incomplete content index is retained as offline import evidence";

pub(super) const PROVEN_OFFLINE_CONTENT_INDEX_BYTES: u64 = 6_293_459;

pub(super) const PROVEN_OFFLINE_CONTENT_INDEX_BLAKE3: &str =
    "818266880751860cdae29e39c480f8a81dcd0412f3c4e7731c2e4ae6f12045dd";

pub(super) const PROVEN_OFFLINE_CONTENT_INDEX_SCHEMA: &str = "ffone.asset-index.v1";

pub(super) const PROVEN_OFFLINE_CONTENT_INDEX_PROFILE: &str = "core-v1";

#[derive(Clone, Debug)]
pub(super) struct ProvenOfflineContentIndexProof {
    pub(super) path: String,
    pub(super) source_path: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
    pub(super) schema: String,
    pub(super) profile: String,
}

pub(super) fn proven_offline_content_index_proof(
    manifest: &ProjectAssetManifest,
) -> Option<ProvenOfflineContentIndexProof> {
    has_proven_production_source_pack(manifest).then(|| ProvenOfflineContentIndexProof {
        path: PROVEN_OFFLINE_CONTENT_INDEX_PATH.to_owned(),
        source_path: PROVEN_OFFLINE_CONTENT_INDEX_SOURCE_PATH.to_owned(),
        bytes: PROVEN_OFFLINE_CONTENT_INDEX_BYTES,
        blake3: PROVEN_OFFLINE_CONTENT_INDEX_BLAKE3.to_owned(),
        schema: PROVEN_OFFLINE_CONTENT_INDEX_SCHEMA.to_owned(),
        profile: PROVEN_OFFLINE_CONTENT_INDEX_PROFILE.to_owned(),
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMetadataRegistryCopy {
    pub source_path: String,
    pub runtime_path: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversionMetadataArchiveIndex<'a> {
    pub(super) schema: &'static str,
    pub(super) source_build: &'a str,
    pub(super) files: &'a [ArchivedRuntimeMetadata],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompletedConversionMetadataArchiveIndex {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) files: Vec<ArchivedRuntimeMetadata>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ConversionMetadataRevisionIndex {
    pub(super) schema: String,
    pub(super) source_build: String,
    pub(super) plan_blake3: String,
    pub(super) files: Vec<ArchivedRuntimeMetadata>,
}

pub(super) fn normalized_asset_path(asset_root: &Path, path: &Path) -> Result<String> {
    path.strip_prefix(asset_root)
        .map_err(|_| invalid_error("asset path escaped the canonical asset root"))?
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_error("asset path is not valid UTF-8")),
            _ => Err(invalid_error("asset path is not a normal relative path")),
        })
        .collect::<Result<Vec<_>>>()
        .map(|components| components.join("/"))
}

pub(super) fn find_archived_asset_reference(
    document_path: &str,
    value: &Value,
    cohort_paths: &BTreeSet<String>,
) -> Option<(String, String)> {
    match value {
        Value::String(reference) => resolve_json_reference_targets(document_path, reference)
            .into_iter()
            .find(|target| cohort_paths.contains(target))
            .map(|target| (reference.clone(), target)),
        Value::Array(values) => values
            .iter()
            .find_map(|value| find_archived_asset_reference(document_path, value, cohort_paths)),
        Value::Object(values) => values
            .values()
            .find_map(|value| find_archived_asset_reference(document_path, value, cohort_paths)),
        Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

pub(super) fn normalize_direct_asset_reference(reference: &str) -> Option<String> {
    if reference.is_empty() || reference.starts_with('/') {
        return None;
    }
    let components = reference.split('/').collect::<Vec<_>>();
    components
        .iter()
        .all(|component| !component.is_empty() && !matches!(*component, "." | ".."))
        .then(|| components.join("/"))
}

#[derive(Debug, Default)]
pub(super) struct ProvenOfflineContentIndexPlan {
    pub(super) archived: Vec<ArchivedRuntimeMetadata>,
    pub(super) already_archived_files: u64,
}

pub(super) fn plan_proven_offline_content_index(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    archive_history: &ArchiveHistory,
    proof: &ProvenOfflineContentIndexProof,
    other_archived_paths: &BTreeSet<String>,
) -> Result<ProvenOfflineContentIndexPlan> {
    let already_archived = archived_proven_offline_content_index(archive_history, proof)?;
    let manifest_entries = manifest
        .files
        .iter()
        .filter(|entry| entry.path == proof.path)
        .collect::<Vec<_>>();
    if manifest_entries.len() > 1 {
        return invalid(format!(
            "manifest repeats proven offline content index {:?}",
            proof.path
        ));
    }
    let path = asset_root.join(native_path(&proof.path)?);
    let disk_metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(io_at(&path, error)),
    };
    if disk_metadata.is_some() != !manifest_entries.is_empty() {
        return invalid(format!(
            "proven offline content index disk/manifest closure differs for {:?}",
            proof.path
        ));
    }
    let Some(metadata) = disk_metadata else {
        if !already_archived {
            return invalid(format!(
                "proven offline content index {:?} is absent without its immutable archive",
                proof.path
            ));
        }
        return Ok(ProvenOfflineContentIndexPlan {
            archived: Vec::new(),
            already_archived_files: 1,
        });
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return invalid(format!(
            "proven offline content index is not a regular file: {}",
            path.display()
        ));
    }
    let entry = manifest_entries[0];
    if entry.kind != ProjectAssetKind::Data
        || entry.source_path != proof.source_path
        || entry.bytes != proof.bytes
        || entry.blake3 != proof.blake3
        || metadata.len() != proof.bytes
    {
        return invalid(format!(
            "proven offline content index manifest/disk identity drift for {:?}",
            proof.path
        ));
    }
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    let actual_blake3 = blake3::hash(&bytes).to_hex().to_string();
    if bytes.len() as u64 != proof.bytes || actual_blake3 != proof.blake3 {
        return invalid(format!(
            "proven offline content index payload identity drift for {:?}",
            proof.path
        ));
    }
    validate_proven_offline_content_index_contract(&path, &bytes, proof)?;
    if already_archived
        && !archive_history.contains_identity(&proof.path, proof.bytes, &proof.blake3)
    {
        return invalid(
            "proven offline content index reappeared outside its immutable archive identity",
        );
    }
    let target_paths = [proof.path.clone()].into_iter().collect::<BTreeSet<_>>();
    reject_archived_asset_references(asset_root, manifest, &target_paths, other_archived_paths)?;
    Ok(ProvenOfflineContentIndexPlan {
        archived: vec![ArchivedRuntimeMetadata {
            source_path: proof.path.clone(),
            archive_path: format!("{ARCHIVE_PAYLOAD_DIRECTORY}/{}", proof.path),
            reason: PROVEN_OFFLINE_CONTENT_INDEX_REASON.to_owned(),
            bytes: proof.bytes,
            blake3: proof.blake3.clone(),
            manifested: true,
            manifest_source_path: Some(entry.source_path.clone()),
            manifest_kind: Some(entry.kind),
        }],
        already_archived_files: u64::from(already_archived),
    })
}

pub(super) fn archived_proven_offline_content_index(
    history: &ArchiveHistory,
    proof: &ProvenOfflineContentIndexProof,
) -> Result<bool> {
    let mut found = false;
    for entry in history
        .batches
        .iter()
        .flatten()
        .filter(|entry| entry.reason == PROVEN_OFFLINE_CONTENT_INDEX_REASON)
    {
        if found
            || entry.source_path != proof.path
            || entry.archive_path != format!("{ARCHIVE_PAYLOAD_DIRECTORY}/{}", proof.path)
            || entry.bytes != proof.bytes
            || entry.blake3 != proof.blake3
            || !entry.manifested
            || entry.manifest_source_path.is_none()
            || entry.manifest_source_path.as_deref() != Some(proof.source_path.as_str())
            || entry.manifest_kind != Some(ProjectAssetKind::Data)
        {
            return invalid("immutable archive has an invalid proven offline content-index proof");
        }
        found = true;
    }
    Ok(found)
}

pub(super) fn manifest_paths_below(manifest: &ProjectAssetManifest, root: &str) -> BTreeSet<String> {
    let prefix = format!("{root}/");
    manifest
        .files
        .iter()
        .filter(|entry| entry.path.starts_with(&prefix))
        .map(|entry| entry.path.clone())
        .collect()
}

pub(super) fn native_path(relative: &str) -> Result<PathBuf> {
    if relative.is_empty() || relative.contains('\\') || relative.starts_with('/') {
        return invalid(format!("unsafe relative asset path {relative:?}"));
    }
    let mut native = PathBuf::new();
    for segment in relative.split('/') {
        if segment.is_empty() || matches!(segment, "." | "..") {
            return invalid(format!("unsafe relative asset path {relative:?}"));
        }
        native.push(segment);
    }
    Ok(native)
}
