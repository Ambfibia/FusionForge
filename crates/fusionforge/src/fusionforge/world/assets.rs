use super::*;

pub(crate) fn normalize_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_lowercase()
}

pub(super) fn inspect_asset(
    env: &UnityEnvironment,
    asset_index: usize,
    asset: &Asset,
    sample_limit: usize,
) -> Result<JsonValue, String> {
    let mut type_counts = HashMap::<String, usize>::new();
    let mut samples = Vec::new();
    let mut terrains = Vec::new();
    let mut colliders = Vec::new();
    let mut transforms = Vec::new();
    let mut textures = Vec::new();
    let mut text_assets = Vec::new();
    let mut water_candidates = Vec::new();
    let mut errors = Vec::<String>::new();

    for (path_id, info) in &asset.objects {
        let obj_type = asset.object_type_name(info);
        *type_counts.entry(obj_type.clone()).or_default() += 1;
        if samples.len() < sample_limit {
            samples.push(json!({
                "pathId": path_id,
                "type": obj_type,
                "classId": info.class_id,
                "size": info.size,
            }));
        }

        let relevant = matches!(
            obj_type.as_str(),
            "TerrainData"
                | "TerrainCollider"
                | "BoxCollider"
                | "SphereCollider"
                | "CapsuleCollider"
                | "MeshCollider"
                | "CharacterController"
                | "GameObject"
                | "Transform"
                | "Material"
                | "Shader"
                | "Texture2D"
                | "TextAsset"
                | "Font"
                | "MeshRenderer"
                | "SkinnedMeshRenderer"
                | "MeshFilter"
                | "MonoBehaviour"
        ) || obj_type.ends_with("Collider");
        if !relevant {
            continue;
        }

        let body = match asset.read_object(asset_index, info) {
            Ok(body) => body,
            Err(err) => {
                errors.push(format!(
                    "pathId {path_id} {obj_type}: object read failed: {err}"
                ));
                continue;
            }
        };

        match obj_type.as_str() {
            "TerrainData" => terrains.push(summarize_terrain(*path_id, &body)),
            "Transform" if transforms.len() < sample_limit => {
                transforms.push(summarize_transform(*path_id, &body))
            }
            "Texture2D" if textures.len() < sample_limit * 4 => {
                textures.push(summarize_texture(env, *path_id, &body));
            }
            "TextAsset" | "Shader" if text_assets.len() < sample_limit * 4 => {
                text_assets.push(summarize_text_asset(*path_id, &obj_type, &body));
            }
            value if value.ends_with("Collider") || value == "CharacterController" => {
                colliders.push(summarize_collider(*path_id, &obj_type, &body));
            }
            "GameObject" => {
                let summary = summarize_game_object(*path_id, &body);
                if is_water_candidate(&summary) {
                    water_candidates.push(summary);
                }
            }
            "Material"
            | "Shader"
            | "MeshRenderer"
            | "SkinnedMeshRenderer"
            | "MeshFilter"
            | "MonoBehaviour" => {
                let summary = summarize_named(*path_id, &obj_type, &body);
                if is_water_candidate(&summary) {
                    water_candidates.push(summary);
                }
            }
            _ => {}
        }
    }

    Ok(json!({
        "name": asset.name,
        "sourceName": asset.name,
        "objectCount": asset.objects.len(),
        "typeCounts": type_counts,
        "samples": samples,
        "terrains": terrains.into_iter().take(sample_limit).collect::<Vec<_>>(),
        "colliders": colliders.into_iter().take(sample_limit * 2).collect::<Vec<_>>(),
        "transforms": transforms,
        "textures": textures,
        "textAssets": text_assets,
        "waterCandidates": water_candidates.into_iter().take(sample_limit * 2).collect::<Vec<_>>(),
        "errors": errors.into_iter().take(50).collect::<Vec<_>>(),
    }))
}

pub(super) fn summarize_text_asset(path_id: i64, obj_type: &str, body: &UnityValue) -> JsonValue {
    let bytes = body
        .get("m_Script")
        .and_then(UnityValue::as_bytes)
        .unwrap_or(&[]);
    let text = String::from_utf8_lossy(bytes);
    let snippet = text.chars().take(600).collect::<String>();
    json!({
        "pathId": path_id,
        "type": obj_type,
        "name": object_name(body),
        "bytes": bytes.len(),
        "utf8": std::str::from_utf8(bytes).is_ok(),
        "preview": snippet,
    })
}

pub(super) fn note_missing_asset(warnings: &mut Vec<String>, missing: &mut HashSet<String>, name: &str) {
    if missing.insert(name.to_string()) {
        warnings.push(format!("Missing referenced bundle: {name}"));
    }
}

pub(super) fn tile_id_from_scene_asset_name(name: &str) -> Option<String> {
    name.strip_prefix("BuildPlayer-")
        .and_then(|value| value.split('#').next())
        .and_then(|value| value.split('.').next())
        .and_then(|value| value.starts_with("Map_").then(|| value.to_string()))
}

pub(crate) fn build_archive_candidate_index(root: &Path) -> HashMap<String, Vec<PathBuf>> {
    let mut index = HashMap::<String, Vec<PathBuf>>::new();
    for bundle in direct_bundle_files(root) {
        if let Some(stem) = bundle.file_stem().and_then(|value| value.to_str()) {
            index
                .entry(normalize_bundle_name(stem))
                .or_default()
                .push(bundle.clone());
        }
        if let Ok(names) = ffbuildtool::bundle::AssetBundle::entry_names(&bundle) {
            for name in names {
                index.entry(normalize_bundle_name(&name)).or_default().push(bundle.clone());
            }
        }
    }
    for candidates in index.values_mut() {
        candidates.sort();
        candidates.dedup();
    }
    index
}

pub(crate) fn build_archive_index(root: &Path) -> HashMap<String, PathBuf> {
    build_archive_candidate_index(root)
        .into_iter()
        .filter_map(|(archive, candidates)| {
            candidates
                .into_iter()
                .next()
                .map(|candidate| (archive, candidate))
        })
        .collect()
}

pub(super) fn find_manifest_bundle_for_archive(
    target: &str,
    build_root: Option<&Path>,
    fallback_root: &Path,
    repo_root: &Path,
) -> Option<PathBuf> {
    let manifest_root = repo_root.join("vendor/ffbuildtool");
    let Ok(entries) = fs::read_dir(manifest_root) else {
        return None;
    };
    let mut manifests = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.starts_with("manifest") && name.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    manifests.sort();

    let mut search_roots = Vec::new();
    if let Some(build_root) = build_root {
        search_roots.push(build_root.to_path_buf());
    }
    search_roots.push(fallback_root.to_path_buf());

    for manifest in manifests {
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<JsonValue>(&text) else {
            continue;
        };
        let Some(bundles) = value.get("bundles").and_then(JsonValue::as_object) else {
            continue;
        };
        for (bundle_name, bundle) in bundles {
            let Some(uncompressed) = bundle
                .get("uncompressed_info")
                .and_then(JsonValue::as_object)
            else {
                continue;
            };
            if !uncompressed
                .keys()
                .any(|archive_name| normalize_bundle_name(archive_name) == target)
            {
                continue;
            }
            for root in &search_roots {
                let candidate = root.join(bundle_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}
