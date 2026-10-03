use super::*;

pub(super) fn resolve_routes(
    route_plans: &[RoutePlan],
    occurrences: &BTreeMap<String, Vec<EquipmentContainerOwner>>,
) -> (Vec<RouteCandidate>, Vec<EquipmentModelSourceBlocker>) {
    let mut candidates = Vec::new();
    let mut blockers = Vec::new();
    for route in route_plans {
        let owners = occurrences
            .get(&route.normalized_route)
            .cloned()
            .unwrap_or_default();
        match owners.as_slice() {
            [owner] => candidates.push(RouteCandidate {
                route: route.clone(),
                owner: owner.clone(),
            }),
            [] => blockers.push(EquipmentModelSourceBlocker {
                code: "equipmentRouteMissingFromBundleIndex".to_string(),
                category: route.category.clone(),
                exact_route: route.exact_route.clone(),
                normalized_route: route.normalized_route.clone(),
                table_rows: route.table_rows.clone(),
                owners,
                detail:
                    "the exact table-owned NIF route does not occur in any indexed build container"
                        .to_string(),
                evidence: json!({
                    "lookup": "case-insensitive slash-normalized exact container route",
                    "occurrenceCount": 0,
                }),
                required_evidence: vec![
                    "a build AssetBundle container entry with this exact normalized route"
                        .to_string(),
                    "the direct serialized container pointer and its one GameObject root"
                        .to_string(),
                ],
                disposition: "blocked-no-similar-name-or-model-fallback".to_string(),
            }),
            _ => blockers.push(EquipmentModelSourceBlocker {
                code: "equipmentRouteAmbiguousInBundleIndex".to_string(),
                category: route.category.clone(),
                exact_route: route.exact_route.clone(),
                normalized_route: route.normalized_route.clone(),
                table_rows: route.table_rows.clone(),
                owners: owners.clone(),
                detail: format!(
                    "the exact table-owned NIF route occurs in {} distinct indexed owners",
                    owners.len()
                ),
                evidence: json!({
                    "lookup": "case-insensitive slash-normalized exact container route",
                    "occurrenceCount": owners.len(),
                }),
                required_evidence: vec![
                    "an authoritative table/build rule selecting exactly one physical owner"
                        .to_string(),
                ],
                disposition: "blocked-no-owner-precedence-guess".to_string(),
            }),
        }
    }
    (candidates, blockers)
}
