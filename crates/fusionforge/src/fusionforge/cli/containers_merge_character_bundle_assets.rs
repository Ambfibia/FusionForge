use super::*;

pub(super) fn cli_set_unity_object_i64(value: &mut UnityValue, key: &str, number: i64) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), UnityValue::Int(number));
    }
}

pub(super) fn cli_assetbundle_value_has_container_prefix(value: &UnityValue, prefix: &str) -> bool {
    value_array(value.get("m_Container")).iter().any(|entry| {
        pair_name_value(entry)
            .map(|(path, _)| cli_normalized_asset_path(path).starts_with(prefix))
            .unwrap_or(false)
    })
}

pub(super) fn merge_character_bundle_assets(args: &[String]) -> Result<(), String> {
    let input = required_path(
        args,
        0,
        "merge-character-bundle-assets <input.resourceFile> [output.resourceFile]",
    )?;
    let output = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| input.clone());
    let loaded = load_input(&input)?;
    if loaded.env.assets.len() <= 1 {
        if input != output {
            fs::copy(&input, &output).map_err(|err| format!("{}: {err}", output.display()))?;
        }
        println!("{} already has one serialized asset file", input.display());
        return Ok(());
    }

    let env = loaded.env;
    let primary_asset_index = env
        .assets
        .iter()
        .enumerate()
        .find_map(|(asset_index, asset)| {
            asset.objects.values().find_map(|info| {
                (asset.object_type_name(info) == "AssetBundle")
                    .then(|| asset.read_object(asset_index, info).ok())
                    .flatten()
                    .filter(|value| cli_assetbundle_value_has_container_prefix(value, "mob/"))
                    .map(|_| asset_index)
            })
        })
        .ok_or_else(|| {
            format!(
                "{}: no AssetBundle with mob/ container found",
                input.display()
            )
        })?;
    let primary_asset = &env.assets[primary_asset_index];
    let selected = env
        .assets
        .iter()
        .enumerate()
        .flat_map(|(asset_index, asset)| {
            asset
                .objects
                .keys()
                .map(move |path_id| (asset_index, *path_id))
        })
        .collect::<BTreeSet<_>>();
    let mut assetbundle_replacements = BTreeMap::<(usize, i64), UnityValue>::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) == "AssetBundle" {
                assetbundle_replacements.insert(
                    (asset_index, info.path_id),
                    asset.read_object(asset_index, info)?,
                );
            }
        }
    }
    let merged_assetbundle = cli_merge_assetbundle_replacements_to_primary(
        &assetbundle_replacements,
        &env,
        primary_asset_index,
        &selected,
    )
    .ok_or_else(|| {
        format!(
            "{}: failed to merge AssetBundle containers",
            input.display()
        )
    })?;

    let keep_ids = primary_asset
        .objects
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut data_replacements = BTreeMap::<i64, Vec<u8>>::new();
    for path_id in &keep_ids {
        let info = primary_asset
            .objects
            .get(path_id)
            .ok_or_else(|| format!("{}#{} not found", primary_asset.name, path_id))?;
        let mut value = if merged_assetbundle.0 == (primary_asset_index, *path_id) {
            merged_assetbundle.1.clone()
        } else {
            primary_asset.read_object(primary_asset_index, info)?
        };
        cli_rewrite_selected_external_pointers_to_local(
            &mut value,
            &env,
            primary_asset_index,
            &selected,
        );
        data_replacements.insert(
            *path_id,
            primary_asset.serialize_object_value(primary_asset_index, info, &value)?,
        );
    }

    let mut known_objects = BTreeMap::<i64, (String, String)>::new();
    for (path_id, info) in &primary_asset.objects {
        let name = primary_asset
            .read_object(primary_asset_index, info)
            .ok()
            .map(|value| object_name(&value))
            .unwrap_or_default();
        known_objects.insert(
            *path_id,
            (primary_asset.object_type_name(info).to_string(), name),
        );
    }
    let mut extra_objects = Vec::<(ObjectInfo, Vec<u8>)>::new();
    let mut merged_path_ids = keep_ids.clone();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if asset_index == primary_asset_index {
            continue;
        }
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) == "AssetBundle" {
                continue;
            }
            if !merged_path_ids.insert(*path_id) {
                let name = asset
                    .read_object(asset_index, info)
                    .ok()
                    .map(|value| object_name(&value))
                    .unwrap_or_default();
                let current = (asset.object_type_name(info).to_string(), name);
                if known_objects.get(path_id) == Some(&current) {
                    continue;
                }
                return Err(format!(
                    "{}: cannot merge {}, duplicate path id {}",
                    input.display(),
                    asset.name,
                    path_id
                ));
            }
            let mut value = asset.read_object(asset_index, info)?;
            cli_rewrite_selected_external_pointers_to_local(
                &mut value,
                &env,
                primary_asset_index,
                &selected,
            );
            known_objects.insert(
                *path_id,
                (
                    asset.object_type_name(info).to_string(),
                    object_name(&value),
                ),
            );
            extra_objects.push((
                info.clone(),
                asset.serialize_object_value(asset_index, info, &value)?,
            ));
        }
    }

    let session_dir = cli_session_dir(&input)?;
    let out_dir = session_dir.join("merged");
    fs::create_dir_all(&out_dir).map_err(|err| format!("{}: {err}", out_dir.display()))?;
    let merged_asset = primary_asset.rebuild_with_object_data_filtered_and_extra(
        Some(&keep_ids),
        &data_replacements,
        &extra_objects,
    )?;
    fs::write(out_dir.join(&primary_asset.name), merged_asset)
        .map_err(|err| format!("{}: {err}", out_dir.display()))?;
    let packed = ffbuildtool::bundle::AssetBundle::from_directory(&out_dir.to_string_lossy())?;
    let pack_target = if output == input {
        session_dir.join(
            input
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("merged.resourceFile"),
        )
    } else {
        output.clone()
    };
    packed.to_file(&pack_target.to_string_lossy(), 4, None)?;
    if output == input {
        fs::copy(&pack_target, &output).map_err(|err| format!("{}: {err}", output.display()))?;
    }
    println!(
        "{}: merged {} internal asset files into {} ({} extra objects)",
        input.display(),
        env.assets.len(),
        primary_asset.name,
        extra_objects.len()
    );
    Ok(())
}

