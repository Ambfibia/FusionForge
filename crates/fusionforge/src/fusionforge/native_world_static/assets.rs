use super::*;

pub(super) const CATALOG_SCHEMA: &str = "ffone.native-static-world-catalog.v1";

pub(super) const MANIFEST_SCHEMA: &str = "ffone.native-static-world-manifest.v1";

pub(super) fn append_index_accessor(
    binary: &mut Vec<u8>,
    views: &mut Vec<JsonValue>,
    accessors: &mut Vec<JsonValue>,
    values: &[u32],
) -> Result<usize, String> {
    pad_four(binary, 0);
    let offset = binary.len();
    let maximum = values.iter().copied().max().unwrap_or(0);
    let component_type = if maximum <= u16::MAX as u32 {
        for value in values {
            binary.extend_from_slice(&(*value as u16).to_le_bytes());
        }
        5123
    } else {
        for value in values {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        5125
    };
    let byte_length = binary
        .len()
        .checked_sub(offset)
        .ok_or_else(|| "index buffer range underflow".to_string())?;
    let view = views.len();
    views.push(json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": byte_length,
        "target": 34963
    }));
    let accessor = accessors.len();
    accessors.push(json!({
        "bufferView": view,
        "byteOffset": 0,
        "componentType": component_type,
        "count": values.len(),
        "type": "SCALAR",
        "min": [values.iter().copied().min().unwrap_or(0)],
        "max": [maximum]
    }));
    Ok(accessor)
}

pub(super) fn verify_static_scene_against_catalog(
    asset_root: &Path,
    layout: &TileLayout,
    scene: &JsonValue,
    catalog: &JsonValue,
) -> Result<(), String> {
    let counts = catalog
        .get("counts")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| "exact static-world catalog has no counts".to_string())?;
    let expected = [
        ("models", "exportedModels"),
        ("visuals", "runtimeVisuals"),
        ("colliders", "runtimeColliders"),
    ];
    for (scene_field, count_field) in expected {
        let observed = scene
            .get(scene_field)
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("native scene has no {scene_field} array"))?
            .len() as u64;
        let expected = counts
            .get(count_field)
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| format!("exact static-world catalog has no {count_field}"))?;
        if observed != expected {
            return Err(format!(
                "native scene {} count {observed} differs from exact catalog {expected}",
                scene_field
            ));
        }
    }

    let model_prefix = format!("static-{}-", layout.tile_id);
    let path_prefix = format!("{}/", layout.model_relative_root);
    let mut model_ids = BTreeSet::new();
    for model in scene["models"].as_array().expect("validated models array") {
        let id = model.get("id").and_then(JsonValue::as_str).unwrap_or("");
        let path = model.get("path").and_then(JsonValue::as_str).unwrap_or("");
        if !id.starts_with(&model_prefix)
            || !path.starts_with(&path_prefix)
            || !model_ids.insert(id)
        {
            return Err(format!(
                "native scene contains a non-exact or duplicate static model {id:?} at {path:?}"
            ));
        }
    }
    for field in ["visuals", "colliders"] {
        for instance in scene[field]
            .as_array()
            .expect("validated static instance array")
        {
            let model = instance
                .get("model")
                .and_then(JsonValue::as_str)
                .unwrap_or("");
            if !model_ids.contains(model) {
                return Err(format!(
                    "native scene {field} references non-exact model {model:?}"
                ));
            }
        }
    }

    for field in ["hierarchy", "materials"] {
        let entry = catalog
            .get(field)
            .and_then(JsonValue::as_object)
            .ok_or_else(|| format!("exact static-world catalog has no {field} proof"))?;
        let relative = entry
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("exact static-world catalog {field} has no path"))?;
        let expected_hash = entry
            .get("blake3")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("exact static-world catalog {field} has no blake3"))?;
        if !relative.starts_with(&format!("{}/", layout.static_relative_root)) {
            return Err(format!(
                "exact static-world catalog {field} path {relative:?} escapes its tile"
            ));
        }
        let path = join_relative(asset_root, relative)?;
        let bytes = fs::read(&path).map_err(|err| {
            format!(
                "could not read exact {field} proof {}: {err}",
                path.display()
            )
        })?;
        if !matches_published_bytes(&bytes, expected_hash) {
            return Err(format!(
                "exact static-world {field} proof {} differs from catalog hash",
                path.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn build_output_manifest(
    root: &Path,
    source_build: &str,
    tile_id: &str,
    manifest_relative: &str,
) -> Result<(JsonValue, usize, u64), String> {
    let mut paths = collect_regular_files(root)?;
    paths.retain(|path| path != manifest_relative);
    let mut files = Vec::with_capacity(paths.len());
    let mut bytes = 0u64;
    for relative in paths {
        let path = join_relative(root, &relative)?;
        let payload = fs::read(&path)
            .map_err(|err| format!("could not read manifest input {}: {err}", path.display()))?;
        bytes = bytes
            .checked_add(payload.len() as u64)
            .ok_or_else(|| "manifest byte count overflow".to_string())?;
        files.push(json!({
            "path": relative,
            "byteLength": payload.len(),
            "blake3": blake3_hex(&payload),
            "kind": output_file_kind(&relative),
        }));
    }
    let count = files.len();
    Ok((
        json!({
            "schema": MANIFEST_SCHEMA,
            "sourceBuild": source_build,
            "tileId": tile_id,
            "selfExcluded": manifest_relative,
            "files": files,
            "counts": {
                "files": count,
                "bytes": bytes,
            }
        }),
        count,
        bytes,
    ))
}

pub(super) fn finalize_report_and_manifest(
    root: &Path,
    report_relative: &str,
    manifest_relative: &str,
    source_build: &str,
    tile_id: &str,
    report: &mut NativeStaticWorldExportReport,
) -> Result<(), String> {
    if report.manifest_path != manifest_relative {
        return Err(format!(
            "report manifest path {:?} differs from final manifest path {manifest_relative:?}",
            report.manifest_path
        ));
    }
    let report_path = join_relative(root, report_relative)?;
    let manifest_path = join_relative(root, manifest_relative)?;
    for pass in 0..8 {
        let report_bytes = pretty_json_bytes(report)?;
        if pass == 0 {
            write_new_file(&report_path, &report_bytes)?;
        } else {
            replace_regular_file(&report_path, &report_bytes)?;
        }

        let (manifest_document, file_count, byte_count) =
            build_output_manifest(root, source_build, tile_id, manifest_relative)?;
        let manifest_bytes = pretty_json_bytes(&manifest_document)?;
        if pass == 0 {
            write_new_file(&manifest_path, &manifest_bytes)?;
        } else {
            replace_regular_file(&manifest_path, &manifest_bytes)?;
        }

        let output_files = file_count
            .checked_add(1)
            .ok_or_else(|| "output file count overflow".to_string())?;
        let output_bytes = byte_count
            .checked_add(manifest_bytes.len() as u64)
            .ok_or_else(|| "output byte count overflow".to_string())?;
        if report.counts.output_files == output_files && report.counts.output_bytes == output_bytes
        {
            return Ok(());
        }
        report.counts.output_files = output_files;
        report.counts.output_bytes = output_bytes;
    }
    Err("export report/manifest byte counts did not converge".to_string())
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
