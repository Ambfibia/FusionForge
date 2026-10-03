use super::*;

pub(super) fn build_plan(
    inventory: &Inventory,
    cfg: &LegacyConfig,
    patched_audio: &BTreeMap<NodeKey, String>,
    retained_roots: &BTreeMap<NodeKey, Family>,
) -> Result<Plan, String> {
    let mut canonical = inventory
        .nodes
        .keys()
        .copied()
        .map(|key| (key, key))
        .collect::<BTreeMap<_, _>>();
    let mut exact_seen = BTreeMap::<(String, String, String), NodeKey>::new();
    let mut exact_deduplicated = 0usize;
    let mut same_name_hashes = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for (key, node) in &inventory.nodes {
        if !node.name.is_empty() {
            same_name_hashes
                .entry((node.object_type.clone(), node.name.to_ascii_lowercase()))
                .or_default()
                .insert(
                    node.semantic_hash
                        .clone()
                        .unwrap_or_else(|| node.raw_hash.clone()),
                );
        }
        if node.name.is_empty()
            || !node.edges.is_empty()
            || !EXACT_LEAF_TYPES.contains(&node.object_type.as_str())
        {
            continue;
        }
        let semantic_hash = node.semantic_hash.clone().ok_or_else(|| {
            format!(
                "exact leaf {} {:?} has no canonical semantic hash",
                node.object_type, key
            )
        })?;
        let identity = (
            node.object_type.clone(),
            node.name.to_ascii_lowercase(),
            semantic_hash,
        );
        if let Some(existing) = exact_seen.get(&identity) {
            canonical.insert(*key, *existing);
            exact_deduplicated += 1;
        } else {
            exact_seen.insert(identity, *key);
        }
    }
    let same_name_different_content = same_name_hashes
        .values()
        .filter(|hashes| hashes.len() > 1)
        .count();

    let mut patched_canonical = BTreeMap::<NodeKey, String>::new();
    for (key, replacement_sha1) in patched_audio {
        let key = canonical_key(&canonical, *key);
        if let Some(existing) = patched_canonical.insert(key, replacement_sha1.clone()) {
            if existing != *replacement_sha1 {
                return Err(format!(
                    "one canonical AudioClip {key:?} was patched with two different replacements"
                ));
            }
        }
    }
    let mut patched_route_hashes = BTreeMap::<String, BTreeSet<String>>::new();
    for root in &inventory.roots {
        if let Some(hash) = patched_audio.get(&root.target) {
            patched_route_hashes
                .entry(root.normalized_path.clone())
                .or_default()
                .insert(hash.clone());
        }
    }
    let tile_scoped_audio_exceptions = patched_route_hashes
        .iter()
        .filter(|(_, hashes)| hashes.len() > 1)
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    let mut tile_scoped_route_exceptions = tile_scoped_audio_exceptions.clone();
    let mut roots_by_unscoped_path = BTreeMap::<String, Vec<&Root>>::new();
    for root in &inventory.roots {
        roots_by_unscoped_path
            .entry(root.normalized_path.clone())
            .or_default()
            .push(root);
    }
    for (path, candidates) in roots_by_unscoped_path {
        let dong_candidates = candidates
            .iter()
            .copied()
            .filter(|root| {
                dong_tile_from_bundle(source_bundle_name(inventory, root), &cfg.dong_prefix)
                    .is_some()
            })
            .collect::<Vec<_>>();
        let has_non_dong_owner = candidates.iter().any(|root| {
            dong_tile_from_bundle(source_bundle_name(inventory, root), &cfg.dong_prefix).is_none()
        });
        let distinct_dong_targets = dong_candidates
            .iter()
            .map(|root| canonical_key(&canonical, root.target))
            .collect::<BTreeSet<_>>();
        if !has_non_dong_owner && dong_candidates.len() > 1 && distinct_dong_targets.len() > 1 {
            // CheckSurroundingArea intentionally resolves the current tile first. A
            // same-path/different-content declaration that exists only in Dongs is not a
            // global duplicate and must retain that scope even when semantic classification
            // would otherwise move it to a shared pack.
            tile_scoped_route_exceptions.insert(path);
        }
    }

    // A public path has one legacy runtime winner. Translated audio is authoritative; when
    // two distinct translations intentionally exist in different Dongs, retain the route in
    // each tile so CheckSurroundingArea keeps the contextual variant.
    let mut roots_by_path = BTreeMap::<(String, Option<String>), Root>::new();
    let mut route_conflicts = Vec::<LegacyRouteConflict>::new();
    for source in &inventory.roots {
        let mut root = source.clone();
        let source_bundle = source_bundle_name(inventory, &root).to_string();
        if tile_scoped_route_exceptions.contains(&root.normalized_path) {
            if let Some(tile) = dong_tile_from_bundle(&source_bundle, &cfg.dong_prefix) {
                root.family = Family::Dong(tile);
            } else {
                // A global alias would shadow the tile-specific route before the Dong lookup.
                // Its object remains available when referenced directly, but it must not own
                // the public path.
                continue;
            }
        }
        root.target = canonical_key(&canonical, root.target);
        root.preload_roots = root
            .preload_roots
            .iter()
            .copied()
            .map(|key| canonical_key(&canonical, key))
            .collect();
        // The same public path in different Dong tiles is not a conflict: the original
        // CheckSurroundingArea lookup deliberately selects the currently loaded tile.
        let scope = root_scope(&root.family);
        let scoped_path = (root.normalized_path.clone(), scope);
        if let Some(existing) = roots_by_path.get_mut(&scoped_path) {
            if existing.target != root.target {
                let left = inventory.nodes.get(&existing.target).ok_or_else(|| {
                    format!(
                        "container '{}' target {:?} is absent",
                        root.path, existing.target
                    )
                })?;
                let right = inventory.nodes.get(&root.target).ok_or_else(|| {
                    format!(
                        "container '{}' target {:?} is absent",
                        root.path, root.target
                    )
                })?;
                let existing_bundle = source_bundle_name(inventory, existing).to_string();
                let existing_patched = patched_canonical.contains_key(&existing.target);
                let incoming_patched = patched_canonical.contains_key(&root.target);
                if existing_patched && incoming_patched {
                    return Err(format!(
                        "public route '{}' has two different non-tile translated variants: {} and {}",
                        root.path, existing_bundle, source_bundle
                    ));
                }
                let incoming_wins = if incoming_patched != existing_patched {
                    incoming_patched
                } else {
                    legacy_route_rank(&root.path, &source_bundle)
                        < legacy_route_rank(&existing.path, &existing_bundle)
                };
                let reason = if incoming_patched || existing_patched {
                    "patched-audio-override"
                } else {
                    "legacy-resource-locator-priority"
                };
                let (winner_bundle, winner_hash, shadowed_bundle, shadowed_hash) = if incoming_wins
                {
                    (
                        source_bundle.clone(),
                        right.raw_hash.clone(),
                        existing_bundle.clone(),
                        left.raw_hash.clone(),
                    )
                } else {
                    (
                        existing_bundle.clone(),
                        left.raw_hash.clone(),
                        source_bundle.clone(),
                        right.raw_hash.clone(),
                    )
                };
                route_conflicts.push(LegacyRouteConflict {
                    path: root.normalized_path.clone(),
                    scope: scoped_path
                        .1
                        .clone()
                        .unwrap_or_else(|| "global".to_string()),
                    winner_bundle,
                    winner_hash,
                    shadowed_bundle,
                    shadowed_hash,
                    object_type: if incoming_wins {
                        right.object_type.clone()
                    } else {
                        left.object_type.clone()
                    },
                    object_name: if incoming_wins {
                        right.name.clone()
                    } else {
                        left.name.clone()
                    },
                    reason: reason.to_string(),
                });
                if left.object_type == "AudioClip"
                    && right.object_type == "AudioClip"
                    && (incoming_patched || existing_patched)
                {
                    let (loser, winner) = if incoming_wins {
                        (existing.target, root.target)
                    } else {
                        (root.target, existing.target)
                    };
                    canonical.insert(loser, winner);
                    exact_deduplicated += 1;
                }
                if incoming_wins {
                    root.sections.extend(existing.sections.iter().cloned());
                    *existing = root;
                } else {
                    existing.sections.extend(root.sections.iter().cloned());
                }
                continue;
            }
            // Equal targets are a true duplicate declaration, but the public container
            // owner still follows the literal legacy ResourceLocator priority. Sending
            // every cross-bundle duplicate to Core would recreate the old junk drawer.
            let existing_bundle = source_bundle_name(inventory, existing).to_string();
            if legacy_route_rank(&root.path, &source_bundle)
                < legacy_route_rank(&existing.path, &existing_bundle)
            {
                let mut winner = root;
                merge_root_preloads(&mut winner, existing);
                *existing = winner;
            } else {
                merge_root_preloads(existing, &root);
            }
            continue;
        }
        roots_by_path.insert(scoped_path, root);
    }
    let mut roots = roots_by_path.into_values().collect::<Vec<_>>();
    let canonical_snapshot = canonical.clone();
    for key in canonical.keys().copied().collect::<Vec<_>>() {
        canonical.insert(key, canonical_key(&canonical_snapshot, key));
    }
    for root in &mut roots {
        root.target = canonical_key(&canonical, root.target);
        root.preload_roots = root
            .preload_roots
            .iter()
            .copied()
            .map(|key| canonical_key(&canonical, key))
            .collect();
        root.preload_roots.sort();
        root.preload_roots.dedup();
    }

    // Preserve the exact source lifecycle on object graphs as well as on container roots.
    // Exact-leaf deduplication can collapse identical objects from several legacy bundles,
    // so the canonical object owns the union of those source phase sets.
    let mut canonical_sections = BTreeMap::<NodeKey, BTreeSet<String>>::new();
    for key in inventory.nodes.keys().copied() {
        let canonical_key = canonical_key(&canonical, key);
        let Some(source_bundle) = inventory
            .assets
            .get(key.0)
            .and_then(|asset| inventory.bundles.get(asset.bundle_index))
        else {
            continue;
        };
        canonical_sections
            .entry(canonical_key)
            .or_default()
            .extend(source_bundle.sections.iter().cloned());
    }

    // First resolve path-ambiguous roots from their graph consumers. The second compact
    // propagation is also the mark phase, so unreachable shadow/orphan objects are not
    // copied into CoreShared merely because they existed in an old serialized file.
    let (initial_usage, initial_tiles) =
        propagate_compact_usage(&roots, inventory, &canonical, retained_roots);
    let mut unclassified_roots = Vec::new();
    for root in &mut roots {
        if root.family != Family::Auto {
            continue;
        }
        let state = initial_usage
            .get(&canonical_key(&canonical, root.target))
            .copied()
            .unwrap_or_default();
        if state.mask == 0 && state.dong.is_none() && !state.many_dongs {
            unclassified_roots.push(root.normalized_path.clone());
        }
        root.family = resolve_compact_family(state, &initial_tiles);
        if root.family == Family::Auto {
            root.family = Family::Core;
        }
    }
    if roots.iter().any(|root| root.family == Family::Auto) {
        return Err(
            "unresolved automatic root family remained after graph propagation".to_string(),
        );
    }
    unclassified_roots.sort();
    unclassified_roots.dedup();
    let (usage, tile_names) =
        propagate_compact_usage(&roots, inventory, &canonical, retained_roots);
    let canonical_nodes = usage
        .iter()
        .filter_map(|(key, state)| state.reachable.then_some(*key))
        .collect::<BTreeSet<_>>();
    let mut units = Vec::<PackingUnit>::new();
    let mut node_units = BTreeMap::<NodeKey, usize>::new();
    // Object cycles remain indivisible. Unlike the old first-fit pass, these SCC units are
    // subsequently emitted in one global dependency-first order, so unrelated SCCs cannot
    // be folded into two mutually-dependent parts.
    for component in strongly_connected_components(&canonical_nodes, &inventory.nodes, &canonical) {
        let mut component_usage = CompactUsage::default();
        for key in &component {
            if let Some(node_usage) = usage.get(key) {
                merge_usage(&mut component_usage, *node_usage);
            }
        }
        let family = resolve_compact_family(component_usage, &tile_names);
        let estimated_bytes = component
            .iter()
            .filter_map(|key| inventory.nodes.get(key))
            .map(|node| node.size)
            .sum::<u64>();
        let source_sections = component
            .iter()
            .filter_map(|key| canonical_sections.get(key))
            .flatten()
            .cloned()
            .collect::<BTreeSet<_>>();
        let sections = runtime_sections_for_family(&family, &source_sections);
        let family = lifecycle_safe_family(family, &sections);
        let sections_pinned = !sections.is_empty();
        let index = units.len();
        for key in &component {
            node_units.insert(*key, index);
        }
        units.push(PackingUnit {
            family,
            sections,
            sections_pinned,
            nodes: component,
            roots: Vec::new(),
            estimated_bytes,
            owner: component_usage.owner,
            dependencies: Vec::new(),
        });
    }
    for index in 0..units.len() {
        let mut dependencies = units[index]
            .nodes
            .iter()
            .filter_map(|key| inventory.nodes.get(key))
            .flat_map(|node| node.edges.iter().copied())
            .map(|edge| canonical_key(&canonical, edge))
            .filter_map(|edge| node_units.get(&edge).copied())
            .filter(|dependency| *dependency != index)
            .collect::<Vec<_>>();
        dependencies.sort_unstable();
        dependencies.dedup();
        units[index].dependencies = dependencies;
    }
    // AssetBundle containers are graph vertices too. Giving every route an explicit
    // zero-byte unit removes root_owner_part's former arbitrary "first family part" edge.
    let root_runtime_classes = normalize_root_runtime_classes(&mut roots, cfg);
    let owner_ids = logical_owner_ids(&roots);
    for (root_index, root) in roots.iter().enumerate() {
        let mut dependencies = std::iter::once(root.target)
            .chain(root.preload_roots.iter().copied())
            .map(|key| canonical_key(&canonical, key))
            .filter_map(|key| node_units.get(&key).copied())
            .collect::<Vec<_>>();
        dependencies.sort_unstable();
        dependencies.dedup();
        let (sections, sections_pinned) = &root_runtime_classes[root_index];
        units.push(PackingUnit {
            family: root.family.clone(),
            sections: sections.clone(),
            sections_pinned: *sections_pinned,
            nodes: Vec::new(),
            roots: vec![root_index],
            estimated_bytes: synthetic_root_estimated_bytes(root),
            owner: root_compact_owner(root, &owner_ids),
            dependencies,
        });
    }
    // The client hardcodes these download names. They remain tiny AssetBundle shells;
    // all actual content roots above are owned by the new semantic families.
    for name in COMPAT_BUNDLE_NAMES {
        let mut unit = PackingUnit::empty(Family::Compat(name.to_string()));
        if let Some(source) = inventory
            .bundles
            .iter()
            .find(|bundle| bundle.name.eq_ignore_ascii_case(name))
        {
            unit.sections = runtime_sections_for_family(&unit.family, &source.sections);
            unit.sections_pinned = !unit.sections.is_empty();
        }
        units.push(unit);
    }

    propagate_dependency_sections(&mut units);
    let literal_dong_sections = literal_dong_source_sections(inventory, cfg)?;
    let units = contract_exact_dong_units(units, &literal_dong_sections)?;
    let packed_units = pack_dependency_units(&units, cfg.max_part_bytes)?;
    let family_counts =
        packed_units
            .iter()
            .fold(BTreeMap::<Family, usize>::new(), |mut counts, part| {
                *counts.entry(part.family.clone()).or_default() += 1;
                counts
            });
    let mut parts = Vec::<Part>::new();
    let oversized_parts = Vec::new();
    let mut family_ordinals = BTreeMap::<Family, usize>::new();
    for packed in packed_units {
        let family = packed.family;
        let sections = packed.sections;
        let ordinal = *family_ordinals.entry(family.clone()).or_default();
        *family_ordinals.entry(family.clone()).or_default() += 1;
        let count = family_counts[&family];
        if matches!(family, Family::Dong(_)) && count != 1 {
            return Err(format!(
                "exact {} was split into {count} parts",
                family_label(&family)
            ));
        }
        let mut keys = packed
            .units
            .iter()
            .flat_map(|unit| units[*unit].nodes.iter().copied())
            .collect::<Vec<_>>();
        keys.sort();
        keys.dedup();
        let output_name = output_part_name(&family, ordinal, count, cfg);
        let internal_name = if matches!(family, Family::Dong(_)) {
            let source_assets = keys.iter().map(|key| key.0).collect::<BTreeSet<_>>();
            source_assets
                .iter()
                .next()
                .filter(|_| source_assets.len() == 1)
                .and_then(|asset_index| inventory.assets.get(*asset_index))
                .map(|asset| asset.name.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| output_internal_name(&output_name))
        } else {
            output_internal_name(&output_name)
        };
        let part_index = parts.len();
        for root_index in packed
            .units
            .iter()
            .flat_map(|unit| units[*unit].roots.iter().copied())
        {
            roots[root_index].output_part = Some(part_index);
        }
        parts.push(Part {
            family,
            sections,
            ordinal,
            nodes: keys,
            estimated_bytes: packed.estimated_bytes,
            internal_name,
            output_name,
        });
    }
    for (index, root) in roots.iter().enumerate() {
        let part_index = root.output_part.ok_or_else(|| {
            format!(
                "container root {index} '{}' has no explicit output part",
                root.path
            )
        })?;
        let part = parts.get(part_index).ok_or_else(|| {
            format!(
                "container root {index} '{}' output part {part_index} is out of bounds",
                root.path
            )
        })?;
        if part.family != root.family {
            return Err(format!(
                "container root {index} '{}' output family {} does not match root family {}",
                root.path,
                family_label(&part.family),
                family_label(&root.family)
            ));
        }
    }

    let mut locations = BTreeMap::<NodeKey, OutputLocation>::new();
    for (part_index, part) in parts.iter().enumerate() {
        let preserve_source_ids = matches!(part.family, Family::Dong(_))
            && part
                .nodes
                .first()
                .and_then(|key| inventory.assets.get(key.0))
                .is_some_and(|asset| {
                    part.nodes.iter().all(|key| key.0 == part.nodes[0].0)
                        && asset.name.eq_ignore_ascii_case(&part.internal_name)
                });
        let mut used_path_ids = BTreeSet::from([1i64]);
        let mut next_path_id = 2i64;
        for key in &part.nodes {
            let path_id =
                if preserve_source_ids && key.1 != 0 && key.1 != 1 && used_path_ids.insert(key.1) {
                    key.1
                } else {
                    while used_path_ids.contains(&next_path_id) {
                        next_path_id = next_path_id
                            .checked_add(1)
                            .ok_or_else(|| "part pathID overflow".to_string())?;
                    }
                    let assigned = next_path_id;
                    used_path_ids.insert(assigned);
                    next_path_id = next_path_id
                        .checked_add(1)
                        .ok_or_else(|| "part pathID overflow".to_string())?;
                    assigned
                };
            locations.insert(
                *key,
                OutputLocation {
                    part: part_index,
                    path_id,
                },
            );
        }
    }
    let dangling_object_pointers_to_clear = inventory
        .dangling_object_pointers
        .iter()
        .filter(|(key, _)| locations.contains_key(key))
        .map(|(key, pointers)| (*key, pointers.clone()))
        .collect::<BTreeMap<_, _>>();
    Ok(Plan {
        canonical,
        parts,
        locations,
        roots,
        exact_deduplicated,
        same_name_different_content,
        route_conflicts,
        tile_scoped_audio_exceptions: tile_scoped_audio_exceptions.into_iter().collect(),
        tile_scoped_route_exceptions: tile_scoped_route_exceptions.into_iter().collect(),
        unclassified_roots,
        oversized_parts,
        dangling_object_pointers_to_clear,
    })
}