pub(super) const DUMP_OBJECT_EVIDENCE_USAGE: &str =
    "dump-object-evidence <source-alias> <source-root> <source-relative-container> <path-id> \
     [--serialized-asset <exact-name>] [--type <exact-unity-type>] [--out <out.json|->] \
     [--allow-unresolved-pointers]";

pub(super) fn resolve_source_relative_container(
    source_root: &Path,
    relative_container: &str,
) -> Result<(PathBuf, String), String> {
    let relative = Path::new(relative_container);
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err("source-relative container must be a non-empty relative path".to_string());
    }

    let mut normalized_parts = Vec::<String>::new();
    for component in relative.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| {
                    "source-relative container must be valid Unicode for evidence".to_string()
                })?;
                normalized_parts.push(part.to_string());
            }
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(
                    "source-relative container may not escape its declared source root".to_string(),
                );
            }
        }
    }
    if normalized_parts.is_empty() {
        return Err("source-relative container resolves to an empty path".to_string());
    }

    let canonical_root = fs::canonicalize(source_root)
        .map_err(|error| format!("{}: {error}", source_root.display()))?;
    if !canonical_root.is_dir() {
        return Err(format!(
            "source root is not a directory: {}",
            source_root.display()
        ));
    }
    let candidate = canonical_root.join(relative);
    let canonical_candidate = fs::canonicalize(&candidate)
        .map_err(|error| format!("{}: {error}", candidate.display()))?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(
            "source-relative container resolves outside its declared source root".to_string(),
        );
    }
    if !canonical_candidate.is_file() {
        return Err(format!(
            "source-relative container is not a file: {}",
            candidate.display()
        ));
    }

    Ok((canonical_candidate, normalized_parts.join("/")))
}

pub(super) fn dump_object_evidence(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || dump_object_evidence_inner(&args))
}

