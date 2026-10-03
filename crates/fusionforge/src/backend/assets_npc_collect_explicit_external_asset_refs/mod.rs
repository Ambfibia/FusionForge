use super::super::*;

pub(in super::super) fn first_matching_resolved_path<'a>(
    preferred: Option<&str>,
    candidates: impl IntoIterator<Item = &'a String>,
    resolved_paths: &BTreeSet<String>,
) -> Option<String> {
    let preferred = preferred.map(normalized_asset_path);
    if let Some(preferred) = preferred
        .as_ref()
        .filter(|path| resolved_paths.contains(*path))
    {
        return Some(preferred.clone());
    }
    candidates
        .into_iter()
        .find(|path| resolved_paths.contains(*path))
        .cloned()
}

pub(in super::super) fn npc_key_has_required_container_path(
    key: &(usize, i64),
    source_container_paths: &BTreeMap<(usize, i64), BTreeSet<String>>,
    required_container_paths: &BTreeSet<String>,
) -> bool {
    source_container_paths.get(key).is_some_and(|paths| {
        paths
            .iter()
            .any(|path| npc_standalone_container_path_matches(path, required_container_paths))
    })
}

pub(in super::super) fn npc_external_asset_refs(
    externalized: &BTreeMap<(usize, i64), NpcTargetExternalRef>,
    referenced_asset_names: &BTreeSet<String>,
) -> (Vec<fusionforge::AssetRef>, BTreeMap<String, i32>) {
    let mut names = externalized
        .values()
        .filter(|reference| referenced_asset_names.contains(&reference.asset_name))
        .map(|reference| reference.asset_name.clone())
        .collect::<BTreeSet<_>>();
    let mut refs = Vec::new();
    let mut indices = BTreeMap::new();
    for name in std::mem::take(&mut names) {
        let file_id = refs.len() as i32 + 1;
        indices.insert(name.clone(), file_id);
        refs.push(fusionforge::AssetRef {
            asset_path: String::new(),
            guid: [0; 16],
            type_id: 0,
            file_path: name,
        });
    }
    (refs, indices)
}

pub(in super::super) fn explicit_asset_ref_key(asset_ref: &fusionforge::AssetRef) -> (String, [u8; 16], i32, String) {
    (
        asset_ref.asset_path.clone(),
        asset_ref.guid,
        asset_ref.type_id,
        asset_ref.file_path.clone(),
    )
}

pub(in super::super) fn npc_collect_explicit_external_asset_refs(
    env: &fusionforge::UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
    externalized: &BTreeMap<(usize, i64), NpcTargetExternalRef>,
) -> Result<Vec<fusionforge::AssetRef>, String> {
    let mut refs = Vec::new();
    let mut seen = BTreeSet::new();
    for (asset_index, path_id) in selected {
        if externalized.contains_key(&(*asset_index, *path_id)) {
            continue;
        }
        let Some(asset) = env.assets.get(*asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(path_id) else {
            continue;
        };
        if asset.object_type_name(info) == "AssetBundle" {
            continue;
        }
        let body = asset.read_object(*asset_index, info)?;
        let mut pointers = Vec::new();
        collect_value_pointers(&body, &mut pointers);
        for pointer in pointers {
            if pointer.is_null() || pointer.file_id == 0 {
                continue;
            }
            let Some(source_ref) = env
                .assets
                .get(pointer.source_asset)
                .and_then(|source_asset| {
                    usize::try_from(pointer.file_id)
                        .ok()
                        .and_then(|index| source_asset.asset_refs.get(index))
                })
            else {
                continue;
            };
            let key = explicit_asset_ref_key(source_ref);
            if seen.insert(key) {
                refs.push(source_ref.clone());
            }
        }
    }
    Ok(refs)
}

pub(in super::super) fn ensure_external_asset_ref(
    refs: &mut Vec<fusionforge::AssetRef>,
    indices: &mut BTreeMap<String, i32>,
    asset_name: &str,
) -> i32 {
    if let Some(file_id) = indices.get(asset_name) {
        return *file_id;
    }
    let file_id = refs.len() as i32 + 1;
    refs.push(fusionforge::AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: asset_name.to_string(),
    });
    indices.insert(asset_name.to_string(), file_id);
    file_id
}
