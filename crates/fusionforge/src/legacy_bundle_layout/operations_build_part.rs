use super::*;

pub(super) fn rewrite_output_pointers(
    value: &mut UnityValue,
    source_asset: usize,
    current_part: usize,
    inventory: &Inventory,
    by_internal_name: &BTreeMap<String, Vec<usize>>,
    plan: &Plan,
    external_file_ids: &BTreeMap<usize, i32>,
    preserved_file_ids: &BTreeMap<(usize, i32), i32>,
) -> Result<(), String> {
    match value {
        UnityValue::Array(items) => {
            for item in items {
                rewrite_output_pointers(
                    item,
                    source_asset,
                    current_part,
                    inventory,
                    by_internal_name,
                    plan,
                    external_file_ids,
                    preserved_file_ids,
                )?;
            }
        }
        UnityValue::Object(fields) => {
            for item in fields.values_mut() {
                rewrite_output_pointers(
                    item,
                    source_asset,
                    current_part,
                    inventory,
                    by_internal_name,
                    plan,
                    external_file_ids,
                    preserved_file_ids,
                )?;
            }
        }
        UnityValue::Pair(left, right) => {
            rewrite_output_pointers(
                left,
                source_asset,
                current_part,
                inventory,
                by_internal_name,
                plan,
                external_file_ids,
                preserved_file_ids,
            )?;
            rewrite_output_pointers(
                right,
                source_asset,
                current_part,
                inventory,
                by_internal_name,
                plan,
                external_file_ids,
                preserved_file_ids,
            )?;
        }
        UnityValue::Pointer(pointer) if !pointer.is_null() => {
            // `source_asset` is authoritative. Values cloned from another asset retain the
            // read-time source index, but freshly-created container pointers do not.
            pointer.source_asset = source_asset;
            let Some(target) =
                resolve_source_pointer(pointer, &inventory.assets, by_internal_name)?
            else {
                if preserved_external_ref(pointer, &inventory.assets, by_internal_name).is_some() {
                    let old_file_id = pointer.file_id;
                    pointer.file_id = *preserved_file_ids
                        .get(&(source_asset, old_file_id))
                        .ok_or_else(|| {
                            format!(
                                "preserved external ref {source_asset}:{old_file_id} was not copied"
                            )
                        })?;
                    pointer.source_asset = 0;
                    return Ok(());
                }
                return Err(format!(
                    "cannot rewrite dangling PPtr from asset {source_asset}: {}:{}",
                    pointer.file_id, pointer.path_id
                ));
            };
            let target = canonical_key(&plan.canonical, target);
            let location = plan
                .locations
                .get(&target)
                .ok_or_else(|| format!("resolved PPtr target {target:?} has no output location"))?;
            pointer.source_asset = 0;
            pointer.path_id = location.path_id;
            if location.part == current_part {
                pointer.file_id = 0;
            } else {
                pointer.file_id = *external_file_ids.get(&location.part).ok_or_else(|| {
                    format!(
                        "{} lacks external ref to {}",
                        plan.parts[current_part].output_name, plan.parts[location.part].output_name
                    )
                })?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn clear_matching_source_pointers(
    value: &mut UnityValue,
    allowed: &BTreeSet<SourcePointerKey>,
) -> usize {
    match value {
        UnityValue::Array(items) => items
            .iter_mut()
            .map(|item| clear_matching_source_pointers(item, allowed))
            .sum(),
        UnityValue::Object(fields) => fields
            .values_mut()
            .map(|item| clear_matching_source_pointers(item, allowed))
            .sum(),
        UnityValue::Pair(left, right) => {
            clear_matching_source_pointers(left, allowed)
                + clear_matching_source_pointers(right, allowed)
        }
        UnityValue::Pointer(pointer)
            if !pointer.is_null() && allowed.contains(&SourcePointerKey::from(&*pointer)) =>
        {
            pointer.source_asset = 0;
            pointer.file_id = 0;
            pointer.path_id = 0;
            1
        }
        _ => 0,
    }
}

pub(super) fn clear_recorded_dangling_pointers(
    value: &mut UnityValue,
    recorded: &[DanglingObjectPointer],
) -> Result<usize, String> {
    let allowed = recorded
        .iter()
        .map(|entry| entry.source)
        .collect::<BTreeSet<_>>();
    let cleared = clear_matching_source_pointers(value, &allowed);
    if cleared != recorded.len() {
        return Err(format!(
            "planned {} proven-missing PPtr occurrence(s), but exact clearing found {cleared}",
            recorded.len()
        ));
    }
    Ok(cleared)
}

pub(super) fn assign_output_type(
    preferred: i32,
    tree: &TypeTree,
    trees: &mut BTreeMap<i32, (String, TypeTree)>,
    by_signature: &mut BTreeMap<(i32, String), i32>,
    next_synthetic: &mut i32,
) -> i32 {
    let signature = tree_signature(tree);
    // Equal field layouts do not imply equal Unity classes. A few legacy objects use an
    // AudioClip-shaped tree under another class ID; merging by tree alone silently changes
    // their runtime type after repacking.
    let identity = (preferred, signature.clone());
    if let Some(existing) = by_signature.get(&identity) {
        return *existing;
    }
    let output_id = match trees.get(&preferred) {
        None => preferred,
        Some((existing, _)) if *existing == signature => preferred,
        Some(_) => {
            while trees.contains_key(next_synthetic) {
                *next_synthetic = next_synthetic.saturating_sub(1);
            }
            let value = *next_synthetic;
            *next_synthetic = next_synthetic.saturating_sub(1);
            value
        }
    };
    trees.insert(output_id, (signature.clone(), tree.clone()));
    by_signature.insert(identity, output_id);
    output_id
}

pub(super) fn pointer_value(file_id: i32, path_id: i64) -> UnityValue {
    UnityValue::Pointer(Pointer {
        source_asset: 0,
        file_id,
        path_id,
    })
}

pub(super) fn build_part(
    part_index: usize,
    inventory: &Inventory,
    plan: &Plan,
    dependencies: &BTreeMap<usize, BTreeSet<usize>>,
    staging_root: &Path,
    max_part_bytes: u64,
) -> Result<PathBuf, String> {
    let part = &plan.parts[part_index];
    let by_internal_name = source_name_index(&inventory.assets);
    let required = dependencies.get(&part_index).cloned().unwrap_or_default();
    let mut output_refs = vec![AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: part.internal_name.clone(),
    }];
    let mut external_file_ids = BTreeMap::<usize, i32>::new();
    for dependency in required {
        let file_id = i32::try_from(output_refs.len())
            .map_err(|_| "too many output external refs".to_string())?;
        external_file_ids.insert(dependency, file_id);
        output_refs.push(AssetRef {
            asset_path: String::new(),
            guid: [0; 16],
            type_id: 0,
            // Unity 2 resolves this by internal serialized file name, not disk bundle name.
            file_path: plan.parts[dependency].internal_name.clone(),
        });
    }
    let mut preserved_file_ids = BTreeMap::<(usize, i32), i32>::new();
    let mut preserved_by_key = BTreeMap::<(String, [u8; 16], i32, String), i32>::new();
    for key in &part.nodes {
        let meta = &inventory.assets[key.0];
        for (old_index, reference) in meta.refs.iter().enumerate().skip(1) {
            let pointer = Pointer {
                source_asset: key.0,
                file_id: i32::try_from(old_index)
                    .map_err(|_| "source AssetRef index overflow".to_string())?,
                path_id: 1,
            };
            if preserved_external_ref(&pointer, &inventory.assets, &by_internal_name).is_none() {
                continue;
            }
            let reference_key = asset_ref_key(reference);
            let file_id = if let Some(existing) = preserved_by_key.get(&reference_key) {
                *existing
            } else {
                let file_id = i32::try_from(output_refs.len())
                    .map_err(|_| "too many preserved AssetRefs".to_string())?;
                output_refs.push(reference.clone());
                preserved_by_key.insert(reference_key, file_id);
                file_id
            };
            preserved_file_ids.insert((key.0, pointer.file_id), file_id);
        }
    }
    for root in &plan.roots {
        if root_owner_part(root, plan) != Some(part_index) {
            continue;
        }
        for (reference, _) in &root.preserved_preloads {
            let key = asset_ref_key(reference);
            if preserved_by_key.contains_key(&key) {
                continue;
            }
            let file_id = i32::try_from(output_refs.len())
                .map_err(|_| "too many preserved preload AssetRefs".to_string())?;
            output_refs.push(reference.clone());
            preserved_by_key.insert(key, file_id);
        }
    }

    let template_meta = &inventory.assets[inventory.template.source_asset];
    let template_asset = Asset::from_path(&template_meta.path)?;
    let mut assetbundle = inventory.template.value.clone();
    let mut preloads = Vec::<UnityValue>::new();
    let mut containers = Vec::<UnityValue>::new();
    let mut main_asset = None::<(String, UnityValue)>;
    for root in &plan.roots {
        if root_owner_part(root, plan) != Some(part_index) {
            continue;
        }
        let Some(target) = plan.locations.get(&root.target) else {
            continue;
        };
        let mut entry = root.source_entry.clone();
        let preload_start = preloads.len();
        let mut preload_keys = vec![root.target];
        preload_keys.extend(root.preload_roots.iter().copied());
        preload_keys.sort();
        preload_keys.dedup();
        for key in preload_keys {
            let key = canonical_key(&plan.canonical, key);
            let location = plan
                .locations
                .get(&key)
                .ok_or_else(|| format!("preload {key:?} has no output location"))?;
            let file_id = if location.part == part_index {
                0
            } else {
                *external_file_ids
                    .get(&location.part)
                    .ok_or_else(|| format!("preload dependency part {} is absent", location.part))?
            };
            preloads.push(pointer_value(file_id, location.path_id));
        }
        for (reference, path_id) in &root.preserved_preloads {
            let file_id = *preserved_by_key
                .get(&asset_ref_key(reference))
                .ok_or_else(|| "preserved preload AssetRef was not copied".to_string())?;
            preloads.push(pointer_value(file_id, *path_id));
        }
        let preload_size = preloads.len().saturating_sub(preload_start);
        let target_file_id = if target.part == part_index {
            0
        } else {
            *external_file_ids.get(&target.part).ok_or_else(|| {
                format!("container target dependency part {} is absent", target.part)
            })?
        };
        let target_pointer = pointer_value(target_file_id, target.path_id);
        let metadata = pair_value_mut(&mut entry)
            .ok_or_else(|| format!("container '{}' has invalid pair metadata", root.path))?;
        set_i64(metadata, "preloadIndex", preload_start as i64);
        set_i64(metadata, "preloadSize", preload_size as i64);
        if let Some(object) = metadata.as_object_mut() {
            object.insert("asset".to_string(), target_pointer.clone());
        }
        if main_asset.is_none() {
            main_asset = Some((root.path.clone(), target_pointer));
        }
        containers.push(entry);
    }
    if let Some(object) = assetbundle.as_object_mut() {
        object.insert("m_PreloadTable".to_string(), UnityValue::Array(preloads));
        object.insert("m_Container".to_string(), UnityValue::Array(containers));
        if let Some(main) = object
            .get_mut("m_MainAsset")
            .and_then(UnityValue::as_object_mut)
        {
            if let Some((path, pointer)) = main_asset {
                main.insert("name".to_string(), UnityValue::String(path));
                main.insert("asset".to_string(), pointer);
            } else {
                main.insert("name".to_string(), UnityValue::String(String::new()));
                main.insert("asset".to_string(), pointer_value(0, 0));
            }
        }
    }
    set_string(&mut assetbundle, "m_Name", &part.internal_name);

    let mut output_asset = template_asset.clone();
    output_asset.name = part.internal_name.clone();
    output_asset.asset_refs = output_refs;
    output_asset.tree.type_trees.clear();
    let mut trees = BTreeMap::<i32, (String, TypeTree)>::new();
    let mut by_signature = BTreeMap::<(i32, String), i32>::new();
    let mut next_synthetic = -1_i32;
    let template_tree = template_asset.object_type_tree(&inventory.template.info)?;
    let assetbundle_type_id = assign_output_type(
        type_tree_key(&inventory.template.info),
        template_tree,
        &mut trees,
        &mut by_signature,
        &mut next_synthetic,
    );
    let mut assetbundle_info = inventory.template.info.clone();
    assetbundle_info.path_id = 1;
    assetbundle_info.type_id = assetbundle_type_id;
    let mut output_objects = vec![(
        assetbundle_info,
        template_asset.serialize_object_value(
            inventory.template.source_asset,
            &inventory.template.info,
            &assetbundle,
        )?,
    )];

    let mut loaded_index = None::<usize>;
    let mut loaded_asset = None::<Asset>;
    for key in &part.nodes {
        let meta = &inventory.assets[key.0];
        if loaded_index != Some(key.0) {
            loaded_asset = Some(Asset::from_path(&meta.path)?);
            loaded_index = Some(key.0);
        }
        let source = loaded_asset
            .as_ref()
            .ok_or_else(|| "source asset cache is empty".to_string())?;
        let info = source
            .objects
            .get(&key.1)
            .ok_or_else(|| format!("{}#{} disappeared", source.name, key.1))?;
        let mut value = strict_read_object(source, key.0, info)?;
        if let Some(recorded) = plan.dangling_object_pointers_to_clear.get(key) {
            clear_recorded_dangling_pointers(&mut value, recorded)
                .map_err(|err| format!("{}#{}: {err}", source.name, key.1))?;
        }
        rewrite_output_pointers(
            &mut value,
            key.0,
            part_index,
            inventory,
            &by_internal_name,
            plan,
            &external_file_ids,
            &preserved_file_ids,
        )?;
        let tree = source.object_type_tree(info)?;
        let output_type_id = assign_output_type(
            type_tree_key(info),
            tree,
            &mut trees,
            &mut by_signature,
            &mut next_synthetic,
        );
        let mut output_info = info.clone();
        output_info.path_id = plan.locations[key].path_id;
        output_info.type_id = output_type_id;
        output_objects.push((
            output_info,
            source.serialize_object_value(key.0, info, &value)?,
        ));
    }
    for (type_id, (_, tree)) in trees {
        output_asset.tree.type_trees.insert(type_id, tree);
    }
    output_objects.sort_by_key(|(info, _)| info.path_id);
    let serialized = output_asset.rebuild_from_object_data_as_format(&output_objects, Some(6))?;
    let serialized_bytes = serialized.len() as u64;
    validate_inner_serialized_size(part, serialized_bytes, max_part_bytes)?;
    let part_dir = staging_root.join(format!("part_{part_index:04}"));
    fs::create_dir_all(&part_dir).map_err(|err| format!("{}: {err}", part_dir.display()))?;
    let serialized_path = part_dir.join(&part.internal_name);
    fs::write(&serialized_path, serialized)
        .map_err(|err| format!("{}: {err}", serialized_path.display()))?;
    let parsed = Asset::from_path(&serialized_path)?;
    if parsed.objects.len() != part.nodes.len() + 1 {
        return Err(format!(
            "{} wrote {} objects, expected {}",
            part.output_name,
            parsed.objects.len(),
            part.nodes.len() + 1
        ));
    }
    let bundle_path = staging_root.join(&part.output_name);
    crate::pack_bundle_native_from_dir(&part_dir, &bundle_path)?;
    let bundle_bytes = fs::metadata(&bundle_path)
        .map_err(|err| format!("{}: {err}", bundle_path.display()))?
        .len();
    validate_physical_bundle_size(&part.output_name, bundle_bytes, max_part_bytes)?;
    Ok(bundle_path)
}

pub(super) fn retained_opaque_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "dll" | "pdb" | "mdb"
            )
        })
}

