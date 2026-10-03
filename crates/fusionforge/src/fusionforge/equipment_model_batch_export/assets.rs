use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentRouteTiming {
    pub ordinal: u64,
    pub bundle_name: String,
    pub normalized_route: String,
    pub outcome: String,
    pub milliseconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BundleIndex {
    #[serde(default)]
    pub(super) bundles: Vec<BundleEntry>,
    #[serde(default)]
    pub(super) unity_files: Vec<BundleEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BundleAsset {
    pub(super) name: String,
    #[serde(default)]
    pub(super) container_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RoutePlan {
    pub(super) category: String,
    pub(super) exact_route: String,
    pub(super) normalized_route: String,
    pub(super) table_rows: Vec<EquipmentTableRowProof>,
}

#[derive(Debug, Clone)]
pub(super) struct RouteCandidate {
    pub(super) route: RoutePlan,
    pub(super) owner: EquipmentContainerOwner,
}

#[derive(Debug)]
pub(super) struct StagedRoute {
    pub(super) candidate: RouteCandidate,
    pub(super) true_name: String,
    pub(super) safe_true_name: String,
    pub(super) route_qualifier: Option<String>,
    pub(super) temporary_path: PathBuf,
    pub(super) source_byte_length: u64,
    pub(super) source_sha256: String,
    pub(super) physical_target: EquipmentPhysicalTarget,
    pub(super) facts: EquipmentSourceFacts,
}

pub(super) fn collect_route_plans(
    semantic: &SemanticPlan,
) -> Result<(Vec<RoutePlan>, usize, usize, BTreeMap<String, u64>), String> {
    let mut routes = BTreeMap::<String, RoutePlan>::new();
    let mut equipment_entities = 0usize;
    let mut entities_with_routes = 0usize;
    let mut categories = BTreeMap::<String, u64>::new();
    for entity in &semantic.entities {
        let Some(category) = entity.category.strip_prefix("equipment_") else {
            continue;
        };
        equipment_entities += 1;
        if !ALLOWED_CATEGORIES.contains(&category) {
            return Err(format!(
                "unsupported equipment category {:?} on {}",
                entity.category, entity.id
            ));
        }
        let expected_directory = format!("characters/player/equipment/{category}");
        if entity.semantic_directory != expected_directory {
            return Err(format!(
                "equipment entity {} has semantic directory {:?}, expected {:?}",
                entity.id, entity.semantic_directory, expected_directory
            ));
        }
        *categories.entry(category.to_string()).or_default() += 1;
        if !entity.model_routes.is_empty() {
            entities_with_routes += 1;
        }
        for exact_route in &entity.model_routes {
            let normalized_route = normalize_logical_model_route(exact_route);
            if !normalized_route.starts_with("wear/") || !normalized_route.ends_with(".nif") {
                return Err(format!(
                    "equipment entity {} has non-wear/NIF model route {exact_route:?}",
                    entity.id
                ));
            }
            let table_row = EquipmentTableRowProof {
                entity_id: entity.id.clone(),
                table_owner: entity.table_owner.clone(),
            };
            match routes.entry(normalized_route.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(RoutePlan {
                        category: category.to_string(),
                        exact_route: exact_route.clone(),
                        normalized_route,
                        table_rows: vec![table_row],
                    });
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let route = entry.get_mut();
                    if route.category != category {
                        return Err(format!(
                            "one equipment route belongs to two categories without proof: {:?} -> {:?}, {:?}",
                            route.normalized_route, route.category, category
                        ));
                    }
                    route.table_rows.push(table_row);
                }
            }
        }
    }
    for route in routes.values_mut() {
        route
            .table_rows
            .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    }
    Ok((
        routes.into_values().collect(),
        equipment_entities,
        entities_with_routes,
        categories,
    ))
}

pub(super) fn collect_route_occurrences(index: BundleIndex) -> BTreeMap<String, Vec<EquipmentContainerOwner>> {
    let mut occurrences = BTreeMap::<String, Vec<EquipmentContainerOwner>>::new();
    for bundle in index.bundles.into_iter().chain(index.unity_files) {
        for asset in bundle.assets {
            for exact_route in asset.container_paths {
                let normalized_route = normalize_logical_model_route(&exact_route);
                if !normalized_route.starts_with("wear/") || !normalized_route.ends_with(".nif") {
                    continue;
                }
                occurrences
                    .entry(normalized_route)
                    .or_default()
                    .push(EquipmentContainerOwner {
                        bundle_path: bundle.path.clone(),
                        bundle_name: bundle.name.clone(),
                        asset_name: asset.name.clone(),
                        exact_container_route: exact_route,
                    });
            }
        }
    }
    for owners in occurrences.values_mut() {
        owners.sort_by(|left, right| {
            portable_key(&left.bundle_path)
                .cmp(&portable_key(&right.bundle_path))
                .then_with(|| left.asset_name.cmp(&right.asset_name))
                .then_with(|| left.exact_container_route.cmp(&right.exact_container_route))
        });
        owners.dedup();
    }
    occurrences
}

pub(super) fn stage_route_sources(
    candidates: &[RouteCandidate],
    project_dir: &Path,
    route_stage: &Path,
) -> Result<StagingResult, String> {
    let mut routes = Vec::new();
    let mut blockers = Vec::new();
    let mut bundle_warmups = Vec::new();
    let mut per_route = Vec::with_capacity(candidates.len());
    let mut warmed_bundle = None::<String>;
    for (index, candidate) in candidates.iter().enumerate() {
        let bundle_key = portable_key(&candidate.owner.bundle_path);
        if warmed_bundle.as_deref() != Some(bundle_key.as_str()) {
            let group_routes = candidates[index..]
                .iter()
                .take_while(|next| portable_key(&next.owner.bundle_path) == bundle_key)
                .map(|next| next.owner.exact_container_route.clone())
                .collect::<Vec<_>>();
            let warmup_started = Instant::now();
            let warmup = crate::prewarm_exact_logical_model_environment(
                &candidate.owner.bundle_path,
                project_dir,
                &group_routes,
            )?;
            bundle_warmups.push(EquipmentBundleWarmupTiming {
                bundle_path: warmup.bundle_path,
                requested_routes: u64_count(warmup.requested_routes)?,
                extract_directories: u64_count(warmup.extract_directories)?,
                unity_assets: u64_count(warmup.unity_assets)?,
                indexed_objects: u64_count(warmup.indexed_objects)?,
                indexed_container_routes: u64_count(warmup.indexed_container_routes)?,
                milliseconds: elapsed_milliseconds(warmup_started),
            });
            warmed_bundle = Some(bundle_key);
        }
        if index == 0 || (index + 1) % 25 == 0 || index + 1 == candidates.len() {
            eprintln!(
                "equipment logical sources: {}/{} ({})",
                index + 1,
                candidates.len(),
                candidate.owner.bundle_name
            );
        }
        let route_started = Instant::now();
        let source = crate::preview_bundle_container_model_exact(
            candidate.owner.bundle_path.clone(),
            Some(project_dir.to_string_lossy().to_string()),
            candidate.owner.exact_container_route.clone(),
        );
        let source = match source {
            Ok(source) => source,
            Err(err) => {
                per_route.push(route_timing(
                    index,
                    candidate,
                    "blocked-export",
                    route_started,
                )?);
                blockers.push(source_stage_blocker(candidate, err));
                continue;
            }
        };
        let validated = validate_exact_source(&source, candidate);
        let (true_name, safe_true_name, physical_target, facts) = match validated {
            Ok(value) => value,
            Err(err) => {
                per_route.push(route_timing(
                    index,
                    candidate,
                    "blocked-validation",
                    route_started,
                )?);
                blockers.push(source_stage_blocker(candidate, err));
                continue;
            }
        };
        let mut bytes = serde_json::to_vec_pretty(&source)
            .map_err(|err| format!("could not encode exact equipment source: {err}"))?;
        bytes.push(b'\n');
        let temporary_path = route_stage.join(format!("route-{index:04}.source.json"));
        write_new_bytes(&temporary_path, &bytes)?;
        routes.push(StagedRoute {
            candidate: candidate.clone(),
            true_name,
            safe_true_name,
            route_qualifier: None,
            temporary_path,
            source_byte_length: u64_count(bytes.len())?,
            source_sha256: sha256_hex(&bytes),
            physical_target,
            facts,
        });
        per_route.push(route_timing(index, candidate, "staged", route_started)?);
    }
    Ok(StagingResult {
        routes,
        blockers,
        bundle_warmups,
        per_route,
    })
}

pub(super) fn route_timing(
    index: usize,
    candidate: &RouteCandidate,
    outcome: &str,
    started: Instant,
) -> Result<EquipmentRouteTiming, String> {
    Ok(EquipmentRouteTiming {
        ordinal: u64_count(index + 1)?,
        bundle_name: candidate.owner.bundle_name.clone(),
        normalized_route: candidate.route.normalized_route.clone(),
        outcome: outcome.to_string(),
        milliseconds: elapsed_milliseconds(started),
    })
}

pub(super) fn source_relative_path(route: &StagedRoute) -> PathBuf {
    let mut path = PathBuf::from(FAMILY);
    for component in SEMANTIC_PREFIX {
        path.push(component);
    }
    path.push(&route.candidate.route.category);
    if let Some(route_qualifier) = &route.route_qualifier {
        path.push(route_qualifier);
    }
    path.push(&route.safe_true_name);
    path.push(format!("{}.source.json", route.safe_true_name));
    path
}

pub(super) fn route_stem_qualifier(exact_route: &str) -> Result<String, String> {
    let stem = Path::new(exact_route)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("equipment route has no UTF-8 stem: {exact_route:?}"))?;
    let safe_glb = minimal_windows_glb_filename(stem)
        .map_err(|err| format!("equipment route stem is not safely publishable: {err}"))?;
    safe_glb
        .strip_suffix(".glb")
        .map(str::to_owned)
        .ok_or_else(|| "safe equipment route qualifier has no suffix".to_string())
}

pub(super) fn resolve_bundle_index_input(input: &Path) -> Result<PathBuf, String> {
    let path = if input.is_dir() {
        input.join("cache").join("bundle-index.json")
    } else {
        input.to_path_buf()
    };
    if !path.is_file() {
        return Err(format!(
            "equipment batch input must be a client-project directory or cache/bundle-index.json: {}",
            input.display()
        ));
    }
    Ok(path)
}

pub(super) fn project_dir_for_bundle_index(path: &Path) -> Result<PathBuf, String> {
    let cache = path
        .parent()
        .ok_or_else(|| "bundle-index path has no cache parent".to_string())?;
    if !cache
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("cache"))
    {
        return Err("bundle-index must be inside the client-project cache directory".to_string());
    }
    cache
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "bundle-index cache has no project parent".to_string())
}

pub(super) fn batch_manifest_path(output: &Path) -> Result<PathBuf, String> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "equipment source output has no UTF-8 directory name".to_string())?;
    Ok(parent.join(format!("{name}.manifest.json")))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