pub(super) fn dump_object_evidence_inner(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(DUMP_OBJECT_EVIDENCE_USAGE.to_string());
    }

    let source_alias = &args[0];
    validate_evidence_source_alias(source_alias)?;
    let source_root = Path::new(&args[1]);
    let (container_path, relative_container) =
        resolve_source_relative_container(source_root, &args[2])?;
    let path_id = parse_i64(&args[3])?;

    let mut serialized_asset = None::<String>;
    let mut expected_type = None::<String>;
    let mut output = None::<PathBuf>;
    let mut allow_unresolved_pointers = false;
    let mut index = 4usize;
    while index < args.len() {
        match args[index].as_str() {
            "--serialized-asset" => {
                let value = args.get(index + 1).cloned().ok_or_else(|| {
                    "--serialized-asset requires an exact serialized asset name".to_string()
                })?;
                if serialized_asset.replace(value).is_some() {
                    return Err("--serialized-asset may be supplied only once".to_string());
                }
                index += 2;
            }
            "--type" => {
                let value = args
                    .get(index + 1)
                    .cloned()
                    .ok_or_else(|| "--type requires an exact Unity type".to_string())?;
                if expected_type.replace(value).is_some() {
                    return Err("--type may be supplied only once".to_string());
                }
                index += 2;
            }
            "--out" => {
                let value = args
                    .get(index + 1)
                    .cloned()
                    .ok_or_else(|| "--out requires a JSON path or '-'".to_string())?;
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err("--out may be supplied only once".to_string());
                }
                index += 2;
            }
            "--allow-unresolved-pointers" => {
                if allow_unresolved_pointers {
                    return Err("--allow-unresolved-pointers may be supplied only once".to_string());
                }
                allow_unresolved_pointers = true;
                index += 1;
            }
            option => {
                return Err(format!(
                    "unknown dump-object-evidence option '{option}'\n{DUMP_OBJECT_EVIDENCE_USAGE}"
                ));
            }
        }
    }

    let container_bytes = fs::read(&container_path)
        .map_err(|error| format!("{}: {error}", container_path.display()))?;
    let loaded = load_input(&container_path)?;
    let evidence = super::super::object_evidence::build_object_evidence(
        &loaded.env,
        super::super::object_evidence::ObjectEvidenceRequest {
            source_alias,
            relative_container: &relative_container,
            container_bytes: &container_bytes,
            serialized_asset: serialized_asset.as_deref(),
            expected_type: expected_type.as_deref(),
            path_id,
            allow_unresolved_pointers,
        },
    )?;

    match output.as_deref() {
        None => write_or_print_json(None, &evidence),
        Some(path) if path == Path::new("-") => write_or_print_json(None, &evidence),
        Some(path) => write_or_print_json(Some(path), &evidence),
    }
}

pub(super) fn dump_object(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        let path = required_path(
            &args,
            0,
            "dump-object <asset-or-bundle> [path-id|all] [out.json]",
        )?;
        let id = args.get(1).map(String::as_str).unwrap_or("all");
        let output = args.get(2).map(PathBuf::from);
        let loaded = load_input(&path)?;
        let value = if id.eq_ignore_ascii_case("all") {
            let mut objects = Vec::new();
            for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
                for (path_id, info) in &asset.objects {
                    let body = asset.read_object(asset_index, info)?;
                    objects.push(json!({
                        "asset": asset.name,
                        "pathId": path_id,
                        "typeId": info.type_id,
                        "classId": info.class_id,
                        "type": asset.object_type_name(info),
                        "name": object_name(&body),
                        "value": unity_to_json(&body),
                    }));
                }
            }
            JsonValue::Array(objects)
        } else {
            let path_id = parse_i64(id)?;
            let (asset_index, asset, info) = find_object(&loaded.env, path_id)?;
            let body = asset.read_object(asset_index, info)?;
            json!({
                "asset": asset.name,
                "pathId": path_id,
                "typeId": info.type_id,
                "classId": info.class_id,
                "type": asset.object_type_name(info),
                "name": object_name(&body),
                "value": unity_to_json(&body),
            })
        };
        write_or_print_json(output.as_deref(), &value)
    })
}

