use super::*;

pub(super) const AUDIO_MANIFEST_FORMAT: &str = "fftools.audio-patch.v1";

pub fn patch_audio_clips(args: &[String]) -> Result<(), String> {
    let usage = "patch-audio-clips <extracted-dir> <audio-dir> --asset name [--container name] [--status-file path]";
    let extracted_dir = required_path(args, 0, usage)?;
    let audio_dir = required_path(args, 1, usage)?;
    let asset_name = option_value(args, "--asset").ok_or_else(|| usage.to_string())?;
    let container = option_value(args, "--container").unwrap_or_default();
    let status_file = option_value(args, "--status-file").map(PathBuf::from);
    let manifest_path = audio_dir.join("manifest.json");
    let manifest = read_json(&manifest_path)?;
    if manifest.get("format").and_then(JsonValue::as_str) != Some(AUDIO_MANIFEST_FORMAT) {
        return Err(format!(
            "Unsupported audio manifest format in {}",
            manifest_path.display()
        ));
    }
    let asset = Asset::from_path(&extracted_dir.join(&asset_name))?;
    let asset_index = 0usize;
    let mut replacements = BTreeMap::new();
    let mut patched = 0usize;
    let mut missing = 0usize;
    let mut unchanged = 0usize;
    let mut changed = Vec::<JsonValue>::new();
    for entry in manifest
        .get("entries")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        if entry.get("asset").and_then(JsonValue::as_str) != Some(asset_name.as_str()) {
            continue;
        }
        let entry_container = entry
            .get("container")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        if !container.is_empty() && !entry_container.is_empty() && entry_container != container {
            continue;
        }
        let Some(replacement) = audio_replacement_path(entry, &audio_dir) else {
            missing += 1;
            continue;
        };
        if !replacement.exists() {
            missing += 1;
            continue;
        }
        let path_id = entry_i64(entry, "path_id").or_else(|_| entry_i64(entry, "pathId"))?;
        let Some(info) = asset.objects.get(&path_id) else {
            missing += 1;
            continue;
        };
        if asset.object_type_name(info) != "AudioClip" {
            return Err(format!("{asset_name}: pathId {path_id} is not AudioClip"));
        }
        let data =
            fs::read(&replacement).map_err(|err| format!("{}: {err}", replacement.display()))?;
        let mut value = asset.read_object(asset_index, info)?;
        let actual_name = object_name(&value);
        if let Some(expected_name) = entry
            .get("name")
            .and_then(JsonValue::as_str)
            .filter(|name| !name.is_empty())
        {
            if expected_name != actual_name {
                return Err(format!(
                    "{asset_name}: pathId {path_id} AudioClip name mismatch: manifest='{expected_name}', object='{actual_name}'"
                ));
            }
        }
        if audio_clip_bytes(&value).is_some_and(|current| current == data.as_slice()) {
            unchanged += 1;
            continue;
        }
        let replacement_sha1 = format!("{:x}", Sha1::digest(&data));
        patch_audio_clip_value(&mut value, data)?;
        replacements.insert(path_id, value);
        changed.push(json!({
            "container": entry_container,
            "asset": asset_name,
            "pathId": path_id,
            "name": actual_name,
            "replacementSha1": replacement_sha1,
        }));
        patched += 1;
    }
    if missing > 0 {
        return Err(format!(
            "{asset_name}: {missing} audio manifest target(s) or replacement file(s) are missing"
        ));
    }
    if patched > 0 {
        let bytes = asset.rebuild_with_object_values(asset_index, &replacements)?;
        fs::write(extracted_dir.join(&asset_name), bytes)
            .map_err(|err| format!("{}: {err}", extracted_dir.join(&asset_name).display()))?;
    }
    println!("Patched {patched} audio clip(s), unchanged {unchanged}, missing {missing}.");
    write_status(
        status_file.as_deref(),
        json!({ "patched": patched, "unchanged": unchanged, "missing": missing, "changed": changed }),
    )?;
    Ok(())
}

pub(super) fn audio_replacement_path(entry: &JsonValue, audio_dir: &Path) -> Option<PathBuf> {
    entry
        .get("file")
        .and_then(JsonValue::as_str)
        .map(|file| audio_dir.join(file.replace('/', "\\")))
}

pub(super) fn audio_clip_bytes(value: &UnityValue) -> Option<&[u8]> {
    value
        .get("audio data")
        .and_then(UnityValue::as_bytes)
        .or_else(|| value.get("m_AudioData").and_then(UnityValue::as_bytes))
}

pub(crate) fn audio_duration_seconds(data: &[u8]) -> Option<f64> {
    parse_ogg_vorbis_metadata(data).map(|metadata| metadata.duration_seconds)
}

pub(super) fn patch_audio_clip_value(value: &mut UnityValue, data: Vec<u8>) -> Result<(), String> {
    let ogg_metadata = parse_ogg_vorbis_metadata(&data);
    let has_legacy_payload = value.get("audio data").is_some();
    let has_modern_payload = value.get("m_AudioData").is_some();
    if !has_legacy_payload && !has_modern_payload {
        return Err("AudioClip has neither 'audio data' nor 'm_AudioData'".to_string());
    }
    if has_legacy_payload {
        set_object_field(value, "audio data", UnityValue::Bytes(data.clone()))?;
    }
    if has_modern_payload {
        set_object_field(value, "m_AudioData", UnityValue::Bytes(data.clone()))?;
    }
    if value.get("m_Size").is_some() {
        set_object_field(value, "m_Size", UnityValue::Int(data.len() as i64))?;
    }
    if value.get("m_CompleteImageSize").is_some() {
        set_object_field(
            value,
            "m_CompleteImageSize",
            UnityValue::Int(data.len() as i64),
        )?;
    }
    if let Some(metadata) = ogg_metadata {
        if value.get("m_Frequency").is_some() {
            set_object_field(
                value,
                "m_Frequency",
                UnityValue::Int(metadata.sample_rate as i64),
            )?;
        }
        if value.get("m_Length").is_some() {
            set_object_field(
                value,
                "m_Length",
                UnityValue::Float(metadata.duration_seconds),
            )?;
        }
    }
    let written = audio_clip_bytes(value)
        .ok_or_else(|| "AudioClip replacement payload disappeared after patching".to_string())?;
    if written != data.as_slice() {
        return Err("AudioClip replacement payload differs after patching".to_string());
    }
    if let Some(size) = value.get("m_Size") {
        if size.as_i64() != Some(data.len() as i64) {
            return Err(format!(
                "AudioClip m_Size does not match replacement payload length {}",
                data.len()
            ));
        }
    }
    Ok(())
}
