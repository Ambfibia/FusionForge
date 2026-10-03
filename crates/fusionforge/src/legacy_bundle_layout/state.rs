use super::*;

pub(super) struct Inventory {
    pub(super) _temp: crate::NativeBuildTempDir,
    pub(super) bundles: Vec<SourceBundle>,
    pub(super) assets: Vec<SourceAsset>,
    pub(super) nodes: BTreeMap<NodeKey, Node>,
    pub(super) roots: Vec<Root>,
    pub(super) template: AssetBundleTemplate,
    pub(super) unresolved: Vec<String>,
    pub(super) unreadable_orphans: Vec<String>,
    pub(super) dangling_preloads_removed: usize,
    pub(super) dangling_preload_examples: Vec<String>,
    pub(super) dangling_object_pointers: BTreeMap<NodeKey, Vec<DanglingObjectPointer>>,
}

pub(super) fn inventory(
    out_dir: &Path,
    cfg: &LegacyConfig,
    semantics: &SemanticIndex,
) -> Result<Inventory, String> {
    let manifest_sections = source_caching_manifest_sections(out_dir)?;
    let bundles = discover_resource_bundles(out_dir, &manifest_sections)?;
    if bundles.is_empty() {
        return Err(format!(
            "{} contains no legacy resourceFile bundles",
            out_dir.display()
        ));
    }
    let temp = crate::native_build_temp_dir("legacy_bundle_layout")?;
    let mut assets = Vec::<SourceAsset>::new();
    for (bundle_index, bundle) in bundles.iter().enumerate() {
        if bundle_index % 16 == 0 || bundle_index + 1 == bundles.len() {
            eprintln!(
                "[ffclient:layout] inventory {}/{} {}",
                bundle_index + 1,
                bundles.len(),
                bundle.name
            );
        }
        let dir = temp.path().join(format!("source_{bundle_index:04}"));
        crate::extract_bundle_native_to_dir(&bundle.path, &dir)?;
        let mut files = fs::read_dir(&dir)
            .map_err(|err| format!("{}: {err}", dir.display()))?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            let asset = Asset::from_path(&path).map_err(|err| {
                format!(
                    "{} in {} is not a supported serialized asset; opaque bundle files must be preserved explicitly: {err}",
                    path.display(), bundle.name
                )
            })?;
            if asset.format > 7 {
                return Err(format!(
                    "{} in {} uses serialized format {}; legacy writer supports <= 7",
                    asset.name, bundle.name, asset.format
                ));
            }
            assets.push(SourceAsset {
                bundle_index,
                path,
                name: asset.name.clone(),
                format: asset.format,
                tree: asset.tree.clone(),
                objects: asset.objects.clone(),
                refs: asset.asset_refs.clone(),
            });
        }
    }

    let mut by_internal_name = BTreeMap::<String, Vec<usize>>::new();
    for (asset_index, asset) in assets.iter().enumerate() {
        by_internal_name
            .entry(asset.name.to_ascii_lowercase())
            .or_default()
            .push(asset_index);
    }
    let duplicate_names = by_internal_name
        .iter()
        .filter(|(_, indices)| indices.len() > 1)
        .map(|(name, indices)| format!("{name}: {indices:?}"))
        .collect::<Vec<_>>();
    if !duplicate_names.is_empty() {
        return Err(format!(
            "duplicate internal serialized asset names make external refs ambiguous: {}",
            duplicate_names.join("; ")
        ));
    }

    let mut nodes = BTreeMap::<NodeKey, Node>::new();
    let mut roots = Vec::<Root>::new();
    let mut template = None::<AssetBundleTemplate>;
    let mut unresolved = Vec::<String>::new();
    let mut unreadable = BTreeMap::<NodeKey, String>::new();
    let mut dangling_preloads_removed = 0usize;
    let mut dangling_preload_examples = Vec::<String>::new();
    let mut dangling_object_pointers = BTreeMap::<NodeKey, Vec<DanglingObjectPointer>>::new();
    for (asset_index, meta) in assets.iter().enumerate() {
        let asset = Asset::from_path(&meta.path)?;
        for (path_id, info) in &asset.objects {
            let object_type = asset.object_type_name(info);
            let value = match strict_read_object(&asset, asset_index, info) {
                Ok(value) => value,
                Err(err) if object_type != "AssetBundle" => {
                    unreadable.insert((asset_index, *path_id), err);
                    continue;
                }
                Err(err) => return Err(err),
            };
            if object_type == "AssetBundle" {
                if template.is_none() {
                    template = Some(AssetBundleTemplate {
                        source_asset: asset_index,
                        path_id: *path_id,
                        value: value.clone(),
                        info: info.clone(),
                    });
                }
                let preloads = value_array(value.get("m_PreloadTable")).to_vec();
                for entry in value_array(value.get("m_Container")) {
                    let Some((path, metadata)) = pair_name_value(entry) else {
                        continue;
                    };
                    let Some(pointer) = metadata.get("asset").and_then(UnityValue::as_pointer)
                    else {
                        unresolved.push(format!(
                            "{}#{} container '{path}' has no asset PPtr",
                            asset.name, path_id
                        ));
                        continue;
                    };
                    let Some(target) = resolve_source_pointer(pointer, &assets, &by_internal_name)?
                    else {
                        unresolved.push(format!(
                            "{}#{} container '{path}' target {}:{} is unresolved",
                            asset.name, path_id, pointer.file_id, pointer.path_id
                        ));
                        continue;
                    };
                    let (start, end) = preload_range(metadata, preloads.len());
                    let mut preload_roots = Vec::new();
                    let mut preserved_preloads = Vec::new();
                    for preload in &preloads[start..end] {
                        let Some(pointer) = preload.as_pointer() else {
                            continue;
                        };
                        match resolve_source_pointer(pointer, &assets, &by_internal_name)? {
                            Some(key) => preload_roots.push(key),
                            None => {
                                if let Some(reference) =
                                    preserved_external_ref(pointer, &assets, &by_internal_name)
                                {
                                    preserved_preloads.push((reference, pointer.path_id));
                                } else {
                                    dangling_preloads_removed += 1;
                                    if dangling_preload_examples.len() < 64 {
                                        dangling_preload_examples.push(format!(
                                            "{}#{} container '{path}' preload {}:{} is dangling",
                                            asset.name, path_id, pointer.file_id, pointer.path_id
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    let (family, semantic_owner) = classify_root_with_semantics(
                        path,
                        &bundles[meta.bundle_index].name,
                        cfg,
                        semantics,
                    );
                    roots.push(Root {
                        path: path.to_string(),
                        normalized_path: normalized_path(path),
                        target,
                        preload_roots,
                        preserved_preloads,
                        source_asset: asset_index,
                        source_assetbundle: *path_id,
                        source_entry: entry.clone(),
                        family,
                        semantic_owner,
                        sections: bundles[meta.bundle_index].sections.clone(),
                        output_part: None,
                    });
                }
                continue;
            }

            let mut pointers = Vec::new();
            collect_pointers(&value, &mut pointers);
            let mut edges = Vec::<NodeKey>::new();
            for pointer in pointers {
                match resolve_source_pointer(&pointer, &assets, &by_internal_name)? {
                    Some(key) => edges.push(key),
                    None => {
                        if preserved_external_ref(&pointer, &assets, &by_internal_name).is_some() {
                            continue;
                        }
                        let description = format!(
                            "{}#{} {} '{}' PPtr {}:{} is dangling",
                            asset.name,
                            path_id,
                            object_type,
                            object_name(&value),
                            pointer.file_id,
                            pointer.path_id
                        );
                        if proven_missing_source_target(&pointer, &assets, &by_internal_name)? {
                            dangling_object_pointers
                                .entry((asset_index, *path_id))
                                .or_default()
                                .push(DanglingObjectPointer {
                                    source: SourcePointerKey::from(&pointer),
                                    description,
                                });
                        } else {
                            unresolved.push(format!(
                                "{description}; target absence cannot be proven from source metadata"
                            ));
                        }
                    }
                }
            }
            edges.sort();
            edges.dedup();
            let mut shape = value.clone();
            zero_pointers(&mut shape);
            let shape_hash = sha1_hex(&asset.serialize_object_value(asset_index, info, &shape)?);
            let type_tree_hash = tree_signature(asset.object_type_tree(info)?);
            let name = object_name(&value);
            let audio_identity = if object_type == "AudioClip" {
                Some(audio_object_identity(&value).map_err(|err| {
                    format!("{}#{} AudioClip '{}': {err}", asset.name, path_id, name)
                })?)
            } else {
                None
            };
            let semantic_hash = EXACT_LEAF_TYPES.contains(&object_type.as_str()).then(|| {
                audio_identity
                    .as_ref()
                    .map(|identity| identity.semantic_sha1.clone())
                    .unwrap_or_else(|| semantic_value_sha1(&object_type, &value))
            });
            let audio_payload_sha1 = audio_identity
                .as_ref()
                .map(|identity| identity.payload_sha1.clone());
            nodes.insert(
                (asset_index, *path_id),
                Node {
                    key: (asset_index, *path_id),
                    object_type,
                    name,
                    raw_hash: sha1_hex(asset.object_raw_data(info)?),
                    semantic_hash,
                    audio_payload_sha1,
                    shape_hash,
                    type_tree_hash,
                    size: u64::from(info.size),
                    edges,
                },
            );
        }
    }
    let template = template.ok_or_else(|| "no AssetBundle object was found".to_string())?;
    if roots.is_empty() {
        return Err("legacy resource bundles contain no AssetBundle container roots".to_string());
    }
    let mut referenced = roots
        .iter()
        .flat_map(|root| std::iter::once(root.target).chain(root.preload_roots.iter().copied()))
        .collect::<BTreeSet<_>>();
    referenced.extend(nodes.values().flat_map(|node| node.edges.iter().copied()));
    let reachable_unreadable = referenced_unreadable_errors(&unreadable, &referenced);
    if !reachable_unreadable.is_empty() {
        return Err(format!(
            "{} unreadable legacy object(s) are still referenced and cannot be removed safely: {}",
            reachable_unreadable.len(),
            reachable_unreadable.join("; ")
        ));
    }
    let unreadable_orphans = unreadable.into_values().collect::<Vec<_>>();
    for object in &unreadable_orphans {
        eprintln!("[ffclient:layout] removing unreachable corrupt orphan: {object}");
    }
    if !unresolved.is_empty() {
        let shown = unresolved.iter().take(32).cloned().collect::<Vec<_>>();
        return Err(format!(
            "legacy layout found {} unresolved PPtr/container references; refusing destructive repack:\n{}",
            unresolved.len(),
            shown.join("\n")
        ));
    }
    Ok(Inventory {
        _temp: temp,
        bundles,
        assets,
        nodes,
        roots,
        template,
        unresolved,
        unreadable_orphans,
        dangling_preloads_removed,
        dangling_preload_examples,
        dangling_object_pointers,
    })
}

/// `DownloadDongs` only starts the coordinate-derived DongResources/Map pair. Arbitrary
/// semantic packs listed in a Complete section would therefore wait forever. Once content is
/// moved out of an exact Dong it must be available in the corresponding base world phase.
pub(super) fn runtime_sections_for_family(
    family: &Family,
    source_sections: &BTreeSet<String>,
) -> BTreeSet<String> {
    if matches!(family, Family::Dong(_)) {
        return source_sections.clone();
    }
    source_sections
        .iter()
        .map(|section| match section.as_str() {
            "m_FreeZoneComplete" => "m_FreeZone".to_string(),
            "m_PaidZoneComplete" => "m_PaidZone".to_string(),
            _ => section.clone(),
        })
        .collect()
}

/// Character creation and character selection are independent entry paths. Wearable/player
/// routes are requested by string (mesh and texture are loaded separately), so a PPtr-only
/// dependency closure cannot infer the missing companion phase. HNPC uses the same player
/// model vocabulary and follows the same rule, while its world phases remain untouched.
pub(super) fn runtime_sections_for_root(
    family: &Family,
    source_sections: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut sections = runtime_sections_for_family(family, source_sections);
    if matches!(family, Family::Hnpc | Family::Player | Family::Items)
        && (sections.contains("m_CharacterCreation") || sections.contains("m_CharacterSelection"))
    {
        sections.insert("m_CharacterCreation".to_string());
        sections.insert("m_CharacterSelection".to_string());
    }
    sections
}

/// Resolve the actual output family of every container before owner IDs and packing units are
/// created. Runtime phase normalization must not change semantic ownership: the family drives
/// output naming, while the section set independently drives when that output is loaded.
pub(super) fn normalize_root_runtime_classes(
    roots: &mut [Root],
    cfg: &LegacyConfig,
) -> Vec<(BTreeSet<String>, bool)> {
    roots
        .iter_mut()
        .map(|root| {
            let sections_pinned = !root.sections.is_empty();
            let requested_sections = if sections_pinned {
                root.sections.clone()
            } else {
                custom_root_sections(&root.family, cfg)
            };
            let sections = runtime_sections_for_root(&root.family, &requested_sections);
            root.family = lifecycle_safe_family(root.family.clone(), &sections);
            (sections, sections_pinned)
        })
        .collect()
}
