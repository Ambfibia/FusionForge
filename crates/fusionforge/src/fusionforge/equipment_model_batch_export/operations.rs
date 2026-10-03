use super::*;

pub(super) fn destination_key(route: &StagedRoute) -> String {
    portable_key(&slash_path(&source_relative_path(route)))
}

pub(super) fn source_stage_blocker(candidate: &RouteCandidate, detail: String) -> EquipmentModelSourceBlocker {
    EquipmentModelSourceBlocker {
        code: "equipmentSourceStageExportFailed".to_string(),
        category: candidate.route.category.clone(),
        exact_route: candidate.route.exact_route.clone(),
        normalized_route: candidate.route.normalized_route.clone(),
        table_rows: candidate.route.table_rows.clone(),
        owners: vec![candidate.owner.clone()],
        detail,
        evidence: json!({
            "exporter": "preview_bundle_container_model_exact",
            "fallbackApplied": false,
        }),
        required_evidence: vec![
            "a warning-free exact serialized GameObject/material/animation closure".to_string(),
        ],
        disposition: "blocked-no-lossy-source-document".to_string(),
    }
}

pub(super) fn staging_paths(output: &Path, manifest: &Path) -> Result<(PathBuf, PathBuf), String> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let output_name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "equipment output has no directory name".to_string())?;
    let manifest_name = manifest
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "equipment manifest has no filename".to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let suffix = format!("{}.{}", std::process::id(), nonce);
    Ok((
        parent.join(format!(".{output_name}.equipment-stage.{suffix}")),
        parent.join(format!(".{manifest_name}.equipment-stage.{suffix}")),
    ))
}

pub(super) fn sort_blockers(blockers: &mut [EquipmentModelSourceBlocker]) {
    blockers.sort_by(|left, right| {
        left.category
            .cmp(&right.category)
            .then_with(|| left.normalized_route.cmp(&right.normalized_route))
            .then_with(|| left.code.cmp(&right.code))
    });
}

pub(super) fn remove_staging(root: &Path, manifest: &Path) {
    let _ = fs::remove_file(manifest);
    let _ = fs::remove_dir_all(root);
}

pub(super) fn rename_with_transient_permission_retry(
    source: &Path,
    destination: &Path,
) -> std::io::Result<()> {
    const DELAYS: [u64; 6] = [25, 50, 100, 200, 400, 800];
    for delay in DELAYS {
        match fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                thread::sleep(Duration::from_millis(delay));
            }
            Err(err) => return Err(err),
        }
    }
    fs::rename(source, destination)
}

pub(super) fn required_owned_string(value: &JsonValue, field: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("proof field {field:?} is not a string"))
}

pub(super) fn required_u64(value: &JsonValue, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| format!("proof field {field:?} is not u64"))
}

pub(super) fn required_i64(value: &JsonValue, field: &str) -> Result<i64, String> {
    value
        .get(field)
        .and_then(JsonValue::as_i64)
        .ok_or_else(|| format!("proof field {field:?} is not i64"))
}

pub(super) fn u64_count(value: usize) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| "equipment count exceeds u64".to_string())
}

pub(super) fn elapsed_milliseconds(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(super) fn timing_summary(routes: &[EquipmentRouteTiming]) -> EquipmentTimingSummary {
    let mut samples = routes
        .iter()
        .map(|timing| timing.milliseconds)
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let routes = samples.len();
    let median_index = routes.saturating_sub(1) / 2;
    let p95_index = routes
        .saturating_mul(95)
        .saturating_add(99)
        .checked_div(100)
        .unwrap_or_default()
        .saturating_sub(1)
        .min(routes.saturating_sub(1));
    EquipmentTimingSummary {
        routes: u64::try_from(routes).unwrap_or(u64::MAX),
        median_milliseconds: samples.get(median_index).copied().unwrap_or_default(),
        p95_milliseconds: samples.get(p95_index).copied().unwrap_or_default(),
        min_milliseconds: samples.first().copied().unwrap_or_default(),
        max_milliseconds: samples.last().copied().unwrap_or_default(),
        total_milliseconds: samples.iter().copied().sum(),
    }
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn portable_key(value: &str) -> String {
    value.nfkc().flat_map(char::to_lowercase).nfkc().collect()
}