pub(super) fn dependency_parts(plan: &Plan, inventory: &Inventory) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut dependencies = BTreeMap::<usize, BTreeSet<usize>>::new();
    for (part_index, part) in plan.parts.iter().enumerate() {
        for key in &part.nodes {
            let Some(node) = inventory.nodes.get(key) else {
                continue;
            };
            for edge in &node.edges {
                let edge = canonical_key(&plan.canonical, *edge);
                let Some(location) = plan.locations.get(&edge) else {
                    continue;
                };
                if location.part != part_index {
                    dependencies
                        .entry(part_index)
                        .or_default()
                        .insert(location.part);
                }
            }
        }
    }
    for root in &plan.roots {
        let Some(owner_part) = root_owner_part(root, plan) else {
            continue;
        };
        let Some(target) = plan.locations.get(&root.target) else {
            continue;
        };
        if target.part != owner_part {
            dependencies
                .entry(owner_part)
                .or_default()
                .insert(target.part);
        }
        for key in &root.preload_roots {
            let key = canonical_key(&plan.canonical, *key);
            let Some(location) = plan.locations.get(&key) else {
                continue;
            };
            if location.part != owner_part {
                dependencies
                    .entry(owner_part)
                    .or_default()
                    .insert(location.part);
            }
        }
    }
    dependencies
}

