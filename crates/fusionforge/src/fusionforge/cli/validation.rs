use super::*;

pub(super) fn validate_assetbundle_object(
    env: &UnityEnvironment,
    asset_index: usize,
    asset: &Asset,
    info: &ObjectInfo,
    body: &UnityValue,
) -> Vec<String> {
    let mut problems = Vec::new();
    let local_check = |label: &str, pointer: &Pointer, problems: &mut Vec<String>| {
        if pointer.file_id != 0 {
            problems.push(format!("{label}: external {}", pointer_label(pointer)));
            return;
        }
        if !local_pointer_exists(env, pointer) {
            problems.push(format!("{label}: missing local {}", pointer_label(pointer)));
        }
    };

    if let Some(pointer) = body
        .get("m_MainAsset")
        .and_then(|value| value.get("asset"))
        .and_then(UnityValue::as_pointer)
    {
        local_check("m_MainAsset.asset", pointer, &mut problems);
    }

    let preloads = value_array(body.get("m_PreloadTable")).to_vec();
    for (container_index, entry) in value_array(body.get("m_Container")).iter().enumerate() {
        let Some((container_path, metadata)) = pair_name_value(entry) else {
            continue;
        };
        if let Some(pointer) = metadata.get("asset").and_then(UnityValue::as_pointer) {
            local_check(
                &format!("m_Container[{container_index}] {container_path} asset"),
                pointer,
                &mut problems,
            );
        }
        let (start, end) = cli_metadata_preload_range(metadata, preloads.len());
        for (preload_offset, preload) in preloads[start..end].iter().enumerate() {
            let Some(pointer) = preload.as_pointer() else {
                continue;
            };
            local_check(
                &format!(
                    "m_Container[{container_index}] {container_path} preload[{}]",
                    start + preload_offset
                ),
                pointer,
                &mut problems,
            );
        }
    }

    if !problems.is_empty() {
        problems.insert(
            0,
            format!(
                "{}#{} AssetBundle container/preload problems:",
                asset.name, info.path_id
            ),
        );
    }
    let _ = asset_index;
    problems
}

