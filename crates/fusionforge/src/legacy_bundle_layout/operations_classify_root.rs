use super::*;

pub(super) fn json_string(value: &JsonValue, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

pub(super) fn config(patch_config: &JsonValue) -> LegacyConfig {
    let value = patch_config.get("BundleLayout").unwrap_or(&JsonValue::Null);
    LegacyConfig {
        enabled: value
            .get("Enabled")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false),
        max_part_bytes: value
            .get("MaxPartBytes")
            .and_then(JsonValue::as_u64)
            .unwrap_or(DEFAULT_MAX_PART_BYTES)
            .max(1024 * 1024),
        preload_npc_bundles: patch_config
            .get("PreloadNpcBundles")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false),
        load_npc_bundles_in_world: patch_config
            .get("LoadNpcBundlesInWorld")
            .and_then(JsonValue::as_bool)
            .unwrap_or(true),
        npc_bundle_manifest_sections: patch_config
            .get("NpcBundleManifestSections")
            .and_then(JsonValue::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_else(|| {
                BTreeSet::from(["m_FreeZone".to_string(), "m_PaidZone".to_string()])
            }),
        core_name: json_string(value, "CoreSharedName", "CoreShared.resourceFile"),
        tutorial_audio_name: json_string(value, "TutorialAudioName", "TutorialAudio.resourceFile"),
        ui_audio_name: json_string(value, "UiAudioName", "UiAudio.resourceFile"),
        npc_voice_prefix: json_string(value, "NpcVoiceSharedName", "NpcVoiceShared"),
        world_shared_prefix: json_string(value, "WorldSharedPrefix", "WorldShared"),
        dong_prefix: json_string(value, "DongResourcesPrefix", "DongResources"),
        npc_prefix: json_string(value, "NpcPackPrefix", "NPC_Pack"),
        hnpc_prefix: json_string(value, "HnpcPackPrefix", "HNPC_Pack"),
        nano_prefix: json_string(value, "NanoPackPrefix", "Nano_Pack"),
        player_prefix: json_string(value, "PlayerCharacterPackPrefix", "PlayerCharacter_Pack"),
        items_prefix: json_string(value, "ItemsPackPrefix", "Items_Pack"),
        icons_prefix: json_string(value, "IconsPackPrefix", "Icons_Pack"),
    }
}

pub(super) fn sha1_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
}

pub(super) fn hash_len(hasher: &mut Sha1, len: usize) {
    hasher.update((len as u64).to_le_bytes());
}

pub(super) fn semantic_value_sha1(object_type: &str, value: &UnityValue) -> String {
    let mut hasher = Sha1::new();
    hasher.update(b"ffclient.unityvalue.semantic.v1\0");
    hash_len(&mut hasher, object_type.len());
    hasher.update(object_type.as_bytes());
    update_semantic_hash(&mut hasher, value);
    format!("{:x}", hasher.finalize())
}

pub(super) fn internal_ref_name(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    normalized
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .to_ascii_lowercase()
}

pub(super) fn type_tree_key(info: &ObjectInfo) -> i32 {
    if info.type_id != 0 {
        info.type_id
    } else {
        info.class_id
    }
}

pub(super) fn tree_signature(tree: &TypeTree) -> String {
    sha1_hex(format!("{tree:#?}").as_bytes())
}

pub(super) fn pair_value_mut(value: &mut UnityValue) -> Option<&mut UnityValue> {
    match value {
        UnityValue::Pair(_, right) => Some(right.as_mut()),
        UnityValue::Array(items) if items.len() >= 2 => items.get_mut(1),
        _ => None,
    }
}

pub(super) fn set_i64(value: &mut UnityValue, key: &str, number: i64) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), UnityValue::Int(number));
    }
}

pub(super) fn set_string(value: &mut UnityValue, key: &str, text: &str) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), UnityValue::String(text.to_string()));
    }
}

