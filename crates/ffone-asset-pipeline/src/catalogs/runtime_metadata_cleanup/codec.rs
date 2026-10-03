use super::*;

pub(super) const ARCHIVE_PAYLOAD_DIRECTORY: &str = "files";

pub(super) const ORPHAN_WORLD_PAYLOAD_REASON: &str =
    "world payload has no scene and is unreachable from the runtime-world registry";

pub(super) const PROVEN_CONVERSION_PAYLOAD_REASON: &str =
    "native world conversion payload is superseded by the sanitized runtime terrain contract";

#[derive(Clone, Copy, Debug)]
pub(super) struct ProvenConversionPayloadProof {
    pub(super) source_files: u64,
    pub(super) source_bytes: u64,
    pub(super) raw_files: u64,
    pub(super) raw_files_per_name: u64,
    pub(super) raw_bytes: u64,
    pub(super) attribute_json_files: u64,
    pub(super) attribute_json_bytes: u64,
    pub(super) total_files: u64,
    pub(super) total_bytes: u64,
}

pub(super) fn proven_conversion_payload_proof(
    manifest: &ProjectAssetManifest,
) -> Option<ProvenConversionPayloadProof> {
    has_proven_production_source_pack(manifest).then_some(ProvenConversionPayloadProof {
        source_files: 18_356,
        source_bytes: 336_875_833,
        raw_files: 1_530,
        raw_files_per_name: 170,
        raw_bytes: 2_903_940,
        attribute_json_files: 170,
        attribute_json_bytes: 3_805_366,
        total_files: 20_056,
        total_bytes: 343_585_139,
    })
}