pub(super) fn validate_bundle_refs(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        if args.is_empty() {
            return Err("validate-bundle-refs <asset-or-bundle>...".to_string());
        }
        for input in &args {
            let path = PathBuf::from(input);
            let loaded = load_input(&path)?;
            println!("# {}", path.display());
            for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
                println!(
                    "asset {}\tformat={}\tlongIds={}\tobjects={}\tassetRefs={}",
                    asset.name,
                    asset.format,
                    asset.long_object_ids,
                    asset.objects.len(),
                    asset.asset_refs.len()
                );
                let mut total_pointers = 0usize;
                let mut local_ok = 0usize;
                let mut local_missing = Vec::<String>::new();
                let mut external_ok = 0usize;
                let mut external_missing = Vec::<String>::new();
                let mut assetbundle_problems = Vec::<String>::new();
                let mut external_refs = BTreeMap::<String, usize>::new();

                for info in asset.objects.values() {
                    let body = match asset.read_object(asset_index, info) {
                        Ok(body) => body,
                        Err(err) => {
                            println!(
                                "  unreadable {}#{} {}: {err}",
                                asset.name,
                                info.path_id,
                                asset.object_type_name(info)
                            );
                            continue;
                        }
                    };
                    if asset.object_type_name(info) == "AssetBundle" {
                        assetbundle_problems.extend(validate_assetbundle_object(
                            &loaded.env,
                            asset_index,
                            asset,
                            info,
                            &body,
                        ));
                    }
                    let mut pointers = Vec::<(String, Pointer)>::new();
                    collect_pointers_with_path(&body, "", &mut pointers);
                    for (field_path, pointer) in pointers {
                        total_pointers += 1;
                        if pointer.file_id == 0 {
                            if local_pointer_exists(&loaded.env, &pointer) {
                                local_ok += 1;
                            } else if local_missing.len() < 20 {
                                local_missing.push(format!(
                                    "{}#{} {} {} -> {}",
                                    asset.name,
                                    info.path_id,
                                    asset.object_type_name(info),
                                    field_path,
                                    pointer_label(&pointer)
                                ));
                            }
                        } else {
                            let ref_label = asset_ref_label(&loaded.env, &pointer);
                            *external_refs.entry(ref_label.clone()).or_insert(0) += 1;
                            match loaded.env.resolve_pointer(&pointer) {
                                Ok(_) => external_ok += 1,
                                Err(err) => {
                                    if external_missing.len() < 20 {
                                        external_missing.push(format!(
                                            "{}#{} {} {} -> {} ({ref_label}: {err})",
                                            asset.name,
                                            info.path_id,
                                            asset.object_type_name(info),
                                            field_path,
                                            pointer_label(&pointer)
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }

                println!(
                    "  pointers={total_pointers} localOk={local_ok} localMissing={} externalOk={external_ok} externalMissing={}",
                    local_missing.len(),
                    external_missing.len()
                );
                if !assetbundle_problems.is_empty() {
                    for problem in assetbundle_problems.iter().take(30) {
                        println!("  {problem}");
                    }
                }
                if !external_refs.is_empty() {
                    println!("  external refs:");
                    for (name, count) in external_refs.iter().take(12) {
                        println!("    {count}\t{name}");
                    }
                }
                if !local_missing.is_empty() {
                    println!("  local missing examples:");
                    for problem in local_missing {
                        println!("    {problem}");
                    }
                }
                if !external_missing.is_empty() {
                    println!("  external missing examples:");
                    for problem in external_missing {
                        println!("    {problem}");
                    }
                }
            }
        }
        Ok(())
    })
}

pub(super) fn validate_object_sizes(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        if args.is_empty() {
            return Err("validate-object-sizes <asset-or-bundle>...".to_string());
        }
        let mut failed = false;
        for input in &args {
            let path = PathBuf::from(input);
            let loaded = load_input(&path)?;
            println!("# {}", path.display());
            for (asset_index, asset) in loaded.env.assets.iter().enumerate() {
                println!(
                    "asset {}\tformat={}\tobjects={}",
                    asset.name,
                    asset.format,
                    asset.objects.len()
                );
                let mut mismatches = 0usize;
                let mut unreadable = 0usize;
                let mut skin_issues = 0usize;
                let mut animation_issues = 0usize;
                for info in asset.objects.values() {
                    let object_type = asset.object_type_name(info);
                    match asset.read_object_with_size(asset_index, info) {
                        Ok(result) if result.consumed == result.expected => {}
                        Ok(result) => {
                            failed = true;
                            mismatches += 1;
                            println!(
                                "  size mismatch {}#{} {} consumed={} expected={} delta={}",
                                asset.name,
                                info.path_id,
                                object_type,
                                result.consumed,
                                result.expected,
                                result.consumed as isize - result.expected as isize
                            );
                        }
                        Err(err) => {
                            failed = true;
                            unreadable += 1;
                            println!(
                                "  unreadable {}#{} {} size={} offset={}: {err}",
                                asset.name, info.path_id, object_type, info.size, info.data_offset
                            );
                        }
                    }
                    if object_type == "SkinnedMeshRenderer" {
                        let Ok(body) = asset.read_object(asset_index, info) else {
                            continue;
                        };
                        let bones = value_array(body.get("m_Bones")).len();
                        let renderer_bind_poses = value_array(body.get("m_BindPose")).len();
                        if renderer_bind_poses != 0 && bones != renderer_bind_poses {
                            failed = true;
                            skin_issues += 1;
                            println!(
                                "  skin mismatch {}#{} SkinnedMeshRenderer bones={} bindPoses={}",
                                asset.name, info.path_id, bones, renderer_bind_poses
                            );
                        }
                        if let Some(mesh_pointer) =
                            body.get("m_Mesh").and_then(UnityValue::as_pointer)
                        {
                            if mesh_pointer.file_id == 0 {
                                if let Some(mesh_info) = asset.objects.get(&mesh_pointer.path_id) {
                                    if asset.object_type_name(mesh_info) == "Mesh" {
                                        if let Ok(mesh) = asset.read_object(asset_index, mesh_info)
                                        {
                                            let mesh_bind_poses =
                                                value_array(mesh.get("m_BindPose")).len();
                                            if bones != mesh_bind_poses {
                                                failed = true;
                                                skin_issues += 1;
                                                println!(
                                                    "  skin mesh mismatch {}#{} -> Mesh#{} bones={} meshBindPoses={}",
                                                    asset.name,
                                                    info.path_id,
                                                    mesh_pointer.path_id,
                                                    bones,
                                                    mesh_bind_poses
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if object_type == "Animation" {
                        let Ok(body) = asset.read_object(asset_index, info) else {
                            continue;
                        };
                        let issues =
                            validate_animation_component(&loaded.env, &asset.name, info, &body);
                        if !issues.is_empty() {
                            failed = true;
                            animation_issues += issues.len();
                            for issue in issues {
                                println!("  {issue}");
                            }
                        }
                    }
                }
                println!(
                    "  mismatches={mismatches} unreadable={unreadable} skinIssues={skin_issues} animationIssues={animation_issues}"
                );
            }
        }
        if failed {
            Err("object size validation failed".to_string())
        } else {
            Ok(())
        }
    })
}

pub(super) fn validate_evidence_source_alias(alias: &str) -> Result<(), String> {
    if alias.is_empty()
        || !alias
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        return Err(
            "source alias must contain only ASCII letters, digits, '.', '_' or '-'".to_string(),
        );
    }
    Ok(())
}

pub(super) fn audit_world_transform_contract(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err("audit-world-transform-contract <build-root> <fresh-report.json>".to_string());
    }
    let report = super::super::world_transform_audit::audit_world_transform_contract(
        Path::new(&args[0]),
        Path::new(&args[1]),
    )?;
    println!(
        "processed {}/{} Map archives: passed={}, failed={}, scene-nodes={}, nonunit={}, nonuniform={}, negative={}, singular={}, nonzero-root-origins={}, colliders={}, ambiguous-dependencies={}, errors={}",
        report.counts.processed_map_archives,
        report.counts.map_archives,
        report.counts.passed_map_archives,
        report.counts.failed_map_archives,
        report.counts.scene_nodes,
        report.counts.nonunit_scale_nodes,
        report.counts.nonuniform_scale_nodes,
        report.counts.negative_scale_nodes,
        report.counts.singular_scale_nodes,
        report.counts.nonzero_root_origins,
        report.counts.colliders,
        report.counts.ambiguous_dependency_archives,
        report.counts.errors,
    );
    if report.passed {
        Ok(())
    } else {
        Err(format!(
            "world transform-contract audit found blockers; immutable report was written to {}",
            args[1]
        ))
    }
}