pub(super) fn preload_range(metadata: &UnityValue, len: usize) -> (usize, usize) {
    let start = metadata
        .get("preloadIndex")
        .and_then(UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
        .min(len);
    let size = metadata
        .get("preloadSize")
        .and_then(UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    (start, start.saturating_add(size).min(len))
}

pub(super) fn zero_pointers(value: &mut UnityValue) {
    match value {
        UnityValue::Pointer(pointer) => {
            pointer.source_asset = 0;
            pointer.file_id = 0;
            pointer.path_id = 0;
        }
        UnityValue::Array(items) => {
            for item in items {
                zero_pointers(item);
            }
        }
        UnityValue::Object(fields) => {
            for item in fields.values_mut() {
                zero_pointers(item);
            }
        }
        UnityValue::Pair(left, right) => {
            zero_pointers(left);
            zero_pointers(right);
        }
        _ => {}
    }
}

/// Container paths are the stable public API used by ResourceLocator.  Classification is
/// intentionally path-first; source bundle names are consulted only to retain tile locality.
pub(super) fn classify_root(path: &str, source_bundle: &str, cfg: &LegacyConfig) -> Family {
    let path = normalized_path(path);
    let prefix = path.split('/').next().unwrap_or("");
    let source = source_bundle.to_ascii_lowercase();
    match prefix {
        "tut sound" => Family::TutorialAudio,
        "ui sound" | "cc sound" | "fu sound" => Family::UiAudio,
        "vo" => Family::NpcVoice,
        "mob" => Family::Npc,
        "nano" => Family::Nano,
        "icons" | "help" => Family::Icons,
        "wear" => Family::Items,
        "actor" => Family::Player,
        "charactercreationassets" | "tutorialassets" => Family::Core,
        "texture" => {
            let stem = path
                .rsplit('/')
                .next()
                .unwrap_or("")
                .split('.')
                .next()
                .unwrap_or("");
            const GEAR_PREFIXES: &[&str] = &[
                "back_",
                "hat_",
                "helmet_",
                "helmat_",
                "halmet_",
                "helemt_",
                "head_",
                "face_",
                "glass_",
                "glasses_",
                "galss_",
                "shirt_",
                "shirts_",
                "pants_",
                "shoes_",
                "rifle_",
                "rifie_",
                "pistol_",
                "sword_",
                "bazooka_",
                "thbazooka_",
                "bazookarifle_",
                "melee_",
                "thrown_",
                "meleethrown_",
                "weapon_",
                "set_",
                "vehicle_",
                "rocket_",
                "shattergun_",
                "grenade_",
                "car_",
                "board_",
                "cape_",
                "mask_",
                "hand_",
                "hands_",
                "accessory_",
            ];
            const GENDERED_GEAR_PREFIXES: &[&str] = &[
                "back_", "face_", "head_", "helmet_", "pants_", "shirt_", "shirts_", "shoes_",
                "glass_", "hat_", "costume_",
            ];
            if stem.starts_with("nano_") {
                Family::Nano
            } else if stem.starts_with("npc_")
                || stem.starts_with("mob_")
                || stem.starts_with("fusion_")
                || stem.starts_with("fuison_")
            {
                Family::Npc
            } else if GEAR_PREFIXES.iter().any(|prefix| stem.starts_with(prefix))
                || stem
                    .strip_prefix("f_")
                    .or_else(|| stem.strip_prefix("m_"))
                    .is_some_and(|rest| {
                        GENDERED_GEAR_PREFIXES
                            .iter()
                            .any(|prefix| rest.starts_with(prefix))
                    })
            {
                Family::Items
            } else if matches!(
                stem,
                "f_skin" | "m_skin" | "f_naked" | "m_naked" | "naked_f" | "naked_m" | "char_shadow"
            ) || stem.starts_with("actor_")
                || stem.starts_with("avatar_")
            {
                Family::Player
            } else if let Some(tile) = dong_tile_from_bundle(source_bundle, &cfg.dong_prefix) {
                Family::Dong(tile)
            } else if source.contains("world_shared") {
                Family::WorldShared
            } else {
                Family::Auto
            }
        }
        "sound" if source.contains("nano") => Family::Nano,
        "sound" if source.contains("characterselection") => Family::Player,
        "sound" if source.contains("tutorial") || source.contains("world_shared") => {
            Family::WorldShared
        }
        "map" | "textures" | "prefabs" | "resources" | "effect" | "effects" | "plugins"
        | "generateddongs" | "standard assets" | "evsound" | "enviro sound" | "sound" => {
            dong_tile_from_bundle(source_bundle, &cfg.dong_prefix)
                .map(Family::Dong)
                .unwrap_or(Family::WorldShared)
        }
        _ => Family::Core,
    }
}

pub(super) fn family_from_semantic(family: SemanticFamily) -> Family {
    match family {
        SemanticFamily::Npc => Family::Npc,
        SemanticFamily::Hnpc => Family::Hnpc,
        SemanticFamily::Nano => Family::Nano,
        SemanticFamily::Items => Family::Items,
        SemanticFamily::Player => Family::Player,
    }
}

pub(super) fn semantic_family_for_layout(family: &Family) -> Option<SemanticFamily> {
    match family {
        Family::Npc => Some(SemanticFamily::Npc),
        Family::Hnpc => Some(SemanticFamily::Hnpc),
        Family::Nano => Some(SemanticFamily::Nano),
        Family::Items => Some(SemanticFamily::Items),
        Family::Player => Some(SemanticFamily::Player),
        _ => None,
    }
}

pub(super) fn classify_root_with_semantics(
    path: &str,
    source_bundle: &str,
    cfg: &LegacyConfig,
    semantics: &SemanticIndex,
) -> (Family, SemanticOwnerHint) {
    let normalized = normalized_path(path);
    let prefix = normalized.split('/').next().unwrap_or("");
    let fallback = classify_root(path, source_bundle, cfg);
    let Some(hint) = semantics.hint(&normalized) else {
        return (fallback, SemanticOwnerHint::None);
    };

    // These prefixes are themselves a stable runtime contract. TableData is still used
    // for affinity, but can never reinterpret (for example) a mob route as an item route.
    let hard_prefix = matches!(
        prefix,
        "tut sound"
            | "ui sound"
            | "cc sound"
            | "fu sound"
            | "vo"
            | "mob"
            | "nano"
            | "icons"
            | "help"
            | "actor"
            | "charactercreationassets"
            | "tutorialassets"
    );
    if hard_prefix {
        let owner = semantic_family_for_layout(&fallback)
            .map(|family| hint.owner(family))
            .unwrap_or(SemanticOwnerHint::None);
        return (fallback, owner);
    }
    if hint.is_cross_family() {
        return (Family::Core, SemanticOwnerHint::Shared);
    }
    let Some(family) = hint.families.iter().next().copied() else {
        return (fallback, SemanticOwnerHint::None);
    };
    (family_from_semantic(family), hint.owner(family))
}

pub(super) fn preserved_external_ref(
    pointer: &Pointer,
    assets: &[SourceAsset],
    by_internal_name: &BTreeMap<String, Vec<usize>>,
) -> Option<AssetRef> {
    if pointer.file_id == 0 {
        return None;
    }
    let source = assets.get(pointer.source_asset)?;
    let reference = usize::try_from(pointer.file_id)
        .ok()
        .and_then(|index| source.refs.get(index))?;
    let names = [&reference.file_path, &reference.asset_path]
        .into_iter()
        .map(|value| internal_ref_name(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!names.iter().any(|name| by_internal_name.contains_key(name))).then(|| reference.clone())
}

/// Returns true only when serialized metadata identifies a local or known internal target
/// and that exact path ID is already absent from the source inventory. Invalid file IDs and
/// unidentifiable refs are not proof of absence and must remain fatal.
pub(super) fn proven_missing_source_target(
    pointer: &Pointer,
    assets: &[SourceAsset],
    by_internal_name: &BTreeMap<String, Vec<usize>>,
) -> Result<bool, String> {
    if pointer.is_null() {
        return Ok(false);
    }
    let source = assets
        .get(pointer.source_asset)
        .ok_or_else(|| format!("PPtr source asset {} is absent", pointer.source_asset))?;
    if pointer.file_id == 0 {
        return Ok(asset_has_path(source, pointer.path_id).is_none());
    }
    let Ok(reference_index) = usize::try_from(pointer.file_id) else {
        // A negative fileID cannot name an AssetRef in the serialized source. Keeping it
        // while rebuilding the AssetRef table could accidentally make it meaningful.
        return Ok(true);
    };
    let Some(reference) = source.refs.get(reference_index) else {
        // The target's intended identity is unknown, but its runtime absence is proven:
        // Unity cannot resolve a fileID outside the source AssetRef table. Nulling it
        // preserves that no-target behavior without allowing index aliasing after repack.
        return Ok(true);
    };
    let mut known_targets = BTreeSet::<usize>::new();
    for value in [&reference.file_path, &reference.asset_path] {
        let name = internal_ref_name(value);
        if name.is_empty() {
            continue;
        }
        known_targets.extend(by_internal_name.get(&name).into_iter().flatten().copied());
    }
    if known_targets.is_empty() {
        return Ok(false);
    }
    Ok(known_targets
        .into_iter()
        .all(|asset_index| asset_has_path(&assets[asset_index], pointer.path_id).is_none()))
}

pub(super) fn referenced_unreadable_errors(
    unreadable: &BTreeMap<NodeKey, String>,
    referenced: &BTreeSet<NodeKey>,
) -> Vec<String> {
    unreadable
        .iter()
        .filter(|(key, _)| referenced.contains(key))
        .map(|(_, error)| error.clone())
        .collect()
}

pub(super) fn canonical_key(canonical: &BTreeMap<NodeKey, NodeKey>, key: NodeKey) -> NodeKey {
    let mut current = key;
    for _ in 0..64 {
        let next = canonical.get(&current).copied().unwrap_or(current);
        if next == current {
            return current;
        }
        current = next;
    }
    current
}

pub(super) fn family_mask(family: &Family) -> u16 {
    match family {
        Family::Auto | Family::Dong(_) | Family::Compat(_) => 0,
        Family::Core => USE_CORE,
        Family::TutorialAudio => USE_TUTORIAL_AUDIO,
        Family::UiAudio => USE_UI_AUDIO,
        Family::NpcVoice => USE_NPC_VOICE,
        Family::WorldShared => USE_WORLD,
        Family::Npc => USE_NPC,
        Family::Hnpc => USE_HNPC,
        Family::Nano => USE_NANO,
        Family::Player => USE_PLAYER,
        Family::Items => USE_ITEMS,
        Family::Icons => USE_ICONS,
    }
}

pub(super) fn merge_owner(left: CompactOwner, right: CompactOwner) -> CompactOwner {
    match (left, right) {
        (CompactOwner::None, value) | (value, CompactOwner::None) => value,
        (CompactOwner::One(left), CompactOwner::One(right)) if left == right => {
            CompactOwner::One(left)
        }
        _ => CompactOwner::Shared,
    }
}

pub(super) fn merge_usage(target: &mut CompactUsage, source: CompactUsage) -> bool {
    let before = *target;
    target.reachable |= source.reachable;
    target.mask |= source.mask;
    target.owner = merge_owner(target.owner, source.owner);
    if source.many_dongs {
        target.many_dongs = true;
        target.dong = None;
    } else if let Some(incoming) = source.dong {
        match target.dong {
            None if !target.many_dongs => target.dong = Some(incoming),
            Some(existing) if existing != incoming => {
                target.dong = None;
                target.many_dongs = true;
            }
            _ => {}
        }
    }
    *target != before
}

pub(super) fn logical_owner_hint(root: &Root) -> SemanticOwnerHint {
    if !matches!(&root.semantic_owner, SemanticOwnerHint::None) {
        return root.semantic_owner.clone();
    }
    let prefix = root.normalized_path.split('/').next().unwrap_or("");
    let stem = root
        .normalized_path
        .rsplit('/')
        .next()
        .unwrap_or("")
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or_else(|| root.normalized_path.rsplit('/').next().unwrap_or(""));
    match (&root.family, prefix) {
        (Family::Npc, "mob")
        | (Family::Hnpc, "wear")
        | (Family::Nano, "nano")
        | (Family::Player, "actor")
        | (Family::Items, "wear") => {
            SemanticOwnerHint::One(stem.split('@').next().unwrap_or(stem).to_string())
        }
        (Family::NpcVoice, "vo") => {
            let tokens = stem.split('_').collect::<Vec<_>>();
            let count = if matches!(tokens.first().copied(), Some("f" | "m")) {
                2
            } else {
                1
            };
            SemanticOwnerHint::One(tokens.into_iter().take(count).collect::<Vec<_>>().join("_"))
        }
        _ => SemanticOwnerHint::None,
    }
}

pub(super) fn logical_owner_ids(roots: &[Root]) -> BTreeMap<(Family, String), usize> {
    let mut owner_ids = BTreeMap::<(Family, String), usize>::new();
    for root in roots {
        let SemanticOwnerHint::One(owner) = logical_owner_hint(root) else {
            continue;
        };
        let next = owner_ids.len();
        owner_ids
            .entry((root.family.clone(), owner))
            .or_insert(next);
    }
    owner_ids
}

pub(super) fn root_compact_owner(root: &Root, owner_ids: &BTreeMap<(Family, String), usize>) -> CompactOwner {
    match logical_owner_hint(root) {
        SemanticOwnerHint::One(owner) => owner_ids
            .get(&(root.family.clone(), owner))
            .copied()
            .map(CompactOwner::One)
            .unwrap_or(CompactOwner::None),
        SemanticOwnerHint::Shared => CompactOwner::Shared,
        SemanticOwnerHint::None => CompactOwner::None,
    }
}

pub(super) fn propagate_compact_usage(
    roots: &[Root],
    inventory: &Inventory,
    canonical: &BTreeMap<NodeKey, NodeKey>,
    retained_roots: &BTreeMap<NodeKey, Family>,
) -> (BTreeMap<NodeKey, CompactUsage>, Vec<String>) {
    let tile_names = roots
        .iter()
        .filter_map(|root| match &root.family {
            Family::Dong(tile) => Some(tile.clone()),
            _ => None,
        })
        .chain(retained_roots.values().filter_map(|family| match family {
            Family::Dong(tile) => Some(tile.clone()),
            _ => None,
        }))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let tile_ids = tile_names
        .iter()
        .enumerate()
        .map(|(index, tile)| (tile.to_ascii_lowercase(), index))
        .collect::<BTreeMap<_, _>>();
    let owner_ids = logical_owner_ids(roots);

    let mut usage = BTreeMap::<NodeKey, CompactUsage>::new();
    let mut queue = VecDeque::<NodeKey>::new();
    for root in roots {
        let owner = root_compact_owner(root, &owner_ids);
        let dong = match &root.family {
            Family::Dong(tile) => tile_ids.get(&tile.to_ascii_lowercase()).copied(),
            _ => None,
        };
        let seed = CompactUsage {
            reachable: true,
            mask: family_mask(&root.family),
            dong,
            many_dongs: false,
            owner,
        };
        for key in std::iter::once(root.target).chain(root.preload_roots.iter().copied()) {
            let key = canonical_key(canonical, key);
            if merge_usage(usage.entry(key).or_default(), seed) {
                queue.push_back(key);
            }
        }
    }
    for (key, family) in retained_roots {
        let dong = match family {
            Family::Dong(tile) => tile_ids.get(&tile.to_ascii_lowercase()).copied(),
            _ => None,
        };
        let seed = CompactUsage {
            reachable: true,
            mask: family_mask(family),
            dong,
            many_dongs: false,
            owner: CompactOwner::None,
        };
        let key = canonical_key(canonical, *key);
        if merge_usage(usage.entry(key).or_default(), seed) {
            queue.push_back(key);
        }
    }
    while let Some(key) = queue.pop_front() {
        let state = usage.get(&key).copied().unwrap_or_default();
        let Some(node) = inventory.nodes.get(&key) else {
            continue;
        };
        for edge in &node.edges {
            let edge = canonical_key(canonical, *edge);
            if merge_usage(usage.entry(edge).or_default(), state) {
                queue.push_back(edge);
            }
        }
    }
    (usage, tile_names)
}

pub(super) fn semantic_node_signature(key: NodeKey, nodes: &BTreeMap<NodeKey, Node>) -> Option<String> {
    let node = nodes.get(&key)?;
    let mut targets = node
        .edges
        .iter()
        .filter_map(|edge| nodes.get(edge))
        .map(|target| {
            format!(
                "{}|{}|{}",
                target.object_type,
                target.name.to_ascii_lowercase(),
                target.shape_hash
            )
        })
        .collect::<Vec<_>>();
    targets.sort();
    Some(format!(
        "{}|{}|{}|{}",
        node.object_type,
        node.name.to_ascii_lowercase(),
        node.shape_hash,
        targets.join(",")
    ))
}

pub(super) fn semantic_closure_groups(
    start: NodeKey,
    nodes: &BTreeMap<NodeKey, Node>,
) -> BTreeMap<String, Vec<NodeKey>> {
    let mut groups = BTreeMap::<String, Vec<NodeKey>>::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(key) = queue.pop_front() {
        if !seen.insert(key) {
            continue;
        }
        let Some(node) = nodes.get(&key) else {
            continue;
        };
        if let Some(signature) = semantic_node_signature(key, nodes) {
            groups.entry(signature).or_default().push(key);
        }
        queue.extend(node.edges.iter().copied());
    }
    for keys in groups.values_mut() {
        keys.sort();
    }
    groups
}

pub(super) fn family_for_usage(usage: &BTreeSet<usize>, roots: &[Root]) -> Family {
    let families = usage
        .iter()
        .filter_map(|index| roots.get(*index))
        .map(|root| root.family.clone())
        .collect::<BTreeSet<_>>();
    if families.len() == 1 {
        return families.into_iter().next().unwrap_or(Family::Core);
    }
    if !families.is_empty()
        && families
            .iter()
            .all(|family| matches!(family, Family::Dong(_) | Family::WorldShared))
    {
        let tiles = families
            .iter()
            .filter_map(|family| match family {
                Family::Dong(tile) => Some(tile),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if tiles.len() == 1 && !families.contains(&Family::WorldShared) {
            return Family::Dong((*tiles.into_iter().next().unwrap()).clone());
        }
        return Family::WorldShared;
    }
    Family::Core
}

pub(super) fn family_label(family: &Family) -> String {
    match family {
        Family::Auto => "Auto".to_string(),
        Family::Core => "CoreShared".to_string(),
        Family::TutorialAudio => "TutorialAudio".to_string(),
        Family::UiAudio => "UiAudio".to_string(),
        Family::NpcVoice => "NpcVoiceShared".to_string(),
        Family::WorldShared => "WorldShared".to_string(),
        Family::Dong(tile) => format!("Dong:{tile}"),
        Family::Npc => "NPC".to_string(),
        Family::Hnpc => "HNPC".to_string(),
        Family::Nano => "Nano".to_string(),
        Family::Player => "PlayerCharacter".to_string(),
        Family::Items => "Items".to_string(),
        Family::Icons => "Icons".to_string(),
        Family::Compat(name) => format!("Compatibility:{name}"),
    }
}
