use super::*;

pub(super) fn collect_pointers(value: &UnityValue, output: &mut Vec<Pointer>) {
    match value {
        UnityValue::Pointer(pointer) if !pointer.is_null() => output.push(pointer.clone()),
        UnityValue::Array(items) => {
            for item in items {
                collect_pointers(item, output);
            }
        }
        UnityValue::Object(fields) => {
            for item in fields.values() {
                collect_pointers(item, output);
            }
        }
        UnityValue::Pair(left, right) => {
            collect_pointers(left, output);
            collect_pointers(right, output);
        }
        _ => {}
    }
}

pub(super) fn discover_resource_bundles(
    out_dir: &Path,
    manifest_sections: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<SourceBundle>, String> {
    const RETAINED_HARDCODED: &[&str] = &[
        "futuremusic.resourcefile",
        "lobbymusic.resourcefile",
        "pastmusic.resourcefile",
        "retromusic.resourcefile",
        "tabledata.resourcefile",
    ];
    let mut bundles = fs::read_dir(out_dir)
        .map_err(|err| format!("{}: {err}", out_dir.display()))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_str()?.to_string();
            let lower = name.to_ascii_lowercase();
            (path.is_file()
                && lower.ends_with(".resourcefile")
                && !RETAINED_HARDCODED.contains(&lower.as_str()))
            .then(|| SourceBundle {
                size: fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0),
                sections: manifest_sections.get(&lower).cloned().unwrap_or_default(),
                name,
                path,
            })
        })
        .collect::<Vec<_>>();
    bundles.sort_by_key(|bundle| bundle.name.to_ascii_lowercase());
    Ok(bundles)
}

/// Resolve only semantics that can be proven from the serialized metadata.  The old
/// preview resolver has a useful global pathID fallback, but silently using it during a
/// destructive repack can connect an object to the wrong asset when path IDs collide.
pub(super) fn resolve_source_pointer(
    pointer: &Pointer,
    assets: &[SourceAsset],
    by_internal_name: &BTreeMap<String, Vec<usize>>,
) -> Result<Option<NodeKey>, String> {
    if pointer.is_null() {
        return Ok(None);
    }
    let source = assets
        .get(pointer.source_asset)
        .ok_or_else(|| format!("PPtr source asset {} is absent", pointer.source_asset))?;
    if pointer.file_id == 0 {
        return Ok(
            asset_has_path(source, pointer.path_id).map(|path_id| (pointer.source_asset, path_id))
        );
    }
    if let Ok(index) = usize::try_from(pointer.file_id) {
        if let Some(reference) = source.refs.get(index) {
            return source_asset_for_ref(reference, assets, by_internal_name, pointer.path_id);
        }
    }

    // Some donor format-7 assets contain a bogus non-zero fileID with a valid local
    // 32-bit pathID.  Accept only the unambiguous local recovery used by the game.
    if source.format == 7 {
        return Ok(
            asset_has_path(source, pointer.path_id).map(|path_id| (pointer.source_asset, path_id))
        );
    }
    Ok(None)
}

pub(super) fn resolve_compact_family(usage: CompactUsage, tile_names: &[String]) -> Family {
    if usage.mask == 0 {
        if usage.many_dongs {
            return Family::WorldShared;
        }
        if let Some(tile) = usage.dong.and_then(|index| tile_names.get(index)) {
            return Family::Dong(tile.clone());
        }
        return Family::Core;
    }
    if usage.mask == USE_CORE {
        return Family::Core;
    }
    if usage.mask.count_ones() == 1 && !usage.many_dongs && usage.dong.is_none() {
        return match usage.mask {
            USE_TUTORIAL_AUDIO => Family::TutorialAudio,
            USE_UI_AUDIO => Family::UiAudio,
            USE_NPC_VOICE => Family::NpcVoice,
            USE_WORLD => Family::WorldShared,
            USE_NPC => Family::Npc,
            USE_HNPC => Family::Hnpc,
            USE_NANO => Family::Nano,
            USE_PLAYER => Family::Player,
            USE_ITEMS => Family::Items,
            USE_ICONS => Family::Icons,
            _ => Family::Core,
        };
    }
    if usage.mask & USE_CORE != 0 {
        return Family::Core;
    }
    if usage.mask & USE_NANO != 0 {
        // Nano packs are registered in every managed loading phase and are therefore
        // a safe owner for a Nano object shared with one later phase.
        return Family::Nano;
    }
    let tutorial_phase = USE_TUTORIAL_AUDIO | USE_NPC_VOICE | USE_WORLD | USE_NPC | USE_HNPC;
    if usage.mask & !tutorial_phase == 0 {
        return Family::WorldShared;
    }
    if usage.mask & !(USE_HNPC | USE_PLAYER) == 0 && usage.mask & USE_PLAYER != 0 {
        return Family::Player;
    }
    if usage.mask & !(USE_HNPC | USE_ITEMS) == 0 && usage.mask & USE_ITEMS != 0 {
        return Family::Items;
    }
    let character_phase = USE_UI_AUDIO | USE_HNPC | USE_PLAYER | USE_ITEMS | USE_ICONS;
    if usage.mask & !character_phase == 0 {
        return Family::Items;
    }
    // Cross-phase resources must exist before both CharacterCreation and Tutorial.
    Family::Core
}

/// Resolve only references owned by the newly-written layout. An external reference whose
/// AssetRef does not name a staged output is a preserved Unity/system reference and is left
/// deliberately unresolved here.
pub(super) fn resolve_staged_pointer(
    asset: &Asset,
    pointer: &Pointer,
    output_objects: &BTreeMap<String, BTreeSet<i64>>,
) -> Result<Option<(String, i64)>, String> {
    if pointer.is_null() {
        return Ok(None);
    }
    let target_name = if pointer.file_id == 0 {
        asset.name.to_ascii_lowercase()
    } else {
        let reference = usize::try_from(pointer.file_id)
            .ok()
            .and_then(|index| asset.asset_refs.get(index))
            .ok_or_else(|| {
                format!(
                    "{} has invalid output fileID {}",
                    asset.name, pointer.file_id
                )
            })?;
        let Some(target_name) = output_reference_name(reference, output_objects)? else {
            return Ok(None);
        };
        target_name
    };
    let path_ids = output_objects.get(&target_name).ok_or_else(|| {
        format!(
            "{} local PPtr target asset '{}' is absent from staged outputs",
            asset.name, target_name
        )
    })?;
    let path_id = path_id_candidates(pointer.path_id)
        .into_iter()
        .find(|candidate| path_ids.contains(candidate))
        .ok_or_else(|| {
            format!(
                "{} has unresolved staged PPtr {}:{} into {}",
                asset.name, pointer.file_id, pointer.path_id, target_name
            )
        })?;
    Ok(Some((target_name, path_id)))
}
