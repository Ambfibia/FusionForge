use super::*;

pub(super) fn sort_runtime_textures(runtime: &mut Value) -> Result<(), String> {
    let textures = runtime["textures"]
        .as_array_mut()
        .ok_or_else(|| "runtime texture catalog has no textures".to_owned())?;
    textures.sort_by(|left, right| {
        let left_key = (
            left.pointer("/nativeAsset/path")
                .and_then(Value::as_str)
                .unwrap_or(""),
            left["trueName"].as_str().unwrap_or(""),
        );
        let right_key = (
            right
                .pointer("/nativeAsset/path")
                .and_then(Value::as_str)
                .unwrap_or(""),
            right["trueName"].as_str().unwrap_or(""),
        );
        left_key.cmp(&right_key)
    });
    Ok(())
}

pub(super) fn refresh_runtime_coverage(avatar: &Value, runtime: &mut Value) -> Result<(), String> {
    let published_paths = runtime["textures"]
        .as_array()
        .ok_or_else(|| "runtime texture catalog has no textures".to_owned())?
        .iter()
        .filter_map(|entry| entry.pointer("/nativeAsset/path").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let published_names = runtime["textures"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["trueName"].as_str())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut references = 0_u64;
    let mut verified_paths = BTreeSet::<String>::new();
    let mut used_published_paths = BTreeSet::<String>::new();
    let mut missing = BTreeSet::<String>::new();
    let mut ambiguous = BTreeSet::<String>::new();
    for item in avatar["items"].as_array().into_iter().flatten() {
        for gender in ["male", "female"] {
            for slot in ["primaryTexture", "secondaryTexture"] {
                let Some(reference) = item.get(gender).and_then(|visual| visual.get(slot)) else {
                    continue;
                };
                if reference.is_null() {
                    continue;
                }
                references += 1;
                let true_name = reference["trueName"].as_str().unwrap_or("").to_owned();
                match reference["status"].as_str() {
                    Some("missing") => {
                        missing.insert(true_name);
                    }
                    Some("verified_unique")
                        if reference["candidates"].as_array().map(Vec::len) == Some(1) =>
                    {
                        let path = reference
                            .pointer("/candidates/0/path")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        verified_paths.insert(path.clone());
                        if published_paths.contains(&path) {
                            used_published_paths.insert(path);
                        }
                    }
                    _ => {
                        ambiguous.insert(true_name);
                    }
                }
            }
        }
    }
    let coverage = runtime
        .get_mut("coverage")
        .ok_or_else(|| "runtime texture catalog has no coverage".to_owned())?;
    coverage["avatarTextureReferences"] = json!(references);
    coverage["avatarVerifiedUniqueRoutes"] = json!(verified_paths.len());
    coverage["avatarPublishedRoutes"] = json!(used_published_paths.len());
    coverage["avatarDeferredVerifiedRoutes"] =
        json!(verified_paths.len() - used_published_paths.len());
    coverage["avatarMissingTrueNames"] = json!(missing);
    coverage["avatarAmbiguousTrueNames"] = json!(ambiguous);
    if let Some(values) = coverage["avatarMissingSourceMetadata"].as_array_mut() {
        values.retain(|value| {
            value
                .as_str()
                .is_none_or(|name| !published_names.contains(name))
        });
    }
    Ok(())
}
