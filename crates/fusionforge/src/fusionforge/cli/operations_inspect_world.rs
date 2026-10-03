use super::*;

pub(super) fn npc_snapshot_expected_types(path: &str) -> &'static [&'static str] {
    let normalized = cli_normalized_asset_path(path);
    if normalized.starts_with("mob/")
        || normalized.ends_with(".kfm")
        || normalized.ends_with(".nif")
    {
        &["GameObject", "TextAsset", "Mesh"]
    } else {
        cli_container_expected_export_types(path)
    }
}

pub(super) fn inspect_world(args: &[String]) -> Result<(), String> {
    let map_bundle = required_path(
        args,
        0,
        "inspect-world <map-bundle> [build-root] [out.json] [neighbor-radius]",
    )?;
    let build_root = args
        .get(1)
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| map_bundle.parent().map(Path::to_path_buf));
    let output = args.get(2).map(PathBuf::from);
    let neighbor_radius = args
        .get(3)
        .map(|value| parse_i64(value).map(|value| value.clamp(0, 8) as u8))
        .transpose()?
        .unwrap_or(0);
    let repo_root = crate::repository_root().to_path_buf()
        .parent()
        .ok_or_else(|| "could not resolve repo root".to_string())?
        .to_path_buf();
    let value = inspect_world_bundles(WorldInspectOptions {
        repo_root,
        map_bundle: Some(map_bundle),
        resource_bundle: None,
        build_root,
        neighbor_radius,
    })?;
    write_or_print_json(output.as_deref(), &value)
}

pub(super) fn show_gameobject(args: &[String]) -> Result<(), String> {
    let path = required_path(
        args,
        0,
        "show-gameobject <asset-or-bundle> <container-path-or-name> [out.dot]",
    )?;
    let wanted = args
        .get(1)
        .ok_or_else(|| "show-gameobject requires a container path or object name".to_string())?;
    let out = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("graph.dot"));
    let loaded = load_input(&path)?;
    let root = find_gameobject_root(&loaded.env, wanted)
        .ok_or_else(|| format!("game object/container entry not found: {wanted}"))?;
    let mut dot = String::from(
        "digraph {\n\tgraph [fontname=Arial, nodesep=0.125, ranksep=0.25];\n\tnode [fontcolor=white, fontname=Arial, height=0, shape=box, style=filled, width=0];\n\tedge [fontname=Arial];\n",
    );
    let mut seen = HashSet::new();
    write_graph_node(&loaded.env, root, &mut dot, &mut seen, None)?;
    dot.push_str("}\n");
    fs::write(&out, dot).map_err(|err| err.to_string())?;
    println!("Wrote {}", out.display());
    Ok(())
}

pub(super) fn value_to_bytes(value: &UnityValue) -> Option<Vec<u8>> {
    match value {
        UnityValue::Bytes(value) => Some(value.clone()),
        UnityValue::String(value) => Some(value.as_bytes().to_vec()),
        _ => None,
    }
}

pub(super) fn safe_name(name: &str, fallback_id: i64) -> String {
    let name = if name.trim().is_empty() {
        format!("object_{fallback_id}")
    } else {
        name.to_string()
    };
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn fix_extension_for_type(name: &str, object_type: &str) -> String {
    let rules: &[(&str, &str)] = match object_type {
        "Texture2D" => &[
            ("dds.asset", "png"),
            ("dds.mat", "png"),
            ("dds", "png"),
            ("jpg", "png"),
            ("psd", "png"),
            ("tga", "png"),
            ("tif", "png"),
            ("asset", "png"),
        ],
        "Mesh" => &[],
        "AudioClip" => &[("wav", "ogg"), ("mp3", "ogg")],
        _ => &[],
    };
    for (from, to) in rules {
        if name.to_lowercase().ends_with(from) {
            return format!("{}{}", &name[..name.len() - from.len()], to);
        }
    }
    name.to_string()
}

pub(super) fn fix_extension(name: &str) -> String {
    for (from, to) in [
        ("dds.asset", "png"),
        ("dds.mat", "png"),
        ("dds", "png"),
        ("nif", "obj"),
        ("kfm", "obj"),
        ("wav", "ogg"),
        ("mp3", "ogg"),
        ("jpg", "png"),
        ("psd", "png"),
        ("tga", "png"),
        ("tif", "png"),
        ("asset", "png"),
    ] {
        if name.to_lowercase().ends_with(from) {
            return format!("{}{}", &name[..name.len() - from.len()], to);
        }
    }
    name.to_string()
}

pub(super) fn color_for_type(value: &str) -> String {
    let hash = value.bytes().fold(0_u32, |acc, value| {
        acc.wrapping_mul(31).wrapping_add(value as u32)
    });
    format!(
        "#{:02x}{:02x}{:02x}",
        hash & 0xff,
        (hash >> 8) & 0xff,
        (hash >> 16) & 0xff
    )
}

pub(super) fn dot_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
