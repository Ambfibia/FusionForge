use super::*;

pub(super) fn output_reference_name(
    reference: &AssetRef,
    output_objects: &BTreeMap<String, BTreeSet<i64>>,
) -> Result<Option<String>, String> {
    let candidates = [&reference.file_path, &reference.asset_path]
        .into_iter()
        .map(|value| internal_ref_name(value))
        .filter(|value| !value.is_empty() && output_objects.contains_key(value))
        .collect::<BTreeSet<_>>();
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.into_iter().next()),
        _ => Err(format!(
            "AssetRef resolves to multiple staged outputs: filePath='{}', assetPath='{}'",
            reference.file_path, reference.asset_path
        )),
    }
}

/// The only source-unregistered roots with an explicit legacy load policy are imported
/// NPC/HNPC bundles and the portrait routes generated alongside them. Every source bundle
/// keeps its exact CachingManifest phase; all other unpinned dependencies inherit phases
/// solely from their actual consumers.
pub(super) fn custom_root_sections(family: &Family, cfg: &LegacyConfig) -> BTreeSet<String> {
    if !matches!(family, Family::Npc | Family::Hnpc | Family::Icons) {
        return BTreeSet::new();
    }
    let mut sections = BTreeSet::new();
    if cfg.preload_npc_bundles {
        sections.insert("m_CharacterCreation".to_string());
    }
    if cfg.load_npc_bundles_in_world {
        sections.extend(cfg.npc_bundle_manifest_sections.iter().cloned());
    }
    sections
}

pub(super) fn disabled_plan() -> LegacyLayoutPlan {
    LegacyLayoutPlan {
        output_bundles: Vec::new(),
        routeable_bundles: Vec::new(),
        retired_bundles: Vec::new(),
        manifest_sections: Vec::new(),
        rewritten_retained_bundles: Vec::new(),
        dependencies: Vec::new(),
        source_objects: 0,
        output_objects: 0,
        exact_objects_deduplicated: 0,
        same_name_different_content: 0,
        unresolved_pointers_preserved: 0,
        unreadable_orphans_removed: Vec::new(),
        dangling_preloads_removed: 0,
        dangling_preload_examples: Vec::new(),
        dangling_object_pointers_cleared: 0,
        dangling_object_pointer_examples: Vec::new(),
        translated_audio_pairs_preserved: 0,
        patched_audio_entries_preserved: 0,
        tile_scoped_audio_exceptions: Vec::new(),
        tile_scoped_route_exceptions: Vec::new(),
        route_conflicts: Vec::new(),
        unclassified_roots: Vec::new(),
        oversized_parts: Vec::new(),
    }
}

pub(super) fn sha256_file(path: &Path) -> Result<(u64, String), String> {
    let mut file = fs::File::open(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes = bytes.saturating_add(read as u64);
        hasher.update(&buffer[..read]);
    }
    Ok((bytes, format!("{:x}", hasher.finalize())))
}

pub(super) fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn result_cache_key(
    patch_config: &JsonValue,
    stable_input_digest: &str,
) -> Result<String, String> {
    let stable_input_digest = stable_input_digest.trim().to_ascii_lowercase();
    if stable_input_digest.len() != 64
        || !stable_input_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!(
            "layout cache key received an invalid stable input SHA-256: {stable_input_digest}"
        ));
    }

    let mut hasher = Sha256::new();
    update_sha256_field(&mut hasher, LAYOUT_CACHE_FORMAT.as_bytes());
    update_sha256_field(&mut hasher, b"ffclient.legacy-layout-key.v2");
    update_sha256_field(&mut hasher, env!("CARGO_PKG_VERSION").as_bytes());
    // Invalidate even when the package version was not bumped. These cover the layout,
    // semantic classifier, binary writer and orchestration code used by a cached result.
    for source in [
        include_bytes!("../legacy_bundle_layout.rs").as_slice(),
        include_bytes!("../legacy_semantic_index.rs").as_slice(),
        include_bytes!("../fusionforge/unity.rs").as_slice(),
        include_bytes!("../lib.rs").as_slice(),
    ] {
        update_sha256_field(&mut hasher, &Sha256::digest(source));
    }
    let normalized_config = serde_json::json!({
        "bundleLayout": patch_config.get("BundleLayout").cloned().unwrap_or(JsonValue::Null),
        "preloadNpcBundles": patch_config.get("PreloadNpcBundles").cloned().unwrap_or(JsonValue::Null),
        "loadNpcBundlesInWorld": patch_config.get("LoadNpcBundlesInWorld").cloned().unwrap_or(JsonValue::Null),
        "npcBundleManifestSections": patch_config.get("NpcBundleManifestSections").cloned().unwrap_or(JsonValue::Null),
    });
    update_sha256_field(
        &mut hasher,
        &serde_json::to_vec(&normalized_config).map_err(|err| err.to_string())?,
    );
    update_sha256_field(&mut hasher, stable_input_digest.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn layout_cache_parent(project: &Path) -> PathBuf {
    project.join("cache")
}

pub(super) fn layout_cache_current(project: &Path) -> PathBuf {
    layout_cache_parent(project).join("legacy-layout-result")
}

pub(super) fn safe_layout_cache_name(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name).file_name().and_then(|value| value.to_str()) == Some(name)
        && (name.to_ascii_lowercase().ends_with(".resourcefile")
            || name.to_ascii_lowercase().ends_with(".unity3d"))
}

