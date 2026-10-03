use super::*;

pub(super) fn duplicate_analysis(inventory: &Inventory) -> JsonValue {
    let mut exact = BTreeMap::<(String, String, String, String), Vec<NodeKey>>::new();
    let mut by_name = BTreeMap::<(String, String), BTreeMap<(String, String), Vec<NodeKey>>>::new();
    for (key, node) in &inventory.nodes {
        if node.name.is_empty() {
            continue;
        }
        let (identity_kind, content_sha1) = if let Some(hash) = &node.semantic_hash {
            ("semanticUnityValueSha1".to_string(), hash.clone())
        } else {
            let mut hasher = Sha1::new();
            hasher.update(b"ffclient.serialized-leaf-fallback.v1\0");
            hasher.update(node.type_tree_hash.as_bytes());
            hasher.update(node.raw_hash.as_bytes());
            (
                "typeTreeAndSerializedSha1".to_string(),
                format!("{:x}", hasher.finalize()),
            )
        };
        exact
            .entry((
                node.object_type.clone(),
                node.name.to_ascii_lowercase(),
                identity_kind.clone(),
                content_sha1.clone(),
            ))
            .or_default()
            .push(*key);
        by_name
            .entry((node.object_type.clone(), node.name.to_ascii_lowercase()))
            .or_default()
            .entry((identity_kind, content_sha1))
            .or_default()
            .push(*key);
    }
    let locations = |keys: &[NodeKey]| {
        keys.iter()
            .take(24)
            .filter_map(|key| {
                let asset = inventory.assets.get(key.0)?;
                let bundle = inventory.bundles.get(asset.bundle_index)?;
                Some(serde_json::json!({
                    "bundle": bundle.name,
                    "asset": asset.name,
                    "pathId": key.1
                }))
            })
            .collect::<Vec<_>>()
    };
    let exact_groups = exact
        .into_iter()
        .filter(|(_, keys)| keys.len() > 1)
        .map(|((object_type, name, identity_kind, content_sha1), keys)| {
            let eligible = keys.iter().all(|key| {
                inventory.nodes.get(key).is_some_and(|node| {
                    node.edges.is_empty()
                        && EXACT_LEAF_TYPES.contains(&node.object_type.as_str())
                        && node.semantic_hash.is_some()
                })
            });
            serde_json::json!({
                "objectType": object_type,
                "name": name,
                "identityKind": identity_kind,
                "contentSha1": content_sha1,
                "occurrences": keys.len(),
                "eligibleForAutomaticDedupe": eligible,
                "locations": locations(&keys),
                "locationsTruncated": keys.len().saturating_sub(24)
            })
        })
        .collect::<Vec<_>>();
    let same_name_different_content = by_name
        .into_iter()
        .filter(|(_, variants)| variants.len() > 1)
        .map(|((object_type, name), variants)| {
            let variants = variants
                .into_iter()
                .map(|((identity_kind, content_sha1), keys)| {
                    serde_json::json!({
                        "identityKind": identity_kind,
                        "contentSha1": content_sha1,
                        "occurrences": keys.len(),
                        "locations": locations(&keys),
                        "locationsTruncated": keys.len().saturating_sub(24)
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "objectType": object_type,
                "name": name,
                "variants": variants
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "exactSameNameAndContent": exact_groups,
        "sameNameDifferentContent": same_name_different_content
    })
}

pub(super) fn root_scope(family: &Family) -> Option<String> {
    match family {
        Family::Dong(tile) => Some(tile.to_ascii_lowercase()),
        _ => None,
    }
}

pub(super) fn merge_root_preloads(target: &mut Root, source: &Root) {
    target.sections.extend(source.sections.iter().cloned());
    target
        .preload_roots
        .extend(source.preload_roots.iter().copied());
    target.preload_roots.sort();
    target.preload_roots.dedup();
    target
        .preserved_preloads
        .extend(source.preserved_preloads.iter().cloned());
    target.preserved_preloads.sort_by(|left, right| {
        asset_ref_key(&left.0)
            .cmp(&asset_ref_key(&right.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    target.preserved_preloads.dedup_by(|left, right| {
        asset_ref_key(&left.0) == asset_ref_key(&right.0) && left.1 == right.1
    });
}

pub(super) fn family_base_name(family: &Family, cfg: &LegacyConfig) -> String {
    match family {
        Family::Auto => cfg.core_name.clone(),
        Family::Core => cfg.core_name.clone(),
        Family::TutorialAudio => cfg.tutorial_audio_name.clone(),
        Family::UiAudio => cfg.ui_audio_name.clone(),
        Family::NpcVoice => cfg.npc_voice_prefix.clone(),
        Family::WorldShared => cfg.world_shared_prefix.clone(),
        Family::Dong(tile) => format!("{tile}.resourceFile"),
        Family::Npc => cfg.npc_prefix.clone(),
        Family::Hnpc => cfg.hnpc_prefix.clone(),
        Family::Nano => cfg.nano_prefix.clone(),
        Family::Player => cfg.player_prefix.clone(),
        Family::Items => cfg.items_prefix.clone(),
        Family::Icons => cfg.icons_prefix.clone(),
        Family::Compat(name) => name.clone(),
    }
}

pub(super) fn resource_stem(value: &str) -> &str {
    value
        .strip_suffix(".resourceFile")
        .or_else(|| value.strip_suffix(".resourcefile"))
        .unwrap_or(value)
}

pub(super) fn output_part_name(family: &Family, ordinal: usize, count: usize, cfg: &LegacyConfig) -> String {
    let base = family_base_name(family, cfg);
    if (count == 1 || ordinal == 0)
        && matches!(
            family,
            Family::Auto
                | Family::Core
                | Family::TutorialAudio
                | Family::UiAudio
                | Family::Dong(_)
                | Family::Compat(_)
        )
    {
        return if base.to_ascii_lowercase().ends_with(".resourcefile") {
            base
        } else {
            format!("{base}.resourceFile")
        };
    }
    let suffix = if matches!(family, Family::Dong(_)) {
        "part"
    } else {
        ""
    };
    let separator = if suffix.is_empty() { "" } else { "_" };
    format!(
        "{}{separator}{suffix}_{:03}.resourceFile",
        resource_stem(&base),
        ordinal + 1
    )
}

pub(super) fn output_internal_name(output_name: &str) -> String {
    let digest = sha1_hex(output_name.to_ascii_lowercase().as_bytes());
    format!("customassetbundle-fflayout-{}", &digest[..24])
}

/// Iterative Kosaraju keeps huge world graphs off the Rust call stack.  Every SCC is an
/// indivisible packing unit, preventing a cyclic PPtr group from being split across parts.
pub(super) fn strongly_connected_components(
    keys: &BTreeSet<NodeKey>,
    nodes: &BTreeMap<NodeKey, Node>,
    canonical: &BTreeMap<NodeKey, NodeKey>,
) -> Vec<Vec<NodeKey>> {
    let mut adjacency = BTreeMap::<NodeKey, Vec<NodeKey>>::new();
    let mut reverse = BTreeMap::<NodeKey, Vec<NodeKey>>::new();
    for key in keys {
        let mut edges = nodes
            .get(key)
            .into_iter()
            .flat_map(|node| node.edges.iter().copied())
            .map(|edge| canonical_key(canonical, edge))
            .filter(|edge| keys.contains(edge))
            .collect::<Vec<_>>();
        edges.sort();
        edges.dedup();
        for edge in &edges {
            reverse.entry(*edge).or_default().push(*key);
        }
        adjacency.insert(*key, edges);
    }

    let mut visited = BTreeSet::<NodeKey>::new();
    let mut order = Vec::<NodeKey>::new();
    for start in keys {
        if visited.contains(start) {
            continue;
        }
        let mut stack = vec![(*start, false)];
        while let Some((key, expanded)) = stack.pop() {
            if expanded {
                order.push(key);
                continue;
            }
            if !visited.insert(key) {
                continue;
            }
            stack.push((key, true));
            for edge in adjacency.get(&key).into_iter().flatten().rev() {
                if !visited.contains(edge) {
                    stack.push((*edge, false));
                }
            }
        }
    }

    let mut assigned = BTreeSet::<NodeKey>::new();
    let mut components = Vec::<Vec<NodeKey>>::new();
    for start in order.into_iter().rev() {
        if !assigned.insert(start) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([start]);
        while let Some(key) = queue.pop_front() {
            component.push(key);
            for parent in reverse.get(&key).into_iter().flatten() {
                if assigned.insert(*parent) {
                    queue.push_back(*parent);
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components
}

pub(super) fn align4_estimate(value: u64) -> u64 {
    value.saturating_add(3) & !3
}

pub(super) fn synthetic_root_estimated_bytes(root: &Root) -> u64 {
    let unique_target_preloads = std::iter::once(root.target)
        .chain(root.preload_roots.iter().copied())
        .collect::<BTreeSet<_>>()
        .len() as u64;
    let preserved_preloads = root.preserved_preloads.len() as u64;
    let path_bytes = align4_estimate(4u64.saturating_add(root.path.len() as u64));
    let pointer_bytes = 16u64.saturating_mul(
        1u64.saturating_add(unique_target_preloads)
            .saturating_add(preserved_preloads),
    );
    let preserved_ref_bytes = root
        .preserved_preloads
        .iter()
        .map(|(reference, _)| conservative_asset_ref_bytes(reference))
        .sum::<u64>();
    path_bytes
        .saturating_add(32)
        .saturating_add(pointer_bytes)
        .saturating_add(preserved_ref_bytes)
}

pub(super) fn literal_dong_source_sections(
    inventory: &Inventory,
    cfg: &LegacyConfig,
) -> Result<BTreeMap<Family, BTreeSet<String>>, String> {
    let mut result = BTreeMap::<Family, BTreeSet<String>>::new();
    for bundle in &inventory.bundles {
        let Some(tile) = dong_tile_from_bundle(&bundle.name, &cfg.dong_prefix) else {
            continue;
        };
        let family = Family::Dong(tile);
        if let Some(existing) = result.insert(family.clone(), bundle.sections.clone()) {
            if existing != bundle.sections {
                return Err(format!(
                    "exact {} has conflicting source CachingManifest sections",
                    family_label(&family)
                ));
            }
        }
    }
    Ok(result)
}

/// Exact DongResources names and lifecycle registrations are a runtime contract, so every tile
/// remains one atomic unit and receives only the literal source bundle's manifest sections.
/// Canonical/shared object provenance must never add another Complete phase to the tile.
/// Contracting arbitrary DAG vertices can itself introduce a cycle; the dependency-first
/// packer detects that case and refuses the layout instead of silently merging other families.
pub(super) fn contract_exact_dong_units(
    units: Vec<PackingUnit>,
    literal_sections: &BTreeMap<Family, BTreeSet<String>>,
) -> Result<Vec<PackingUnit>, String> {
    let mut dong_groups = BTreeMap::<Family, usize>::new();
    let mut old_to_new = Vec::with_capacity(units.len());
    let mut grouped = Vec::<PackingUnit>::new();
    for unit in &units {
        let index = if matches!(unit.family, Family::Dong(_)) {
            if let Some(existing) = dong_groups.get(&unit.family) {
                *existing
            } else {
                let index = grouped.len();
                grouped.push(PackingUnit::empty(unit.family.clone()));
                dong_groups.insert(unit.family.clone(), index);
                index
            }
        } else {
            let index = grouped.len();
            grouped.push(PackingUnit::empty(unit.family.clone()));
            index
        };
        old_to_new.push(index);
    }
    for (old_index, unit) in units.into_iter().enumerate() {
        let new_index = old_to_new[old_index];
        let mapped_dependencies = unit
            .dependencies
            .into_iter()
            .map(|dependency| old_to_new[dependency])
            .filter(|dependency| *dependency != new_index)
            .collect::<Vec<_>>();
        let target = &mut grouped[new_index];
        target.sections.extend(unit.sections);
        target.sections_pinned |= unit.sections_pinned;
        target.nodes.extend(unit.nodes);
        target.roots.extend(unit.roots);
        target.estimated_bytes = target.estimated_bytes.saturating_add(unit.estimated_bytes);
        target.owner = merge_owner(target.owner, unit.owner);
        target.dependencies.extend(mapped_dependencies);
    }
    for unit in &mut grouped {
        unit.nodes.sort();
        unit.nodes.dedup();
        unit.roots.sort_unstable();
        unit.roots.dedup();
        unit.dependencies.sort_unstable();
        unit.dependencies.dedup();
        if matches!(unit.family, Family::Dong(_)) {
            unit.sections = literal_sections.get(&unit.family).cloned().ok_or_else(|| {
                format!(
                    "{} has no literal source CachingManifest registration",
                    family_label(&unit.family)
                )
            })?;
            unit.sections_pinned = true;
        }
    }
    Ok(grouped)
}

/// Make every runtime phase self-contained after splitting a monolithic serialized graph.
///
/// AssetBundle container roots are separate zero-byte units and are never dependencies, so
/// widening a dependency-only SCC does not expose a later NPC/Nano public route early. It only
/// makes the objects actually referenced by the consumer available in that phase. This is
/// essential because CharacterCreation and CharacterSelection are alternative entry paths;
/// neither can assume the other phase has already loaded. Complete phases are normalized to
/// their base zone for non-Dong dependencies, preserving the DownloadDongs contract.
pub(super) fn propagate_dependency_sections(units: &mut [PackingUnit]) {
    loop {
        let snapshot = units
            .iter()
            .map(|unit| (unit.sections.clone(), unit.dependencies.clone()))
            .collect::<Vec<_>>();
        let mut changed = false;
        for (consumer_sections, dependencies) in snapshot {
            if consumer_sections.is_empty() {
                continue;
            }
            for dependency in dependencies {
                let Some(dependency_unit) = units.get_mut(dependency) else {
                    continue;
                };
                // A public route must keep its literal lifecycle. In the unit graph roots are
                // not dependency targets, but keep this guard fail-safe if that changes later.
                if !dependency_unit.roots.is_empty() {
                    continue;
                }
                let required =
                    runtime_sections_for_family(&dependency_unit.family, &consumer_sections);
                let before = dependency_unit.sections.len();
                dependency_unit.sections.extend(required);
                changed |= dependency_unit.sections.len() != before;
            }
        }
        if !changed {
            break;
        }
    }

    // Keep semantic ownership even when a dependency SCC is shared across lifecycle phases.
    // The same Nano payload may legitimately be used in both future and past; registering a
    // Nano_Pack in both phases is clearer and preserves the project's family contract.
    for unit in units.iter_mut().filter(|unit| unit.roots.is_empty()) {
        unit.family = lifecycle_safe_family(unit.family.clone(), &unit.sections);
    }
}

/// A family can be registered in more than one lifecycle phase. In particular, Nano routes
/// reused by both future and past must remain in Nano_Pack_* instead of leaking into CoreShared.
pub(super) fn lifecycle_safe_family(family: Family, _sections: &BTreeSet<String>) -> Family {
    family
}

pub(super) fn packing_cycle(units: &[PackingUnit], emitted: &[bool]) -> Vec<usize> {
    let Some(mut current) = emitted.iter().position(|value| !*value) else {
        return Vec::new();
    };
    let mut trail = Vec::<usize>::new();
    let mut positions = BTreeMap::<usize, usize>::new();
    loop {
        if let Some(start) = positions.get(&current).copied() {
            let mut cycle = trail[start..].to_vec();
            cycle.push(current);
            return cycle;
        }
        positions.insert(current, trail.len());
        trail.push(current);
        let Some(next) = units[current]
            .dependencies
            .iter()
            .copied()
            .find(|dependency| !emitted[*dependency])
        else {
            return trail;
        };
        current = next;
    }
}

/// Emit a global dependency-first order and close a part permanently whenever its family
/// changes or it reaches the byte limit. Consequently every output part is a contiguous
/// interval of one topological order. Contracting those intervals preserves acyclicity.
pub(super) fn pack_dependency_units(
    units: &[PackingUnit],
    max_part_bytes: u64,
) -> Result<Vec<PackedUnitsPart>, String> {
    reject_oversized_atomic_units(units, max_part_bytes)?;
    let mut pending = vec![0usize; units.len()];
    let mut consumers = vec![Vec::<usize>::new(); units.len()];
    for (index, unit) in units.iter().enumerate() {
        pending[index] = unit.dependencies.len();
        for dependency in &unit.dependencies {
            if *dependency >= units.len() {
                return Err(format!(
                    "packing unit {index} has invalid dependency {dependency}"
                ));
            }
            consumers[*dependency].push(index);
        }
    }
    let mut ready = ReadyUnitIndex::default();
    for (index, unit) in units.iter().enumerate() {
        if pending[index] == 0 {
            ready.insert(index, unit);
        }
    }
    let mut emitted = vec![false; units.len()];
    let mut emitted_count = 0usize;
    let mut packed = Vec::<PackedUnitsPart>::new();
    let mut open = None::<PackedUnitsPart>;
    let mut preferred_class = None::<(Family, BTreeSet<String>)>;
    let mut preferred_owner = CompactOwner::None;
    while emitted_count < units.len() {
        let candidate = if let Some(part) = &open {
            let remaining = max_part_bytes.saturating_sub(part.estimated_bytes);
            ready.select(
                &(part.family.clone(), part.sections.clone()),
                preferred_owner,
                Some(remaining),
            )
        } else {
            let class = preferred_class
                .as_ref()
                .filter(|class| ready.has_class(class))
                .cloned()
                .or_else(|| ready.first_class());
            class.and_then(|class| ready.select(&class, preferred_owner, None))
        };
        let Some(candidate) = candidate else {
            if let Some(part) = open.take() {
                preferred_class = Some((part.family.clone(), part.sections.clone()));
                packed.push(part);
                continue;
            }
            let cycle = packing_cycle(units, &emitted);
            let labels = cycle
                .iter()
                .map(|index| {
                    format!(
                        "{}(unit {index}, {} objects, {} roots)",
                        family_label(&units[*index].family),
                        units[*index].nodes.len(),
                        units[*index].roots.len()
                    )
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            let exact_dong = cycle
                .iter()
                .any(|index| matches!(units[*index].family, Family::Dong(_)));
            return Err(if exact_dong {
                format!("keeping exact DongResources_* atomic creates a dependency cycle: {labels}")
            } else {
                format!("packing-unit dependency graph is cyclic: {labels}")
            });
        };
        let unit = &units[candidate];
        ready.remove(candidate, unit);
        let part = open.get_or_insert_with(|| PackedUnitsPart {
            family: unit.family.clone(),
            sections: unit.sections.clone(),
            units: Vec::new(),
            estimated_bytes: PART_METADATA_RESERVE_BYTES,
        });
        debug_assert_eq!(part.family, unit.family);
        debug_assert_eq!(part.sections, unit.sections);
        part.units.push(candidate);
        part.estimated_bytes = part.estimated_bytes.saturating_add(unit.estimated_bytes);
        if matches!(unit.owner, CompactOwner::One(_)) {
            preferred_owner = unit.owner;
        }
        emitted[candidate] = true;
        emitted_count += 1;
        for consumer in &consumers[candidate] {
            pending[*consumer] = pending[*consumer].saturating_sub(1);
            if pending[*consumer] == 0 {
                ready.insert(*consumer, &units[*consumer]);
            }
        }
    }
    if let Some(part) = open {
        packed.push(part);
    }
    Ok(packed)
}
