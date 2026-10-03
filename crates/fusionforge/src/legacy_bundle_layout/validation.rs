use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LegacyRouteConflict {
    pub path: String,
    pub scope: String,
    pub winner_bundle: String,
    pub winner_hash: String,
    pub shadowed_bundle: String,
    pub shadowed_hash: String,
    pub object_type: String,
    pub object_name: String,
    pub reason: String,
}

pub(super) fn reject_oversized_atomic_units(units: &[PackingUnit], max_part_bytes: u64) -> Result<(), String> {
    for (index, unit) in units.iter().enumerate() {
        let estimated_part_bytes = PART_METADATA_RESERVE_BYTES.saturating_add(unit.estimated_bytes);
        if estimated_part_bytes <= max_part_bytes {
            continue;
        }
        // DongResources_<tile> is a literal runtime lookup contract and cannot be split.  Its
        // inner serialized asset is packed/compressed into the served .resourceFile, so only
        // the latter is the user-visible part-size limit.  Keep the conservative estimate as
        // a diagnostic; build_part still rejects an oversized physical bundle.
        if matches!(unit.family, Family::Dong(_)) {
            eprintln!(
                "[ffclient:layout] exact {} atomic estimate is {} bytes ({} payload + {} reserve), above MaxPartBytes {}; preserving the exact Dong name and enforcing the limit on the final physical .resourceFile",
                family_label(&unit.family),
                estimated_part_bytes,
                unit.estimated_bytes,
                PART_METADATA_RESERVE_BYTES,
                max_part_bytes
            );
            continue;
        }
        let kind = if !unit.nodes.is_empty() {
            "atomic object SCC"
        } else if !unit.roots.is_empty() {
            "atomic synthetic root unit"
        } else {
            "atomic compatibility unit"
        };
        return Err(format!(
            "{kind} {index} [{}] requires an estimated {} bytes ({} payload + {} fixed metadata/type-tree reserve), exceeding MaxPartBytes {}; it cannot be split safely",
            family_label(&unit.family),
            estimated_part_bytes,
            unit.estimated_bytes,
            PART_METADATA_RESERVE_BYTES,
            max_part_bytes
        ));
    }
    Ok(())
}

pub(super) fn validate_inner_serialized_size(
    part: &Part,
    serialized_bytes: u64,
    max_part_bytes: u64,
) -> Result<(), String> {
    if serialized_bytes <= max_part_bytes {
        return Ok(());
    }
    if matches!(part.family, Family::Dong(_)) {
        eprintln!(
            "[ffclient:layout] {} inner serialized asset '{}' is {} bytes, above MaxPartBytes {}; exact DongResources runtime naming requires one atomic asset, so the final physical .resourceFile remains the enforced limit",
            part.output_name, part.internal_name, serialized_bytes, max_part_bytes
        );
        return Ok(());
    }
    Err(format!(
        "{}: inner serialized asset '{}' is {} bytes, exceeding MaxPartBytes {}",
        part.output_name, part.internal_name, serialized_bytes, max_part_bytes
    ))
}

pub(super) fn validate_physical_bundle_size(
    output_name: &str,
    bundle_bytes: u64,
    max_part_bytes: u64,
) -> Result<(), String> {
    if bundle_bytes <= max_part_bytes {
        return Ok(());
    }
    Err(format!(
        "{}: final .resourceFile is {} bytes, exceeding MaxPartBytes {}",
        output_name, bundle_bytes, max_part_bytes
    ))
}

