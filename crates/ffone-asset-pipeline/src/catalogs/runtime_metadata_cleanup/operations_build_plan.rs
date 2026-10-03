use super::*;

pub(super) fn has_proven_production_source_pack(manifest: &ProjectAssetManifest) -> bool {
    manifest.source_pack.schema == PROVEN_ORPHAN_WORLD_SOURCE_PACK_SCHEMA
        && manifest.source_pack.manifest_blake3 == PROVEN_ORPHAN_WORLD_SOURCE_PACK_BLAKE3
}

pub(super) fn proven_orphan_world_payloads(manifest: &ProjectAssetManifest) -> Vec<ProvenOrphanWorldPayload> {
    if manifest.source_pack.schema != PROVEN_ORPHAN_WORLD_SOURCE_PACK_SCHEMA
        || manifest.source_pack.manifest_blake3 != PROVEN_ORPHAN_WORLD_SOURCE_PACK_BLAKE3
    {
        return Vec::new();
    }
    [
        (
            "map_00_08",
            1_663_055,
            "79a738dec1cd7b3d3c0fe6bf02e8a9ec6ec64a22dbb4e12c09934f4f00d16f6e",
            "4bdbec48e801209e4b36dc945744a518a688c1582741a9520e52b4a52841d40c",
        ),
        (
            "map_01_09",
            1_663_055,
            "b7bfaf8b2e1997b5491985335421b54f9b70945ae842dbd1ce23b9c108bda02e",
            "dc3c0de858b2c0d100e925de6b2e4e3516810e88054e5954c512bebc263c3bc7",
        ),
        (
            "map_11_15",
            1_663_091,
            "8cca5774ae588ea1767f706994af2d08cb8a7683eaef48962f2766e93c4b5a56",
            "803807aaad08d5d59aa68e68bb2f4b887e08b41ad630af59291537ff28e9e40b",
        ),
    ]
    .into_iter()
    .map(
        |(id, bytes, terrain_blake3, environment_blake3)| ProvenOrphanWorldPayload {
            id: id.to_owned(),
            root: format!("world/maps/{id}"),
            files: 70,
            json_files: 2,
            bytes,
            terrain_blake3: terrain_blake3.to_owned(),
            environment_blake3: environment_blake3.to_owned(),
        },
    )
    .collect()
}