/// Direct PPtrs from main/Map/TableData/music are roots too. They are not represented by an
/// AssetBundle m_Container entry, so omitting them from mark-and-sweep can retire an object that
/// a retained consumer still needs. Exact Dong targets keep their tile affinity; all other such
/// objects are promoted to Core so they are available before retained consumers can use them.
pub(super) fn retained_repack_roots(
    out_dir: &Path,
    inventory: &Inventory,
    cfg: &LegacyConfig,
) -> Result<BTreeMap<NodeKey, Family>, String> {
    let by_internal_name = source_name_index(&inventory.assets);
    let extracted_root = crate::native_build_temp_dir("retained_root_scan")?;
    let mut roots = BTreeMap::<NodeKey, Family>::new();
    for (bundle_index, bundle_path) in retained_bundle_paths(out_dir)?.into_iter().enumerate() {
        let bundle_name = bundle_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let extracted = extracted_root
            .path()
            .join(format!("bundle_{bundle_index:04}"));
        crate::extract_bundle_native_to_dir(&bundle_path, &extracted)?;
        let mut files = fs::read_dir(&extracted)
            .map_err(|err| format!("{}: {err}", extracted.display()))?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            if retained_opaque_file(&path) {
                continue;
            }
            let Ok(source) = Asset::from_path(&path) else {
                continue;
            };
            if source.format > 7 {
                return Err(format!(
                    "retained {} internal asset {} uses unsupported format {}",
                    bundle_name, source.name, source.format
                ));
            }
            for info in source.objects.values() {
                let value = strict_read_object(&source, 0, info)?;
                let mut pointers = Vec::new();
                collect_pointers(&value, &mut pointers);
                for pointer in pointers {
                    if pointer.file_id == 0 {
                        continue;
                    }
                    let Some(reference) = usize::try_from(pointer.file_id)
                        .ok()
                        .and_then(|index| source.asset_refs.get(index))
                    else {
                        continue;
                    };
                    let Some(target) = source_asset_for_ref(
                        reference,
                        &inventory.assets,
                        &by_internal_name,
                        pointer.path_id,
                    )?
                    else {
                        let references_retired_asset =
                            [&reference.file_path, &reference.asset_path]
                                .into_iter()
                                .map(|value| internal_ref_name(value))
                                .filter(|value| !value.is_empty())
                                .any(|name| by_internal_name.contains_key(&name));
                        if references_retired_asset {
                            return Err(format!(
                                "retained {} {}#{} PPtr {}:{} names a retired serialized asset but its pathID is absent",
                                bundle_name,
                                source.name,
                                info.path_id,
                                pointer.file_id,
                                pointer.path_id
                            ));
                        }
                        continue;
                    };
                    let asset = &inventory.assets[target.0];
                    let source_bundle = &inventory.bundles[asset.bundle_index].name;
                    let family = dong_tile_from_bundle(source_bundle, &cfg.dong_prefix)
                        .map(Family::Dong)
                        .unwrap_or(Family::Core);
                    roots
                        .entry(target)
                        .and_modify(|existing| {
                            if *existing != family {
                                *existing = Family::Core;
                            }
                        })
                        .or_insert(family);
                }
            }
        }
    }
    Ok(roots)
}

