use super::*;

pub(super) fn validate_enrichment_source_documents(
    manifest: &JsonValue,
    publication_plan: &JsonValue,
) -> Result<(), String> {
    if manifest.get("schema").and_then(JsonValue::as_str) != Some(BATCH_SCHEMA) {
        return Err(format!(
            "enrichment source manifest schema must be {BATCH_SCHEMA}"
        ));
    }
    if publication_plan.get("schema").and_then(JsonValue::as_str) != Some(PUBLICATION_PLAN_SCHEMA) {
        return Err(format!(
            "enrichment source publication plan schema must be {PUBLICATION_PLAN_SCHEMA}"
        ));
    }
    let exported = manifest
        .get("exported")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "enrichment source manifest has no exported array".to_string())?;
    let entries = publication_plan
        .get("entries")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "enrichment source publication plan has no entries array".to_string())?;
    if exported.is_empty() || exported.len() != entries.len() {
        return Err(format!(
            "enrichment source exported/plan entry counts disagree: {} vs {}",
            exported.len(),
            entries.len()
        ));
    }
    if manifest.get("enrichment").is_some() {
        return Err("source batch is already enriched; refusing enrichment chaining".to_string());
    }
    Ok(())
}

pub(super) fn reject_existing_output(output: &Path) -> Result<(), String> {
    if output.exists() {
        Err(format!(
            "native terrain batch output must not already exist: {}",
            output.display()
        ))
    } else {
        Ok(())
    }
}