pub(super) fn unity_extract(args: &[String]) -> Result<(), String> {
    let options = parse_extract_options(args)?;
    for file in &options.files {
        let loaded = load_input(file)?;
        for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
            for (path_id, info) in &asset.objects {
                export_object(&loaded.env, asset_index, *path_id, info, &options, None)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn snapshot_npc_bundle(bundle: &Path, outdir: &Path) -> Result<(), String> {
    snapshot_npc_bundle_impl(bundle, outdir, true)
}

pub(crate) fn snapshot_npc_bundle_quiet(bundle: &Path, outdir: &Path) -> Result<(), String> {
    snapshot_npc_bundle_impl(bundle, outdir, false)
}

pub(super) fn snapshot_npc_bundle_impl(bundle: &Path, outdir: &Path, log_writes: bool) -> Result<(), String> {
    if outdir.exists() {
        clear_readonly_recursive(outdir)?;
        fs::remove_dir_all(outdir).map_err(|err| format!("{}: {err}", outdir.display()))?;
    }
    fs::create_dir_all(outdir).map_err(|err| format!("{}: {err}", outdir.display()))?;

    let loaded = load_input(bundle)?;
    let mut entries = Vec::<JsonValue>::new();
    let mut selected = BTreeSet::<(usize, i64)>::new();
    let mut queue = VecDeque::<(usize, i64)>::new();
    let mut root_keys = BTreeSet::<(usize, i64)>::new();
    let mut container_roots = Vec::<(String, (usize, i64))>::new();
    let mut written = BTreeSet::<String>::new();

    for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info)?;
            let preloads = value_array(body.get("m_PreloadTable")).to_vec();
            for (container_path, metadata) in container_entries(&body) {
                let Some((_target_index, pointer, _target_info)) =
                    resolve_npc_snapshot_container_target(
                        &loaded.env,
                        asset_index,
                        &container_path,
                        metadata,
                        &preloads,
                    )
                else {
                    entries.push(json!({
                        "role": "container",
                        "containerPath": container_path,
                        "file": null,
                        "status": "missing"
                    }));
                    continue;
                };
                let Some(target_key) =
                    resolved_object_key_for_snapshot(&loaded.env, asset_index, pointer)
                else {
                    continue;
                };
                root_keys.insert((target_key.asset, target_key.path_id));
                container_roots.push((
                    container_path.clone(),
                    (target_key.asset, target_key.path_id),
                ));
                add_snapshot_key(
                    &mut selected,
                    &mut queue,
                    (target_key.asset, target_key.path_id),
                );
                let (start, end) = cli_metadata_preload_range(metadata, preloads.len());
                for preload in &preloads[start..end] {
                    if let Some(pointer) = preload.as_pointer() {
                        for key in
                            snapshot_pointer_candidate_keys(&loaded.env, asset_index, pointer)
                        {
                            if snapshot_key_type(&loaded.env, key).as_deref() == Some("Texture2D") {
                                continue;
                            }
                            add_snapshot_key(&mut selected, &mut queue, key);
                        }
                    }
                }
            }
        }
    }

    while let Some((asset_index, path_id)) = queue.pop_front() {
        let Some(asset) = loaded.env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        if asset.object_type_name(info) == "AssetBundle" {
            continue;
        }
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        for (_label, pointer) in collect_pointers(&body, "") {
            for key in snapshot_pointer_candidate_keys(&loaded.env, asset_index, &pointer) {
                add_snapshot_key(&mut selected, &mut queue, key);
            }
        }
    }

    let material_texture_refs = snapshot_material_texture_refs(&loaded.env, &selected);
    for (container_path, (asset_index, path_id)) in &container_roots {
        let Some(asset) = loaded.env.assets.get(*asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(path_id) else {
            continue;
        };
        let object_type = asset.object_type_name(info);
        if object_type == "Texture2D"
            && !material_texture_refs.is_empty()
            && !material_texture_refs.contains(&(*asset_index, *path_id))
        {
            root_keys.remove(&(*asset_index, *path_id));
            selected.remove(&(*asset_index, *path_id));
            continue;
        }
        let out_path = outdir.join(npc_snapshot_container_output_path(
            container_path,
            &object_type,
        ));
        let record = export_npc_snapshot_object(
            &loaded.env,
            *asset_index,
            *path_id,
            info,
            &out_path,
            Some(container_path),
            "container",
            log_writes,
            &mut written,
        )?;
        entries.push(record);
    }

    for (asset_index, path_id) in selected {
        if root_keys.contains(&(asset_index, path_id)) {
            continue;
        }
        let Some(asset) = loaded.env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        if asset.object_type_name(info) == "AssetBundle" {
            continue;
        }
        let body = asset.read_object(asset_index, info).ok();
        let name = body
            .as_ref()
            .map(object_name)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("object_{path_id}"));
        let object_type = asset.object_type_name(info);
        let out_path = outdir.join(npc_snapshot_dependency_output_path(
            &object_type,
            &name,
            path_id,
        ));
        let record = export_npc_snapshot_object(
            &loaded.env,
            asset_index,
            path_id,
            info,
            &out_path,
            None,
            "dependency",
            log_writes,
            &mut written,
        )?;
        entries.push(record);
    }

    entries.sort_by(|left, right| {
        left.get("file")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .cmp(right.get("file").and_then(JsonValue::as_str).unwrap_or(""))
    });
    let manifest = json!({
        "format": "fftools.npc-assets.v3",
        "layout": "final-bundle-snapshot",
        "sourceBundle": bundle.to_string_lossy().replace('\\', "/"),
        "entries": entries,
    });
    write_or_print_json(Some(&outdir.join("_manifest.json")), &manifest)?;
    Ok(())
}

pub(super) fn resolved_object_key_for_snapshot(
    env: &UnityEnvironment,
    source_asset_index: usize,
    pointer: &Pointer,
) -> Option<ObjectKey> {
    env.resolve_pointer(pointer).ok().or_else(|| {
        (pointer.file_id == 0).then_some(ObjectKey {
            asset: source_asset_index,
            path_id: pointer.path_id,
        })
    })
}