pub(super) fn ensure_part_ref(
    output_refs: &mut Vec<AssetRef>,
    part_refs: &mut BTreeMap<usize, i32>,
    part: usize,
    plan: &Plan,
) -> Result<i32, String> {
    if let Some(file_id) = part_refs.get(&part) {
        return Ok(*file_id);
    }
    let file_id = i32::try_from(output_refs.len())
        .map_err(|_| "retained asset has too many external refs".to_string())?;
    output_refs.push(AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: plan.parts[part].internal_name.clone(),
    });
    part_refs.insert(part, file_id);
    Ok(file_id)
}

pub(super) fn rewrite_retained_pointers(
    value: &mut UnityValue,
    source: &Asset,
    inventory: &Inventory,
    by_internal_name: &BTreeMap<String, Vec<usize>>,
    plan: &Plan,
    output_refs: &mut Vec<AssetRef>,
    part_refs: &mut BTreeMap<usize, i32>,
) -> Result<bool, String> {
    let mut changed = false;
    match value {
        UnityValue::Array(items) => {
            for item in items {
                changed |= rewrite_retained_pointers(
                    item,
                    source,
                    inventory,
                    by_internal_name,
                    plan,
                    output_refs,
                    part_refs,
                )?;
            }
        }
        UnityValue::Object(fields) => {
            for item in fields.values_mut() {
                changed |= rewrite_retained_pointers(
                    item,
                    source,
                    inventory,
                    by_internal_name,
                    plan,
                    output_refs,
                    part_refs,
                )?;
            }
        }
        UnityValue::Pair(left, right) => {
            changed |= rewrite_retained_pointers(
                left,
                source,
                inventory,
                by_internal_name,
                plan,
                output_refs,
                part_refs,
            )?;
            changed |= rewrite_retained_pointers(
                right,
                source,
                inventory,
                by_internal_name,
                plan,
                output_refs,
                part_refs,
            )?;
        }
        UnityValue::Pointer(pointer) if pointer.file_id != 0 && !pointer.is_null() => {
            let Some(reference) = usize::try_from(pointer.file_id)
                .ok()
                .and_then(|index| source.asset_refs.get(index))
            else {
                return Ok(false);
            };
            let Some(target) = source_asset_for_ref(
                reference,
                &inventory.assets,
                by_internal_name,
                pointer.path_id,
            )?
            else {
                let references_retired_asset = [&reference.file_path, &reference.asset_path]
                    .into_iter()
                    .map(|value| internal_ref_name(value))
                    .filter(|value| !value.is_empty())
                    .any(|name| by_internal_name.contains_key(&name));
                if references_retired_asset {
                    return Err(format!(
                        "retained {} PPtr {}:{} names a retired serialized asset but its pathID is absent",
                        source.name, pointer.file_id, pointer.path_id
                    ));
                }
                // A name outside the repack inventory is a genuine system/retained
                // dependency (for example `library/unity default resources`). Keeping
                // its original AssetRef and fileID is the only correct representation.
                return Ok(false);
            };
            let target = canonical_key(&plan.canonical, target);
            let location = plan
                .locations
                .get(&target)
                .ok_or_else(|| format!("retained PPtr target {target:?} has no output location"))?;
            let target_internal = &plan.parts[location.part].internal_name;
            let already_targets_output = [&reference.file_path, &reference.asset_path]
                .into_iter()
                .map(|value| internal_ref_name(value))
                .any(|name| name.eq_ignore_ascii_case(target_internal))
                && path_id_candidates(pointer.path_id)
                    .into_iter()
                    .any(|candidate| candidate == location.path_id);
            if already_targets_output {
                // Exact-name Dong parts retain their original internal asset name/pathID,
                // so the common Map -> Dong references can stay byte-for-byte unchanged.
                return Ok(false);
            }
            pointer.file_id = ensure_part_ref(output_refs, part_refs, location.part, plan)?;
            pointer.path_id = location.path_id;
            pointer.source_asset = 0;
            changed = true;
        }
        _ => {}
    }
    Ok(changed)
}

