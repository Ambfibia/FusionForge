use super::*;

/// Hash the parsed Unity value rather than its serialized representation. Legacy format 6/7
/// files may encode the same value with different padding and PPtr widths, and output PPtr IDs
/// are deliberately rewritten by the packer. Pointer graph equivalence is validated separately.
pub(super) fn update_semantic_hash(hasher: &mut Sha1, value: &UnityValue) {
    match value {
        UnityValue::Bool(value) => {
            hasher.update(b"B");
            hasher.update([u8::from(*value)]);
        }
        UnityValue::Int(value) => {
            hasher.update(b"I");
            hasher.update(value.to_le_bytes());
        }
        UnityValue::UInt(value) => {
            hasher.update(b"U");
            hasher.update(value.to_le_bytes());
        }
        UnityValue::Float(value) => {
            hasher.update(b"F");
            hasher.update(value.to_bits().to_le_bytes());
        }
        UnityValue::String(value) => {
            hasher.update(b"S");
            hash_len(hasher, value.len());
            hasher.update(value.as_bytes());
        }
        UnityValue::Bytes(value) => {
            hasher.update(b"Y");
            hash_len(hasher, value.len());
            hasher.update(value);
        }
        UnityValue::Array(values) => {
            hasher.update(b"A");
            hash_len(hasher, values.len());
            for value in values {
                update_semantic_hash(hasher, value);
            }
        }
        UnityValue::Object(fields) => {
            hasher.update(b"O");
            hash_len(hasher, fields.len());
            for (name, value) in fields {
                hash_len(hasher, name.len());
                hasher.update(name.as_bytes());
                update_semantic_hash(hasher, value);
            }
        }
        UnityValue::Pair(left, right) => {
            hasher.update(b"R");
            update_semantic_hash(hasher, left);
            update_semantic_hash(hasher, right);
        }
        UnityValue::Pointer(_) => {
            // Keep the field/type position, but deliberately ignore local source/output IDs.
            hasher.update(b"PPtr");
        }
    }
}

pub(super) fn update_sha256_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