pub(super) fn plan_proven_conversion_payloads(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    archive_history: &ArchiveHistory,
    proof: &ProvenConversionPayloadProof,
    other_archived_paths: &BTreeSet<String>,
) -> Result<ProvenConversionPayloadPlan> {
    let archived_paths = archived_proven_conversion_payload_paths(archive_history, proof)?;
    let physical = collect_proven_conversion_payloads(asset_root)?;
    let physical_paths = physical.keys().cloned().collect::<BTreeSet<_>>();
    let mut manifested = BTreeMap::new();
    for entry in &manifest.files {
        if classify_proven_conversion_payload(&entry.path).is_none() {
            continue;
        }
        if entry.kind != ProjectAssetKind::Data {
            return invalid(format!(
                "proven conversion payload {:?} is not manifest-owned Data",
                entry.path
            ));
        }
        if manifested.insert(entry.path.clone(), entry).is_some() {
            return invalid(format!(
                "manifest contains duplicate proven conversion payload {:?}",
                entry.path
            ));
        }
    }
    let manifested_paths = manifested.keys().cloned().collect::<BTreeSet<_>>();
    if physical_paths != manifested_paths {
        let physical_only = physical_paths
            .difference(&manifested_paths)
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        let manifested_only = manifested_paths
            .difference(&physical_paths)
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        return invalid(format!(
            "proven conversion payload disk/manifest closure differs: disk={}, manifest={}, \
             diskOnly={physical_only:?}, manifestOnly={manifested_only:?}",
            physical_paths.len(),
            manifested_paths.len(),
        ));
    }

    let mut stats = ProvenConversionPayloadStats::default();
    for (path, identity) in &physical {
        stats.add(path, identity.bytes)?;
        let entry = manifested
            .get(path)
            .expect("physical/manifest cohort closures were compared");
        if entry.bytes != identity.bytes || entry.blake3 != identity.blake3 {
            return invalid(format!(
                "proven conversion payload manifest identity mismatch for {path:?}: \
                 manifest bytes={} blake3={}, disk bytes={} blake3={}",
                entry.bytes, entry.blake3, identity.bytes, identity.blake3
            ));
        }
    }

    if let Some(expected_paths) = &archived_paths {
        let unexpected = physical_paths
            .difference(expected_paths)
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        if !unexpected.is_empty() {
            return invalid(format!(
                "production conversion payload reappeared at paths outside its immutable proof: \
                 {unexpected:?}"
            ));
        }
        for (path, identity) in &physical {
            if !archive_history.contains_identity(path, identity.bytes, &identity.blake3) {
                return invalid(format!(
                    "proven conversion payload {path:?} reappeared with identity outside its \
                     immutable archive"
                ));
            }
        }
    } else {
        stats.validate(proof, "production conversion payload")?;
    }

    if !physical_paths.is_empty() {
        reject_archived_asset_references(
            asset_root,
            manifest,
            &physical_paths,
            other_archived_paths,
        )?;
    }

    let archived = physical
        .into_iter()
        .map(|(path, identity)| {
            let entry = manifested
                .get(&path)
                .expect("physical/manifest cohort closures were compared");
            ArchivedRuntimeMetadata {
                source_path: path.clone(),
                archive_path: format!("{ARCHIVE_PAYLOAD_DIRECTORY}/{path}"),
                reason: PROVEN_CONVERSION_PAYLOAD_REASON.to_owned(),
                bytes: identity.bytes,
                blake3: identity.blake3,
                manifested: true,
                manifest_source_path: Some(entry.source_path.clone()),
                manifest_kind: Some(entry.kind),
            }
        })
        .collect();
    Ok(ProvenConversionPayloadPlan {
        archived,
        already_archived_files: archived_paths.map_or(0, |paths| paths.len() as u64),
    })
}

