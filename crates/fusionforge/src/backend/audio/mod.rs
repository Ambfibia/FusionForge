use super::super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ClientAudioRecord {
    pub(in super::super) asset: String,
    pub(in super::super) container: String,
    pub(in super::super) bundle_path: String,
    pub(in super::super) cache_dir: String,
    pub(in super::super) path_id: i64,
    pub(in super::super) name: String,
    pub(in super::super) bytes: usize,
    pub(in super::super) file: String,
}

pub(in super::super) fn audio_clip_data(body: &fusionforge::UnityValue) -> Option<&[u8]> {
    body.get("audio data")
        .and_then(fusionforge::UnityValue::as_bytes)
        .or_else(|| {
            body.get("m_AudioData")
                .and_then(fusionforge::UnityValue::as_bytes)
        })
}

pub(in super::super) fn audio_file_name(container: &str, asset: &str, path_id: i64, name: &str) -> PathBuf {
    PathBuf::from(safe_segment(container))
        .join(safe_segment(asset))
        .join(format!("{path_id}__{}.ogg", safe_segment(name)))
}

pub(in super::super) fn inspect_cached_bundle_audio(
    cache_dir: &Path,
    container: &str,
    bundle_path: &str,
) -> Vec<ClientAudioRecord> {
    use fusionforge::{object_name, UnityEnvironment};

    let env = UnityEnvironment::from_dir(cache_dir);
    let mut clips = Vec::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "AudioClip" {
                continue;
            }
            let body = match asset.read_object(asset_index, info) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let name = object_name(&body);
            let file = audio_file_name(container, &asset.name, *path_id, &name)
                .to_string_lossy()
                .replace('\\', "/");
            clips.push(ClientAudioRecord {
                asset: asset.name.clone(),
                container: container.to_string(),
                bundle_path: bundle_path.to_string(),
                cache_dir: cache_dir.to_string_lossy().to_string(),
                path_id: *path_id,
                name,
                bytes: audio_clip_data(&body)
                    .map(|data| data.len())
                    .unwrap_or_default(),
                file,
            });
        }
    }
    clips
}

pub(in super::super) fn export_all_audio(project_dir: String) -> EditorResult<serde_json::Value> {
    use fusionforge::{object_name, Asset};

    let project = PathBuf::from(&project_dir);
    if !project.exists() || !project.is_dir() {
        return Err(EditorError::MissingPath(project_dir).to_string());
    }
    let index_path = project.join("cache").join("bundle-index.json");
    let index: ClientFileIndex = serde_json::from_str(
        &fs::read_to_string(&index_path).map_err(|err| EditorError::Io(err).to_string())?,
    )
    .map_err(|err| EditorError::Json(err).to_string())?;
    let audio_root = project.join("audio");
    fs::create_dir_all(&audio_root).map_err(|err| EditorError::Io(err).to_string())?;
    let mut entries = Vec::new();
    let mut exported = 0usize;
    for clip in index.audio_clips {
        let asset_path = PathBuf::from(&clip.cache_dir).join(&clip.asset);
        let asset = match Asset::from_path(&asset_path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(info) = asset.objects.get(&clip.path_id) else {
            continue;
        };
        let body = match asset.read_object(0, info) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let data = audio_clip_data(&body).unwrap_or_default();
        let relative_file = audio_file_name(
            &clip.container,
            &clip.asset,
            clip.path_id,
            &object_name(&body),
        );
        let target = audio_root.join(&relative_file);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
        }
        if !data.is_empty() {
            fs::write(&target, data).map_err(|err| EditorError::Io(err).to_string())?;
            exported += 1;
        }
        entries.push(json!({
            "container": clip.container,
            "asset": clip.asset,
            "path_id": clip.path_id,
            "name": object_name(&body),
            "bytes": data.len(),
            "originalSha256": ffbuildtool::util::get_buffer_hash(data),
            "file": relative_file.to_string_lossy().replace('\\', "/"),
        }));
    }
    let manifest = json!({
        "format": "fftools.audio-patch.v1",
        "entries": entries,
    });
    let data = serde_json::to_string_pretty(&manifest)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(audio_root.join("manifest.json"), format!("{data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    let mut patch_config = ensure_patch_project_config(&project)?;
    patch_config["AudioDir"] = json!("audio");
    let patch_data = serde_json::to_string_pretty(&patch_config)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    Ok(
        json!({ "audioDir": audio_root, "manifestPath": audio_root.join("manifest.json"), "exported": exported, "entries": entries.len() }),
    )
}

pub(in super::super) fn import_all_audio(project_dir: String, import_dir: String) -> EditorResult<serde_json::Value> {
    let project = PathBuf::from(&project_dir);
    if !project.exists() || !project.is_dir() {
        return Err(EditorError::MissingPath(project_dir).to_string());
    }
    let source_root = PathBuf::from(&import_dir);
    if !source_root.exists() || !source_root.is_dir() {
        return Err(EditorError::MissingPath(import_dir).to_string());
    }
    let source_manifest = source_root.join("manifest.json");
    if !source_manifest.exists() {
        return Err(format!(
            "Audio manifest was not found: {}",
            source_manifest.display()
        ));
    }
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&source_manifest).map_err(|err| EditorError::Io(err).to_string())?,
    )
    .map_err(|err| EditorError::Json(err).to_string())?;
    if manifest.get("format").and_then(serde_json::Value::as_str) != Some("fftools.audio-patch.v1")
    {
        return Err(format!(
            "Unsupported audio manifest format in {}",
            source_manifest.display()
        ));
    }
    let audio_root = project.join("audio");
    fs::create_dir_all(&audio_root).map_err(|err| EditorError::Io(err).to_string())?;
    let mut imported = 0usize;
    for entry in manifest
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(file) = entry.get("file").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let source = source_root.join(file.replace('/', "\\"));
        if !source.is_file() {
            continue;
        }
        let target = audio_root.join(file.replace('/', "\\"));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
        }
        fs::copy(&source, &target).map_err(|err| EditorError::Io(err).to_string())?;
        imported += 1;
    }
    let manifest_data = serde_json::to_string_pretty(&manifest)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(
        audio_root.join("manifest.json"),
        format!("{manifest_data}\n"),
    )
    .map_err(|err| EditorError::Io(err).to_string())?;
    let mut patch_config = ensure_patch_project_config(&project)?;
    patch_config["AudioDir"] = json!("audio");
    let patch_data = serde_json::to_string_pretty(&patch_config)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    Ok(
        json!({ "audioDir": audio_root, "manifestPath": audio_root.join("manifest.json"), "imported": imported }),
    )
}

