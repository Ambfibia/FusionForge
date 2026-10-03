use super::*;

pub(super) fn load_legacy_evidence(path: &Path) -> Result<LegacyEvidence, String> {
    let value: Value = serde_json::from_slice(&read_file(path)?)
        .map_err(|error| format!("cannot parse legacy plan {}: {error}", path.display()))?;
    let mut evidence = LegacyEvidence::default();
    for root in value
        .get("logicalRoots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(route) = root
            .pointer("/kfm/exactRoute")
            .and_then(Value::as_str)
            .map(|route| route.replace('\\', "/").to_lowercase())
        {
            evidence.ready.insert(route);
        }
    }
    for blocker in value
        .get("blockers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let code = blocker
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("unknown_primary_blocker");
        for route in blocker
            .get("exactRoutes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if route.to_ascii_lowercase().ends_with(".kfm") {
                evidence
                    .blockers
                    .entry(route.replace('\\', "/").to_lowercase())
                    .or_default()
                    .insert(code.to_owned());
            }
        }
    }
    Ok(evidence)
}

pub(super) fn load_supplemental_blockers(path: &Path, evidence: &mut LegacyEvidence) -> Result<(), String> {
    let value: Value = serde_json::from_slice(&read_file(path)?)
        .map_err(|error| format!("cannot parse blocker report {}: {error}", path.display()))?;
    apply_supplemental_blockers(&value, evidence);
    Ok(())
}

pub(super) fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}