pub(super) fn plan_proven_orphan_world_payloads(
    asset_root: &Path,
    manifest: &ProjectAssetManifest,
    manifest_by_path: &BTreeMap<&str, Vec<&ProjectAssetFile>>,
    archive_history: &ArchiveHistory,
    proofs: &[ProvenOrphanWorldPayload],
) -> Result<OrphanWorldPayloadPlan> {
    let mut plan = OrphanWorldPayloadPlan::default();
    let expected_roots = proofs
        .iter()
        .map(|proof| proof.root.as_str())
        .collect::<BTreeSet<_>>();
    let known_incomplete = proofs.iter().any(|proof| {
        let root = asset_root.join(native_path(&proof.root).unwrap_or_default());
        root.join("terrain/terrain.json").is_file() && !root.join("scene.json").exists()
    });
    if !manifested_file_is_valid(asset_root, manifest_by_path, RUNTIME_WORLD_REGISTRY_PATH)? {
        if known_incomplete {
            plan.blockers.push(format!(
                "proven orphan world payloads cannot be archived until manifested \
                 {RUNTIME_WORLD_REGISTRY_PATH} exists and matches disk"
            ));
        }
        return Ok(plan);
    }
    let registry_entry = manifest_by_path
        .get(RUNTIME_WORLD_REGISTRY_PATH)
        .and_then(|entries| entries.first())
        .copied()
        .ok_or_else(|| invalid_error("runtime-world registry has no manifest entry"))?;
    if registry_entry.kind != ProjectAssetKind::Data {
        return invalid("runtime-world registry manifest entry is not Data");
    }
    let registry_path = asset_root.join(native_path(RUNTIME_WORLD_REGISTRY_PATH)?);
    let registry_bytes = fs::read(&registry_path).map_err(|error| io_at(&registry_path, error))?;
    let registry: RuntimeWorldRegistry =
        serde_json::from_slice(&registry_bytes).map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != RUNTIME_WORLD_REGISTRY_SCHEMA {
        return invalid(format!(
            "unsupported runtime-world registry schema {:?}",
            registry.schema
        ));
    }

    let discovered = discover_unregistered_world_roots(asset_root, &registry)?;
    let unexpected = discovered
        .iter()
        .filter(|root| !expected_roots.contains(root.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        plan.blockers.push(format!(
            "unrecognized unregistered world payload roots require a separate proof: {}",
            unexpected.join(", ")
        ));
    }

    for proof in proofs {
        let disk_root = asset_root.join(native_path(&proof.root)?);
        let manifest_paths = manifest_paths_below(manifest, &proof.root);
        if !discovered.contains(&proof.root) {
            if runtime_world_references_root(&registry, &proof.id, &proof.root) {
                continue;
            }
            if disk_root.exists() {
                let residual_files = collect_payload_tree(asset_root, &disk_root)?;
                if !residual_files.is_empty() {
                    plan.blockers.push(format!(
                        "proven orphan world root {:?} has {} unregistered residual files",
                        proof.root,
                        residual_files.len()
                    ));
                    continue;
                }
            }
            if !manifest_paths.is_empty() {
                return invalid(format!(
                    "orphan world root {:?} is missing from disk but still has {} manifest entries",
                    proof.root,
                    manifest_paths.len()
                ));
            }
            let archived_paths = archive_history.paths_below(&proof.root);
            if archived_paths.len() as u64 == proof.files {
                plan.already_archived_roots.push(proof.root.clone());
            } else if archived_paths.is_empty() {
                plan.blockers.push(format!(
                    "proven orphan world root {:?} is absent without its immutable archive",
                    proof.root
                ));
            } else {
                plan.blockers.push(format!(
                    "immutable archive for {:?} is incomplete: files={}, expected={}",
                    proof.root,
                    archived_paths.len(),
                    proof.files
                ));
            }
            continue;
        }
        if runtime_world_references_root(&registry, &proof.id, &proof.root) {
            return invalid(format!(
                "refusing to archive runtime-world referenced root {:?}",
                proof.root
            ));
        }
        let files = collect_payload_tree(asset_root, &disk_root)?;
        validate_orphan_payload_proof(proof, &files)?;
        let disk_paths = files.keys().cloned().collect::<BTreeSet<_>>();
        if disk_paths != manifest_paths {
            return invalid(format!(
                "orphan world root {:?} disk/manifest closure differs: disk={}, manifest={}",
                proof.root,
                disk_paths.len(),
                manifest_paths.len()
            ));
        }
        for (path, identity) in &files {
            let entries = manifest_by_path
                .get(path.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let [entry] = entries else {
                return invalid(format!(
                    "orphan payload {path:?} has no unique manifest entry"
                ));
            };
            if entry.bytes != identity.bytes || entry.blake3 != identity.blake3 {
                return invalid(format!(
                    "orphan payload manifest identity mismatch for {path:?}"
                ));
            }
            plan.archived.push(ArchivedRuntimeMetadata {
                source_path: path.clone(),
                archive_path: format!("{ARCHIVE_PAYLOAD_DIRECTORY}/{path}"),
                reason: ORPHAN_WORLD_PAYLOAD_REASON.to_owned(),
                bytes: identity.bytes,
                blake3: identity.blake3.clone(),
                manifested: true,
                manifest_source_path: Some(entry.source_path.clone()),
                manifest_kind: Some(entry.kind),
            });
        }
        plan.payload_roots.push(proof.root.clone());
    }
    reject_orphan_world_references(
        asset_root,
        manifest,
        &plan.payload_roots,
        &mut plan.blockers,
    )?;
    plan.payload_roots.sort();
    plan.already_archived_roots.sort();
    Ok(plan)
}

pub(super) fn build_plan(
    asset_root: PathBuf,
    archive_root: PathBuf,
    source_build: &str,
    archive_history: &ArchiveHistory,
) -> Result<CleanupPlan> {
    reject_stale_transactions(&asset_root, archive_root.parent())?;

    let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| io_at(&manifest_path, error))?;
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
            path: manifest_path.display().to_string(),
            source,
        })?;
    if manifest.schema != PROJECT_ASSET_SCHEMA {
        return invalid(format!(
            "unsupported project-asset manifest schema {:?}",
            manifest.schema
        ));
    }

    let mut manifest_by_path: BTreeMap<&str, Vec<&ProjectAssetFile>> = BTreeMap::new();
    for entry in &manifest.files {
        manifest_by_path
            .entry(entry.path.as_str())
            .or_default()
            .push(entry);
    }

    let json_paths = collect_json_paths(&asset_root)?;
    let physical_paths = json_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut tutorial_static_paths = json_paths
        .iter()
        .filter(|relative| {
            matches!(
                classify_metadata(relative),
                MetadataDisposition::TutorialStatic(_)
            )
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    tutorial_static_paths.extend(manifest.files.iter().filter_map(|entry| {
        matches!(
            classify_metadata(&entry.path),
            MetadataDisposition::TutorialStatic(_)
        )
        .then(|| entry.path.clone())
    }));
    let tutorial_static_gate =
        validate_tutorial_static_gate(&asset_root, &manifest_by_path, &tutorial_static_paths)?;
    let mut blockers = Vec::new();
    if !tutorial_static_paths.is_empty() && !tutorial_static_gate.ready {
        blockers.push(tutorial_static_gate.reason.clone());
    }
    let mut archived = Vec::new();
    let mut deferred_world = Vec::new();
    for relative in json_paths {
        let disposition = classify_metadata(&relative);
        if disposition == MetadataDisposition::Keep {
            continue;
        }
        let native = native_path(&relative)?;
        let source = asset_root.join(&native);
        let metadata = fs::metadata(&source).map_err(|error| io_at(&source, error))?;
        let manifest_entries = manifest_by_path
            .get(relative.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if manifest_entries.len() > 1 {
            return invalid(format!(
                "manifest contains duplicate entries for {relative:?}"
            ));
        }
        match disposition {
            MetadataDisposition::Keep => unreachable!(),
            MetadataDisposition::DeferWorld(reason) => {
                deferred_world.push(DeferredWorldMetadata {
                    source_path: relative,
                    reason: reason.to_owned(),
                    bytes: metadata.len(),
                });
            }
            MetadataDisposition::TutorialStatic(_) if !tutorial_static_gate.ready => {
                deferred_world.push(DeferredWorldMetadata {
                    source_path: relative,
                    reason: tutorial_static_gate.reason.clone(),
                    bytes: metadata.len(),
                });
            }
            MetadataDisposition::Archive(reason) | MetadataDisposition::TutorialStatic(reason) => {
                let actual_hash = hash_file(&source)?;
                let manifest_entry = manifest_entries.first().copied();
                if let Some(entry) = manifest_entry {
                    if entry.bytes != metadata.len() || entry.blake3 != actual_hash {
                        return invalid(format!(
                            "manifest identity mismatch for {relative:?}: manifest bytes={} \
                             blake3={}, actual bytes={} blake3={actual_hash}",
                            entry.bytes,
                            entry.blake3,
                            metadata.len()
                        ));
                    }
                }
                archived.push(ArchivedRuntimeMetadata {
                    archive_path: format!("{ARCHIVE_PAYLOAD_DIRECTORY}/{relative}"),
                    source_path: relative,
                    reason: reason.to_owned(),
                    bytes: metadata.len(),
                    blake3: actual_hash,
                    manifested: manifest_entry.is_some(),
                    manifest_source_path: manifest_entry.map(|entry| entry.source_path.clone()),
                    manifest_kind: manifest_entry.map(|entry| entry.kind),
                });
            }
        }
    }
    let other_archived_paths = archived
        .iter()
        .map(|entry| entry.source_path.clone())
        .collect::<BTreeSet<_>>();
    let proven_conversion_payload = proven_conversion_payload_proof(&manifest)
        .map(|proof| {
            plan_proven_conversion_payloads(
                &asset_root,
                &manifest,
                archive_history,
                &proof,
                &other_archived_paths,
            )
        })
        .transpose()?
        .unwrap_or_default();
    let ProvenConversionPayloadPlan {
        archived: proven_conversion_archived,
        already_archived_files: already_archived_proven_conversion_payload_files,
    } = proven_conversion_payload;
    archived.extend(proven_conversion_archived);
    let other_archived_paths = archived
        .iter()
        .map(|entry| entry.source_path.clone())
        .collect::<BTreeSet<_>>();
    let proven_offline_content_index = proven_offline_content_index_proof(&manifest)
        .map(|proof| {
            plan_proven_offline_content_index(
                &asset_root,
                &manifest,
                archive_history,
                &proof,
                &other_archived_paths,
            )
        })
        .transpose()?
        .unwrap_or_default();
    let ProvenOfflineContentIndexPlan {
        archived: proven_offline_content_index_archived,
        already_archived_files: already_archived_proven_offline_content_index_files,
    } = proven_offline_content_index;
    archived.extend(proven_offline_content_index_archived);
    let orphan_world = plan_proven_orphan_world_payloads(
        &asset_root,
        &manifest,
        &manifest_by_path,
        archive_history,
        &proven_orphan_world_payloads(&manifest),
    )?;
    archived.extend(orphan_world.archived);
    blockers.extend(orphan_world.blockers);
    archived.sort_by(|left, right| left.source_path.cmp(&right.source_path));
    deferred_world.sort_by(|left, right| left.source_path.cmp(&right.source_path));

    for entry in &manifest.files {
        let selected = match classify_metadata(&entry.path) {
            MetadataDisposition::Archive(_) => true,
            MetadataDisposition::TutorialStatic(_) => tutorial_static_gate.ready,
            MetadataDisposition::Keep | MetadataDisposition::DeferWorld(_) => false,
        };
        if selected && !physical_paths.contains(&entry.path) {
            return invalid(format!(
                "manifested conversion metadata is missing from disk: {:?}",
                entry.path
            ));
        }
    }

    if archived
        .iter()
        .any(|entry| entry.source_path.starts_with("characters/"))
    {
        let registry = CHARACTER_RUNTIME_REGISTRIES.iter().find(|relative| {
            manifested_file_is_valid(&asset_root, &manifest_by_path, relative).unwrap_or(false)
        });
        if registry.is_none() {
            blockers.push(format!(
                "character reports/catalogs cannot be archived until exactly one manifested \
                 runtime registry exists at one of: {}",
                CHARACTER_RUNTIME_REGISTRIES.join(", ")
            ));
        }
    }
    if archived
        .iter()
        .any(|entry| entry.source_path.starts_with("audio/"))
        && !manifested_file_is_valid(&asset_root, &manifest_by_path, AUDIO_RUNTIME_REGISTRY)?
    {
        blockers.push(format!(
            "audio catalog/report cannot be archived until manifested \
             {AUDIO_RUNTIME_REGISTRY} exists and matches its manifest identity"
        ));
    }

    let mut runtime_copies = Vec::new();
    if let Some(localization) = archived
        .iter()
        .find(|entry| entry.source_path == LEGACY_LOCALIZATION_CATALOG)
    {
        let runtime_path = asset_root.join(native_path(RUNTIME_LOCALIZATION_CATALOG)?);
        if runtime_path.exists() {
            if !manifested_file_is_valid(
                &asset_root,
                &manifest_by_path,
                RUNTIME_LOCALIZATION_CATALOG,
            )? {
                blockers.push(format!(
                    "{RUNTIME_LOCALIZATION_CATALOG} exists without one matching manifest entry"
                ));
            } else {
                let runtime_bytes =
                    fs::metadata(&runtime_path).map_err(|error| io_at(&runtime_path, error))?;
                let runtime_hash = hash_file(&runtime_path)?;
                if runtime_bytes.len() != localization.bytes || runtime_hash != localization.blake3
                {
                    blockers.push(format!(
                        "{RUNTIME_LOCALIZATION_CATALOG} differs from \
                         {LEGACY_LOCALIZATION_CATALOG}; refusing an ambiguous registry migration"
                    ));
                }
            }
        } else {
            runtime_copies.push(RuntimeMetadataRegistryCopy {
                source_path: LEGACY_LOCALIZATION_CATALOG.to_owned(),
                runtime_path: RUNTIME_LOCALIZATION_CATALOG.to_owned(),
                bytes: localization.bytes,
                blake3: localization.blake3.clone(),
            });
        }
    }

    let archived_paths = archived
        .iter()
        .map(|entry| entry.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let mut next_manifest = manifest.clone();
    next_manifest
        .files
        .retain(|entry| !archived_paths.contains(entry.path.as_str()));
    for copy in &runtime_copies {
        if next_manifest
            .files
            .iter()
            .all(|entry| entry.path != copy.runtime_path)
        {
            let archived_source = archived
                .iter()
                .find(|entry| entry.source_path == copy.source_path)
                .expect("runtime copy source is an archived entry");
            next_manifest.files.push(ProjectAssetFile {
                source_path: archived_source
                    .manifest_source_path
                    .clone()
                    .unwrap_or_else(|| copy.source_path.clone()),
                path: copy.runtime_path.clone(),
                kind: archived_source
                    .manifest_kind
                    .unwrap_or(ProjectAssetKind::Data),
                bytes: copy.bytes,
                blake3: copy.blake3.clone(),
            });
        }
    }
    next_manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));

    let primary_archive_exists = archive_root.exists();
    let (revision_archived, removal_only) = if primary_archive_exists {
        archived
            .iter()
            .cloned()
            .partition::<Vec<_>, _>(|entry| !archive_history.contains(entry))
    } else {
        (Vec::new(), Vec::new())
    };
    let revision_plan_blake3 = (!revision_archived.is_empty())
        .then(|| revision_plan_blake3(source_build, &revision_archived))
        .transpose()?;
    Ok(CleanupPlan {
        asset_root,
        archive_root,
        manifest_path,
        manifest,
        next_manifest,
        archived,
        revision_archived,
        removal_only,
        revision_plan_blake3,
        primary_archive_exists,
        runtime_copies,
        deferred_world,
        orphan_world_payload_roots: orphan_world.payload_roots,
        already_archived_orphan_world_payload_roots: orphan_world.already_archived_roots,
        already_archived_proven_conversion_payload_files,
        already_archived_proven_offline_content_index_files,
        blockers,
    })
}

pub(super) fn classify_metadata(relative: &str) -> MetadataDisposition {
    if !relative.ends_with(".json") {
        return MetadataDisposition::Keep;
    }
    if relative == "_runtime/world.json"
        || relative.contains("/_runtime/")
        || relative.starts_with("_runtime/")
        || relative == "characters/registry.json"
    {
        return MetadataDisposition::Keep;
    }
    if ACTIVE_RUNTIME_CATALOGS.contains(&relative) {
        return MetadataDisposition::Keep;
    }

    if relative.starts_with("world/") {
        let file_name = relative.rsplit('/').next().unwrap_or(relative);
        if relative == TUTORIAL_STATIC_INSTALL_MANIFEST
            || tutorial_static_tile_id(relative).is_some()
        {
            return MetadataDisposition::TutorialStatic(
                "tutorial static conversion metadata is superseded by the pinned merged scene",
            );
        }
        if relative == "world/catalog.json" {
            return MetadataDisposition::DeferWorld(
                "world source catalog remains mandatory until _runtime/world.json migration",
            );
        }
        if file_name == "provenance.json" {
            return MetadataDisposition::DeferWorld(
                "catalog provenance reference requires runtime-registry migration",
            );
        }
        if file_name == "manifest.json" {
            return MetadataDisposition::DeferWorld("per-tile conversion manifest");
        }
        if file_name == "scene-instance.json" {
            return MetadataDisposition::DeferWorld(
                "startup currently opens and verifies scene-instance metadata",
            );
        }
        if relative.contains("/components/") && file_name.ends_with(".parsed.json") {
            return MetadataDisposition::DeferWorld("parsed terrain component evidence");
        }
        if relative.contains("/environment/source/") && file_name.ends_with(".parsed.json") {
            return MetadataDisposition::DeferWorld(
                "environment JSON retains parsedData path/hash references",
            );
        }
        if relative.ends_with("/details/detail-database.raw.json") {
            return MetadataDisposition::DeferWorld(
                "terrain detailAndTrees.rawDocument reference must be stripped",
            );
        }
        if relative.ends_with("/details/trees.raw.json") {
            return MetadataDisposition::DeferWorld(
                "terrain detailAndTrees.trees.rawDocument reference must be stripped",
            );
        }
        return MetadataDisposition::Keep;
    }

    if matches!(
        relative,
        "ui/character/catalog.json" | "ui/gameplay/catalog.json"
    ) {
        return MetadataDisposition::Archive(
            "UI installer catalog; runtime uses manifest-owned layouts and images",
        );
    }

    if relative == "audio/catalog.json" {
        return MetadataDisposition::Archive("superseded audio import catalog");
    }
    if relative == "audio/voice/localization-report.json" {
        return MetadataDisposition::Archive("superseded voice localization report");
    }
    if relative == "characters/player/male/base/male_skeleton.json" {
        return MetadataDisposition::Archive("player rig source conversion contract");
    }
    if relative.starts_with("characters/") {
        let file_name = relative.rsplit('/').next().unwrap_or(relative);
        if file_name.ends_with(".publish.json") {
            return MetadataDisposition::Archive("logical-model publish evidence");
        }
        if file_name == "catalog.json"
            || file_name.ends_with("report.json")
            || file_name.contains("violations")
        {
            return MetadataDisposition::Archive("superseded character catalog/report");
        }
    }
    if matches!(relative, "icons/catalog.json" | "icons/unmatched.json") {
        return MetadataDisposition::Archive("icon import classification report");
    }
    if relative.starts_with("tutorial/models/") && relative.ends_with(".publish.json") {
        return MetadataDisposition::Archive("tutorial logical-model publish evidence");
    }
    if relative == "tutorial/models/catalog.json" {
        return MetadataDisposition::Archive(
            "tutorial model import catalog; runtime uses manifest-owned direct GLB routes",
        );
    }
    if relative.starts_with("data/catalog/") {
        let file_name = relative.rsplit('/').next().unwrap_or(relative);
        if file_name.ends_with("_report.json") || file_name == "asset-composition.json" {
            return MetadataDisposition::Archive("source content conversion report");
        }
        return MetadataDisposition::Keep;
    }
    if relative.starts_with("data/localization/") {
        return MetadataDisposition::Archive("source localization dump");
    }
    if relative.starts_with("data/fonts/") {
        return MetadataDisposition::Archive("source font index");
    }
    MetadataDisposition::Keep
}

pub(super) fn tutorial_static_tile_id(relative: &str) -> Option<&str> {
    let segments = relative.split('/').collect::<Vec<_>>();
    (segments.len() == 6
        && segments[..4] == ["world", "tutorial", "static", "tiles"]
        && matches!(
            segments[5],
            "catalog.json" | "hierarchy.json" | "materials.json"
        ))
    .then(|| segments[4])
}

pub(super) fn tutorial_tile_coordinates(tile_id: &str) -> Option<[i32; 2]> {
    let coordinates = tile_id.strip_prefix("tile_")?;
    let (x, y) = coordinates.split_once('_')?;
    if x.len() != 2 || y.len() != 2 {
        return None;
    }
    Some([x.parse().ok()?, y.parse().ok()?])
}
