use super::*;

pub fn export_native_terrains_batch(
    input: impl AsRef<Path>,
    output_root: impl AsRef<Path>,
    options: NativeTerrainBatchOptions,
) -> Result<NativeTerrainBatchManifest, String> {
    let input = input.as_ref();
    let output_root = output_root.as_ref();
    let (build_root, input_kind) = resolve_effective_build_root(input)?;
    reject_existing_output(output_root)?;
    let preflight = preflight_build(&build_root, &options)?;

    let parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "could not create batch output parent {}: {err}",
            parent.display()
        )
    })?;
    let nonce = unique_nonce()?;
    let output_name = output_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("invalid batch output root: {}", output_root.display()))?;
    let staging_path = parent.join(format!(
        ".{output_name}.native-terrain-staging-{}-{nonce:x}",
        std::process::id()
    ));
    let work_path = parent.join(format!(
        ".{output_name}.native-terrain-work-{}-{nonce:x}",
        std::process::id()
    ));
    for path in [output_root, staging_path.as_path(), work_path.as_path()] {
        if path.exists() {
            return Err(format!(
                "refusing existing native terrain batch path: {}",
                path.display()
            ));
        }
    }
    fs::create_dir(&staging_path)
        .map_err(|err| format!("could not create {}: {err}", staging_path.display()))?;
    let mut staging = SessionDirectory::new(staging_path);
    fs::create_dir(&work_path)
        .map_err(|err| format!("could not create {}: {err}", work_path.display()))?;
    let work = SessionDirectory::new(work_path);

    // This is the expensive global lookup. Building it once makes duplicate
    // dependency CABs across 100+ tiles deterministic and avoids rescanning
    // every bundle for every TerrainData.
    let archive_index = build_archive_index(&build_root);
    let repo_root = std::env::current_dir().map_err(|err| err.to_string())?;

    let mut blocked = preflight.blocked;
    let mut exported = Vec::with_capacity(preflight.candidates.len());
    let outcomes = process_tiles_parallel(
        preflight.candidates.clone(),
        &build_root,
        &repo_root,
        &archive_index,
        staging.path(),
        work.path(),
    )?;
    for outcome in outcomes {
        if let Some(message) = outcome.fatal {
            return Err(message);
        }
        if let Some(value) = outcome.exported {
            exported.push(value);
        }
        blocked.extend(outcome.blocked);
    }

    blocked.sort_by(|left, right| {
        left.tile_id
            .cmp(&right.tile_id)
            .then_with(|| left.stage.cmp(right.stage))
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    exported.sort_by(|left, right| {
        left.scope
            .cmp(right.scope)
            .then_with(|| left.tile_id.cmp(&right.tile_id))
    });
    let status = if blocked.is_empty() {
        "complete"
    } else {
        "complete-with-blocked"
    };
    let manifest = NativeTerrainBatchManifest {
        schema: BATCH_SCHEMA,
        status,
        scope: BatchScope {
            mode: if options.tile_filter.is_empty() {
                "all"
            } else {
                "selected"
            },
            requested_tiles: options.tile_filter.iter().cloned().collect(),
        },
        source: BatchSource {
            input_path: canonical_string(input)?,
            input_kind,
            effective_build_root: canonical_string(&build_root)?,
        },
        output_root: output_root.to_string_lossy().to_string(),
        counts: BatchCounts {
            discovered_dong_resource_count: preflight.discovered_count,
            discovered_tutorial_terrain_count: preflight.discovered_tutorial_count,
            selected_tile_count: preflight.selected_count,
            selected_world_tile_count: preflight.selected_world_count,
            selected_tutorial_terrain_count: preflight.selected_tutorial_count,
            preflight_candidate_count: preflight.candidates.len(),
            exported_count: exported.len(),
            blocked_count: blocked.len(),
        },
        exported,
        blocked,
        publication_plan_document: "publication-plan.json",
    };
    let publication_plan = build_publication_plan(
        &manifest,
        staging.path(),
        output_root.to_string_lossy().as_ref(),
    )?;
    write_json_new(
        &staging.path().join("publication-plan.json"),
        &publication_plan,
    )?;
    write_json_new(&staging.path().join("manifest.json"), &manifest)?;

    reject_existing_output(output_root)?;
    fs::rename(staging.path(), output_root).map_err(|err| {
        format!(
            "could not atomically commit native terrain batch {} -> {}: {err}",
            staging.path().display(),
            output_root.display()
        )
    })?;
    staging.keep();
    drop(work);
    Ok(manifest)
}

pub(super) fn write_json_new(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|err| format!("could not serialize {}: {err}", path.display()))?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| format!("could not create {}: {err}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not sync {}: {err}", path.display()))
}

pub(super) fn write_bytes_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| format!("could not create {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not sync {}: {err}", path.display()))
}