pub(super) fn validate_staged_outputs(
    staged_parts: &[PathBuf],
    inventory: &Inventory,
    plan: &Plan,
    patched_audio: &BTreeMap<NodeKey, String>,
) -> Result<usize, String> {
    if staged_parts.len() != plan.parts.len() {
        return Err(format!(
            "staged part census mismatch: expected {}, received {}",
            plan.parts.len(),
            staged_parts.len()
        ));
    }

    let validation = crate::native_build_temp_dir("legacy_layout_validation")?;
    let mut extracted_assets = Vec::<PathBuf>::with_capacity(staged_parts.len());
    let mut output_objects = BTreeMap::<String, BTreeSet<i64>>::new();
    let mut output_audio = BTreeSet::<(String, String)>::new();
    let mut output_audio_objects = BTreeMap::<(String, i64), AudioObjectIdentity>::new();
    let mut actual_objects = 0_usize;

    // Pass 1: hold one serialized asset at a time. Build the small cross-part lookup used by
    // pass 2 and collect content summaries before dropping the parsed asset.
    for (part_index, bundle) in staged_parts.iter().enumerate() {
        let part = &plan.parts[part_index];
        let dir = validation.path().join(format!("part_{part_index:04}"));
        let (path, asset) = extract_single_staged_asset(bundle, &dir, &part.output_name)?;
        let internal_name = asset.name.to_ascii_lowercase();
        if internal_name != part.internal_name.to_ascii_lowercase() {
            return Err(format!(
                "{} serialized asset is named '{}', expected '{}'",
                part.output_name, asset.name, part.internal_name
            ));
        }
        let expected_part_objects = part.nodes.len() + 1;
        if asset.objects.len() != expected_part_objects {
            return Err(format!(
                "{} object census mismatch: expected {}, parsed {}",
                part.output_name,
                expected_part_objects,
                asset.objects.len()
            ));
        }
        actual_objects = actual_objects.saturating_add(asset.objects.len());
        let path_ids = asset.objects.keys().copied().collect::<BTreeSet<_>>();
        if output_objects
            .insert(internal_name.clone(), path_ids)
            .is_some()
        {
            return Err(format!(
                "duplicate staged serialized asset name '{}'",
                asset.name
            ));
        }
        let mut expected_types = BTreeMap::<i64, String>::new();
        expected_types.insert(1, "AssetBundle".to_string());
        for key in &part.nodes {
            let location = plan.locations.get(key).ok_or_else(|| {
                format!(
                    "{} node {key:?} has no planned output location",
                    part.output_name
                )
            })?;
            if location.part != part_index {
                return Err(format!(
                    "{} node {key:?} points to output part {}, expected {part_index}",
                    part.output_name, location.part
                ));
            }
            let object_type = inventory
                .nodes
                .get(key)
                .ok_or_else(|| format!("{} node {key:?} is absent", part.output_name))?
                .object_type
                .clone();
            if expected_types
                .insert(location.path_id, object_type)
                .is_some()
            {
                return Err(format!(
                    "{} has duplicate planned pathId {}",
                    part.output_name, location.path_id
                ));
            }
        }
        for info in asset.objects.values() {
            let actual_type = asset.object_type_name(info);
            let expected_type = expected_types.get(&info.path_id).ok_or_else(|| {
                format!(
                    "{} has unplanned object #{} of type {}",
                    part.output_name, info.path_id, actual_type
                )
            })?;
            if &actual_type != expected_type {
                return Err(format!(
                    "{}#{} changed Unity type during repack: expected {}, wrote {}",
                    part.output_name, info.path_id, expected_type, actual_type
                ));
            }
            if actual_type != "AudioClip" {
                continue;
            }
            let value = strict_read_object(&asset, part_index, info)?;
            let identity = audio_object_identity(&value).map_err(|err| {
                format!(
                    "{}#{} AudioClip '{}': {err}",
                    asset.name,
                    info.path_id,
                    object_name(&value)
                )
            })?;
            output_audio.insert((identity.name.clone(), identity.semantic_sha1.clone()));
            output_audio_objects.insert((internal_name.clone(), info.path_id), identity);
        }
        extracted_assets.push(path);
    }
    let expected_objects = plan
        .parts
        .iter()
        .map(|part| part.nodes.len() + 1)
        .sum::<usize>();
    if actual_objects != expected_objects {
        return Err(format!(
            "output object census mismatch: expected {expected_objects}, parsed {actual_objects}"
        ));
    }

    let mut actual_paths = BTreeSet::<(Family, String)>::new();
    let mut actual_audio_routes = BTreeSet::<(Family, String, String, String)>::new();
    let mut actual_route_targets = BTreeSet::<(Family, String, String, i64)>::new();

    // Pass 2: reopen and drop one serialized asset per iteration. All cross-part PPtrs can now
    // be checked against the compact name/pathID index without a multi-gigabyte environment.
    for (part_index, path) in extracted_assets.iter().enumerate() {
        let part = &plan.parts[part_index];
        let asset = Asset::from_path(path)?;
        for info in asset.objects.values() {
            let value = strict_read_object(&asset, part_index, info)?;
            let mut pointers = Vec::new();
            collect_pointers(&value, &mut pointers);
            for pointer in pointers {
                resolve_staged_pointer(&asset, &pointer, &output_objects).map_err(|err| {
                    format!(
                        "{}#{} {}: {err}",
                        asset.name,
                        info.path_id,
                        asset.object_type_name(info)
                    )
                })?;
            }
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            for entry in value_array(value.get("m_Container")) {
                let Some((path, metadata)) = pair_name_value(entry) else {
                    continue;
                };
                let route = normalized_path(path);
                actual_paths.insert((part.family.clone(), route.clone()));
                let Some(pointer) = metadata.get("asset").and_then(UnityValue::as_pointer) else {
                    return Err(format!(
                        "{} AssetBundle container '{}' has no asset PPtr",
                        part.output_name, path
                    ));
                };
                let target =
                    resolve_staged_pointer(&asset, pointer, &output_objects)?.ok_or_else(|| {
                        format!(
                            "{} AssetBundle container '{}' targets a preserved system AssetRef",
                            part.output_name, path
                        )
                    })?;
                actual_route_targets.insert((
                    part.family.clone(),
                    route.clone(),
                    target.0.clone(),
                    target.1,
                ));
                if let Some(identity) = output_audio_objects.get(&target) {
                    actual_audio_routes.insert((
                        part.family.clone(),
                        route,
                        identity.name.clone(),
                        identity.semantic_sha1.clone(),
                    ));
                }
            }
        }
    }

    let expected_paths = plan
        .roots
        .iter()
        .map(|root| (root.family.clone(), root.normalized_path.clone()))
        .collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        let missing = expected_paths
            .difference(&actual_paths)
            .take(32)
            .collect::<Vec<_>>();
        let extra = actual_paths
            .difference(&expected_paths)
            .take(32)
            .collect::<Vec<_>>();
        return Err(format!(
            "output container census mismatch; missing={missing:?}, extra={extra:?}"
        ));
    }

    let mut expected_route_targets = BTreeSet::<(Family, String, String, i64)>::new();
    for root in &plan.roots {
        let target = canonical_key(&plan.canonical, root.target);
        let location = plan.locations.get(&target).ok_or_else(|| {
            format!(
                "container '{}' target {target:?} has no reachable output location",
                root.path
            )
        })?;
        expected_route_targets.insert((
            root.family.clone(),
            root.normalized_path.clone(),
            plan.parts[location.part].internal_name.to_ascii_lowercase(),
            location.path_id,
        ));
    }
    if actual_route_targets != expected_route_targets {
        let missing = expected_route_targets
            .difference(&actual_route_targets)
            .take(32)
            .collect::<Vec<_>>();
        let extra = actual_route_targets
            .difference(&expected_route_targets)
            .take(32)
            .collect::<Vec<_>>();
        return Err(format!(
            "output container target mismatch; missing={missing:?}, extra={extra:?}"
        ));
    }

    // Locations are the mark-and-sweep result. Inventory also contains unreachable legacy
    // objects, which must not be expected in the rewritten bundles.
    let mut expected_audio = BTreeSet::<(String, String)>::new();
    for (key, location) in &plan.locations {
        let node = inventory
            .nodes
            .get(key)
            .ok_or_else(|| format!("output location {key:?} has no inventory node"))?;
        if node.object_type == "AudioClip" {
            let expected = AudioObjectIdentity {
                name: node.name.clone(),
                semantic_sha1: node
                    .semantic_hash
                    .clone()
                    .ok_or_else(|| format!("source AudioClip {key:?} has no semantic hash"))?,
                payload_sha1: node
                    .audio_payload_sha1
                    .clone()
                    .ok_or_else(|| format!("source AudioClip {key:?} has no payload hash"))?,
            };
            let output_key = (
                plan.parts[location.part].internal_name.to_ascii_lowercase(),
                location.path_id,
            );
            let actual = output_audio_objects.get(&output_key).ok_or_else(|| {
                format!(
                    "source AudioClip {key:?} '{}' is absent at staged output {:?}",
                    node.name, output_key
                )
            })?;
            if actual != &expected {
                return Err(format!(
                    "AudioClip {key:?} '{}' changed during repack at {:?}: expected={expected:?}, actual={actual:?}",
                    node.name, output_key
                ));
            }
            expected_audio.insert((expected.name, expected.semantic_sha1));
        }
    }
    if output_audio != expected_audio {
        let missing = expected_audio
            .difference(&output_audio)
            .take(32)
            .collect::<Vec<_>>();
        let extra = output_audio
            .difference(&expected_audio)
            .take(32)
            .collect::<Vec<_>>();
        return Err(format!(
            "AudioClip semantic verification failed: expected {} unique exact-name+content pairs, wrote {}; missing={missing:?}, extra={extra:?}",
            expected_audio.len(),
            output_audio.len()
        ));
    }

    for (source_key, replacement_sha1) in patched_audio {
        let canonical = canonical_key(&plan.canonical, *source_key);
        let location = plan.locations.get(&canonical).ok_or_else(|| {
            format!(
                "patched AudioClip {source_key:?} canonical target {canonical:?} has no output location"
            )
        })?;
        let output_key = (
            plan.parts[location.part].internal_name.to_ascii_lowercase(),
            location.path_id,
        );
        let actual = output_audio_objects.get(&output_key).ok_or_else(|| {
            format!(
                "patched AudioClip {source_key:?} is absent from staged output {:?}",
                output_key
            )
        })?;
        if &actual.payload_sha1 != replacement_sha1 {
            return Err(format!(
                "patched AudioClip {source_key:?} payload changed at {:?}: expected={}, wrote={}",
                output_key, replacement_sha1, actual.payload_sha1
            ));
        }
    }

    let mut expected_audio_routes = BTreeSet::<(Family, String, String, String)>::new();
    for root in &plan.roots {
        let target = canonical_key(&plan.canonical, root.target);
        if !plan.locations.contains_key(&target) {
            return Err(format!(
                "container '{}' target {target:?} has no reachable output location",
                root.path
            ));
        }
        let node = inventory
            .nodes
            .get(&target)
            .ok_or_else(|| format!("container '{}' target {target:?} is absent", root.path))?;
        if node.object_type == "AudioClip" {
            expected_audio_routes.insert((
                root.family.clone(),
                root.normalized_path.clone(),
                node.name.clone(),
                node.semantic_hash.clone().ok_or_else(|| {
                    format!(
                        "container '{}' AudioClip target has no semantic hash",
                        root.path
                    )
                })?,
            ));
        }
    }
    if actual_audio_routes != expected_audio_routes {
        let missing = expected_audio_routes
            .difference(&actual_audio_routes)
            .take(32)
            .collect::<Vec<_>>();
        let extra = actual_audio_routes
            .difference(&expected_audio_routes)
            .take(32)
            .collect::<Vec<_>>();
        return Err(format!(
            "AudioClip route verification failed: expected {} path+exact-name+semantic-content routes, wrote {}; missing={missing:?}, extra={extra:?}",
            expected_audio_routes.len(),
            actual_audio_routes.len()
        ));
    }
    Ok(output_audio.len())
}

