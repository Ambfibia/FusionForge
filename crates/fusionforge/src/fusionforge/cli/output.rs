use super::*;

pub(super) fn export_native_static_world(args: &[String]) -> Result<(), String> {
    if args.len() != 4 {
        return Err(
            "export-native-static-world <Map_XX_YY.unity3d> <effective-build-root> <native-asset-root> <fresh-output-root>"
                .to_string(),
        );
    }
    let report = super::super::native_world_static::export_native_static_world(
        super::super::native_world_static::NativeStaticWorldExportOptions {
            map_bundle: PathBuf::from(&args[0]),
            build_root: PathBuf::from(&args[1]),
            native_asset_root: PathBuf::from(&args[2]),
            output_root: PathBuf::from(&args[3]),
        },
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
    );
    Ok(())
}

pub(super) fn export_native_static_behaviours(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err(
            "export-native-static-behaviours <Map_XX_YY.unity3d> <effective-build-root> <fresh-out.json>"
                .to_string(),
        );
    }
    let report = super::super::native_world_static::export_native_static_behaviours(
        super::super::native_world_static::NativeStaticBehaviourExportOptions {
            map_bundle: PathBuf::from(&args[0]),
            build_root: PathBuf::from(&args[1]),
            output_file: PathBuf::from(&args[2]),
        },
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
    );
    Ok(())
}

pub(super) fn export_native_terrains(args: &[String]) -> Result<(), String> {
    let mut positional = Vec::new();
    let mut tile_filter = BTreeSet::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--tile" => {
                let tile = args
                    .get(index + 1)
                    .ok_or_else(|| "--tile requires <xx_yy>".to_string())?;
                if !tile_filter.insert(tile.clone()) {
                    return Err(format!("duplicate --tile selection: {tile}"));
                }
                index += 2;
            }
            value if value.starts_with("--") => {
                return Err(format!("unknown export-native-terrains option: {value}"));
            }
            _ => {
                positional.push(args[index].clone());
                index += 1;
            }
        }
    }
    if positional.len() != 2 {
        return Err(
            "export-native-terrains <client-project|effective-build-root> <fresh-output-root> [--tile <xx_yy>...]"
                .to_string(),
        );
    }
    let manifest = super::super::native_terrain_batch::export_native_terrains_batch(
        Path::new(&positional[0]),
        Path::new(&positional[1]),
        super::super::native_terrain_batch::NativeTerrainBatchOptions { tile_filter },
    )?;
    println!(
        "Native terrain batch {}: exported={}, blocked={}, output={}",
        manifest.status,
        manifest.counts.exported_count,
        manifest.counts.blocked_count,
        manifest.output_root
    );
    Ok(())
}

pub(super) fn export_logical_prop_sources(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err(
            "export-logical-prop-sources <work-dir|cache/bundle-index.json> <reviewed-props.json> <fresh-source-root>"
                .to_string(),
        );
    }
    let manifest = super::super::logical_prop_batch_export::export_logical_prop_sources_batch(
        Path::new(&args[0]),
        Path::new(&args[1]),
        Path::new(&args[2]),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?
    );
    Ok(())
}

pub(super) fn write_snapshot_output(path: &Path, data: &[u8], log_writes: bool) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    fs::write(path, data).map_err(|err| format!("{}: {err}", path.display()))?;
    if log_writes {
        println!("Wrote {} bytes to {}", data.len(), path.display());
    }
    Ok(())
}

pub(super) fn write_graph_node(
    env: &UnityEnvironment,
    key: super::super::unity::ObjectKey,
    dot: &mut String,
    seen: &mut HashSet<String>,
    parent: Option<(String, String)>,
) -> Result<(), String> {
    let asset = &env.assets[key.asset];
    let info = asset
        .objects
        .get(&key.path_id)
        .ok_or_else(|| "graph object missing".to_string())?;
    let body = asset.read_object(key.asset, info)?;
    let qid = format!("{}#{}", asset.name, key.path_id);
    if let Some((parent_qid, label)) = parent {
        dot.push_str(&format!(
            "\t\"{parent_qid}\" -> \"{qid}\" [label=\"{}\"];\n",
            dot_escape(&label)
        ));
    }
    if !seen.insert(qid.clone()) {
        return Ok(());
    }
    let obj_type = asset.object_type_name(info);
    dot.push_str(&format!(
        "\t\"{qid}\" [label=\"{} {}\\n{}\", color=\"{}\"];\n",
        dot_escape(&obj_type),
        key.path_id,
        dot_escape(&object_name(&body)),
        color_for_type(&obj_type)
    ));
    for (label, pointer) in collect_pointers(&body, "") {
        if let Ok(child) = env.resolve_pointer(&pointer) {
            write_graph_node(env, child, dot, seen, Some((qid.clone(), label)))?;
        }
    }
    Ok(())
}

pub(super) fn write_or_print_json(path: Option<&Path>, value: &JsonValue) -> Result<(), String> {
    let data = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    if let Some(path) = path {
        write_output(path, data.as_bytes(), false)
    } else {
        println!("{data}");
        Ok(())
    }
}

pub(super) fn write_output(path: &Path, data: &[u8], dry_run: bool) -> Result<(), String> {
    if dry_run {
        println!("Would write {} bytes to {}", data.len(), path.display());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    fs::write(path, data).map_err(|err| format!("{}: {err}", path.display()))?;
    println!("Wrote {} bytes to {}", data.len(), path.display());
    Ok(())
}
