use super::*;

pub(in super::super) fn animation_clip_names(path: &Path, serialized_asset: &str) -> Result<std::collections::BTreeMap<i64,String>,String> {
    let loaded=load_input(path)?;
    let matches:Vec<_>=loaded.env.assets.iter().enumerate().filter(|(_,a)|a.name==serialized_asset).collect();
    let [(index,asset)]=matches.as_slice() else {return Err("animation name lookup requires one exact serialized asset".into());};
    let mut out=std::collections::BTreeMap::new();
    for (id,info) in &asset.objects {
        if asset.object_type_name(info)=="AnimationClip" {out.insert(*id,object_name(&asset.read_object(*index,info)?));}
    }
    Ok(out)
}

pub(super) fn animation_clip_paths(value: &UnityValue) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for field in [
        "m_CompressedRotationCurves",
        "m_PositionCurves",
        "m_RotationCurves",
        "m_ScaleCurves",
        "m_FloatCurves",
        "m_PPtrCurves",
        "m_EditorCurves",
    ] {
        for curve in value_array(value.get(field)) {
            if let Some(path) = curve
                .get("m_Path")
                .or_else(|| curve.get("path"))
                .and_then(UnityValue::as_str)
            {
                paths.insert(path.to_string());
            }
        }
    }
    paths
}

pub(super) fn validate_animation_component(
    env: &UnityEnvironment,
    asset_name: &str,
    info: &ObjectInfo,
    body: &UnityValue,
) -> Vec<String> {
    let mut issues = Vec::new();
    let Some(root_gameobject_pointer) = body.get("m_GameObject").and_then(UnityValue::as_pointer)
    else {
        issues.push(format!(
            "animation {}#{} missing m_GameObject",
            asset_name, info.path_id
        ));
        return issues;
    };
    let (_, root_asset, root_info, root_gameobject) =
        match resolved_body(env, root_gameobject_pointer) {
            Ok(value) => value,
            Err(err) => {
                issues.push(format!(
                    "animation {}#{} root resolve failed: {err}",
                    asset_name, info.path_id
                ));
                return issues;
            }
        };
    if root_asset.object_type_name(root_info) != "GameObject" {
        issues.push(format!(
            "animation {}#{} root expected GameObject, got {} at {}#{}",
            asset_name,
            info.path_id,
            root_asset.object_type_name(root_info),
            root_asset.name,
            root_info.path_id
        ));
        return issues;
    }
    let root_name = object_name(&root_gameobject);
    let Some(root_transform_pointer) = gameobject_transform_pointer(&root_gameobject) else {
        issues.push(format!(
            "animation {}#{} root '{}' has no Transform",
            asset_name, info.path_id, root_name
        ));
        return issues;
    };
    let runtime_paths = match transform_relative_paths(env, root_transform_pointer) {
        Ok(paths) => paths,
        Err(err) => {
            issues.push(format!(
                "animation {}#{} root '{}' hierarchy failed: {err}",
                asset_name, info.path_id, root_name
            ));
            return issues;
        }
    };

    let mut clip_pointers = Vec::new();
    if let Some(pointer) = body.get("m_Animation").and_then(UnityValue::as_pointer) {
        clip_pointers.push(pointer.clone());
    }
    for pointer in value_array(body.get("m_Animations")) {
        if let Some(pointer) = pointer.as_pointer() {
            clip_pointers.push(pointer.clone());
        }
    }

    let mut seen_clips = BTreeSet::new();
    for clip_pointer in clip_pointers {
        let clip_key = match env.resolve_pointer(&clip_pointer) {
            Ok(key) => key,
            Err(err) => {
                issues.push(format!(
                    "animation {}#{} root '{}' clip resolve failed: {err}",
                    asset_name, info.path_id, root_name
                ));
                continue;
            }
        };
        if !seen_clips.insert((clip_key.asset, clip_key.path_id)) {
            continue;
        }
        let Some(clip_asset) = env.assets.get(clip_key.asset) else {
            issues.push(format!(
                "animation {}#{} root '{}' clip asset {} missing",
                asset_name, info.path_id, root_name, clip_key.asset
            ));
            continue;
        };
        let Some(clip_info) = clip_asset.objects.get(&clip_key.path_id) else {
            issues.push(format!(
                "animation {}#{} root '{}' clip {}#{} missing",
                asset_name, info.path_id, root_name, clip_asset.name, clip_key.path_id
            ));
            continue;
        };
        if clip_asset.object_type_name(clip_info) != "AnimationClip" {
            issues.push(format!(
                "animation {}#{} root '{}' expected AnimationClip, got {} at {}#{}",
                asset_name,
                info.path_id,
                root_name,
                clip_asset.object_type_name(clip_info),
                clip_asset.name,
                clip_info.path_id
            ));
            continue;
        }
        let clip_body = match clip_asset.read_object(clip_key.asset, clip_info) {
            Ok(value) => value,
            Err(err) => {
                issues.push(format!(
                    "animation {}#{} root '{}' clip {}#{} unreadable: {err}",
                    asset_name, info.path_id, root_name, clip_asset.name, clip_info.path_id
                ));
                continue;
            }
        };
        let missing = animation_clip_paths(&clip_body)
            .into_iter()
            .filter(|path| !runtime_paths.contains(path))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            continue;
        }
        let preview = missing
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let suffix = if missing.len() > 8 { ", ..." } else { "" };
        issues.push(format!(
            "animation path mismatch {}#{} root='{}' clip {}#{} '{}' missingPaths={} [{}{}]",
            asset_name,
            info.path_id,
            root_name,
            clip_asset.name,
            clip_info.path_id,
            object_name(&clip_body),
            missing.len(),
            preview,
            suffix
        ));
    }

    issues
}
