use super::*;

pub(super) fn apply_supplemental_blockers(value: &Value, evidence: &mut LegacyEvidence) {
    for field in ["blocked", "blockers"] {
        for blocker in value
            .get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let code = blocker
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("unknown_primary_blocker");
            let mut routes = BTreeSet::new();
            if let Some(route) = blocker.get("exactRoute").and_then(Value::as_str) {
                routes.insert(route.replace('\\', "/").to_lowercase());
            }
            for route in blocker
                .get("exactRoutes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                routes.insert(route.replace('\\', "/").to_lowercase());
            }
            if let Some(logical_name) = blocker.get("logicalName").and_then(Value::as_str) {
                routes.insert(normalized_route(logical_name));
            }
            for route in routes {
                if !route.ends_with(".kfm") {
                    continue;
                }
                evidence.ready.remove(&route);
                evidence
                    .blockers
                    .entry(route)
                    .or_default()
                    .insert(code.to_owned());
            }
        }
    }
}