/// Rebuild retained main/Map/TableData/music bundles in staging when they directly point at
/// an object whose original serialized asset will disappear. Opaque DLL files and untouched
/// serialized assets are copied byte-for-byte.
pub(super) fn build_retained_consumers(
    out_dir: &Path,
    inventory: &Inventory,
    plan: &Plan,
    staging_root: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    let by_internal_name = source_name_index(&inventory.assets);
    let consumers_root = staging_root.join("retained");
    fs::create_dir_all(&consumers_root)
        .map_err(|err| format!("{}: {err}", consumers_root.display()))?;
    let mut outputs = Vec::new();
    for (bundle_index, bundle_path) in retained_bundle_paths(out_dir)?.into_iter().enumerate() {
        let bundle_name = bundle_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        let extracted = consumers_root.join(format!("source_{bundle_index:04}"));
        crate::extract_bundle_native_to_dir(&bundle_path, &extracted)?;
        let mut changed_bundle = false;
        let mut files = fs::read_dir(&extracted)
            .map_err(|err| format!("{}: {err}", extracted.display()))?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            if retained_opaque_file(&path) {
                continue;
            }
            let Ok(source) = Asset::from_path(&path) else {
                continue;
            };
            if source.format > 7 {
                return Err(format!(
                    "retained {} internal asset {} uses unsupported format {}",
                    bundle_name, source.name, source.format
                ));
            }
            let mut output_asset = source.clone();
            let mut output_refs = source.asset_refs.clone();
            let mut part_refs = BTreeMap::<usize, i32>::new();
            let mut replacements = BTreeMap::<i64, Vec<u8>>::new();
            for (path_id, info) in &source.objects {
                let mut value = strict_read_object(&source, 0, info)?;
                if !rewrite_retained_pointers(
                    &mut value,
                    &source,
                    inventory,
                    &by_internal_name,
                    plan,
                    &mut output_refs,
                    &mut part_refs,
                )? {
                    continue;
                }
                replacements.insert(*path_id, source.serialize_object_value(0, info, &value)?);
            }
            if replacements.is_empty() {
                continue;
            }
            output_asset.asset_refs = output_refs;
            let rebuilt = output_asset.rebuild_with_object_data(&replacements)?;
            fs::write(&path, rebuilt).map_err(|err| format!("{}: {err}", path.display()))?;
            Asset::from_path(&path)?;
            changed_bundle = true;
        }
        if changed_bundle {
            let staged = consumers_root.join(format!("output_{bundle_index:04}_{bundle_name}"));
            crate::pack_bundle_native_from_dir(&extracted, &staged)?;
            outputs.push((bundle_name, staged));
        }
        fs::remove_dir_all(&extracted).map_err(|err| format!("{}: {err}", extracted.display()))?;
    }
    Ok(outputs)
}
