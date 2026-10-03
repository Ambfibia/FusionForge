use super::*;

pub(super) fn sanitize_terrain(
    path: &str,
    original: &[u8],
    environment_path: &str,
    environment_blake3: &str,
) -> Result<(Vec<u8>, u64)> {
    let mut value: Value =
        serde_json::from_slice(original).map_err(|source| PipelineError::Json {
            path: path.to_owned(),
            source,
        })?;
    let root = object_mut(&mut value, path)?;
    let mut removed = 0_u64;
    removed += remove_required(root, "source", path)?;
    removed += remove_required(root, "sceneInstance", path)?;
    if let Some(gameplay) = root
        .get_mut("gameplayAttributes")
        .and_then(Value::as_object_mut)
    {
        removed += remove_optional(gameplay, "rawParsedDocument");
        removed += remove_optional(gameplay, "source");
    }
    if let Some(detail) = root
        .get_mut("detailAndTrees")
        .and_then(Value::as_object_mut)
    {
        removed += remove_optional(detail, "assetClosure");
        removed += remove_optional(detail, "rawDocument");
        if let Some(trees) = detail.get_mut("trees").and_then(Value::as_object_mut) {
            removed += remove_optional(trees, "rawDocument");
        }
    }
    if let Some(splat) = root.get_mut("splat").and_then(Value::as_object_mut) {
        if let Some(weights) = splat.get_mut("weightMaps").and_then(Value::as_array_mut) {
            for weight in weights {
                if let Some(weight) = weight.as_object_mut() {
                    removed += remove_optional(weight, "source");
                    removed += strip_mip_source_evidence(weight);
                }
            }
        }
        if let Some(layers) = splat.get_mut("layers").and_then(Value::as_array_mut) {
            for layer in layers {
                if let Some(layer) = layer.as_object_mut() {
                    removed += remove_optional(layer, "modeSource");
                    removed += remove_optional(layer, "modeEvidence");
                    if let Some(albedo) = layer.get_mut("albedo").and_then(Value::as_object_mut) {
                        removed += remove_optional(albedo, "source");
                        removed += strip_mip_source_evidence(albedo);
                    }
                }
            }
        }
    }
    if let Some(lightmap) = root.get_mut("lightmap").and_then(Value::as_object_mut) {
        removed += remove_optional(lightmap, "sourcePointer");
        removed += remove_optional(lightmap, "source");
        removed += remove_optional(lightmap, "runtimeSelection");
        removed += strip_mip_source_evidence(lightmap);
    }

    let environment = child_object_mut(root, "environment", path)?;
    let relative_environment = environment
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error(format!("{path:?} environment has no path")))?;
    let resolved_environment = resolve_document_reference(path, relative_environment)?;
    if resolved_environment != environment_path {
        return invalid(format!(
            "{path:?} environment path resolves to {resolved_environment:?}, expected {environment_path:?}"
        ));
    }
    environment.insert(
        "blake3".to_owned(),
        Value::String(format!("blake3:{environment_blake3}")),
    );
    replace_provenance_words(&mut value);
    let bytes = pretty_value_bytes(&value, Path::new(path))?;
    audit_sanitized_json(path, &bytes)?;
    Ok((bytes, removed))
}