pub(in super::super) fn audio_mime_type(data: &[u8]) -> &'static str {
    if data.starts_with(b"OggS") {
        "audio/ogg"
    } else if data.starts_with(b"RIFF") {
        "audio/wav"
    } else if data.starts_with(b"ID3")
        || data.get(0) == Some(&0xff) && data.get(1).is_some_and(|byte| byte & 0xe0 == 0xe0)
    {
        "audio/mpeg"
    } else {
        "audio/ogg"
    }
}

pub(in super::super) fn preview_audio_clip(clip: ClientAudioRecord) -> EditorResult<serde_json::Value> {
    use fusionforge::Asset;

    let asset_path = PathBuf::from(&clip.cache_dir).join(&clip.asset);
    let asset =
        Asset::from_path(&asset_path).map_err(|err| format!("{}: {err}", asset_path.display()))?;
    let info = asset
        .objects
        .get(&clip.path_id)
        .ok_or_else(|| format!("{}: pathId {} was not found", clip.asset, clip.path_id))?;
    if asset.object_type_name(info) != "AudioClip" {
        return Err(format!(
            "{}: pathId {} is not AudioClip",
            clip.asset, clip.path_id
        ));
    }
    let body = asset.read_object(0, info)?;
    let data = audio_clip_data(&body)
        .ok_or_else(|| format!("{}: pathId {} has no audio data", clip.asset, clip.path_id))?;
    if data.is_empty() {
        return Err(format!(
            "{}: pathId {} has empty audio data",
            clip.asset, clip.path_id
        ));
    }
    let mime_type = audio_mime_type(data);
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    Ok(json!({
        "dataUrl": format!("data:{mime_type};base64,{encoded}"),
        "mimeType": mime_type,
        "bytes": data.len(),
        "name": clip.name,
    }))
}

