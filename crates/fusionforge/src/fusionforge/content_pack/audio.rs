use super::*;

pub(super) fn cook_audio(
    body: &UnityValue,
    name: &str,
    state: &mut CookState,
) -> Result<EmittedAsset, ObjectCookError> {
    let bytes = body
        .get("audio data")
        .and_then(UnityValue::as_bytes)
        .or_else(|| body.get("m_AudioData").and_then(UnityValue::as_bytes))
        .ok_or_else(|| ObjectCookError::Object("AudioClip has no embedded payload".to_string()))?;
    if !is_valid_ogg(bytes) {
        return Err(ObjectCookError::Object(
            "AudioClip payload is not a valid Ogg stream".to_string(),
        ));
    }
    state
        .emit_validated_asset(
            "audio",
            semantic_or(name, "audio"),
            "ogg",
            ContentKind::Audio,
            bytes,
        )
        .map_err(ObjectCookError::Output)
}

pub(super) fn process_audio_overlay(
    project: &Path,
    ffpatch: &JsonValue,
    state: &mut CookState,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
    mappings: &mut Vec<CookMapping>,
    roots: &ErrorRoots<'_>,
) -> Result<(), String> {
    let audio_rel = config_relative(ffpatch, "AudioDir", "audio", errors)?;
    let audio_root = project.join(&audio_rel);
    let manifest_path = audio_root.join("manifest.json");
    if !manifest_path.is_file() {
        return Ok(());
    }
    let manifest = read_json_value(&manifest_path).map_err(|err| roots.sanitize(&err))?;
    require_manifest_format(&manifest, "fftools.audio-patch.v1", "audio")?;
    let entries = require_entries(&manifest, "audio")?;
    let mut seen_files = BTreeSet::new();
    let mut seen_semantics = BTreeSet::new();
    for entry in entries {
        state.coverage.overlay_entries_scanned += 1;
        let relative = required_string(entry, "file", "audio entry")?;
        let declared_name = entry
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();
        let container = required_string(entry, "container", "audio entry")?;
        let path_id = entry
            .get("path_id")
            .or_else(|| entry.get("pathId"))
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| format!("audio entry {relative} has no valid path_id"))?;
        let asset = entry
            .get("asset")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();
        let name = if declared_name.trim().is_empty() {
            Path::new(relative)
                .file_stem()
                .and_then(|value| value.to_str())
                .and_then(|value| value.split_once("__").map(|(_, name)| name).or(Some(value)))
                .filter(|value| !value.is_empty())
                .unwrap_or("audio")
        } else {
            declared_name
        };
        let declared_bytes = entry
            .get("bytes")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| format!("audio entry {relative} has no valid bytes field"))?;
        let relative_normalized = relative.replace('\\', "/");
        if !seen_files.insert(relative_normalized.clone()) {
            return Err(format!("duplicate audio manifest file: {relative}"));
        }
        if !seen_semantics.insert((
            container.to_string(),
            asset.to_string(),
            path_id,
            name.to_string(),
        )) {
            return Err(format!(
                "duplicate audio semantic entry: {container}/{asset}/{path_id}/{name}"
            ));
        }
        let path = resolve_overlay_file(project, &audio_root, relative)
            .map_err(|err| roots.sanitize(&err))?;
        let bytes = read_regular_file(&path).map_err(|err| roots.sanitize(&err))?;
        if bytes.len() as u64 != declared_bytes {
            state.coverage.audio_size_mismatches += 1;
            warnings.push(format!(
                "audio entry {relative} size mismatch: manifest {declared_bytes}, actual {}",
                bytes.len()
            ));
        }
        let emitted = state
            .emit_asset("audio", name, "ogg", ContentKind::Audio, &bytes)
            .map_err(|err| format!("audio entry {relative}: {err}"))?;
        state.coverage.overlay_emitted += 1;
        mappings.push(CookMapping {
            kind: "audio".to_string(),
            name: emitted.name,
            native_key: emitted.key,
            native_path: emitted.path,
            source: CookSourceDescriptor {
                bundle: Some(container.to_string()),
                file: Some(relative_normalized),
                path_id: Some(path_id),
                ..CookSourceDescriptor::default()
            },
        });
    }
    Ok(())
}