pub(super) fn root_owner_part(root: &Root, plan: &Plan) -> Option<usize> {
    let part = root.output_part?;
    let output = plan.parts.get(part)?;
    (output.family == root.family).then_some(part)
}

pub(super) fn dependency_order(
    parts: &[Part],
    dependencies: &BTreeMap<usize, BTreeSet<usize>>,
) -> Result<Vec<usize>, String> {
    let mut remaining = (0..parts.len()).collect::<BTreeSet<_>>();
    let mut emitted = BTreeSet::<usize>::new();
    let mut order = Vec::with_capacity(parts.len());
    while !remaining.is_empty() {
        let ready = remaining.iter().copied().find(|part| {
            dependencies
                .get(part)
                .is_none_or(|required| required.iter().all(|value| emitted.contains(value)))
        });
        let Some(ready) = ready else {
            let cycle = dependency_cycle_components(&remaining, dependencies)
                .into_iter()
                .map(|component| {
                    let component_set = component.iter().copied().collect::<BTreeSet<_>>();
                    let members = component
                        .iter()
                        .map(|index| {
                            format!(
                                "{} [{}]",
                                parts[*index].output_name,
                                family_label(&parts[*index].family)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let edges = component
                        .iter()
                        .flat_map(|source| {
                            dependencies
                                .get(source)
                                .into_iter()
                                .flatten()
                                .filter(|target| component_set.contains(target))
                                .map(|target| {
                                    format!(
                                        "{} -> {}",
                                        parts[*source].output_name, parts[*target].output_name
                                    )
                                })
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("SCC({members}); edges: {edges}")
                })
                .collect::<Vec<_>>();
            return Err(format!(
                "cross-part dependency cycle remained after SCC packing: {}",
                cycle.join(" | ")
            ));
        };
        remaining.remove(&ready);
        emitted.insert(ready);
        order.push(ready);
    }
    Ok(order)
}

/// Return only the actual cyclic SCCs in the still-blocked part graph. Parts that merely
/// depend on a cycle are deliberately excluded from the diagnostic.
pub(super) fn dependency_cycle_components(
    remaining: &BTreeSet<usize>,
    dependencies: &BTreeMap<usize, BTreeSet<usize>>,
) -> Vec<Vec<usize>> {
    let mut adjacency = BTreeMap::<usize, Vec<usize>>::new();
    let mut reverse = BTreeMap::<usize, Vec<usize>>::new();
    for source in remaining {
        let edges = dependencies
            .get(source)
            .into_iter()
            .flatten()
            .filter(|target| remaining.contains(target))
            .copied()
            .collect::<Vec<_>>();
        for target in &edges {
            reverse.entry(*target).or_default().push(*source);
        }
        adjacency.insert(*source, edges);
    }

    let mut visited = BTreeSet::<usize>::new();
    let mut order = Vec::<usize>::new();
    for start in remaining {
        if visited.contains(start) {
            continue;
        }
        let mut stack = vec![(*start, false)];
        while let Some((part, expanded)) = stack.pop() {
            if expanded {
                order.push(part);
                continue;
            }
            if !visited.insert(part) {
                continue;
            }
            stack.push((part, true));
            for target in adjacency.get(&part).into_iter().flatten().rev() {
                if !visited.contains(target) {
                    stack.push((*target, false));
                }
            }
        }
    }

    let mut assigned = BTreeSet::<usize>::new();
    let mut cycles = Vec::<Vec<usize>>::new();
    for start in order.into_iter().rev() {
        if !assigned.insert(start) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([start]);
        while let Some(part) = queue.pop_front() {
            component.push(part);
            for parent in reverse.get(&part).into_iter().flatten() {
                if assigned.insert(*parent) {
                    queue.push_back(*parent);
                }
            }
        }
        component.sort_unstable();
        let self_loop = component.len() == 1
            && dependencies
                .get(&component[0])
                .is_some_and(|targets| targets.contains(&component[0]));
        if component.len() > 1 || self_loop {
            cycles.push(component);
        }
    }
    cycles.sort();
    cycles
}
