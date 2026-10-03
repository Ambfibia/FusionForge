use super::*;

pub(super) fn publish_staged_routes(
    staging_root: &Path,
    mut routes: Vec<StagedRoute>,
) -> Result<PublicationResult, String> {
    // A few authoritative XDT routes point at physically distinct GameObjects
    // that intentionally share the same root m_Name. The route stem is the
    // table-owned semantic distinction, so qualify only those collision groups
    // rather than inventing a hash/PathID suffix or overwriting a sibling.
    let mut base_groups = BTreeMap::<String, BTreeSet<(String, EquipmentPhysicalTarget)>>::new();
    let mut base_route_counts = BTreeMap::<String, usize>::new();
    for route in &routes {
        *base_route_counts.entry(destination_key(route)).or_default() += 1;
        base_groups
            .entry(destination_key(route))
            .or_default()
            .insert((
                portable_key(&route.candidate.owner.bundle_path),
                route.physical_target.clone(),
            ));
    }
    let qualified_destinations = base_groups
        .into_iter()
        .filter_map(|(destination, physical)| (physical.len() > 1).then_some(destination))
        .collect::<BTreeSet<_>>();
    for route in &mut routes {
        let base_destination = destination_key(route);
        let route_qualifier = route_stem_qualifier(&route.candidate.route.exact_route)?;
        let is_unproven_single_route_alias = base_route_counts
            .get(&base_destination)
            .copied()
            .unwrap_or_default()
            == 1
            && portable_key(&route_qualifier) != portable_key(&route.safe_true_name);
        if qualified_destinations.contains(&base_destination) || is_unproven_single_route_alias {
            route.route_qualifier = Some(route_qualifier);
        }
    }
    routes.sort_by(|left, right| {
        destination_key(left)
            .cmp(&destination_key(right))
            .then_with(|| {
                left.candidate
                    .route
                    .normalized_route
                    .cmp(&right.candidate.route.normalized_route)
            })
    });
    let mut by_destination = BTreeMap::<String, Vec<StagedRoute>>::new();
    for route in routes {
        by_destination
            .entry(destination_key(&route))
            .or_default()
            .push(route);
    }
    let mut exported = Vec::new();
    let mut blockers = Vec::new();
    for group in by_destination.into_values() {
        let physical = group
            .iter()
            .map(|route| {
                (
                    portable_key(&route.candidate.owner.bundle_path),
                    route.physical_target.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        if physical.len() != 1 {
            for route in group {
                blockers.push(EquipmentModelSourceBlocker {
                    code: "distinctPhysicalEquipmentTrueNameCollision".to_string(),
                    category: route.candidate.route.category.clone(),
                    exact_route: route.candidate.route.exact_route.clone(),
                    normalized_route: route.candidate.route.normalized_route.clone(),
                    table_rows: route.candidate.route.table_rows.clone(),
                    owners: vec![route.candidate.owner.clone()],
                    detail: format!(
                        "true m_Name {:?} collides inside category {:?}, but direct container targets differ",
                        route.true_name, route.candidate.route.category
                    ),
                    evidence: json!({
                        "portableDestination": destination_key(&route),
                        "distinctPhysicalTargets": physical,
                    }),
                    required_evidence: vec![
                        "an authoritative semantic distinction expressible without a hash or PathID directory"
                            .to_string(),
                    ],
                    disposition: "blocked-no-hash-suffix-or-overwrite".to_string(),
                });
                let _ = fs::remove_file(route.temporary_path);
            }
            continue;
        }
        let canonical = &group[0];
        let relative = source_relative_path(canonical);
        let destination = staging_root.join(&relative);
        let parent = destination
            .parent()
            .ok_or_else(|| format!("equipment source has no parent: {}", destination.display()))?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
        fs::rename(&canonical.temporary_path, &destination).map_err(|err| {
            format!(
                "could not publish staged equipment source {}: {err}",
                destination.display()
            )
        })?;
        let mut alias_routes = group
            .iter()
            .skip(1)
            .map(|route| route.candidate.route.exact_route.clone())
            .collect::<Vec<_>>();
        alias_routes.sort();
        let mut table_rows = group
            .iter()
            .flat_map(|route| route.candidate.route.table_rows.clone())
            .collect::<Vec<_>>();
        table_rows.sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
        table_rows.dedup_by(|left, right| left.entity_id == right.entity_id);
        exported.push(EquipmentModelSourceExported {
            category: canonical.candidate.route.category.clone(),
            true_name: canonical.true_name.clone(),
            safe_true_name: canonical.safe_true_name.clone(),
            route_qualifier: canonical.route_qualifier.clone(),
            source_relative_path: slash_path(&relative),
            source_byte_length: canonical.source_byte_length,
            source_sha256: canonical.source_sha256.clone(),
            canonical_route: canonical.candidate.route.exact_route.clone(),
            normalized_route: canonical.candidate.route.normalized_route.clone(),
            proven_alias_routes: alias_routes,
            table_rows,
            owner: canonical.candidate.owner.clone(),
            physical_target: canonical.physical_target.clone(),
            facts: canonical.facts.clone(),
        });
        for alias in group.into_iter().skip(1) {
            fs::remove_file(&alias.temporary_path).map_err(|err| {
                format!(
                    "could not remove proven alias source {}: {err}",
                    alias.temporary_path.display()
                )
            })?;
        }
    }
    exported.sort_by(|left, right| left.source_relative_path.cmp(&right.source_relative_path));
    Ok(PublicationResult { exported, blockers })
}

pub(super) fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|err| format!("could not encode {}: {err}", path.display()))?;
    bytes.push(b'\n');
    write_new_bytes(path, &bytes)
}

pub(super) fn write_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
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
