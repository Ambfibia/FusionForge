use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PhysicalRouteIndexBuildStats {
    pub(super) asset_bundle_container_parse_count: usize,
    pub(super) serialized_asset_hash_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PhysicalNormalizedRouteIndex {
    pub(super) exact_spellings: BTreeSet<String>,
    pub(super) exact_occurrences: BTreeMap<String, Vec<Result<LogicalModelPhysicalFingerprint, String>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PhysicalOwnerAssetRouteIndex {
    pub(super) serialized_asset_match_count: usize,
    pub(super) routes: Result<BTreeMap<String, PhysicalNormalizedRouteIndex>, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PhysicalEnvironmentRouteIndex {
    pub(super) owners: BTreeMap<String, PhysicalOwnerAssetRouteIndex>,
    pub(super) stats: PhysicalRouteIndexBuildStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerAssetRefEvidence {
    pub asset_ref_index: usize,
    pub asset_path: String,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referenced_asset_name: Option<String>,
}

pub(super) fn sole_route_provenance(
    group: &LogicalModelRouteGroup,
) -> Result<LogicalModelRouteProvenance, LogicalModelPlanBlocker> {
    let subject_kind = match group.kind {
        LogicalModelKind::Kfm => "kfm",
        LogicalModelKind::Nif => "nif",
    };
    if group.case_or_separator_collision || group.exact_routes.len() != 1 {
        return Err(LogicalModelPlanBlocker {
            subject_kind: subject_kind.to_string(),
            normalized_route: Some(group.normalized_route.clone()),
            exact_routes: group.exact_routes.clone(),
            owners: group.owners.clone(),
            code: format!("{subject_kind}RouteCollision"),
            detail: "multiple exact route spellings normalize to the same route".to_string(),
            kfm_pointer_traversal: None,
            kfm_preload_ownership: None,
        });
    }
    let Some(owner) = group
        .preferred_owner
        .clone()
        .filter(|_| !group.ambiguous_owner && group.owners.len() == 1)
    else {
        return Err(LogicalModelPlanBlocker {
            subject_kind: subject_kind.to_string(),
            normalized_route: Some(group.normalized_route.clone()),
            exact_routes: group.exact_routes.clone(),
            owners: group.owners.clone(),
            code: format!("{subject_kind}OwnerAmbiguous"),
            detail: "the bundle index does not prove one distinct owner".to_string(),
            kfm_pointer_traversal: None,
            kfm_preload_ownership: None,
        });
    };
    Ok(LogicalModelRouteProvenance {
        exact_route: group.exact_routes[0].clone(),
        normalized_route: group.normalized_route.clone(),
        owner,
    })
}

pub(super) fn resolved_route_for_group(
    group: &LogicalModelRouteGroup,
    resolved_routes: &BTreeMap<String, Result<LogicalModelResolvedRoute, LogicalModelPlanBlocker>>,
) -> Result<LogicalModelResolvedRoute, LogicalModelPlanBlocker> {
    resolved_routes
        .get(&group.normalized_route)
        .cloned()
        .unwrap_or_else(|| {
            Err(LogicalModelPlanBlocker {
                subject_kind: match group.kind {
                    LogicalModelKind::Kfm => "kfm",
                    LogicalModelKind::Nif => "nif",
                }
                .to_string(),
                normalized_route: Some(group.normalized_route.clone()),
                exact_routes: group.exact_routes.clone(),
                owners: group.owners.clone(),
                code: "physicalRouteProofMissing".to_string(),
                detail: "no exact physical target proof was produced for this route group"
                    .to_string(),
                kfm_pointer_traversal: None,
                kfm_preload_ownership: None,
            })
        })
}

pub(super) fn project_dir_for_bundle_index(bundle_index_path: &Path) -> Result<PathBuf, String> {
    let cache_dir = bundle_index_path.parent().ok_or_else(|| {
        format!(
            "bundle-index path has no cache directory: {}",
            bundle_index_path.display()
        )
    })?;
    if !cache_dir
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("cache"))
    {
        return Err(format!(
            "bundle-index must be inside a project cache directory: {}",
            bundle_index_path.display()
        ));
    }
    cache_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "bundle-index cache directory has no project parent".to_string())
}

pub(super) fn normalized_filesystem_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

pub(super) fn route_group_subject_kind(group: &LogicalModelRouteGroup) -> &'static str {
    match group.kind {
        LogicalModelKind::Kfm => "kfm",
        LogicalModelKind::Nif => "nif",
    }
}

pub(super) fn route_group_blocker(
    group: &LogicalModelRouteGroup,
    code_suffix: &str,
    detail: impl Into<String>,
) -> LogicalModelPlanBlocker {
    let subject_kind = route_group_subject_kind(group);
    LogicalModelPlanBlocker {
        subject_kind: subject_kind.to_string(),
        normalized_route: Some(group.normalized_route.clone()),
        exact_routes: group.exact_routes.clone(),
        owners: group.owners.clone(),
        code: format!("{subject_kind}{code_suffix}"),
        detail: detail.into(),
        kfm_pointer_traversal: None,
        kfm_preload_ownership: None,
    }
}

pub(super) fn resolve_physical_route_group(
    group: &LogicalModelRouteGroup,
    proofs: &[LogicalModelOccurrenceFingerprint],
) -> Result<LogicalModelResolvedRoute, LogicalModelPlanBlocker> {
    if group.case_or_separator_collision || group.exact_routes.len() != 1 {
        return Err(route_group_blocker(
            group,
            "RouteCollision",
            "multiple exact route spellings normalize to the same route",
        ));
    }
    if proofs.len() != group.occurrence_count {
        return Err(route_group_blocker(
            group,
            "PhysicalProofCountMismatch",
            format!(
                "resolved {} physical occurrence proofs for {} catalog occurrences",
                proofs.len(),
                group.occurrence_count
            ),
        ));
    }
    let expected_route = &group.exact_routes[0];
    if proofs
        .iter()
        .any(|proof| &proof.exact_route != expected_route)
    {
        return Err(route_group_blocker(
            group,
            "PhysicalProofRouteMismatch",
            "a physical target proof came from a different exact container route",
        ));
    }
    let expected_owners = group.owners.iter().cloned().collect::<BTreeSet<_>>();
    let actual_owners = proofs
        .iter()
        .map(|proof| proof.owner.clone())
        .collect::<BTreeSet<_>>();
    if actual_owners != expected_owners {
        return Err(route_group_blocker(
            group,
            "PhysicalProofOwnerMismatch",
            "physical target proofs do not cover exactly the catalog owners",
        ));
    }
    let fingerprints = proofs
        .iter()
        .map(|proof| proof.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    if fingerprints.len() != 1 {
        return Err(route_group_blocker(
            group,
            "PhysicalTargetConflict",
            format!(
                "{} owners resolve to {} distinct serialized-asset SHA-256/PathID/object-type fingerprints; semantic-copy aliases are not accepted",
                actual_owners.len(),
                fingerprints.len()
            ),
        ));
    }
    let Some(canonical_owner) = expected_owners.iter().next().cloned() else {
        return Err(route_group_blocker(
            group,
            "PhysicalProofOwnerMissing",
            "route group contains no catalog owner",
        ));
    };
    let alias_owners = expected_owners
        .into_iter()
        .filter(|owner| owner != &canonical_owner)
        .collect::<Vec<_>>();
    Ok(LogicalModelResolvedRoute {
        provenance: LogicalModelRouteProvenance {
            exact_route: expected_route.clone(),
            normalized_route: group.normalized_route.clone(),
            owner: canonical_owner,
        },
        alias_owners,
        physical_fingerprint: fingerprints.into_iter().next(),
    })
}

pub(super) fn build_physical_environment_route_index(
    env: &super::super::UnityEnvironment,
    occurrences: &[&LogicalModelOccurrence],
) -> PhysicalEnvironmentRouteIndex {
    let mut required_routes_by_owner = BTreeMap::<String, BTreeSet<String>>::new();
    for occurrence in occurrences {
        required_routes_by_owner
            .entry(occurrence.owner.asset_name.clone())
            .or_default()
            .insert(occurrence.normalized_route.clone());
    }

    let mut asset_indices_by_name = BTreeMap::<String, Vec<usize>>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if required_routes_by_owner.contains_key(&asset.name) {
            asset_indices_by_name
                .entry(asset.name.clone())
                .or_default()
                .push(asset_index);
        }
    }

    let mut stats = PhysicalRouteIndexBuildStats::default();
    let mut serialized_asset_hashes = BTreeMap::<usize, String>::new();
    let mut owners = BTreeMap::new();
    for (asset_name, required_routes) in required_routes_by_owner {
        let matching_assets = asset_indices_by_name
            .get(&asset_name)
            .cloned()
            .unwrap_or_default();
        let routes = match matching_assets.as_slice() {
            [owner_asset_index] => {
                let owner_asset = &env.assets[*owner_asset_index];
                let asset_bundle_objects = owner_asset
                    .objects
                    .values()
                    .filter(|info| owner_asset.object_type_name(info) == "AssetBundle")
                    .collect::<Vec<_>>();
                build_physical_owner_route_index(
                    asset_bundle_objects,
                    &required_routes,
                    &mut stats,
                    |info| {
                        let body =
                            owner_asset
                                .read_object(*owner_asset_index, info)
                                .map_err(|error| {
                                    format!(
                                        "could not read exact owner AssetBundle {} #{}: {error}",
                                        owner_asset.name, info.path_id
                                    )
                                })?;
                        let mut entries = Vec::new();
                        for entry in super::super::value_array(body.get("m_Container")) {
                            let Some((path, metadata)) = super::super::pair_name_value(entry) else {
                                continue;
                            };
                            let normalized_route = normalize_logical_model_route(path);
                            if !required_routes.contains(&normalized_route) {
                                continue;
                            }
                            let fingerprint = metadata
                                .get("asset")
                                .and_then(super::super::UnityValue::as_pointer)
                                .cloned()
                                .ok_or_else(|| {
                                    "exact container occurrence has no asset PPtr".to_string()
                                })
                                .and_then(|pointer| {
                                    physical_fingerprint_for_pointer(
                                        env,
                                        &pointer,
                                        path,
                                        &mut serialized_asset_hashes,
                                    )
                                });
                            entries.push(PhysicalIndexedContainerOccurrence {
                                exact_route: path.to_string(),
                                fingerprint,
                            });
                        }
                        Ok(entries)
                    },
                )
            }
            _ => Ok(BTreeMap::new()),
        };
        owners.insert(
            asset_name,
            PhysicalOwnerAssetRouteIndex {
                serialized_asset_match_count: matching_assets.len(),
                routes,
            },
        );
    }
    stats.serialized_asset_hash_count = serialized_asset_hashes.len();
    PhysicalEnvironmentRouteIndex { owners, stats }
}

pub(super) fn build_physical_owner_route_index<T, I, F>(
    asset_bundle_objects: I,
    required_routes: &BTreeSet<String>,
    stats: &mut PhysicalRouteIndexBuildStats,
    mut parse_container: F,
) -> Result<BTreeMap<String, PhysicalNormalizedRouteIndex>, String>
where
    I: IntoIterator<Item = T>,
    F: FnMut(T) -> Result<Vec<PhysicalIndexedContainerOccurrence>, String>,
{
    let mut routes = BTreeMap::<String, PhysicalNormalizedRouteIndex>::new();
    for asset_bundle_object in asset_bundle_objects {
        stats.asset_bundle_container_parse_count += 1;
        for occurrence in parse_container(asset_bundle_object)? {
            let normalized_route = normalize_logical_model_route(&occurrence.exact_route);
            if !required_routes.contains(&normalized_route) {
                continue;
            }
            let route = routes.entry(normalized_route).or_default();
            route.exact_spellings.insert(occurrence.exact_route.clone());
            route
                .exact_occurrences
                .entry(occurrence.exact_route)
                .or_default()
                .push(occurrence.fingerprint);
        }
    }
    Ok(routes)
}

pub(super) fn cached_serialized_asset_paths(extract_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = fs::read_dir(extract_dir)
        .map_err(|err| format!("{}: {err}", extract_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("cache-meta.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

pub(super) fn is_unity_system_asset_ref(value: &str) -> bool {
    matches!(
        value
            .trim()
            .replace('\\', "/")
            .to_ascii_lowercase()
            .as_str(),
        "library/unity default resources" | "resources/unity_builtin_extra"
    )
}