pub(in super::super) fn stage_audio_replacement(
    project_dir: String,
    clip: ClientAudioRecord,
    replacement_path: String,
) -> EditorResult<serde_json::Value> {
    let project = PathBuf::from(project_dir);
    let source = PathBuf::from(replacement_path);
    if !source.is_file() {
        return Err(EditorError::MissingPath(source.to_string_lossy().to_string()).to_string());
    }

    let replacement_data = fs::read(&source).map_err(|err| EditorError::Io(err).to_string())?;
    if replacement_data.is_empty() {
        return Err(format!("Audio replacement is empty: {}", source.display()));
    }

    let mut patch_config = ensure_patch_project_config(&project)?;
    let audio_root = project.join("audio");
    let relative_file = audio_file_name(&clip.container, &clip.asset, clip.path_id, &clip.name);
    let target = audio_root.join(&relative_file);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    fs::write(&target, &replacement_data).map_err(|err| EditorError::Io(err).to_string())?;

    let manifest_path = audio_root.join("manifest.json");
    let mut manifest = if manifest_path.exists() {
        serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(&manifest_path).map_err(|err| EditorError::Io(err).to_string())?,
        )
        .map_err(|err| EditorError::Json(err).to_string())?
    } else {
        json!({ "format": "fftools.audio-patch.v1", "entries": [] })
    };
    if manifest.get("format").and_then(serde_json::Value::as_str) != Some("fftools.audio-patch.v1")
    {
        return Err(format!(
            "Unsupported audio manifest format in {}",
            manifest_path.display()
        ));
    }
    if !manifest
        .get("entries")
        .is_some_and(serde_json::Value::is_array)
    {
        manifest["entries"] = json!([]);
    }

    let relative_file_string = relative_file.to_string_lossy().replace('\\', "/");
    let replacement_hash = ffbuildtool::util::get_buffer_hash(&replacement_data);
    let entry = json!({
        "container": clip.container,
        "asset": clip.asset,
        "path_id": clip.path_id,
        "name": clip.name,
        "bytes": clip.bytes,
        "file": relative_file_string.clone(),
        "replacementBytes": replacement_data.len(),
        "replacementSha256": replacement_hash.clone(),
    });
    let entries = manifest["entries"]
        .as_array_mut()
        .ok_or_else(|| "Audio manifest entries is not an array".to_string())?;
    entries.retain(|item| {
        item.get("container") != entry.get("container")
            || item.get("asset") != entry.get("asset")
            || item.get("path_id") != entry.get("path_id")
    });
    entries.push(entry);
    let manifest_data = serde_json::to_string_pretty(&manifest)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&manifest_path, format!("{manifest_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;

    patch_config["AudioDir"] = json!("audio");
    let patch_data = serde_json::to_string_pretty(&patch_config)
        .map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(project.join("ffpatch.json"), format!("{patch_data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;

    Ok(json!({
        "audioDir": audio_root,
        "manifestPath": manifest_path,
        "file": relative_file_string,
        "targetPath": target,
        "replacementBytes": replacement_data.len(),
        "replacementSha256": replacement_hash,
    }))
}

pub(in super::super) fn npc_audio_container_path_candidates(name: &str) -> Vec<String> {
    let stem = name
        .trim()
        .rsplit_once('.')
        .filter(|(_, extension)| extension.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .map(|(stem, _)| stem)
        .unwrap_or_else(|| name.trim());
    let mut basenames = BTreeSet::new();
    let underscored = collapsed_ascii_token(stem, true);
    if !underscored.is_empty() {
        basenames.insert(underscored);
    }
    let compact = collapsed_ascii_token(stem, false);
    if !compact.is_empty() {
        basenames.insert(compact);
    }

    let mut paths = Vec::new();
    for basename in basenames {
        for prefix in ["sound", "vo", "ui sound"] {
            paths.push(format!("{prefix}/{basename}.wav"));
        }
    }
    paths
}

pub(in super::super) fn extend_npc_audio_prefix_candidates(prefixes: &mut BTreeSet<String>, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    for candidate in [
        collapsed_ascii_token(trimmed, true),
        collapsed_ascii_token(trimmed, false),
    ] {
        let candidate = candidate.trim().to_ascii_lowercase();
        if candidate.len() >= 2 && !candidate.contains(' ') {
            prefixes.insert(candidate);
        }
    }
}

pub(in super::super) fn npc_audio_match_score(clip: &ClientAudioRecord, prefixes: &[String], tokens: &[String]) -> i32 {
    let name = clip.name.to_ascii_lowercase();
    if name.contains("_nan") || name.contains("nano") || name.contains("minimap") {
        return 0;
    }
    let container = clip.container.to_ascii_lowercase();
    if container.contains("nano") {
        return 0;
    }
    let text = format!("{}\n{}\n{}", clip.name, clip.container, clip.asset).to_ascii_lowercase();
    let mut score = 0;
    for prefix in prefixes {
        let normalized = prefix.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if name == normalized
            || name.starts_with(&format!("{normalized}_"))
            || name.starts_with(&format!("{normalized} "))
        {
            score += 100;
        } else if name.contains(&format!("_{normalized}_"))
            || name.contains(&format!(" {normalized} "))
        {
            score += 20;
        }
    }
    for token in tokens {
        if token.is_empty() {
            continue;
        }
        if name.as_str() == token.as_str() || name.starts_with(&format!("{token}_")) {
            score += if token.contains('_') { 35 } else { 20 };
        } else if token.contains('_') && text.contains(token) {
            score += 15;
        }
    }
    score
}

pub(in super::super) fn npc_audio_token_allowed(token: &str) -> bool {
    !matches!(token, "fusion")
}

pub(in super::super) fn add_npc_audio_asset_hints_from_index(
    hints: &mut NpcAssetHints,
    blueprint: &NpcBlueprint,
    index: &ClientFileIndex,
) {
    let source_paths = client_index_container_paths(index);
    let mut prefixes = BTreeSet::new();
    if let Some(value) = blueprint.audio_prefix.as_deref() {
        extend_npc_audio_prefix_candidates(&mut prefixes, value);
    }
    extend_npc_audio_prefix_candidates(&mut prefixes, &blueprint.name);
    extend_npc_audio_prefix_candidates(&mut prefixes, &blueprint.internal_name);
    for value in [
        blueprint.model_asset.as_deref(),
        blueprint.texture_asset.as_deref(),
        blueprint.icon_asset.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(stem) = blueprint_asset_stem(value) {
            extend_npc_audio_prefix_candidates(&mut prefixes, &stem);
        }
    }
    let tokens = hints
        .tokens
        .iter()
        .filter(|token| token.len() >= 3 || token.contains('_'))
        .filter(|token| npc_audio_token_allowed(token))
        .cloned()
        .collect::<Vec<_>>();
    let prefixes = prefixes.into_iter().collect::<Vec<_>>();
    if prefixes.is_empty() && tokens.is_empty() {
        return;
    }

    let mut matches = index
        .audio_clips
        .iter()
        .filter_map(|clip| {
            let score = npc_audio_match_score(clip, &prefixes, &tokens);
            (score > 0).then_some((score, clip))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.name.cmp(&right.1.name))
    });

    for (_, clip) in matches.into_iter().take(48) {
        for path in npc_audio_container_path_candidates(&clip.name) {
            let normalized = normalized_asset_path(&path);
            if source_paths.contains(&normalized) {
                hints.exact_paths.insert(normalized.clone());
                hints.audio_paths.insert(normalized);
            }
        }
    }
}

pub(in super::super) fn add_npc_audio_asset_hints_from_source_bundles(
    project: &Path,
    source_paths: &[PathBuf],
    hints: &mut NpcAssetHints,
) -> Result<(), String> {
    let container_paths = npc_container_paths_from_source_bundles(project, source_paths)?;
    let tokens = hints
        .tokens
        .iter()
        .filter(|token| token.len() >= 3 || token.contains('_'))
        .filter(|token| npc_audio_token_allowed(token))
        .cloned()
        .collect::<Vec<_>>();
    for normalized in container_paths {
        if !(normalized.starts_with("vo/")
            || normalized.starts_with("sound/")
            || normalized.starts_with("ui sound/"))
        {
            continue;
        }
        let basename = npc_asset_basename(&normalized);
        let matches_npc = tokens.iter().any(|token| {
            basename == *token
                || basename.starts_with(&format!("{token}_"))
                || basename.contains(&format!("_{token}_"))
        });
        if matches_npc {
            hints.exact_paths.insert(normalized.clone());
            hints.audio_paths.insert(normalized);
        }
    }
    Ok(())
}

pub(in super::super) fn tutorial_voice_durations(
    entries: &[serde_json::Value],
    audio_dir: &Path,
) -> Result<BTreeMap<String, f32>, String> {
    let mut durations = BTreeMap::new();
    for entry in entries {
        let Some(name) = entry.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("_tut") {
            continue;
        }
        let Some(file) = entry.get("file").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let path = audio_dir.join(file.replace('/', "\\"));
        if !path.is_file() {
            continue;
        }
        let data = fs::read(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        let duration =
            fusionforge::build_patch::audio_duration_seconds(&data).ok_or_else(|| {
                format!(
                    "Tutorial voice replacement is not a readable Ogg Vorbis clip: {}",
                    path.display()
                )
            })?;
        if !duration.is_finite() || duration <= 0.0 || duration > f32::MAX as f64 {
            return Err(format!(
                "Tutorial voice replacement has invalid duration {duration}: {}",
                path.display()
            ));
        }
        let duration = duration as f32;
        if let Some(previous) = durations.insert(name.to_string(), duration) {
            if (previous - duration).abs() > 0.001 {
                return Err(format!(
                    "Tutorial AudioClip {name} has conflicting durations: {previous:.3}s and {duration:.3}s"
                ));
            }
        }
    }
    Ok(durations)
}

pub(in super::super) fn patch_audio_assets_native(
    extracted_dir: &Path,
    audio_dir: &Path,
    entries: &[serde_json::Value],
    container: &str,
    changed_audio: &mut Vec<legacy_bundle_layout::PatchedAudioIdentity>,
) -> Result<bool, String> {
    let assets = unique_manifest_values(entries, "asset", Some(container));
    let mut patched_any = false;
    for asset in assets {
        let status_path = build_status_path(extracted_dir, "audio", &asset);
        let args = vec![
            extracted_dir.to_string_lossy().to_string(),
            audio_dir.to_string_lossy().to_string(),
            "--asset".to_string(),
            asset.clone(),
            "--container".to_string(),
            container.to_string(),
            "--status-file".to_string(),
            status_path.to_string_lossy().to_string(),
        ];
        fusionforge::build_patch::patch_audio_clips(&args)?;
        let status_text = fs::read_to_string(&status_path)
            .map_err(|err| format!("{}: {err}", status_path.display()))?;
        let status = serde_json::from_str::<JsonValue>(&status_text).map_err(|err| {
            format!(
                "{}: invalid audio patch status: {err}",
                status_path.display()
            )
        })?;
        let patched = status
            .get("patched")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| {
                format!(
                    "{}: audio status has no integer 'patched'",
                    status_path.display()
                )
            })? as usize;
        let missing = status
            .get("missing")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| {
                format!(
                    "{}: audio status has no integer 'missing'",
                    status_path.display()
                )
            })? as usize;
        if missing != 0 {
            return Err(format!(
                "{}: audio patch left {missing} manifest target(s) missing",
                status_path.display()
            ));
        }
        let changed_entries = status
            .get("changed")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| {
                format!(
                    "{}: audio status has no 'changed' array",
                    status_path.display()
                )
            })?;
        if changed_entries.len() != patched {
            return Err(format!(
                "{}: audio status says patched={patched}, but has {} changed identities",
                status_path.display(),
                changed_entries.len()
            ));
        }
        if patched > 0 {
            patched_any = true;
        }
        for (index, changed) in changed_entries.iter().enumerate() {
            let changed_asset = changed
                .get("asset")
                .and_then(JsonValue::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("{}: changed[{index}] has no asset", status_path.display())
                })?;
            let path_id = changed
                .get("pathId")
                .and_then(JsonValue::as_i64)
                .ok_or_else(|| {
                    format!("{}: changed[{index}] has no pathId", status_path.display())
                })?;
            let name = changed
                .get("name")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    format!("{}: changed[{index}] has no name", status_path.display())
                })?;
            let replacement_sha1 = changed
                .get("replacementSha1")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .ok_or_else(|| {
                    format!(
                        "{}: changed[{index}] has no replacementSha1",
                        status_path.display()
                    )
                })?;
            if replacement_sha1.len() != 40
                || !replacement_sha1
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(format!(
                    "{}: changed[{index}] has invalid replacementSha1 '{}'",
                    status_path.display(),
                    replacement_sha1
                ));
            }
            let changed_container = changed
                .get("container")
                .and_then(JsonValue::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or(container)
                .to_string();
            if !changed_asset.eq_ignore_ascii_case(&asset) {
                return Err(format!(
                    "{}: changed[{index}] asset '{}' does not match requested '{}'",
                    status_path.display(),
                    changed_asset,
                    asset
                ));
            }
            if changed_audio.iter().any(|identity| {
                identity.container.eq_ignore_ascii_case(&changed_container)
                    && identity.asset.eq_ignore_ascii_case(changed_asset)
                    && identity.path_id == path_id
            }) {
                return Err(format!(
                    "duplicate changed AudioClip identity: {changed_container} / {changed_asset} #{path_id}"
                ));
            }
            changed_audio.push(legacy_bundle_layout::PatchedAudioIdentity {
                container: changed_container,
                asset: changed_asset.to_string(),
                path_id,
                name: name.to_string(),
                replacement_sha1: replacement_sha1.to_ascii_lowercase(),
            });
        }
    }
    Ok(patched_any)
}
