use super::*;

/// Publish the exact serialized state of every selected non-geometry component.
///
/// Records are joined to `hierarchy.json` by `node`. Their exact source-derived
/// world matrix is also retained so terrain-only tutorial scenes can place
/// behaviour without a separately published static hierarchy; the installer
/// requires both copies to agree whenever a hierarchy exists.
pub(super) fn export_behaviours(
    env: &UnityEnvironment,
    extraction: &SceneExtraction,
) -> Result<
    (
        Vec<JsonValue>,
        Vec<JsonValue>,
        Vec<JsonValue>,
        Vec<JsonValue>,
        BehaviourCounts,
    ),
    String,
> {
    let mut counts = BehaviourCounts::default();
    let mut records = Vec::with_capacity(extraction.behaviour_components.len());
    let mut scripts = BTreeMap::<String, JsonValue>::new();
    let mut animation_clips = BTreeMap::<String, JsonValue>::new();
    let mut effect_prefab_closures = BTreeMap::<String, JsonValue>::new();

    for component in &extraction.behaviour_components {
        let body = env.read_object(component.key).map_err(|err| {
            format!(
                "could not read {} {}: {err}",
                component.object_type,
                source_id(env, component.key)
            )
        })?;
        match component.object_type.as_str() {
            "Animation" => counts.animations += 1,
            "BoxCollider" => counts.box_colliders += 1,
            "CapsuleCollider" => counts.capsule_colliders += 1,
            "MonoBehaviour" => counts.mono_behaviours += 1,
            "Rigidbody" => counts.rigidbodies += 1,
            "SphereCollider" => counts.sphere_colliders += 1,
            other => return Err(format!("unexpected behaviour component type {other:?}")),
        }

        let mut fields = JsonMap::new();
        if let Some(object) = body.as_object() {
            for (key, value) in object {
                if BEHAVIOUR_OWNERSHIP_KEYS.contains(&key.as_str()) {
                    continue;
                }
                fields.insert(key.clone(), unity_value_to_json(value));
            }
        }

        let mut record = JsonMap::new();
        record.insert("node".to_string(), json!(component.node));
        record.insert("worldMatrix".to_string(), json!(component.world_matrix));
        record.insert("type".to_string(), json!(component.object_type));
        record.insert("id".to_string(), json!(source_id(env, component.key)));
        record.insert("source".to_string(), source_json(env, component.key)?);
        record.insert(
            "enabled".to_string(),
            json!(
                body.get("m_Enabled")
                    .and_then(UnityValue::as_i64)
                    .unwrap_or(1)
                    != 0
            ),
        );
        if component.object_type == "Animation" {
            let default_clip = resolve_animation_clip_identity(
                env,
                body.get("m_Animation"),
                &mut animation_clips,
            )?;
            record.insert(
                "resolvedDefaultClip".to_string(),
                default_clip.map_or(JsonValue::Null, JsonValue::String),
            );
            let mut resolved_clips = Vec::new();
            for pointer in value_array(body.get("m_Animations")) {
                if let Some(id) =
                    resolve_animation_clip_identity(env, Some(pointer), &mut animation_clips)?
                {
                    resolved_clips.push(JsonValue::String(id));
                }
            }
            record.insert(
                "resolvedClips".to_string(),
                JsonValue::Array(resolved_clips),
            );
        }
        if component.object_type == "MonoBehaviour" {
            let script_id = resolve_script_identity(env, &body, &mut scripts)?;
            match script_id {
                Some(id) => record.insert("script".to_string(), json!(id)),
                None => {
                    counts.unresolved_scripts += 1;
                    record.insert("script".to_string(), JsonValue::Null)
                }
            };
            if let Some(particles) = body.get("particles") {
                let mut resolved_prefabs = Vec::new();
                for particle in value_array(Some(particles)) {
                    let resolved = particle
                        .get("particlePrefab")
                        .and_then(UnityValue::as_pointer)
                        .filter(|pointer| !pointer.is_null())
                        .map(|pointer| {
                            let root = env.resolve_pointer(pointer).map_err(|err| {
                                format!(
                                    "MonoBehaviour {} particlePrefab cannot resolve: {err}",
                                    source_id(env, component.key)
                                )
                            })?;
                            collect_effect_prefab_closure(env, root, &mut effect_prefab_closures)
                        })
                        .transpose()?;
                    resolved_prefabs.push(resolved.map_or(JsonValue::Null, JsonValue::String));
                }
                record.insert(
                    "resolvedParticlePrefabs".to_string(),
                    JsonValue::Array(resolved_prefabs),
                );
            }
        }
        record.insert("fields".to_string(), JsonValue::Object(fields));
        records.push(JsonValue::Object(record));
    }

    counts.distinct_scripts = scripts.len();
    counts.distinct_animation_clips = animation_clips.len();
    counts.distinct_effect_prefab_closures = effect_prefab_closures.len();
    Ok((
        records,
        scripts.into_values().collect(),
        animation_clips.into_values().collect(),
        effect_prefab_closures.into_values().collect(),
        counts,
    ))
}

pub(super) fn copy_tree_exact(source: &Path, destination: &Path) -> Result<(), String> {
    let source = canonical_directory(source, "native terrain tile source")?;
    if fs::symlink_metadata(destination).is_ok() {
        return Err(format!(
            "fresh tile destination already exists: {}",
            destination.display()
        ));
    }
    fs::create_dir_all(destination)
        .map_err(|err| format!("could not create {}: {err}", destination.display()))?;
    let destination = canonical_directory(destination, "native terrain tile destination")?;
    let mut pending = vec![(source, destination)];
    while let Some((source_dir, destination_dir)) = pending.pop() {
        let mut entries = fs::read_dir(&source_dir)
            .map_err(|err| format!("could not enumerate {}: {err}", source_dir.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("could not enumerate {}: {err}", source_dir.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let source_path = entry.path();
            let metadata = fs::symlink_metadata(&source_path)
                .map_err(|err| format!("could not inspect {}: {err}", source_path.display()))?;
            if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
                return Err(format!(
                    "refusing symlink/reparse point in native terrain source: {}",
                    source_path.display()
                ));
            }
            let destination_path = destination_dir.join(entry.file_name());
            if metadata.is_dir() {
                fs::create_dir(&destination_path).map_err(|err| {
                    format!("could not create {}: {err}", destination_path.display())
                })?;
                pending.push((source_path, destination_path));
            } else if metadata.is_file() {
                let copied = fs::copy(&source_path, &destination_path).map_err(|err| {
                    format!(
                        "could not copy immutable terrain payload {} -> {}: {err}",
                        source_path.display(),
                        destination_path.display()
                    )
                })?;
                if copied != metadata.len() {
                    return Err(format!(
                        "short copy for {}: copied {copied}, expected {}",
                        source_path.display(),
                        metadata.len()
                    ));
                }
            } else {
                return Err(format!(
                    "refusing non-file/non-directory terrain entry {}",
                    source_path.display()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("output file has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| format!("could not create fresh file {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not sync {}: {err}", path.display()))
}
