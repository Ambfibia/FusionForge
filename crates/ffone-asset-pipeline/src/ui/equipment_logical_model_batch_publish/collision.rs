use super::*;

pub(super) fn global_collision_blockers(
    plans: &[EquipmentPlan],
) -> Result<BTreeMap<usize, EquipmentLogicalModelBlocker>> {
    let mut claims = Vec::new();
    for (index, plan) in plans.iter().enumerate() {
        for output in &plan.output_files {
            register_output_claims(&mut claims, Some(index), output)?;
        }
    }
    register_output_claims(
        &mut claims,
        None,
        Path::new(EQUIPMENT_LOGICAL_MODEL_BATCH_REPORT_FILE),
    )?;
    collision_blockers_from_claims(plans, &claims)
}

pub(super) fn collision_blockers_from_claims(
    plans: &[EquipmentPlan],
    claims: &[(String, OutputClaim)],
) -> Result<BTreeMap<usize, EquipmentLogicalModelBlocker>> {
    let mut by_key = BTreeMap::<String, BTreeSet<OutputClaim>>::new();
    for (key, claim) in claims {
        by_key.entry(key.clone()).or_default().insert(claim.clone());
    }
    let mut evidence = BTreeMap::<usize, Vec<Value>>::new();
    for (portable_key, group) in by_key {
        let file_claims = group.iter().filter(|claim| claim.kind == "file").count();
        let kinds = group
            .iter()
            .map(|claim| claim.kind)
            .collect::<BTreeSet<_>>();
        let directory_spellings = group
            .iter()
            .filter(|claim| claim.kind == "directory")
            .map(|claim| claim.exact.as_str())
            .collect::<BTreeSet<_>>();
        let conflict = file_claims > 1 || kinds.len() > 1 || directory_spellings.len() > 1;
        if !conflict {
            continue;
        }
        let group_value = json!({
            "portableKey": portable_key,
            "claims": group.iter().map(|claim| json!({
                "ownerIndex": claim.owner,
                "exactPath": claim.exact,
                "kind": claim.kind,
            })).collect::<Vec<_>>(),
        });
        for owner in group.iter().filter_map(|claim| claim.owner) {
            evidence.entry(owner).or_default().push(group_value.clone());
        }
    }

    let mut blockers = BTreeMap::new();
    for (index, mut groups) in evidence {
        groups.sort_by_key(|value| {
            value
                .get("portableKey")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        });
        let plan = plans
            .get(index)
            .ok_or_else(|| equipment_error_value("collision owner index is out of range"))?;
        blockers.insert(
            index,
            EquipmentLogicalModelBlocker {
                stage: "global-output-preflight".to_owned(),
                code: "portableSemanticOutputCollision".to_owned(),
                upstream_code: None,
                category: plan.export.category.clone(),
                exact_route: plan.export.canonical_route.clone(),
                true_name: Some(plan.export.true_name.clone()),
                source: Some(plan.export.source_relative_path.clone()),
                source_sha256: Some(plan.export.source_sha256.clone()),
                detail: "semantic GLB/texture/sidecar paths collide after Windows Unicode/case folding"
                    .to_owned(),
                evidence: json!({
                    "schema": "ffone.portable-semantic-output-collision.v1",
                    "conflicts": groups,
                    "fallbackApplied": false,
                }),
                required_evidence: vec![
                    "an authoritative semantic distinction that changes the true source name or category without hashes, PathIDs or guessed suffixes".to_owned(),
                ],
                disposition: "blocked-no-overwrite-or-generated-suffix".to_owned(),
            },
        );
    }
    Ok(blockers)
}