pub(super) fn validate_retained_consumers(retained: &[(String, PathBuf)], plan: &Plan) -> Result<(), String> {
    let validation = crate::native_build_temp_dir("retained_layout_validation")?;
    let output_paths = plan
        .parts
        .iter()
        .enumerate()
        .map(|(part_index, part)| {
            let mut ids = part
                .nodes
                .iter()
                .filter_map(|key| plan.locations.get(key).map(|location| location.path_id))
                .collect::<BTreeSet<_>>();
            ids.insert(1);
            (part.internal_name.to_ascii_lowercase(), (part_index, ids))
        })
        .collect::<BTreeMap<_, _>>();
    for (bundle_index, (name, path)) in retained.iter().enumerate() {
        let dir = validation
            .path()
            .join(format!("consumer_{bundle_index:04}"));
        crate::extract_bundle_native_to_dir(path, &dir)?;
        for entry in fs::read_dir(&dir)
            .map_err(|err| format!("{}: {err}", dir.display()))?
            .flatten()
        {
            let Ok(asset) = Asset::from_path(&entry.path()) else {
                continue;
            };
            for info in asset.objects.values() {
                let value = strict_read_object(&asset, 0, info)?;
                let mut pointers = Vec::new();
                collect_pointers(&value, &mut pointers);
                for pointer in pointers {
                    if pointer.file_id == 0 {
                        // Local pointers were not changed by the retained-consumer rewrite;
                        // preserve the baseline (including known legacy dangling pointers).
                        continue;
                    }
                    let reference = usize::try_from(pointer.file_id)
                        .ok()
                        .and_then(|index| asset.asset_refs.get(index))
                        .ok_or_else(|| {
                            format!(
                                "retained {name} {}#{} has invalid fileID {}",
                                asset.name, info.path_id, pointer.file_id
                            )
                        })?;
                    let output = [
                        internal_ref_name(&reference.file_path),
                        internal_ref_name(&reference.asset_path),
                    ]
                    .into_iter()
                    .find_map(|internal| output_paths.get(&internal));
                    if let Some((_, path_ids)) = output {
                        if !path_id_candidates(pointer.path_id)
                            .into_iter()
                            .any(|candidate| path_ids.contains(&candidate))
                        {
                            return Err(format!(
                                "retained {name} {}#{} points to absent repacked pathID {}",
                                asset.name, info.path_id, pointer.path_id
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_inputs(out_dir: &Path, patch_config: &JsonValue) -> Result<String, String> {
    let cfg = config(patch_config);
    if !cfg.enabled {
        return Err("BundleLayout.Enabled is false".to_string());
    }
    let semantic_index = SemanticIndex::build(out_dir)?;
    let semantic_counts = semantic_index.counts();
    let inventory = inventory(out_dir, &cfg, &semantic_index)?;
    let retained_roots = retained_repack_roots(out_dir, &inventory, &cfg)?;
    let plan = build_plan(&inventory, &cfg, &BTreeMap::new(), &retained_roots)?;
    let dependencies = dependency_parts(&plan, &inventory);
    dependency_order(&plan.parts, &dependencies)?;
    let dangling_object_pointer_examples = plan
        .dangling_object_pointers_to_clear
        .values()
        .flatten()
        .take(64)
        .map(|pointer| pointer.description.clone())
        .collect::<Vec<_>>();
    let dangling_object_pointers_cleared = plan
        .dangling_object_pointers_to_clear
        .values()
        .map(Vec::len)
        .sum::<usize>();
    serde_json::to_string_pretty(&serde_json::json!({
        "format": "ffclient.legacy-bundle-input-preflight.v2",
        "sourceBundles": inventory.bundles.len(),
        "serializedAssets": inventory.assets.len(),
        "sourceObjects": inventory.nodes.len() + inventory.unreadable_orphans.len(),
        "containerRoutes": inventory.roots.len(),
        "retainedDirectRoots": retained_roots.len(),
        "plannedParts": plan.parts.len(),
        "semanticIndex": semantic_counts,
        "unreadableOrphansRemoved": inventory.unreadable_orphans,
        "danglingPreloadsRemoved": inventory.dangling_preloads_removed,
        "danglingPreloadExamples": inventory.dangling_preload_examples,
        "danglingObjectPointersCleared": dangling_object_pointers_cleared,
        "danglingObjectPointerExamples": dangling_object_pointer_examples,
        "unresolvedReferences": inventory.unresolved
    }))
    .map_err(|err| err.to_string())
}