pub(super) fn remove_layout_cache_dir(path: &Path, parent: &Path) -> Result<(), String> {
    if path.parent() != Some(parent) {
        return Err(format!(
            "refusing to remove layout cache outside {}: {}",
            parent.display(),
            path.display()
        ));
    }
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|err| format!("{}: {err}", path.display()))?;
    } else if path.exists() {
        return Err(format!(
            "layout cache path is not a directory: {}",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn cleanup_stale_layout_cache_dirs(parent: &Path) -> Result<(), String> {
    if !parent.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(parent).map_err(|err| format!("{}: {err}", parent.display()))? {
        let entry = entry.map_err(|err| format!("{}: {err}", parent.display()))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(".legacy-layout-result-")
            || !(name.ends_with(".pending") || name.ends_with(".old"))
            || !entry
                .file_type()
                .map_err(|err| format!("{}: {err}", entry.path().display()))?
                .is_dir()
        {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("{}: {err}", entry.path().display()))?;
        let stale = modified
            .elapsed()
            .map(|age| age.as_secs() >= LAYOUT_CACHE_STALE_SECONDS)
            .unwrap_or(false);
        if stale {
            remove_layout_cache_dir(&entry.path(), parent)?;
        }
    }
    Ok(())
}

pub(super) fn cached_plan_file_names(plan: &LegacyLayoutPlan) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for name in plan
        .output_bundles
        .iter()
        .chain(plan.rewritten_retained_bundles.iter())
    {
        if !safe_layout_cache_name(name) {
            return Err(format!("unsafe filename in cached layout plan: {name}"));
        }
        let lower = name.to_ascii_lowercase();
        if let Some(existing) = result.insert(lower, name.clone()) {
            return Err(format!(
                "duplicate filename in cached layout plan: {existing}, {name}"
            ));
        }
    }
    Ok(result)
}

pub(super) fn try_restore_layout_cache(
    project: &Path,
    out_dir: &Path,
    patch_config: &JsonValue,
    key: &str,
) -> Result<Option<LegacyLayoutPlan>, String> {
    let current = layout_cache_current(project);
    let receipt_path = current.join("receipt.json");
    if !receipt_path.is_file() {
        return Ok(None);
    }
    let receipt = serde_json::from_str::<LayoutCacheReceipt>(
        &fs::read_to_string(&receipt_path)
            .map_err(|err| format!("{}: {err}", receipt_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", receipt_path.display()))?;
    if receipt.format != LAYOUT_CACHE_FORMAT || !receipt.complete || receipt.key != key {
        return Ok(None);
    }
    let plan_path = current.join("plan.json");
    let report_path = current.join("report.json");
    let plan_data =
        fs::read(&plan_path).map_err(|err| format!("{}: {err}", plan_path.display()))?;
    let report_data =
        fs::read(&report_path).map_err(|err| format!("{}: {err}", report_path.display()))?;
    if sha256_bytes(&plan_data) != receipt.plan_sha256
        || sha256_bytes(&report_data) != receipt.report_sha256
    {
        return Err("layout result cache metadata hash mismatch".to_string());
    }
    let plan = serde_json::from_slice::<LegacyLayoutPlan>(&plan_data)
        .map_err(|err| format!("{}: {err}", plan_path.display()))?;
    let expected = cached_plan_file_names(&plan)?;
    let mut receipt_files = BTreeMap::<String, &LayoutCacheFile>::new();
    for file in &receipt.files {
        if !safe_layout_cache_name(&file.name) {
            return Err(format!(
                "unsafe filename in layout cache receipt: {}",
                file.name
            ));
        }
        if receipt_files
            .insert(file.name.to_ascii_lowercase(), file)
            .is_some()
        {
            return Err(format!(
                "duplicate filename in layout cache receipt: {}",
                file.name
            ));
        }
    }
    if expected.keys().collect::<Vec<_>>() != receipt_files.keys().collect::<Vec<_>>() {
        return Err("layout cache payload set does not match cached plan".to_string());
    }
    for (lower, expected_name) in &expected {
        let received_name = &receipt_files
            .get(lower)
            .expect("key sets were compared above")
            .name;
        if received_name != expected_name {
            return Err(format!(
                "layout cache filename case mismatch: expected {expected_name}, got {received_name}"
            ));
        }
    }
    let files_dir = current.join("files");
    let mut actual = BTreeSet::new();
    for entry in
        fs::read_dir(&files_dir).map_err(|err| format!("{}: {err}", files_dir.display()))?
    {
        let entry = entry.map_err(|err| format!("{}: {err}", files_dir.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("{}: {err}", entry.path().display()))?;
        if !file_type.is_file() {
            return Err(format!(
                "layout cache files directory contains a non-file entry: {}",
                entry.path().display()
            ));
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "layout cache payload filename is not UTF-8".to_string())?;
        if !actual.insert(name.to_ascii_lowercase()) {
            return Err(format!(
                "layout cache files directory contains a case-insensitive duplicate: {name}"
            ));
        }
    }
    let expected_actual = receipt_files.keys().cloned().collect::<BTreeSet<_>>();
    if actual != expected_actual {
        return Err("layout cache files directory has a missing or extra payload".to_string());
    }

    let mut staged = Vec::new();
    for file in receipt_files.values() {
        let path = files_dir.join(&file.name);
        let (bytes, digest) = sha256_file(&path)?;
        if bytes != file.bytes || digest != file.sha256 {
            return Err(format!("layout cache payload hash mismatch: {}", file.name));
        }
        staged.push((file.name.clone(), path));
    }
    let report_text = String::from_utf8(report_data)
        .map_err(|err| format!("cached layout report is not UTF-8: {err}"))?;
    transactional_publish(out_dir, &staged)?;
    if let Err(err) = publish_layout_report(project, patch_config, report_text.trim_end()) {
        // The runtime payload is already published transactionally and fully hash-checked.
        // Do not fall through into a cold repack over those post-layout bundles merely
        // because a diagnostic report path is temporarily unwritable.
        eprintln!("[ffclient:layout] cached payload restored; report update skipped: {err}");
    }
    eprintln!(
        "[ffclient:layout] cache hit: restored {} validated output bundle(s)",
        staged.len()
    );
    Ok(Some(plan))
}

pub(super) fn store_layout_cache(
    project: &Path,
    key: &str,
    plan: &LegacyLayoutPlan,
    report_data: &str,
    staged: &[(String, PathBuf)],
) -> Result<(), String> {
    let parent = layout_cache_parent(project);
    fs::create_dir_all(&parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    if let Err(err) = cleanup_stale_layout_cache_dirs(&parent) {
        eprintln!("[ffclient:layout] stale cache cleanup skipped: {err}");
    }
    let current = layout_cache_current(project);
    let sequence = LAYOUT_CACHE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let pending = parent.join(format!(
        ".legacy-layout-result-{}-{sequence}.pending",
        process::id()
    ));
    let backup = parent.join(format!(
        ".legacy-layout-result-{}-{sequence}.old",
        process::id()
    ));
    remove_layout_cache_dir(&pending, &parent)?;
    remove_layout_cache_dir(&backup, &parent)?;
    let _write_guard = LayoutCacheWriteGuard {
        parent: parent.clone(),
        pending: pending.clone(),
        backup: backup.clone(),
    };
    let files_dir = pending.join("files");
    fs::create_dir_all(&files_dir).map_err(|err| format!("{}: {err}", files_dir.display()))?;

    let expected = cached_plan_file_names(plan)?;
    let mut receipt_files = Vec::new();
    let mut staged_names = BTreeSet::new();
    for (name, source) in staged {
        if !safe_layout_cache_name(name) || !source.is_file() {
            return Err(format!("invalid staged layout cache payload: {name}"));
        }
        let lower = name.to_ascii_lowercase();
        if !staged_names.insert(lower) {
            return Err(format!("duplicate staged layout cache payload: {name}"));
        }
        let target = files_dir.join(name);
        fs::copy(source, &target)
            .map_err(|err| format!("{} -> {}: {err}", source.display(), target.display()))?;
        let (bytes, sha256) = sha256_file(&target)?;
        receipt_files.push(LayoutCacheFile {
            name: name.clone(),
            bytes,
            sha256,
        });
    }
    if expected.keys().cloned().collect::<BTreeSet<_>>() != staged_names {
        return Err("staged layout cache payload set does not match plan".to_string());
    }
    receipt_files.sort_by_key(|file| file.name.to_ascii_lowercase());
    let plan_data = serde_json::to_vec_pretty(plan).map_err(|err| err.to_string())?;
    let report_bytes = report_data.as_bytes();
    fs::write(pending.join("plan.json"), &plan_data)
        .map_err(|err| format!("{}: {err}", pending.join("plan.json").display()))?;
    fs::write(pending.join("report.json"), report_bytes)
        .map_err(|err| format!("{}: {err}", pending.join("report.json").display()))?;
    let receipt = LayoutCacheReceipt {
        format: LAYOUT_CACHE_FORMAT.to_string(),
        key: key.to_string(),
        complete: true,
        plan_sha256: sha256_bytes(&plan_data),
        report_sha256: sha256_bytes(report_bytes),
        files: receipt_files,
    };
    let receipt_data = serde_json::to_vec_pretty(&receipt).map_err(|err| err.to_string())?;
    fs::write(pending.join("receipt.json"), receipt_data)
        .map_err(|err| format!("{}: {err}", pending.join("receipt.json").display()))?;

    if current.is_dir() {
        fs::rename(&current, &backup)
            .map_err(|err| format!("{} -> {}: {err}", current.display(), backup.display()))?;
    }
    if let Err(err) = fs::rename(&pending, &current) {
        if backup.is_dir() {
            let _ = fs::rename(&backup, &current);
        }
        return Err(format!(
            "{} -> {}: {err}",
            pending.display(),
            current.display()
        ));
    }
    remove_layout_cache_dir(&backup, &parent)?;
    eprintln!(
        "[ffclient:layout] cached {} validated output bundle(s) for the next identical build",
        staged.len()
    );
    Ok(())
}

/// Build, validate and atomically publish the complete semantic layout. Input bundles that
/// are no longer output names remain on disk until [`retire_inputs`] is called after managed
/// AssetLoader/ResourceLocator/CachingManifest patches have succeeded.
pub(crate) fn restore_result_cache(
    project: &Path,
    out_dir: &Path,
    patch_config: &JsonValue,
    stable_input_digest: &str,
) -> Result<Option<LegacyLayoutPlan>, String> {
    if !config(patch_config).enabled {
        return Ok(None);
    }
    let key = match result_cache_key(patch_config, stable_input_digest) {
        Ok(key) => key,
        Err(err) => {
            eprintln!("[ffclient:layout] result cache disabled for this build: {err}");
            return Ok(None);
        }
    };
    eprintln!("[ffclient:layout] stable result cache key: {key}");
    match try_restore_layout_cache(project, out_dir, patch_config, &key) {
        Ok(plan) => Ok(plan),
        Err(err) => {
            // The cache is only an optimization. Never let a partial/corrupt previous
            // write make a clean source build unusable.
            eprintln!("[ffclient:layout] ignoring invalid result cache: {err}");
            Ok(None)
        }
    }
}

/// Retire obsolete inputs only after the caller has successfully patched and repacked main.
/// Rename-first makes failure rollback possible and avoids a half-deleted build.
pub(crate) fn retire_inputs(out_dir: &Path, plan: &LegacyLayoutPlan) -> Result<(), String> {
    let mut renamed = Vec::<(PathBuf, PathBuf)>::new();
    for name in &plan.retired_bundles {
        let source = out_dir.join(name);
        let backup = out_dir.join(format!(".{name}.fflayout-retired"));
        if backup.is_file() {
            if source.is_file() {
                return Err(format!(
                    "retirement source and recovery file both exist: {} and {}",
                    source.display(),
                    backup.display()
                ));
            }
            fs::rename(&backup, &source)
                .map_err(|err| format!("{} -> {}: {err}", backup.display(), source.display()))?;
        }
        if !source.is_file() {
            continue;
        }
        if let Err(err) = fs::rename(&source, &backup) {
            for (original, staged) in renamed.iter().rev() {
                let _ = fs::rename(staged, original);
            }
            return Err(format!(
                "retirement rolled back: {} -> {}: {err}",
                source.display(),
                backup.display()
            ));
        }
        renamed.push((source, backup));
    }
    for (_, backup) in renamed {
        fs::remove_file(&backup).map_err(|err| format!("{}: {err}", backup.display()))?;
    }
    Ok(())
}