pub(crate) fn apply(
    project: &Path,
    out_dir: &Path,
    patch_config: &JsonValue,
    patched_audio_identities: &[PatchedAudioIdentity],
    stable_input_digest: &str,
) -> Result<LegacyLayoutPlan, String> {
    let cfg = config(patch_config);
    if !cfg.enabled {
        return Ok(disabled_plan());
    }
    let cache_key = result_cache_key(patch_config, stable_input_digest).ok();
    if let Some(plan) = restore_result_cache(project, out_dir, patch_config, stable_input_digest)? {
        return Ok(plan);
    }
    eprintln!("[ffclient:layout] cache miss; building semantic layout");
    let semantic_index = SemanticIndex::build(out_dir)?;
    let semantic_counts = semantic_index.counts();
    eprintln!(
        "[ffclient:layout] TableData semantic index: {} routes, {} cross-family",
        semantic_counts.routes, semantic_counts.cross_family_routes
    );
    eprintln!("[ffclient:layout] indexing legacy resource graph");
    let inventory = inventory(out_dir, &cfg, &semantic_index)?;
    let retained_roots = retained_repack_roots(out_dir, &inventory, &cfg)?;
    eprintln!(
        "[ffclient:layout] retaining {} direct main/Map/TableData/music target(s)",
        retained_roots.len()
    );
    let patched_audio = patched_audio_nodes(&inventory, patched_audio_identities)?;
    eprintln!(
        "[ffclient:layout] planning {} objects / {} roots / {} patched audio entries",
        inventory.nodes.len(),
        inventory.roots.len(),
        patched_audio.len()
    );
    let plan = build_plan(&inventory, &cfg, &patched_audio, &retained_roots)?;
    for key in patched_audio.keys().copied() {
        let canonical = canonical_key(&plan.canonical, key);
        if !plan.locations.contains_key(&canonical) {
            return Err(format!(
                "patched AudioClip {key:?} (canonical {canonical:?}) became unreachable during layout planning"
            ));
        }
    }
    let dependencies = dependency_parts(&plan, &inventory);
    let order = dependency_order(&plan.parts, &dependencies)?;
    let sections_by_part = plan
        .parts
        .iter()
        .map(|part| part.sections.clone())
        .collect::<Vec<_>>();
    for (index, part) in plan.parts.iter().enumerate() {
        let route_count = plan
            .roots
            .iter()
            .filter(|root| root.output_part == Some(index))
            .count();
        if route_count > 0
            && part.sections.is_empty()
            && !matches!(part.family, Family::Dong(_) | Family::Compat(_))
        {
            return Err(format!(
                "routeable semantic bundle {} owns {route_count} public route(s) but has no CachingManifest phase",
                part.output_name
            ));
        }
    }
    // Reject runtime-impossible lifecycle layouts before staging or publishing any files.
    // A managed part may retain a lazy PPtr to another managed phase, but it can never depend
    // on a dynamically tile-scoped Dong.
    for index in order.iter().copied() {
        let consumer_sections = &sections_by_part[index];
        if !matches!(plan.parts[index].family, Family::Dong(_))
            && consumer_sections.iter().any(|section| {
                matches!(
                    section.as_str(),
                    "m_FreeZoneComplete" | "m_PaidZoneComplete"
                )
            })
        {
            return Err(format!(
                "semantic bundle {} remained in a Dong-only Complete phase",
                plan.parts[index].output_name
            ));
        }
        for dependency in dependencies.get(&index).into_iter().flatten() {
            if matches!(plan.parts[*dependency].family, Family::Dong(_))
                && !consumer_sections.is_empty()
            {
                return Err(format!(
                    "managed bundle {} depends on tile-scoped {}; refusing an impossible load order",
                    plan.parts[index].output_name, plan.parts[*dependency].output_name
                ));
            }
            let required_dependency_sections =
                runtime_sections_for_family(&plan.parts[*dependency].family, consumer_sections);
            if !required_dependency_sections.is_subset(&sections_by_part[*dependency]) {
                let missing = required_dependency_sections
                    .difference(&sections_by_part[*dependency])
                    .cloned()
                    .collect::<Vec<_>>();
                return Err(format!(
                    "bundle {} depends on {} but dependency is unavailable in runtime phase(s): {}",
                    plan.parts[index].output_name,
                    plan.parts[*dependency].output_name,
                    missing.join(", ")
                ));
            }
        }
    }
    let staging = crate::native_build_temp_dir("legacy_layout_outputs")?;
    let mut staged_parts = vec![PathBuf::new(); plan.parts.len()];
    for part_index in 0..plan.parts.len() {
        eprintln!(
            "[ffclient:layout] building part {}/{} {} ({} objects)",
            part_index + 1,
            plan.parts.len(),
            plan.parts[part_index].output_name,
            plan.parts[part_index].nodes.len()
        );
        staged_parts[part_index] = build_part(
            part_index,
            &inventory,
            &plan,
            &dependencies,
            staging.path(),
            cfg.max_part_bytes,
        )?;
    }
    eprintln!("[ffclient:layout] validating staged parts");
    let translated_audio_pairs_preserved =
        validate_staged_outputs(&staged_parts, &inventory, &plan, &patched_audio)?;
    let retained = build_retained_consumers(out_dir, &inventory, &plan, staging.path())?;
    eprintln!(
        "[ffclient:layout] validating {} rewritten retained bundle(s)",
        retained.len()
    );
    validate_retained_consumers(&retained, &plan)?;

    let mut staged = plan
        .parts
        .iter()
        .enumerate()
        .map(|(index, part)| (part.output_name.clone(), staged_parts[index].clone()))
        .collect::<Vec<_>>();
    staged.extend(retained.iter().cloned());
    transactional_publish(out_dir, &staged)?;
    eprintln!(
        "[ffclient:layout] published {} output bundle(s)",
        staged.len()
    );

    let output_names = plan
        .parts
        .iter()
        .map(|part| part.output_name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let retired_bundles = inventory
        .bundles
        .iter()
        .filter(|bundle| !output_names.contains(&bundle.name.to_ascii_lowercase()))
        .map(|bundle| bundle.name.clone())
        .collect::<Vec<_>>();
    let output_bundles = order
        .iter()
        .map(|index| plan.parts[*index].output_name.clone())
        .collect::<Vec<_>>();
    let routeable_bundles = order
        .iter()
        .filter(|index| {
            !matches!(
                plan.parts[**index].family,
                Family::Dong(_) | Family::Compat(_)
            )
        })
        .map(|index| plan.parts[*index].output_name.clone())
        .collect::<Vec<_>>();
    let manifest_sections = order
        .iter()
        .filter_map(|index| {
            let sections = sections_by_part[*index].iter().cloned().collect::<Vec<_>>();
            (!sections.is_empty()).then(|| LegacyRoutedBundle {
                name: plan.parts[*index].output_name.clone(),
                sections,
            })
        })
        .collect::<Vec<_>>();
    let dependency_records = order
        .iter()
        .map(|index| LegacyBundleDependency {
            bundle: plan.parts[*index].output_name.clone(),
            depends_on: dependencies
                .get(index)
                .into_iter()
                .flatten()
                .map(|dependency| plan.parts[*dependency].output_name.clone())
                .collect(),
        })
        .collect::<Vec<_>>();
    let rewritten_retained_bundles = retained
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let layout_parts = plan
        .parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let source_bundles = part
                .nodes
                .iter()
                .filter_map(|key| inventory.assets.get(key.0))
                .filter_map(|asset| inventory.bundles.get(asset.bundle_index))
                .map(|bundle| bundle.name.clone())
                .collect::<BTreeSet<_>>();
            let depends_on = dependencies
                .get(&index)
                .into_iter()
                .flatten()
                .map(|dependency| plan.parts[*dependency].output_name.clone())
                .collect::<Vec<_>>();
            serde_json::json!({
                "name": part.output_name,
                "internalAsset": part.internal_name,
                "family": family_label(&part.family),
                "ordinal": part.ordinal + 1,
                "objectCount": part.nodes.len() + 1,
                "containerRouteCount": plan.roots.iter().filter(|root| root_owner_part(root, &plan) == Some(index)).count(),
                "estimatedSerializedBytes": part.estimated_bytes,
                "bundleBytes": fs::metadata(&staged_parts[index]).map(|meta| meta.len()).unwrap_or(0),
                "sections": sections_by_part[index],
                "dependsOn": depends_on,
                "sourceBundles": source_bundles
            })
        })
        .collect::<Vec<_>>();
    let source_bundles = inventory
        .bundles
        .iter()
        .enumerate()
        .map(|(bundle_index, bundle)| {
            serde_json::json!({
                "name": bundle.name,
                "bytes": bundle.size,
                "serializedAssets": inventory.assets.iter().filter(|asset| asset.bundle_index == bundle_index).count(),
                "objects": inventory.assets.iter().filter(|asset| asset.bundle_index == bundle_index).map(|asset| asset.objects.len()).sum::<usize>(),
                "containerRoutes": inventory.roots.iter().filter(|root| inventory.assets[root.source_asset].bundle_index == bundle_index).count()
            })
        })
        .collect::<Vec<_>>();
    let duplicates = duplicate_analysis(&inventory);
    let dangling_object_pointers_cleared = plan
        .dangling_object_pointers_to_clear
        .values()
        .map(Vec::len)
        .sum::<usize>();
    let dangling_object_pointer_examples = plan
        .dangling_object_pointers_to_clear
        .values()
        .flatten()
        .take(64)
        .map(|pointer| pointer.description.clone())
        .collect::<Vec<_>>();
    let result = LegacyLayoutPlan {
        output_bundles,
        routeable_bundles,
        retired_bundles,
        manifest_sections,
        rewritten_retained_bundles,
        dependencies: dependency_records,
        source_objects: inventory.nodes.len() + inventory.unreadable_orphans.len(),
        output_objects: plan.locations.len() + plan.parts.len(),
        exact_objects_deduplicated: plan.exact_deduplicated,
        same_name_different_content: plan.same_name_different_content,
        unresolved_pointers_preserved: inventory.unresolved.len(),
        unreadable_orphans_removed: inventory.unreadable_orphans.clone(),
        dangling_preloads_removed: inventory.dangling_preloads_removed,
        dangling_preload_examples: inventory.dangling_preload_examples.clone(),
        dangling_object_pointers_cleared,
        dangling_object_pointer_examples,
        translated_audio_pairs_preserved,
        patched_audio_entries_preserved: patched_audio.len(),
        tile_scoped_audio_exceptions: plan.tile_scoped_audio_exceptions,
        tile_scoped_route_exceptions: plan.tile_scoped_route_exceptions,
        route_conflicts: plan.route_conflicts,
        unclassified_roots: plan.unclassified_roots,
        oversized_parts: plan.oversized_parts,
    };
    let layout_config = patch_config.get("BundleLayout").unwrap_or(&JsonValue::Null);
    let configured_report = json_string(layout_config, "Report", "bundle-layout-report.json");
    let report_path = if Path::new(&configured_report).is_absolute() {
        PathBuf::from(configured_report)
    } else {
        project.join(configured_report)
    };
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let report_data = serde_json::to_string_pretty(&serde_json::json!({
        "format": "ffclient.legacy-bundle-layout.v3",
        "policy": {
            "maxPartBytes": cfg.max_part_bytes,
            "partSizeEnforcement": "MaxPartBytes is always enforced on the final physical .resourceFile. Non-Dong inner serialized assets and atomic SCC estimates must also fit. Exact atomic DongResources_* may exceed the conservative estimate/inner-asset limit because runtime naming forbids splitting; its final physical .resourceFile must still fit.",
            "dedupeIdentity": ["objectType", "caseFoldedName", "semanticUnityValueSha1"],
            "semanticContentHash": "parsed Unity values with exact scalar/string/blob metadata and fixed PPtr markers; independent of format 6/7 padding and rewritten local PPtr IDs; graph edges are validated separately",
            "differentContentSameName": "keep distinct objects; choose only the legacy public route winner; preserve direct PPtrs",
            "patchedAudio": "replacement OGG SHA1 is verified in patched source and again at the exact staged output location; exact name and full semantic AudioClip value must survive; distinct translated Dong variants stay tile-scoped",
            "unreadableLegacyObjects": "remove only when no container, preload, or readable PPtr references the object; otherwise fail closed",
            "danglingLegacyPreloads": "omit missing preload-only entries and rebuild preloads from resolved graph; missing container targets and unproven ordinary PPtrs remain fatal",
            "danglingObjectPointers": "clear only exact PPtrs whose local/known-internal target was absent or whose fileID was already outside the source AssetRef table; valid unknown external refs are preserved and ambiguous refs remain fatal; pointers in unreachable objects disappear with those objects",
            "retainedHardcodedBundles": ["TableData.resourceFile", "FutureMusic.resourceFile", "LobbyMusic.resourceFile", "PastMusic.resourceFile", "RetroMusic.resourceFile"],
            "retirement": "deferred until managed loader and manifest patch succeeds"
        },
        "result": &result,
        "semanticIndex": semantic_counts,
        "sourceBundles": source_bundles,
        "parts": layout_parts,
        "duplicates": duplicates
    }))
    .map_err(|err| err.to_string())?;
    let report_pending = report_path.with_extension("json.fflayout-new");
    fs::write(&report_pending, format!("{report_data}\n"))
        .map_err(|err| format!("{}: {err}", report_pending.display()))?;
    if report_path.is_file() {
        fs::remove_file(&report_path).map_err(|err| format!("{}: {err}", report_path.display()))?;
    }
    fs::rename(&report_pending, &report_path).map_err(|err| {
        format!(
            "{} -> {}: {err}",
            report_pending.display(),
            report_path.display()
        )
    })?;
    if let Some(key) = cache_key.as_deref() {
        if let Err(err) = store_layout_cache(project, key, &result, &report_data, &staged) {
            // Publishing and validation already succeeded. Cache failure must not turn a
            // valid client build into a failed one.
            eprintln!("[ffclient:layout] result cache write skipped: {err}");
        }
    }
    Ok(result)
}