#[derive(Clone, Debug)]
pub(super) struct ProvenOrphanWorldPayload {
    pub(super) id: String,
    pub(super) root: String,
    pub(super) files: u64,
    pub(super) json_files: u64,
    pub(super) bytes: u64,
    pub(super) terrain_blake3: String,
    pub(super) environment_blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PayloadIdentity {
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProvenConversionPayloadKind {
    SourceEncoded,
    RawComponent,
    GameplayAttributesJson,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct ProvenConversionPayloadStats {
    pub(super) source_files: u64,
    pub(super) source_bytes: u64,
    pub(super) raw_files: u64,
    pub(super) raw_bytes: u64,
    pub(super) raw_files_by_name: BTreeMap<String, u64>,
    pub(super) raw_names_by_root: BTreeMap<String, BTreeSet<String>>,
    pub(super) attribute_json_files: u64,
    pub(super) attribute_json_bytes: u64,
    pub(super) attribute_json_roots: BTreeSet<String>,
    pub(super) total_files: u64,
    pub(super) total_bytes: u64,
}

impl ProvenConversionPayloadStats {
    pub(super) fn add(&mut self, path: &str, bytes: u64) -> Result<()> {
        match classify_proven_conversion_payload(path) {
            Some(ProvenConversionPayloadKind::SourceEncoded) => {
                self.source_files += 1;
                self.source_bytes += bytes;
            }
            Some(ProvenConversionPayloadKind::RawComponent) => {
                let root = proven_conversion_terrain_root(path).ok_or_else(|| {
                    invalid_error(format!(
                        "raw conversion payload has no exact terrain root: {path:?}"
                    ))
                })?;
                let file_name = path.rsplit('/').next().unwrap_or(path).to_owned();
                if !self
                    .raw_names_by_root
                    .entry(root)
                    .or_default()
                    .insert(file_name.clone())
                {
                    return invalid(format!(
                        "terrain root repeats raw conversion payload name {file_name:?}"
                    ));
                }
                self.raw_files += 1;
                self.raw_bytes += bytes;
                *self.raw_files_by_name.entry(file_name).or_default() += 1;
            }
            Some(ProvenConversionPayloadKind::GameplayAttributesJson) => {
                let root = proven_conversion_terrain_root(path).ok_or_else(|| {
                    invalid_error(format!(
                        "gameplay attribute evidence has no exact terrain root: {path:?}"
                    ))
                })?;
                if !self.attribute_json_roots.insert(root) {
                    return invalid("terrain root repeats gameplay/attributes.raw.json");
                }
                self.attribute_json_files += 1;
                self.attribute_json_bytes += bytes;
            }
            None => {
                return invalid(format!(
                    "non-cohort path was counted as a proven conversion payload: {path:?}"
                ));
            }
        }
        self.total_files += 1;
        self.total_bytes += bytes;
        Ok(())
    }

    pub(super) fn matches(&self, proof: &ProvenConversionPayloadProof) -> bool {
        let expected_raw_names = PROVEN_CONVERSION_RAW_NAMES
            .into_iter()
            .map(|name| (name.to_owned(), proof.raw_files_per_name))
            .collect::<BTreeMap<_, _>>();
        let expected_raw_name_set = PROVEN_CONVERSION_RAW_NAMES
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        self.source_files == proof.source_files
            && self.source_bytes == proof.source_bytes
            && self.raw_files == proof.raw_files
            && self.raw_bytes == proof.raw_bytes
            && self.raw_files_by_name == expected_raw_names
            && self.raw_names_by_root.len() as u64 == proof.raw_files_per_name
            && self
                .raw_names_by_root
                .values()
                .all(|names| names == &expected_raw_name_set)
            && self.attribute_json_files == proof.attribute_json_files
            && self.attribute_json_bytes == proof.attribute_json_bytes
            && self.attribute_json_roots
                == self
                    .raw_names_by_root
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>()
            && self.total_files == proof.total_files
            && self.total_bytes == proof.total_bytes
    }

    pub(super) fn validate(&self, proof: &ProvenConversionPayloadProof, label: &str) -> Result<()> {
        if self.matches(proof) {
            return Ok(());
        }
        invalid(format!(
            "{label} proof mismatch: source={}/{}, sourceBytes={}/{}, raw={}/{}, \
             rawBytes={}/{}, rawNames={:?}, attributes={}/{}, attributeBytes={}/{}, \
             total={}/{}, totalBytes={}/{}",
            self.source_files,
            proof.source_files,
            self.source_bytes,
            proof.source_bytes,
            self.raw_files,
            proof.raw_files,
            self.raw_bytes,
            proof.raw_bytes,
            self.raw_files_by_name,
            self.attribute_json_files,
            proof.attribute_json_files,
            self.attribute_json_bytes,
            proof.attribute_json_bytes,
            self.total_files,
            proof.total_files,
            self.total_bytes,
            proof.total_bytes,
        ))
    }
}

#[derive(Debug, Default)]
pub(super) struct ProvenConversionPayloadPlan {
    pub(super) archived: Vec<ArchivedRuntimeMetadata>,
    pub(super) already_archived_files: u64,
}

pub(super) fn classify_proven_conversion_payload(relative: &str) -> Option<ProvenConversionPayloadKind> {
    if !relative.starts_with("world/") {
        return None;
    }
    if relative.ends_with(".source.bin") && !relative.contains("/details/textures/") {
        return Some(ProvenConversionPayloadKind::SourceEncoded);
    }
    let file_name = relative.rsplit('/').next().unwrap_or(relative);
    if PROVEN_CONVERSION_RAW_NAMES.contains(&file_name) {
        return Some(ProvenConversionPayloadKind::RawComponent);
    }
    relative
        .ends_with("/gameplay/attributes.raw.json")
        .then_some(ProvenConversionPayloadKind::GameplayAttributesJson)
}

pub(super) fn archived_proven_conversion_payload_paths(
    history: &ArchiveHistory,
    proof: &ProvenConversionPayloadProof,
) -> Result<Option<BTreeSet<String>>> {
    let mut complete_paths: Option<BTreeSet<String>> = None;
    let mut saw_cohort_entry = false;
    for batch in &history.batches {
        let entries = batch
            .iter()
            .filter(|entry| entry.reason == PROVEN_CONVERSION_PAYLOAD_REASON)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            continue;
        }
        saw_cohort_entry = true;
        let mut stats = ProvenConversionPayloadStats::default();
        let mut paths = BTreeSet::new();
        for entry in entries {
            if classify_proven_conversion_payload(&entry.source_path).is_none()
                || !entry.manifested
                || entry.manifest_kind != Some(ProjectAssetKind::Data)
                || entry.archive_path
                    != format!("{ARCHIVE_PAYLOAD_DIRECTORY}/{}", entry.source_path)
                || !paths.insert(entry.source_path.clone())
            {
                return invalid(format!(
                    "immutable archive has an invalid proven conversion payload entry {:?}",
                    entry.source_path
                ));
            }
            stats.add(&entry.source_path, entry.bytes)?;
        }
        if stats.matches(proof) {
            if complete_paths
                .as_ref()
                .is_some_and(|existing| existing != &paths)
            {
                return invalid(
                    "immutable archive contains contradictory complete conversion payload proofs",
                );
            }
            complete_paths = Some(paths);
        }
    }
    if complete_paths.is_none() && saw_cohort_entry {
        return invalid(
            "immutable archive contains conversion payload entries but no complete production proof",
        );
    }
    Ok(complete_paths)
}

#[derive(Debug, Default)]
pub(super) struct OrphanWorldPayloadPlan {
    pub(super) archived: Vec<ArchivedRuntimeMetadata>,
    pub(super) payload_roots: Vec<String>,
    pub(super) already_archived_roots: Vec<String>,
    pub(super) blockers: Vec<String>,
}

pub(super) fn validate_orphan_payload_proof(
    proof: &ProvenOrphanWorldPayload,
    files: &BTreeMap<String, PayloadIdentity>,
) -> Result<()> {
    let bytes = files.values().map(|identity| identity.bytes).sum::<u64>();
    let json_files = files.keys().filter(|path| path.ends_with(".json")).count() as u64;
    if files.len() as u64 != proof.files || json_files != proof.json_files || bytes != proof.bytes {
        return invalid(format!(
            "orphan world proof mismatch for {:?}: files={}/{}, json={}/{}, bytes={}/{}",
            proof.root,
            files.len(),
            proof.files,
            json_files,
            proof.json_files,
            bytes,
            proof.bytes
        ));
    }
    validate_orphan_anchor(
        files,
        &format!("{}/terrain/terrain.json", proof.root),
        &proof.terrain_blake3,
    )?;
    validate_orphan_anchor(
        files,
        &format!("{}/terrain/environment/environment.json", proof.root),
        &proof.environment_blake3,
    )
}

pub(super) fn collect_payload_tree(
    asset_root: &Path,
    root: &Path,
) -> Result<BTreeMap<String, PayloadIdentity>> {
    let metadata = fs::symlink_metadata(root).map_err(|error| io_at(root, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!(
            "orphan world root is not a regular directory: {}",
            root.display()
        ));
    }
    let mut files = BTreeMap::new();
    collect_payload_tree_at(asset_root, root, &mut files)?;
    Ok(files)
}

pub(super) fn collect_payload_tree_at(
    asset_root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, PayloadIdentity>,
) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_at(directory, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| io_at(directory, error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| io_at(&path, error))?;
        if metadata.file_type().is_symlink() {
            return invalid(format!(
                "symlink is forbidden in orphan world payload: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_payload_tree_at(asset_root, &path, files)?;
            continue;
        }
        if !metadata.is_file() {
            return invalid(format!(
                "unsupported orphan world payload entry: {}",
                path.display()
            ));
        }
        let relative = path
            .strip_prefix(asset_root)
            .map_err(|_| invalid_error("orphan world payload escaped asset root"))?;
        let relative = relative
            .components()
            .map(|component| match component {
                Component::Normal(part) => part
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid_error("orphan world path is not valid UTF-8")),
                _ => Err(invalid_error("unsafe orphan world path component")),
            })
            .collect::<Result<Vec<_>>>()?
            .join("/");
        let identity = PayloadIdentity {
            bytes: metadata.len(),
            blake3: hash_file(&path)?,
        };
        if files.insert(relative.clone(), identity).is_some() {
            return invalid(format!("duplicate orphan world payload path {relative:?}"));
        }
    }
    Ok(())
}

pub(super) fn validate_archive_payload(
    archive_root: &Path,
    files: &[ArchivedRuntimeMetadata],
    label: &str,
) -> Result<()> {
    if files.is_empty() {
        return invalid(format!("{label} conversion-metadata archive is empty"));
    }
    let payload_root = archive_root.join(ARCHIVE_PAYLOAD_DIRECTORY);
    if !payload_root.is_dir() {
        return invalid(format!(
            "{label} conversion-metadata archive has no files payload directory"
        ));
    }
    let mut previous = None;
    let mut indexed_paths = Vec::with_capacity(files.len());
    for entry in files {
        if previous.is_some_and(|path: &str| path >= entry.source_path.as_str()) {
            return invalid(format!(
                "{label} conversion-metadata archive index is not strictly source-path sorted"
            ));
        }
        previous = Some(entry.source_path.as_str());
        if entry.archive_path != format!("{ARCHIVE_PAYLOAD_DIRECTORY}/{}", entry.source_path) {
            return invalid(format!(
                "{label} archive path does not match source path {:?}",
                entry.source_path
            ));
        }
        validate_blake3(&entry.blake3, "archived conversion metadata")?;
        let archived = archive_root.join(native_path(&entry.archive_path)?);
        let metadata = fs::metadata(&archived).map_err(|error| io_at(&archived, error))?;
        let actual_hash = hash_file(&archived)?;
        if !metadata.is_file() || metadata.len() != entry.bytes || actual_hash != entry.blake3 {
            return invalid(format!(
                "existing archived payload identity mismatch for {:?}",
                entry.source_path
            ));
        }
        indexed_paths.push(entry.archive_path.clone());
    }
    let mut physical_paths = Vec::new();
    collect_regular_paths_from(archive_root, &payload_root, &mut physical_paths)?;
    physical_paths.sort();
    indexed_paths.sort();
    if physical_paths != indexed_paths {
        return invalid(format!(
            "{label} conversion-metadata archive payload differs from its index"
        ));
    }
    Ok(())
}

pub(super) fn stage_archive_payload(
    asset_root: &Path,
    archive_stage: &Path,
    files: &[ArchivedRuntimeMetadata],
) -> Result<()> {
    for entry in files {
        let source = asset_root.join(native_path(&entry.source_path)?);
        let target = archive_stage.join(native_path(&entry.archive_path)?);
        let parent = target
            .parent()
            .ok_or_else(|| invalid_error("archive payload has no parent"))?;
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
        validate_file_identity(
            &source,
            entry.bytes,
            &entry.blake3,
            "archive staging source",
        )?;
        let copied = fs::copy(&source, &target).map_err(|error| io_at(&target, error))?;
        OpenOptions::new()
            .write(true)
            .open(&target)
            .map_err(|error| io_at(&target, error))?
            .sync_all()
            .map_err(|error| io_at(&target, error))?;
        let copied_hash = hash_file(&target)?;
        if copied != entry.bytes || copied_hash != entry.blake3 {
            return invalid(format!(
                "staged archive identity mismatch for {:?}",
                entry.source_path
            ));
        }
    }
    Ok(())
}
