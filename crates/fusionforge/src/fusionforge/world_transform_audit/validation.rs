use super::*;

pub const WORLD_TRANSFORM_AUDIT_SCHEMA: &str = "ffone.world-transform-contract-audit.v5";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldTransformAuditReport {
    pub schema: &'static str,
    pub source_build: String,
    pub coordinate_space: &'static str,
    pub coordinate_contract: JsonValue,
    pub collider_contract: ColliderCoordinateContract,
    pub gameplay_facing_rotation_applications: usize,
    pub passed: bool,
    pub counts: WorldCatalogAuditCounts,
    pub errors: Vec<WorldAuditError>,
    pub maps: Vec<MapTransformAudit>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldCatalogAuditCounts {
    pub map_archives: usize,
    pub processed_map_archives: usize,
    pub passed_map_archives: usize,
    pub failed_map_archives: usize,
    pub dependency_bundles: usize,
    pub missing_dependencies: usize,
    pub ambiguous_dependency_archives: usize,
    pub scene_assets: usize,
    pub scene_nodes: usize,
    pub root_nodes: usize,
    pub unit_scale_nodes: usize,
    pub nonunit_scale_nodes: usize,
    pub uniform_scale_nodes: usize,
    pub nonuniform_scale_nodes: usize,
    pub positive_scale_nodes: usize,
    pub negative_scale_nodes: usize,
    pub zero_scale_nodes: usize,
    pub singular_scale_nodes: usize,
    pub nonfinite_nodes: usize,
    pub invalid_rotation_nodes: usize,
    pub nonzero_root_origins: usize,
    pub root_origin_contract_mismatches: usize,
    pub colliders: usize,
    pub collider_centers: usize,
    pub collider_sizes: usize,
    pub collider_radii: usize,
    pub collider_heights: usize,
    pub collider_types: BTreeMap<String, usize>,
    pub invalid_colliders: usize,
    pub transform_exceptions: usize,
    pub collider_exceptions: usize,
    pub errors: usize,
    pub max_abs_scale_component: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapTransformAuditCounts {
    pub dependency_bundles: usize,
    pub missing_dependencies: usize,
    pub ambiguous_dependency_archives: usize,
    pub scene_assets: usize,
    pub scene_nodes: usize,
    pub root_nodes: usize,
    pub unit_scale_nodes: usize,
    pub nonunit_scale_nodes: usize,
    pub uniform_scale_nodes: usize,
    pub nonuniform_scale_nodes: usize,
    pub positive_scale_nodes: usize,
    pub negative_scale_nodes: usize,
    pub zero_scale_nodes: usize,
    pub singular_scale_nodes: usize,
    pub nonfinite_nodes: usize,
    pub invalid_rotation_nodes: usize,
    pub nonzero_root_origins: usize,
    pub root_origin_contract_mismatches: usize,
    pub colliders: usize,
    pub collider_centers: usize,
    pub collider_sizes: usize,
    pub collider_radii: usize,
    pub collider_heights: usize,
    pub collider_types: BTreeMap<String, usize>,
    pub invalid_colliders: usize,
    pub transform_exceptions: usize,
    pub collider_exceptions: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapTransformAudit {
    pub map: String,
    pub bundle_path: String,
    pub bundle_blake3: String,
    pub passed: bool,
    pub scene_assets: Vec<String>,
    pub declared_dependency_archives: Vec<String>,
    pub resolved_dependency_bundles: Vec<DependencyBundleProof>,
    pub missing_dependencies: Vec<String>,
    pub ambiguous_dependency_archives: Vec<AmbiguousDependencyArchive>,
    pub counts: MapTransformAuditCounts,
    pub max_abs_scale: [f64; 3],
    pub max_abs_scale_component: f64,
    pub expected_native_tile_horizontal_origin: Option<[f64; 2]>,
    pub root_origin_matches_tile_contract: bool,
    pub root_origins: Vec<RootOrigin>,
    pub transform_exceptions: Vec<TransformException>,
    pub collider_exceptions: Vec<ColliderException>,
    pub errors: Vec<WorldAuditError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldAuditError {
    pub map: Option<String>,
    pub code: String,
    pub detail: String,
}

impl WorldAuditError {
    pub(super) fn new(map: Option<&str>, code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            map: map.map(str::to_string),
            code: code.into(),
            detail: detail.into(),
        }
    }
}

pub(super) struct AuditSession {
    pub(super) root: PathBuf,
}

impl AuditSession {
    pub(super) fn create() -> Result<Self, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock error: {err}"))?
            .as_nanos();
        for attempt in 0..64_u32 {
            let root = std::env::temp_dir().join(format!(
                "ffone-world-transform-audit-{}-{nanos}-{attempt}",
                process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Ok(Self { root }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(format!(
                        "could not create world audit session {}: {err}",
                        root.display()
                    ));
                }
            }
        }
        Err("could not allocate a unique world-transform audit session".to_string())
    }
}

impl Drop for AuditSession {
    fn drop(&mut self) {
        if is_owned_audit_temp_dir(&self.root) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

pub(super) fn is_owned_audit_temp_dir(path: &Path) -> bool {
    let has_owned_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with("ffone-world-transform-audit-"));
    if !has_owned_name || !path.is_dir() {
        return false;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(parent) = fs::canonicalize(parent) else {
        return false;
    };
    let Ok(temp_root) = fs::canonicalize(std::env::temp_dir()) else {
        return false;
    };
    parent == temp_root
}

/// Audit every physical `Map_*.unity3d` directly contained by `build_root`.
/// The report is created exclusively: an existing report is never overwritten.
pub fn audit_world_transform_contract(
    build_root: impl AsRef<Path>,
    report_path: impl AsRef<Path>,
) -> Result<WorldTransformAuditReport, String> {
    let build_root = build_root.as_ref();
    let report_path = report_path.as_ref();
    if !build_root.is_dir() {
        return Err(format!(
            "world audit build root is not a directory: {}",
            build_root.display()
        ));
    }
    if report_path.exists() {
        return Err(format!(
            "refusing to overwrite immutable world audit report: {}",
            report_path.display()
        ));
    }

    let mut maps = fs::read_dir(build_root)
        .map_err(|err| format!("could not read build root {}: {err}", build_root.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_map_bundle(path))
        .collect::<Vec<_>>();
    maps.sort_by(|left, right| file_name(left).cmp(&file_name(right)));
    if maps.is_empty() {
        return Err(format!(
            "build root contains no Map_*.unity3d archives: {}",
            build_root.display()
        ));
    }

    let repo_root = crate::repository_root().to_path_buf()
        .parent()
        .ok_or_else(|| "could not resolve FusionFallProject root".to_string())?
        .to_path_buf();
    let session = AuditSession::create()?;
    // Resolving archive names requires inspecting the build catalog. Build it
    // once, then give each read-only worker a cheap clone so extraction and
    // serialized-object parsing can proceed independently per Map session.
    let shared_archive_candidates = build_archive_candidate_index(build_root);
    let shared_archive_index = shared_archive_candidates
        .iter()
        .filter_map(|(archive, candidates)| {
            candidates
                .first()
                .cloned()
                .map(|candidate| (archive.clone(), candidate))
        })
        .collect::<HashMap<_, _>>();
    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(1, 8)
        .min(maps.len());
    let mut map_reports = std::iter::repeat_with(|| None)
        .take(maps.len())
        .collect::<Vec<Option<MapTransformAudit>>>();
    std::thread::scope(|scope| -> Result<(), String> {
        let mut handles = Vec::with_capacity(worker_count);
        let maps = &maps;
        let session_root = &session.root;
        let repo_root = &repo_root;
        for worker in 0..worker_count {
            let shared_archive_index = shared_archive_index.clone();
            let shared_archive_candidates = shared_archive_candidates.clone();
            let handle = std::thread::Builder::new()
                .name(format!("world-transform-audit-{worker}"))
                .stack_size(64 * 1024 * 1024)
                .spawn_scoped(scope, move || {
                    let mut archive_indexes =
                        HashMap::from([(build_root.to_path_buf(), shared_archive_index)]);
                    let mut worker_reports = Vec::new();
                    for index in (worker..maps.len()).step_by(worker_count) {
                        let map_session = session_root.join(format!("map-{index:04}"));
                        let mut report = match fs::create_dir(&map_session) {
                            Ok(()) => audit_map(
                                &repo_root,
                                build_root,
                                &maps[index],
                                &map_session,
                                &mut archive_indexes,
                                &shared_archive_candidates,
                            ),
                            Err(err) => failed_map_session_report(&maps[index], &map_session, err),
                        };
                        if map_session.is_dir() {
                            if let Err(detail) = remove_owned_map_session(&map_session) {
                                report.errors.push(WorldAuditError::new(
                                    Some(&report.map),
                                    "auditSessionCleanupFailed",
                                    detail,
                                ));
                                report.counts.errors = report.errors.len();
                                report.passed = false;
                            }
                        }
                        worker_reports.push((index, report));
                    }
                    worker_reports
                })
                .map_err(|err| format!("could not spawn world audit worker {worker}: {err}"))?;
            handles.push(handle);
        }
        for handle in handles {
            let worker_reports = handle
                .join()
                .map_err(|_| "world transform audit worker panicked".to_string())?;
            for (index, report) in worker_reports {
                map_reports[index] = Some(report);
            }
        }
        Ok(())
    })?;
    let map_reports = map_reports
        .into_iter()
        .enumerate()
        .map(|(index, report)| {
            report.ok_or_else(|| format!("world audit worker omitted {}", maps[index].display()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut report = WorldTransformAuditReport {
        schema: WORLD_TRANSFORM_AUDIT_SCHEMA,
        source_build: readable_absolute_path(build_root),
        coordinate_space: "native",
        coordinate_contract: native_coordinate_contract_json(),
        collider_contract: ColliderCoordinateContract::default(),
        gameplay_facing_rotation_applications: 0,
        passed: false,
        counts: aggregate_counts(&map_reports),
        errors: map_reports
            .iter()
            .flat_map(|map| map.errors.iter().cloned())
            .collect(),
        maps: map_reports,
    };
    report.passed = report.errors.is_empty()
        && report.counts.failed_map_archives == 0
        && report.counts.missing_dependencies == 0
        && report.counts.ambiguous_dependency_archives == 0
        && report.counts.nonfinite_nodes == 0
        && report.counts.invalid_rotation_nodes == 0
        && report.counts.singular_scale_nodes == 0
        && report.counts.root_origin_contract_mismatches == 0
        && report.counts.invalid_colliders == 0
        && report.gameplay_facing_rotation_applications == 0;

    write_report_exclusive(report_path, &report)?;
    Ok(report)
}

pub(super) fn audit_map(
    repo_root: &Path,
    build_root: &Path,
    map_path: &Path,
    session_dir: &Path,
    archive_indexes: &mut HashMap<PathBuf, HashMap<String, PathBuf>>,
    archive_candidates: &HashMap<String, Vec<PathBuf>>,
) -> MapTransformAudit {
    let map = map_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("<invalid-map-name>")
        .to_string();
    let mut errors = Vec::<WorldAuditError>::new();
    let bundle_blake3 = match blake3_file(map_path) {
        Ok(hash) => hash,
        Err(err) => {
            errors.push(WorldAuditError::new(Some(&map), "mapHashFailed", err));
            String::new()
        }
    };

    let map_side = extract_bundle_to_session(map_path, session_dir);
    append_side_errors(&map, "mapExtractionFailed", &map_side, &mut errors);
    let mut extracted_bundle_paths = HashSet::from([normalize_path(map_path)]);
    let mut dependency_sides = Vec::<JsonValue>::new();
    let mut missing_dependencies = HashSet::<String>::new();
    let mut env = UnityEnvironment::from_dir(session_dir);
    let mut declared_dependencies = BTreeSet::<String>::new();
    for _ in 0..4096 {
        let dependencies = collect_archive_dependencies(&env);
        declared_dependencies.extend(dependencies.iter().cloned());
        if !extract_archives(
            &dependencies,
            Some(map_path),
            Some(build_root),
            repo_root,
            session_dir,
            &mut extracted_bundle_paths,
            &mut dependency_sides,
            &mut missing_dependencies,
            archive_indexes,
        ) {
            break;
        }
        env = UnityEnvironment::from_dir(session_dir);
    }
    for side in &dependency_sides {
        append_side_errors(&map, "dependencyExtractionFailed", side, &mut errors);
    }

    let mut scene_asset_names = env
        .assets
        .iter()
        .filter(|asset| scene_asset_belongs_to_map(&asset.name, &map))
        .map(|asset| asset.name.clone())
        .collect::<Vec<_>>();
    scene_asset_names.sort();
    scene_asset_names.dedup();
    if scene_asset_names.is_empty() {
        errors.push(WorldAuditError::new(
            Some(&map),
            "missingSceneAsset",
            format!("dependency closure contained no serialized scene asset for {map}"),
        ));
    }

    let scene_assets = scene_asset_names.iter().cloned().collect::<HashSet<_>>();
    let mut counts = MapTransformAuditCounts {
        dependency_bundles: dependency_sides.len(),
        missing_dependencies: missing_dependencies.len(),
        scene_assets: scene_asset_names.len(),
        ..MapTransformAuditCounts::default()
    };
    let mut max_abs_scale = [0.0_f64; 3];
    let mut root_origins = Vec::new();
    let mut transform_exceptions = Vec::new();
    let mut collider_exceptions = Vec::new();

    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !scene_assets.contains(&asset.name) {
            continue;
        }
        for (path_id, info) in &asset.objects {
            let object_type = asset.object_type_name(info);
            if object_type != "Transform"
                && !object_type.ends_with("Collider")
                && object_type != "CharacterController"
            {
                continue;
            }
            let body = match asset.read_object(asset_index, info) {
                Ok(body) => body,
                Err(err) => {
                    errors.push(WorldAuditError::new(
                        Some(&map),
                        "sceneObjectReadFailed",
                        format!("{}#{path_id} {object_type}: {err}", asset.name),
                    ));
                    continue;
                }
            };
            if object_type == "Transform" {
                audit_transform(
                    &env,
                    &asset.name,
                    *path_id,
                    &body,
                    &mut counts,
                    &mut max_abs_scale,
                    &mut root_origins,
                    &mut transform_exceptions,
                    &mut errors,
                    &map,
                );
            } else {
                audit_collider(
                    &env,
                    &asset.name,
                    *path_id,
                    &object_type,
                    &body,
                    &mut counts,
                    &mut collider_exceptions,
                );
            }
        }
    }

    root_origins.sort_by(|left, right| {
        left.asset
            .cmp(&right.asset)
            .then(left.transform_path_id.cmp(&right.transform_path_id))
    });
    transform_exceptions.sort_by(|left, right| {
        left.asset
            .cmp(&right.asset)
            .then(left.transform_path_id.cmp(&right.transform_path_id))
    });
    collider_exceptions.sort_by(|left, right| {
        left.asset
            .cmp(&right.asset)
            .then(left.path_id.cmp(&right.path_id))
    });
    let expected_native_tile_horizontal_origin = expected_native_tile_horizontal_origin(&map);
    let root_origin_matches_tile_contract =
        expected_native_tile_horizontal_origin.is_some_and(|expected| {
            root_origins.len() == 1
                && approx_eq(
                    root_origins[0].native_origin[0],
                    expected[0],
                    APPROX_EPSILON,
                )
                && approx_eq(
                    root_origins[0].native_origin[2],
                    expected[1],
                    APPROX_EPSILON,
                )
        });
    if !root_origin_matches_tile_contract {
        counts.root_origin_contract_mismatches += 1;
        errors.push(WorldAuditError::new(
            Some(&map),
            "rootOriginContractMismatch",
            format!(
                "expected exactly one native Map root at horizontal X/Z {:?} while preserving authored Y, found {:?}",
                expected_native_tile_horizontal_origin, root_origins
            ),
        ));
    }
    let resolved_dependency_bundles = dependency_sides
        .iter()
        .filter_map(dependency_bundle_proof)
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let mut missing_dependencies = missing_dependencies.into_iter().collect::<Vec<_>>();
    missing_dependencies.sort();
    let resolved_paths = resolved_dependency_bundles
        .iter()
        .map(|bundle| normalize_path(Path::new(&bundle.path)))
        .collect::<HashSet<_>>();
    let mut ambiguous_dependency_archives = Vec::<AmbiguousDependencyArchive>::new();
    for archive in &declared_dependencies {
        let candidates = effective_archive_candidates(archive, archive_candidates);
        match candidates.as_slice() {
            [] => errors.push(WorldAuditError::new(
                Some(&map),
                "dependencyCandidateProofMissing",
                format!(
                    "declared archive {archive:?} has no physical candidate in the build catalog"
                ),
            )),
            [candidate] if !resolved_paths.contains(&normalize_path(candidate)) => {
                errors.push(WorldAuditError::new(
                    Some(&map),
                    "dependencyResolutionMismatch",
                    format!(
                        "declared archive {archive:?} uniquely maps to {}, but that bundle was not present in the extracted closure",
                        candidate.display()
                    ),
                ));
            }
            [_] => {}
            _ => {
                let candidate_bundle_paths = candidates
                    .iter()
                    .map(|candidate| readable_absolute_path(candidate))
                    .collect::<Vec<_>>();
                errors.push(WorldAuditError::new(
                    Some(&map),
                    "ambiguousDependencyArchive",
                    format!(
                        "declared archive {archive:?} has {} physical candidates: {}",
                        candidate_bundle_paths.len(),
                        candidate_bundle_paths.join(", ")
                    ),
                ));
                ambiguous_dependency_archives.push(AmbiguousDependencyArchive {
                    archive: archive.clone(),
                    candidate_bundle_paths,
                });
            }
        }
    }
    counts.dependency_bundles = resolved_dependency_bundles.len();
    counts.missing_dependencies = missing_dependencies.len();
    counts.ambiguous_dependency_archives = ambiguous_dependency_archives.len();
    counts.transform_exceptions = transform_exceptions.len();
    counts.collider_exceptions = collider_exceptions.len();
    counts.errors = errors.len();
    let max_abs_scale_component = max_abs_scale.into_iter().fold(0.0_f64, f64::max);
    let passed = errors.is_empty()
        && missing_dependencies.is_empty()
        && ambiguous_dependency_archives.is_empty()
        && counts.nonfinite_nodes == 0
        && counts.invalid_rotation_nodes == 0
        && counts.singular_scale_nodes == 0
        && counts.root_origin_contract_mismatches == 0
        && counts.invalid_colliders == 0;

    MapTransformAudit {
        map,
        bundle_path: readable_absolute_path(map_path),
        bundle_blake3,
        passed,
        scene_assets: scene_asset_names,
        declared_dependency_archives: declared_dependencies.into_iter().collect(),
        resolved_dependency_bundles,
        missing_dependencies,
        ambiguous_dependency_archives,
        counts,
        max_abs_scale,
        max_abs_scale_component,
        expected_native_tile_horizontal_origin,
        root_origin_matches_tile_contract,
        root_origins,
        transform_exceptions,
        collider_exceptions,
        errors,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_transform(
    env: &UnityEnvironment,
    asset_name: &str,
    path_id: i64,
    body: &UnityValue,
    counts: &mut MapTransformAuditCounts,
    max_abs_scale: &mut [f64; 3],
    root_origins: &mut Vec<RootOrigin>,
    exceptions: &mut Vec<TransformException>,
    errors: &mut Vec<WorldAuditError>,
    map: &str,
) {
    counts.scene_nodes += 1;
    let translation = vector(body.get("m_LocalPosition"))
        .map(unity_to_native_vec3)
        .unwrap_or((f64::NAN, f64::NAN, f64::NAN));
    let rotation = quaternion(body.get("m_LocalRotation"))
        .map(unity_to_native_quaternion)
        .unwrap_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN));
    let scale = vector(body.get("m_LocalScale"))
        .map(unity_to_native_scale)
        .unwrap_or((f64::NAN, f64::NAN, f64::NAN));
    let native = NativeTrs {
        translation: [translation.0, translation.1, translation.2],
        rotation: [rotation.0, rotation.1, rotation.2, rotation.3],
        scale: [scale.0, scale.1, scale.2],
    };
    for (index, value) in native.scale.iter().enumerate() {
        if value.is_finite() {
            max_abs_scale[index] = max_abs_scale[index].max(value.abs());
        }
    }

    let finite = native
        .translation
        .iter()
        .chain(native.rotation.iter())
        .chain(native.scale.iter())
        .all(|value| value.is_finite());
    let unit_scale = native
        .scale
        .iter()
        .all(|value| approx_eq(*value, 1.0, APPROX_EPSILON));
    let uniform_scale = approx_eq(native.scale[0], native.scale[1], APPROX_EPSILON)
        && approx_eq(native.scale[1], native.scale[2], APPROX_EPSILON);
    let positive_scale = native.scale.iter().all(|value| *value > 0.0);
    let negative_scale = native.scale.iter().any(|value| *value < 0.0);
    let zero_scale = native
        .scale
        .iter()
        .any(|value| value.abs() <= SINGULAR_EPSILON);
    let determinant = native.scale.iter().product::<f64>();
    let singular_scale = !determinant.is_finite() || determinant.abs() <= SINGULAR_EPSILON;
    let rotation_norm_squared = native
        .rotation
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    let valid_rotation = rotation_norm_squared.is_finite()
        && rotation_norm_squared > SINGULAR_EPSILON
        && approx_eq(rotation_norm_squared.sqrt(), 1.0, 2.0e-4);

    if unit_scale {
        counts.unit_scale_nodes += 1;
    } else {
        counts.nonunit_scale_nodes += 1;
    }
    if uniform_scale {
        counts.uniform_scale_nodes += 1;
    } else {
        counts.nonuniform_scale_nodes += 1;
    }
    if positive_scale {
        counts.positive_scale_nodes += 1;
    }
    if negative_scale {
        counts.negative_scale_nodes += 1;
    }
    if zero_scale {
        counts.zero_scale_nodes += 1;
    }
    if singular_scale {
        counts.singular_scale_nodes += 1;
    }
    if !finite {
        counts.nonfinite_nodes += 1;
    }
    if !valid_rotation {
        counts.invalid_rotation_nodes += 1;
    }

    let game_object = game_object_name(env, body);
    let parent = body.get("m_Father").and_then(UnityValue::as_pointer);
    let is_root = parent.is_none_or(|pointer| pointer.is_null());
    if let Some(parent) = parent.filter(|pointer| !pointer.is_null()) {
        if env.resolve_pointer(parent).is_err() {
            errors.push(WorldAuditError::new(
                Some(map),
                "unresolvedTransformParent",
                format!(
                    "{asset_name}#{path_id} {game_object:?} references unresolved parent pathId {}",
                    parent.path_id
                ),
            ));
        }
    }
    if is_root {
        counts.root_nodes += 1;
        let nonzero = native
            .translation
            .iter()
            .any(|value| value.abs() > APPROX_EPSILON);
        if nonzero {
            counts.nonzero_root_origins += 1;
        }
        root_origins.push(RootOrigin {
            asset: asset_name.to_string(),
            transform_path_id: path_id,
            game_object: game_object.clone(),
            native_origin: native.translation,
            nonzero,
        });
    }

    let mut kinds = Vec::new();
    if !finite {
        kinds.push("nonfiniteTrs".to_string());
    }
    if !unit_scale {
        kinds.push("nonunitScale".to_string());
    }
    if !uniform_scale {
        kinds.push("nonuniformScale".to_string());
    }
    if negative_scale {
        kinds.push("negativeScale".to_string());
    }
    if zero_scale {
        kinds.push("zeroScale".to_string());
    }
    if singular_scale {
        kinds.push("singularScale".to_string());
    }
    if !valid_rotation {
        kinds.push("invalidQuaternion".to_string());
    }
    if is_root
        && native
            .translation
            .iter()
            .any(|value| value.abs() > APPROX_EPSILON)
    {
        kinds.push("nonzeroRootOrigin".to_string());
    }
    if !kinds.is_empty() {
        exceptions.push(TransformException {
            asset: asset_name.to_string(),
            transform_path_id: path_id,
            game_object,
            kinds,
            native,
            parent_path_id: parent
                .filter(|pointer| !pointer.is_null())
                .map(|pointer| pointer.path_id),
        });
    }
}