pub(super) fn resolve_npc_snapshot_container_target<'a>(
    env: &'a UnityEnvironment,
    source_asset_index: usize,
    container_path: &str,
    metadata: &'a UnityValue,
    preloads: &'a [UnityValue],
) -> Option<(usize, &'a Pointer, &'a ObjectInfo)> {
    let expected = npc_snapshot_expected_types(container_path);
    if let Some(pointer) = metadata.get("asset").and_then(UnityValue::as_pointer) {
        if let Some((asset_index, info)) =
            resolved_object_index_info(env, source_asset_index, pointer)
        {
            let obj_type = env.assets[asset_index].object_type_name(info);
            if expected.is_empty() || expected.contains(&obj_type.as_str()) {
                return Some((asset_index, pointer, info));
            }
        }
    }
    let (start, end) = cli_metadata_preload_range(metadata, preloads.len());
    for preload in &preloads[start..end] {
        let Some(pointer) = preload.as_pointer() else {
            continue;
        };
        let Some((asset_index, info)) =
            resolved_object_index_info(env, source_asset_index, pointer)
        else {
            continue;
        };
        let obj_type = env.assets[asset_index].object_type_name(info);
        if expected.is_empty() || expected.contains(&obj_type.as_str()) {
            return Some((asset_index, pointer, info));
        }
    }
    None
}

pub(super) fn export_npc_snapshot_object(
    env: &UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    info: &ObjectInfo,
    path: &Path,
    container_path: Option<&str>,
    role: &str,
    log_writes: bool,
    written: &mut BTreeSet<String>,
) -> Result<JsonValue, String> {
    let asset = &env.assets[asset_index];
    let object_type = asset.object_type_name(info);
    let body = asset.read_object(asset_index, info)?;
    let name = object_name(&body);
    let mut status = "ok";
    if written.insert(path.to_string_lossy().to_ascii_lowercase()) {
        match object_type.as_str() {
            "Texture2D" => {
                if let Some(texture) =
                    decode_texture(env, &body).and_then(|texture| texture.image())
                {
                    let mut image = texture;
                    image::imageops::flip_vertical_in_place(&mut image);
                    write_snapshot_png_output(path, &image, log_writes)?;
                } else {
                    status = "unexported";
                }
            }
            "AudioClip" => {
                let data = body
                    .get("audio data")
                    .and_then(value_to_bytes)
                    .or_else(|| body.get("m_AudioData").and_then(value_to_bytes))
                    .unwrap_or_default();
                if data.is_empty() {
                    status = "unexported";
                } else {
                    write_snapshot_output(path, &data, log_writes)?;
                }
            }
            "TextAsset" | "Shader" => {
                let data = body
                    .get("m_Script")
                    .and_then(value_to_bytes)
                    .unwrap_or_default();
                write_snapshot_output(path, &data, log_writes)?;
            }
            _ => {
                let data = asset.object_raw_data(info)?;
                write_snapshot_output(path, data, log_writes)?;
            }
        }
    }
    Ok(json!({
        "role": role,
        "containerPath": container_path,
        "file": path.to_string_lossy().replace('\\', "/"),
        "asset": asset.name,
        "pathId": path_id,
        "type": object_type,
        "name": name,
        "status": status,
    }))
}

pub(super) fn resolve_container_export_target<'a>(
    env: &'a UnityEnvironment,
    source_asset_index: usize,
    container_path: &str,
    metadata: &'a UnityValue,
    preloads: &'a [UnityValue],
) -> Option<(usize, &'a Pointer, &'a ObjectInfo)> {
    let expected = cli_container_expected_export_types(container_path);
    if let Some(pointer) = metadata.get("asset").and_then(UnityValue::as_pointer) {
        if let Some((asset_index, info)) =
            resolved_object_index_info(env, source_asset_index, pointer)
        {
            let obj_type = env.assets[asset_index].object_type_name(info);
            if expected.is_empty() || expected.contains(&obj_type.as_str()) {
                return Some((asset_index, pointer, info));
            }
        }
    }

    let (start, end) = cli_metadata_preload_range(metadata, preloads.len());
    for preload in &preloads[start..end] {
        let Some(pointer) = preload.as_pointer() else {
            continue;
        };
        let Some((asset_index, info)) =
            resolved_object_index_info(env, source_asset_index, pointer)
        else {
            continue;
        };
        let obj_type = env.assets[asset_index].object_type_name(info);
        if expected.is_empty() || expected.contains(&obj_type.as_str()) {
            return Some((asset_index, pointer, info));
        }
    }
    None
}
