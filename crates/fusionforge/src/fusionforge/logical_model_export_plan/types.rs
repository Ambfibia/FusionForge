use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreloadResolvedPointerEvidence {
    pub preload_table_index: usize,
    pub key: ResolvedObjectKeyEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedPointerEvidence {
    pub source_asset_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_asset_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_asset_format: Option<u32>,
    pub file_id: i32,
    pub path_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_ref: Option<PointerAssetRefEvidence>,
    pub loaded_path_id_candidates: Vec<ResolvedObjectKeyEvidence>,
    pub resolution_error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreloadUnresolvedPointerEvidence {
    pub preload_table_index: usize,
    pub value_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer: Option<UnresolvedPointerEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreloadRetainedExternalPointerEvidence {
    pub preload_table_index: usize,
    pub pointer: UnresolvedPointerEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosureUnresolvedPointerEvidence {
    pub source_object: ResolvedObjectKeyEvidence,
    pub pointer_ordinal: usize,
    pub pointer: UnresolvedPointerEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosureRetainedExternalPointerEvidence {
    pub source_object: ResolvedObjectKeyEvidence,
    pub pointer_ordinal: usize,
    pub pointer: UnresolvedPointerEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyArchiveResolutionEvidence {
    pub archive_name: String,
    pub normalized_archive: String,
    pub candidate_bundle_paths: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KfmPreloadOwnershipEvidence {
    pub exact_container_occurrence_count: usize,
    pub asset_bundle_asset_name: String,
    pub asset_bundle_path_id: i64,
    pub preload_index: usize,
    pub preload_size: usize,
    pub preload_range_start: usize,
    pub preload_range_end: usize,
    pub preload_table_length: usize,
    pub dependency_archives: Vec<DependencyArchiveResolutionEvidence>,
    pub container_pointer: ResolvedObjectKeyEvidence,
    pub resolved_preload_pointers: Vec<PreloadResolvedPointerEvidence>,
    pub retained_external_preload_pointers: Vec<PreloadRetainedExternalPointerEvidence>,
    pub unresolved_preload_table_indices: Vec<usize>,
    pub unresolved_preload_pointers: Vec<PreloadUnresolvedPointerEvidence>,
    pub closure_keys: Vec<ResolvedObjectKeyEvidence>,
    pub retained_external_closure_pointers: Vec<ClosureRetainedExternalPointerEvidence>,
    pub closure_unresolved_pointer_count: usize,
    pub closure_unresolved_pointers: Vec<ClosureUnresolvedPointerEvidence>,
    pub closure_unreadable_object_count: usize,
    pub scanned_nif_route_group_count: usize,
    pub scanned_nif_container_occurrence_count: usize,
    pub resolved_nif_container_pointer_count: usize,
    pub nif_pointer_identity_match_count: usize,
    pub matched_nif_routes: Vec<NifPointerOwnershipEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KfmPointerTraversalEvidence {
    pub exact_container_occurrence_count: usize,
    pub container_asset_name: String,
    pub container_path_id: i64,
    pub container_object_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_asset_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_path_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_object_type: Option<String>,
    pub pointer_nodes_visited: usize,
    pub pointer_cycle_guard: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer_safety_budget: Option<usize>,
    pub pointer_budget_exceeded: bool,
    pub pointer_graph_exhausted: bool,
}

pub(super) struct RealKfmResolver {
    pub(super) project_dir: PathBuf,
    pub(super) archive_bundle_paths: BTreeMap<String, Vec<PathBuf>>,
    pub(super) loaded_bundle: Option<(String, Result<LoadedKfmEnvironment, String>)>,
}

pub(super) struct LoadedKfmEnvironment {
    pub(super) env: super::super::UnityEnvironment,
    pub(super) dependency_archives: Vec<DependencyArchiveResolutionEvidence>,
}

impl RealKfmResolver {
    pub(super) fn new(project_dir: PathBuf, bundle_index_path: &Path) -> Result<Self, String> {
        Ok(Self {
            project_dir,
            archive_bundle_paths: exact_archive_bundle_paths(bundle_index_path)?,
            loaded_bundle: None,
        })
    }

    /// Resolves every catalog occurrence from its exact owning AssetBundle
    /// container entry. Multi-owner groups are accepted only when every target
    /// has the same physical serialized-object identity.
    pub(super) fn resolve_physical_route_groups(
        &self,
        catalog: &LogicalModelCatalog,
    ) -> BTreeMap<String, Result<LogicalModelResolvedRoute, LogicalModelPlanBlocker>> {
        let mut occurrences_by_bundle = BTreeMap::<String, Vec<&LogicalModelOccurrence>>::new();
        for occurrence in &catalog.occurrences {
            occurrences_by_bundle
                .entry(occurrence.owner.bundle_path.clone())
                .or_default()
                .push(occurrence);
        }
        for occurrences in occurrences_by_bundle.values_mut() {
            occurrences.sort_by(|left, right| {
                left.normalized_route
                    .cmp(&right.normalized_route)
                    .then_with(|| left.exact_route.cmp(&right.exact_route))
                    .then_with(|| left.owner.cmp(&right.owner))
            });
        }

        let mut proofs_by_route = BTreeMap::<
            String,
            Vec<Result<LogicalModelOccurrenceFingerprint, PhysicalOccurrenceResolutionError>>,
        >::new();
        for (bundle_path, occurrences) in occurrences_by_bundle {
            let loaded = self.load_environment(Path::new(&bundle_path));
            match loaded {
                Ok(loaded) => {
                    let physical_index =
                        build_physical_environment_route_index(&loaded.env, &occurrences);
                    debug_assert!(
                        physical_index.stats.serialized_asset_hash_count <= loaded.env.assets.len()
                    );
                    for occurrence in occurrences {
                        let proof =
                            physical_fingerprint_for_occurrence(&physical_index, occurrence).map(
                                |fingerprint| LogicalModelOccurrenceFingerprint {
                                    exact_route: occurrence.exact_route.clone(),
                                    owner: occurrence.owner.clone(),
                                    fingerprint,
                                },
                            );
                        proofs_by_route
                            .entry(occurrence.normalized_route.clone())
                            .or_default()
                            .push(proof);
                    }
                }
                Err(error) => {
                    for occurrence in occurrences {
                        proofs_by_route
                            .entry(occurrence.normalized_route.clone())
                            .or_default()
                            .push(Err(PhysicalOccurrenceResolutionError::new(
                                "PhysicalTargetUnresolved",
                                format!("could not load exact owner bundle {bundle_path}: {error}"),
                            )));
                    }
                }
            }
        }

        catalog
            .route_groups
            .iter()
            .map(|group| {
                let proofs = proofs_by_route
                    .remove(&group.normalized_route)
                    .unwrap_or_default();
                let resolution =
                    if let Some(error) = proofs.iter().find_map(|proof| proof.as_ref().err()) {
                        Err(route_group_blocker(
                            group,
                            error.code_suffix,
                            error.detail.clone(),
                        ))
                    } else {
                        let proofs = proofs
                            .into_iter()
                            .filter_map(Result::ok)
                            .collect::<Vec<_>>();
                        resolve_physical_route_group(group, &proofs)
                    };
                (group.normalized_route.clone(), resolution)
            })
            .collect()
    }

    pub(super) fn load_environment(&self, primary_bundle: &Path) -> Result<LoadedKfmEnvironment, String> {
        let primary_extract_dir = crate::extract_bundle_cached(&self.project_dir, primary_bundle)?;
        let mut asset_paths = cached_serialized_asset_paths(&primary_extract_dir)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut loaded_bundle_paths = BTreeSet::from([normalized_filesystem_path(primary_bundle)]);
        let mut dependency_archives =
            BTreeMap::<String, DependencyArchiveResolutionEvidence>::new();

        loop {
            let env = super::super::UnityEnvironment::from_paths(
                &asset_paths.iter().cloned().collect::<Vec<_>>(),
            );
            let mut loaded_new_bundle = false;
            for archive_name in super::super::unity::collect_archive_dependencies(&env) {
                let normalized_archive = super::super::unity::normalize_bundle_name(&archive_name);
                if dependency_archives.contains_key(&normalized_archive) {
                    continue;
                }
                let candidates = self
                    .archive_bundle_paths
                    .get(&normalized_archive)
                    .cloned()
                    .unwrap_or_default();
                let candidate_bundle_paths = candidates
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect::<Vec<_>>();
                let status = match candidates.as_slice() {
                    [] => "missingExactArchiveMapping",
                    [candidate] => {
                        let candidate_key = normalized_filesystem_path(candidate);
                        if loaded_bundle_paths.insert(candidate_key) {
                            let extract_dir = crate::extract_bundle_cached(
                                &self.project_dir,
                                candidate,
                            )
                            .map_err(|err| {
                                format!(
                                    "exact dependency archive '{archive_name}' maps to {}, but extraction failed: {err}",
                                    candidate.display()
                                )
                            })?;
                            for path in cached_serialized_asset_paths(&extract_dir)? {
                                asset_paths.insert(path);
                            }
                            loaded_new_bundle = true;
                            "loadedExactBundleIndexOwner"
                        } else {
                            "alreadyLoadedExactBundleIndexOwner"
                        }
                    }
                    _ => "ambiguousExactArchiveMapping",
                };
                dependency_archives.insert(
                    normalized_archive.clone(),
                    DependencyArchiveResolutionEvidence {
                        archive_name,
                        normalized_archive,
                        candidate_bundle_paths,
                        status: status.to_string(),
                    },
                );
            }
            if !loaded_new_bundle {
                return Ok(LoadedKfmEnvironment {
                    env,
                    dependency_archives: dependency_archives.into_values().collect(),
                });
            }
        }
    }

    pub(super) fn resolve(
        &mut self,
        provenance: &LogicalModelRouteProvenance,
    ) -> Result<KfmPayloadEvidence, KfmResolveError> {
        let bundle_key = provenance.owner.bundle_path.clone();
        if self
            .loaded_bundle
            .as_ref()
            .is_none_or(|(loaded_key, _)| loaded_key != &bundle_key)
        {
            let bundle_path = PathBuf::from(&bundle_key);
            let loaded = self
                .load_environment(&bundle_path)
                .map_err(|err| format!("{}: {err}", provenance.owner.bundle_path));
            self.loaded_bundle = Some((bundle_key.clone(), loaded));
        }
        let loaded = self
            .loaded_bundle
            .as_ref()
            .expect("bundle cache entry was inserted")
            .1
            .as_ref()
            .map_err(|err| KfmResolveError::new("kfmBundleLoadFailed", err.clone()))?;

        resolve_kfm_from_environment(&loaded.env, provenance, &loaded.dependency_archives)
    }
}

#[derive(Debug)]
pub(super) struct ExactComponentRecord {
    pub(super) game_object_key: (usize, i64),
    pub(super) key: (usize, i64),
    pub(super) object_type: String,
    pub(super) body: super::super::UnityValue,
}
