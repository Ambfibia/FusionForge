use super::*;

pub const LOGICAL_MODEL_EXPORT_PLAN_SCHEMA: &str = "ffclient.logical-model-export-plan.v1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogicalModelExportPlanOptions {
    /// Limits payload reads to one normalized KFM route for representative
    /// verification. Partial plans can never prove standalone NIF ownership.
    pub requested_kfm_route: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelExportPlan {
    pub schema: &'static str,
    pub source_index_path: String,
    pub scope: LogicalModelExportPlanScope,
    pub emits_glb: bool,
    pub ownership_scan_complete: bool,
    pub standalone_nif_proof_complete: bool,
    pub counts: LogicalModelExportPlanCounts,
    pub physical_aliases: Vec<LogicalModelPhysicalAliasProof>,
    pub logical_roots: Vec<LogicalModelRootPlan>,
    pub owned_parts: Vec<LogicalModelOwnedPart>,
    pub standalone_nifs: Vec<LogicalModelStandaloneNif>,
    pub unresolved: Vec<LogicalModelUnresolvedReference>,
    pub blockers: Vec<LogicalModelPlanBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelExportPlanScope {
    pub mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_kfm_route: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelExportPlanCounts {
    pub catalog_kfm_route_count: usize,
    pub catalog_nif_route_count: usize,
    pub evaluated_kfm_route_count: usize,
    pub parsed_kfm_route_count: usize,
    pub logical_root_count: usize,
    pub ready_logical_root_count: usize,
    pub blocked_logical_root_count: usize,
    pub owned_part_edge_count: usize,
    pub owned_nif_route_count: usize,
    pub standalone_nif_count: usize,
    pub physical_alias_route_count: usize,
    pub physical_alias_owner_count: usize,
    pub unresolved_reference_count: usize,
    pub blocker_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelRouteProvenance {
    pub exact_route: String,
    pub normalized_route: String,
    pub owner: LogicalModelOwner,
}

/// Stable identity of an exact AssetBundle container target. `asset_index` is
/// deliberately absent because it depends on environment load order. Two
/// owners are physical aliases only when the complete serialized asset bytes,
/// PathID and object type all match.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelPhysicalFingerprint {
    pub serialized_asset_sha256: String,
    pub path_id: i64,
    pub object_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelPhysicalAliasProof {
    pub normalized_route: String,
    pub exact_route: String,
    pub kind: LogicalModelKind,
    pub canonical_owner: LogicalModelOwner,
    pub alias_owners: Vec<LogicalModelOwner>,
    pub fingerprint: LogicalModelPhysicalFingerprint,
    pub basis: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LogicalModelResolvedRoute {
    pub(super) provenance: LogicalModelRouteProvenance,
    pub(super) alias_owners: Vec<LogicalModelOwner>,
    pub(super) physical_fingerprint: Option<LogicalModelPhysicalFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LogicalModelOccurrenceFingerprint {
    pub(super) exact_route: String,
    pub(super) owner: LogicalModelOwner,
    pub(super) fingerprint: LogicalModelPhysicalFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NifPointerOwnershipEvidence {
    pub exact_route: String,
    pub normalized_route: String,
    pub container_asset_name: String,
    pub asset_bundle_path_id: i64,
    pub pointer: ResolvedObjectKeyEvidence,
    pub matched_by: String,
    pub matching_preload_table_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogicalModelRootStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelRootPlan {
    pub kfm: LogicalModelRouteProvenance,
    pub status: LogicalModelRootStatus,
    pub payload: KfmPayloadEvidence,
    pub owned_nif_routes: Vec<String>,
    pub unresolved_nif_routes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelOwnedPart {
    pub kfm: LogicalModelRouteProvenance,
    pub reference_exact_route: String,
    pub reference_normalized_route: String,
    pub nif: LogicalModelRouteProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelStandaloneNif {
    pub nif: LogicalModelRouteProvenance,
    pub proof: LogicalModelStandaloneProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelStandaloneProof {
    pub sole_catalog_owner: bool,
    pub exact_route_has_no_collision: bool,
    pub not_referenced_by_any_parsed_kfm: bool,
    pub all_kfm_groups_parsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelUnresolvedReference {
    pub kfm: LogicalModelRouteProvenance,
    pub reference_exact_route: String,
    pub reference_normalized_route: String,
    pub code: String,
    pub candidate_exact_routes: Vec<String>,
    pub candidate_owners: Vec<LogicalModelOwner>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelPlanBlocker {
    pub subject_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_route: Option<String>,
    pub exact_routes: Vec<String>,
    pub owners: Vec<LogicalModelOwner>,
    pub code: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kfm_pointer_traversal: Option<KfmPointerTraversalEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kfm_preload_ownership: Option<KfmPreloadOwnershipEvidence>,
}

/// Builds a plan from the real bundle index and reads payload only after the
/// catalog has proven that a KFM route has one exact spelling and one owner.
pub fn plan_logical_model_exports_from_path(
    bundle_index_path: impl AsRef<Path>,
    options: LogicalModelExportPlanOptions,
) -> Result<LogicalModelExportPlan, String> {
    let bundle_index_path = bundle_index_path.as_ref();
    let catalog = catalog_logical_models_from_path(bundle_index_path)?;
    let project_dir = project_dir_for_bundle_index(bundle_index_path)?;
    let mut resolver = RealKfmResolver::new(project_dir, bundle_index_path)?;
    let resolved_routes = resolver.resolve_physical_route_groups(&catalog);
    Ok(build_logical_model_export_plan_with_resolved_routes(
        &catalog,
        options,
        &resolved_routes,
        |provenance| resolver.resolve(provenance),
    ))
}

/// Pure ownership planner. Tests and alternate offline readers can inject a
/// resolver while retaining exactly the same fail-closed policy.
pub fn build_logical_model_export_plan<F>(
    catalog: &LogicalModelCatalog,
    options: LogicalModelExportPlanOptions,
    resolve_kfm: F,
) -> LogicalModelExportPlan
where
    F: FnMut(&LogicalModelRouteProvenance) -> Result<KfmPayloadEvidence, KfmResolveError>,
{
    let resolved_routes = catalog
        .route_groups
        .iter()
        .map(|group| {
            (
                group.normalized_route.clone(),
                sole_route_provenance(group).map(|provenance| LogicalModelResolvedRoute {
                    provenance,
                    alias_owners: Vec::new(),
                    physical_fingerprint: None,
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    build_logical_model_export_plan_with_resolved_routes(
        catalog,
        options,
        &resolved_routes,
        resolve_kfm,
    )
}

pub(super) fn build_logical_model_export_plan_with_resolved_routes<F>(
    catalog: &LogicalModelCatalog,
    options: LogicalModelExportPlanOptions,
    resolved_routes: &BTreeMap<String, Result<LogicalModelResolvedRoute, LogicalModelPlanBlocker>>,
    mut resolve_kfm: F,
) -> LogicalModelExportPlan
where
    F: FnMut(&LogicalModelRouteProvenance) -> Result<KfmPayloadEvidence, KfmResolveError>,
{
    let requested_kfm_route = options
        .requested_kfm_route
        .as_deref()
        .map(normalize_logical_model_route);
    let full_scope = requested_kfm_route.is_none();
    let groups_by_route = catalog
        .route_groups
        .iter()
        .map(|group| (group.normalized_route.as_str(), group))
        .collect::<BTreeMap<_, _>>();
    let mut selected_kfm_groups = catalog
        .route_groups
        .iter()
        .filter(|group| group.kind == LogicalModelKind::Kfm)
        .filter(|group| {
            requested_kfm_route
                .as_ref()
                .is_none_or(|requested| group.normalized_route == *requested)
        })
        .collect::<Vec<_>>();
    selected_kfm_groups.sort_by(|left, right| {
        left.preferred_owner
            .as_ref()
            .map(|owner| (&owner.bundle_path, &owner.asset_name))
            .cmp(
                &right
                    .preferred_owner
                    .as_ref()
                    .map(|owner| (&owner.bundle_path, &owner.asset_name)),
            )
            .then_with(|| left.normalized_route.cmp(&right.normalized_route))
    });

    let mut roots = Vec::new();
    let mut owned_parts = Vec::new();
    let mut unresolved = Vec::new();
    let mut blockers = Vec::new();
    let mut referenced_nif_routes = BTreeSet::new();
    let mut ownership_scan_complete = full_scope;
    let mut parsed_kfm_route_count = 0;
    let physical_aliases = resolved_routes
        .values()
        .filter_map(|resolution| resolution.as_ref().ok())
        .filter(|resolution| !resolution.alias_owners.is_empty())
        .filter_map(|resolution| {
            let group = groups_by_route.get(resolution.provenance.normalized_route.as_str())?;
            Some(LogicalModelPhysicalAliasProof {
                normalized_route: resolution.provenance.normalized_route.clone(),
                exact_route: resolution.provenance.exact_route.clone(),
                kind: group.kind,
                canonical_owner: resolution.provenance.owner.clone(),
                alias_owners: resolution.alias_owners.clone(),
                fingerprint: resolution.physical_fingerprint.clone()?,
                basis: "serializedAssetSha256PathIdAndObjectType",
            })
        })
        .collect::<Vec<_>>();

    if requested_kfm_route.is_some() && selected_kfm_groups.is_empty() {
        ownership_scan_complete = false;
        blockers.push(LogicalModelPlanBlocker {
            subject_kind: "kfm".to_string(),
            normalized_route: requested_kfm_route.clone(),
            exact_routes: Vec::new(),
            owners: Vec::new(),
            code: "requestedKfmRouteMissing".to_string(),
            detail: "the requested KFM route is not present in the catalog".to_string(),
            kfm_pointer_traversal: None,
            kfm_preload_ownership: None,
        });
    }

    for group in &selected_kfm_groups {
        let provenance = match resolved_route_for_group(group, resolved_routes) {
            Ok(resolution) => resolution.provenance,
            Err(blocker) => {
                ownership_scan_complete = false;
                blockers.push(blocker);
                continue;
            }
        };

        let mut payload = match resolve_kfm(&provenance) {
            Ok(payload) => payload,
            Err(err) => {
                ownership_scan_complete = false;
                let mut blocker = blocker_for_provenance("kfm", &provenance, err.code, err.detail);
                blocker.kfm_pointer_traversal = err.pointer_traversal;
                blocker.kfm_preload_ownership = err.preload_ownership;
                blockers.push(blocker);
                continue;
            }
        };
        payload.references.sort();
        payload.references.dedup();
        payload.nif_references = payload
            .references
            .iter()
            .filter(|reference| normalize_logical_model_route(reference).ends_with(".nif"))
            .cloned()
            .collect();
        payload.nif_references.sort();
        payload.nif_references.dedup();
        if payload.nif_references.is_empty() && payload.self_contained_game_object.is_none() {
            ownership_scan_complete = false;
            let mut blocker = blocker_for_provenance(
                "kfm",
                &provenance,
                "kfmNoNifReferences",
                "the payload parser found no NIF reference and no exact self-contained GameObject proof; ownership cannot be inferred",
            );
            blocker.kfm_pointer_traversal = Some(payload.pointer_traversal.clone());
            blockers.push(blocker);
            continue;
        }
        parsed_kfm_route_count += 1;

        let mut root_owned_routes = Vec::new();
        let mut root_unresolved_routes = Vec::new();
        for reference_exact_route in &payload.nif_references {
            let reference_normalized_route = normalize_logical_model_route(reference_exact_route);
            referenced_nif_routes.insert(reference_normalized_route.clone());
            let Some(nif_group) = groups_by_route.get(reference_normalized_route.as_str()) else {
                root_unresolved_routes.push(reference_normalized_route.clone());
                unresolved.push(LogicalModelUnresolvedReference {
                    kfm: provenance.clone(),
                    reference_exact_route: reference_exact_route.clone(),
                    reference_normalized_route,
                    code: "referencedNifMissingFromCatalog".to_string(),
                    candidate_exact_routes: Vec::new(),
                    candidate_owners: Vec::new(),
                });
                continue;
            };
            match resolved_route_for_group(nif_group, resolved_routes) {
                Ok(resolution) => {
                    let nif = resolution.provenance;
                    root_owned_routes.push(nif.normalized_route.clone());
                    owned_parts.push(LogicalModelOwnedPart {
                        kfm: provenance.clone(),
                        reference_exact_route: reference_exact_route.clone(),
                        reference_normalized_route,
                        nif,
                    });
                }
                Err(blocker) => {
                    root_unresolved_routes.push(reference_normalized_route.clone());
                    unresolved.push(LogicalModelUnresolvedReference {
                        kfm: provenance.clone(),
                        reference_exact_route: reference_exact_route.clone(),
                        reference_normalized_route,
                        code: match blocker.code.as_str() {
                            "nifOwnerAmbiguous" => "referencedNifOwnerAmbiguous".to_string(),
                            "nifRouteCollision" => "referencedNifRouteCollision".to_string(),
                            _ => blocker.code.clone(),
                        },
                        candidate_exact_routes: nif_group.exact_routes.clone(),
                        candidate_owners: nif_group.owners.clone(),
                    });
                    blockers.push(blocker);
                }
            }
        }
        root_owned_routes.sort();
        root_owned_routes.dedup();
        root_unresolved_routes.sort();
        root_unresolved_routes.dedup();
        roots.push(LogicalModelRootPlan {
            kfm: provenance,
            status: if root_unresolved_routes.is_empty() {
                LogicalModelRootStatus::Ready
            } else {
                LogicalModelRootStatus::Blocked
            },
            payload,
            owned_nif_routes: root_owned_routes,
            unresolved_nif_routes: root_unresolved_routes,
        });
    }

    if !full_scope {
        ownership_scan_complete = false;
    }

    let mut standalone_nifs = Vec::new();
    let mut recorded_nif_blockers = BTreeSet::new();
    for group in catalog
        .route_groups
        .iter()
        .filter(|group| group.kind == LogicalModelKind::Nif)
    {
        let resolution = match resolved_route_for_group(group, resolved_routes) {
            Ok(resolution) => resolution,
            Err(blocker) => {
                if recorded_nif_blockers
                    .insert((blocker.code.clone(), group.normalized_route.clone()))
                {
                    blockers.push(blocker);
                }
                continue;
            }
        };
        let provenance = resolution.provenance;
        if referenced_nif_routes.contains(&group.normalized_route) {
            continue;
        }
        if ownership_scan_complete {
            standalone_nifs.push(LogicalModelStandaloneNif {
                nif: provenance,
                proof: LogicalModelStandaloneProof {
                    sole_catalog_owner: resolution.alias_owners.is_empty(),
                    exact_route_has_no_collision: true,
                    not_referenced_by_any_parsed_kfm: true,
                    all_kfm_groups_parsed: true,
                },
            });
        }
    }

    if !ownership_scan_complete {
        blockers.push(LogicalModelPlanBlocker {
            subject_kind: "catalog".to_string(),
            normalized_route: None,
            exact_routes: Vec::new(),
            owners: Vec::new(),
            code: "standaloneNifProofIncomplete".to_string(),
            detail: "at least one KFM group was ambiguous, collided, unparsed, or outside the requested scope; no NIF is planned as standalone"
                .to_string(),
            kfm_pointer_traversal: None,
            kfm_preload_ownership: None,
        });
    }

    roots.sort_by(|left, right| left.kfm.cmp(&right.kfm));
    owned_parts.sort_by(|left, right| {
        left.kfm
            .cmp(&right.kfm)
            .then_with(|| left.nif.cmp(&right.nif))
            .then_with(|| left.reference_exact_route.cmp(&right.reference_exact_route))
    });
    standalone_nifs.sort_by(|left, right| left.nif.cmp(&right.nif));
    unresolved.sort_by(|left, right| {
        left.kfm
            .cmp(&right.kfm)
            .then_with(|| {
                left.reference_normalized_route
                    .cmp(&right.reference_normalized_route)
            })
            .then_with(|| left.reference_exact_route.cmp(&right.reference_exact_route))
    });
    blockers.sort_by(|left, right| {
        left.subject_kind
            .cmp(&right.subject_kind)
            .then_with(|| left.normalized_route.cmp(&right.normalized_route))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.detail.cmp(&right.detail))
    });
    blockers.dedup();

    let ready_logical_root_count = roots
        .iter()
        .filter(|root| root.status == LogicalModelRootStatus::Ready)
        .count();
    let owned_nif_route_count = owned_parts
        .iter()
        .map(|part| part.nif.normalized_route.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let counts = LogicalModelExportPlanCounts {
        catalog_kfm_route_count: catalog.counts.unique_kfm_route_count,
        catalog_nif_route_count: catalog.counts.unique_nif_route_count,
        evaluated_kfm_route_count: selected_kfm_groups.len(),
        parsed_kfm_route_count,
        logical_root_count: roots.len(),
        ready_logical_root_count,
        blocked_logical_root_count: roots.len() - ready_logical_root_count,
        owned_part_edge_count: owned_parts.len(),
        owned_nif_route_count,
        standalone_nif_count: standalone_nifs.len(),
        physical_alias_route_count: physical_aliases.len(),
        physical_alias_owner_count: physical_aliases
            .iter()
            .map(|alias| alias.alias_owners.len())
            .sum(),
        unresolved_reference_count: unresolved.len(),
        blocker_count: blockers.len(),
    };

    LogicalModelExportPlan {
        schema: LOGICAL_MODEL_EXPORT_PLAN_SCHEMA,
        source_index_path: catalog.source_index_path.clone(),
        scope: LogicalModelExportPlanScope {
            mode: if full_scope { "full" } else { "kfm-route" },
            requested_kfm_route,
        },
        emits_glb: false,
        ownership_scan_complete,
        standalone_nif_proof_complete: ownership_scan_complete,
        counts,
        physical_aliases,
        logical_roots: roots,
        owned_parts,
        standalone_nifs,
        unresolved,
        blockers,
    }
}

#[derive(Debug)]
pub(super) struct NifContainerOccurrence {
    pub(super) exact_route: String,
    pub(super) container_asset_name: String,
    pub(super) asset_bundle_path_id: i64,
    pub(super) key: (usize, i64),
}

#[derive(Debug, Default)]
pub(super) struct NifContainerGroup {
    pub(super) exact_routes: BTreeSet<String>,
    pub(super) occurrence_count: usize,
    pub(super) unresolved_occurrence_count: usize,
    pub(super) resolved: Vec<NifContainerOccurrence>,
}
