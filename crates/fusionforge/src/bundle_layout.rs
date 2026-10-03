use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value as JsonValue;
use sha1::{Digest, Sha1};

use crate::fusionforge::{
    object_name, pair_name_value, value_array, Asset, AssetRef, ObjectInfo, Pointer, UnityValue,
};

const DEFAULT_MAX_PART_BYTES: u64 = 24 * 1024 * 1024;
const DEFAULT_MAX_BUNDLES_PER_PART: usize = 12;
const DEFAULT_NPC_PACK_PREFIX: &str = "NPC_Pack";
const EXACT_DEDUPE_TYPES: &[&str] = &[
    "AudioClip",
    "Texture2D",
    "Mesh",
    "Shader",
    "TextAsset",
    "AnimationClip",
];

#[derive(Debug, Clone)]
struct LayoutConfig {
    enabled: bool,
    max_part_bytes: u64,
    max_bundles_per_part: usize,
    npc_pack_prefix: String,
    dedupe_exact: bool,
    report_path: PathBuf,
}

#[derive(Debug, Clone)]
struct SourceBundle {
    path: PathBuf,
    name: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BundleLayoutSummary {
    pub enabled: bool,
    pub source_bundle_count: usize,
    pub output_bundle_count: usize,
    pub output_bundles: Vec<String>,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub exact_objects_deduplicated: usize,
    pub exact_bytes_deduplicated: u64,
    pub unique_audio_clips_preserved: usize,
    pub same_name_different_content: usize,
    pub report_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PartReport {
    output: String,
    sources: Vec<String>,
    source_bytes: u64,
    output_bytes: u64,
    objects_written: usize,
    exact_objects_deduplicated: usize,
    exact_bytes_deduplicated: u64,
    unique_audio_clips_preserved: usize,
    same_name_different_content: usize,
    unresolved_legacy_pointers_preserved: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LayoutReport {
    format: &'static str,
    policy: JsonValue,
    summary: BundleLayoutSummary,
    parts: Vec<PartReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ExactObjectKey {
    object_type: String,
    name: String,
    content_hash: String,
}

#[derive(Debug, Clone)]
struct LoadedAsset {
    source_bundle: String,
    asset: Asset,
}

#[derive(Debug, Clone)]
struct CanonicalObject {
    asset_index: usize,
    path_id: i64,
    output_path_id: i64,
}

fn json_bool(value: &JsonValue, key: &str, default: bool) -> bool {
    value
        .get(key)
        .and_then(JsonValue::as_bool)
        .unwrap_or(default)
}

fn json_u64(value: &JsonValue, key: &str, default: u64) -> u64 {
    value
        .get(key)
        .and_then(JsonValue::as_u64)
        .unwrap_or(default)
}

fn json_string(value: &JsonValue, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn layout_config(project: &Path, patch_config: &JsonValue) -> LayoutConfig {
    let layout = patch_config.get("BundleLayout").unwrap_or(&JsonValue::Null);
    let report = json_string(layout, "Report", "bundle-layout-report.json");
    LayoutConfig {
        enabled: json_bool(layout, "Enabled", false),
        max_part_bytes: json_u64(layout, "MaxPartBytes", DEFAULT_MAX_PART_BYTES).max(1024 * 1024),
        max_bundles_per_part: usize::try_from(json_u64(
            layout,
            "MaxBundlesPerPart",
            DEFAULT_MAX_BUNDLES_PER_PART as u64,
        ))
        .unwrap_or(DEFAULT_MAX_BUNDLES_PER_PART)
        .max(1),
        npc_pack_prefix: json_string(layout, "NpcPackPrefix", DEFAULT_NPC_PACK_PREFIX),
        dedupe_exact: json_bool(layout, "DedupeExact", true),
        report_path: if Path::new(&report).is_absolute() {
            PathBuf::from(report)
        } else {
            project.join(report)
        },
    }
}

fn is_standalone_character_bundle(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("character_")
        && lower.ends_with(".resourcefile")
        && !matches!(
            lower.as_str(),
            "character_creation.resourcefile" | "character_selection.resourcefile"
        )
}

fn collect_character_bundles(out_dir: &Path) -> Result<Vec<SourceBundle>, String> {
    let mut bundles = fs::read_dir(out_dir)
        .map_err(|err| format!("{}: {err}", out_dir.display()))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_str()?.to_string();
            if !path.is_file() || !is_standalone_character_bundle(&name) {
                return None;
            }
            let size = path.metadata().ok()?.len();
            Some(SourceBundle { path, name, size })
        })
        .collect::<Vec<_>>();
    bundles.sort_by_key(|bundle| bundle.name.to_ascii_lowercase());
    Ok(bundles)
}

/// Stable first-fit packing keeps each character and its full dependency closure atomic while
/// preventing a single oversized pack. Sorting by size makes the result compact; the name is the
/// deterministic tie-breaker so repeated builds produce the same parts.
fn split_parts(
    mut bundles: Vec<SourceBundle>,
    max_part_bytes: u64,
    max_bundles_per_part: usize,
) -> Vec<Vec<SourceBundle>> {
    bundles.sort_by(|left, right| {
        right.size.cmp(&left.size).then_with(|| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        })
    });
    let mut parts = Vec::<Vec<SourceBundle>>::new();
    let mut sizes = Vec::<u64>::new();
    for bundle in bundles {
        let target = sizes
            .iter()
            .enumerate()
            .find_map(|(index, size)| {
                (parts[index].len() < max_bundles_per_part)
                    .then_some(())
                    .and_then(|_| size.checked_add(bundle.size))
                    .filter(|combined| *combined <= max_part_bytes)
                    .map(|_| index)
            })
            .unwrap_or_else(|| {
                parts.push(Vec::new());
                sizes.push(0);
                parts.len() - 1
            });
        sizes[target] = sizes[target].saturating_add(bundle.size);
        parts[target].push(bundle);
    }
    for part in &mut parts {
        part.sort_by_key(|bundle| bundle.name.to_ascii_lowercase());
    }
    parts
}

fn sha1_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
}

fn normalized_asset_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

fn pair_value_mut(value: &mut UnityValue) -> Option<&mut UnityValue> {
    match value {
        UnityValue::Pair(_, right) => Some(right.as_mut()),
        UnityValue::Array(items) if items.len() >= 2 => items.get_mut(1),
        _ => None,
    }
}

fn set_i64(value: &mut UnityValue, key: &str, number: i64) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), UnityValue::Int(number));
    }
}

fn set_string(value: &mut UnityValue, key: &str, text: &str) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), UnityValue::String(text.to_string()));
    }
}

fn preload_range(metadata: &UnityValue, preload_len: usize) -> (usize, usize) {
    let start = metadata
        .get("preloadIndex")
        .and_then(UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
        .min(preload_len);
    let size = metadata
        .get("preloadSize")
        .and_then(UnityValue::as_i64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    (start, start.saturating_add(size).min(preload_len))
}

fn asset_ref_key(reference: &AssetRef) -> (String, [u8; 16], i32, String) {
    (
        reference.asset_path.to_ascii_lowercase(),
        reference.guid,
        reference.type_id,
        reference.file_path.to_ascii_lowercase(),
    )
}

fn rewrite_pointers(
    value: &mut UnityValue,
    source_asset: usize,
    output_ids: &BTreeMap<(usize, i64), i64>,
    external_ref_ids: &BTreeMap<(usize, i32), i32>,
    unresolved_legacy_pointers: &mut usize,
) -> Result<(), String> {
    match value {
        UnityValue::Array(items) => {
            for item in items {
                rewrite_pointers(
                    item,
                    source_asset,
                    output_ids,
                    external_ref_ids,
                    unresolved_legacy_pointers,
                )?;
            }
        }
        UnityValue::Object(fields) => {
            for item in fields.values_mut() {
                rewrite_pointers(
                    item,
                    source_asset,
                    output_ids,
                    external_ref_ids,
                    unresolved_legacy_pointers,
                )?;
            }
        }
        UnityValue::Pair(left, right) => {
            rewrite_pointers(
                left,
                source_asset,
                output_ids,
                external_ref_ids,
                unresolved_legacy_pointers,
            )?;
            rewrite_pointers(
                right,
                source_asset,
                output_ids,
                external_ref_ids,
                unresolved_legacy_pointers,
            )?;
        }
        UnityValue::Pointer(pointer) => {
            if pointer.is_null() {
                return Ok(());
            }
            if pointer.file_id == 0 {
                let output_id = output_ids
                    .get(&(source_asset, pointer.path_id))
                    .copied()
                    .ok_or_else(|| {
                        format!(
                            "local pointer {}#{} was not included in NPC pack closure",
                            source_asset, pointer.path_id
                        )
                    })?;
                pointer.source_asset = 0;
                pointer.path_id = output_id;
            } else {
                if let Some(file_id) = external_ref_ids.get(&(source_asset, pointer.file_id)) {
                    pointer.file_id = *file_id;
                } else if let Some(output_id) = output_ids.get(&(source_asset, pointer.path_id)) {
                    // Unity format 7 clients contain legacy PPtrs whose non-zero fileID is
                    // outside m_Externals. The runtime resolver treats those as local when
                    // pathID exists in the current serialized asset; preserve that behavior
                    // explicitly now that all source assets are merged into one file.
                    pointer.file_id = 0;
                    pointer.path_id = *output_id;
                } else {
                    let mut candidates =
                        output_ids
                            .iter()
                            .filter_map(|((_, old_path_id), output_id)| {
                                (*old_path_id == pointer.path_id).then_some(*output_id)
                            });
                    let candidate = candidates.next();
                    if candidate.is_some() && candidates.next().is_none() {
                        pointer.file_id = 0;
                        pointer.path_id = candidate.unwrap_or_default();
                    } else {
                        // Preserve an already-unresolved format 7 PPtr byte-for-byte. It was
                        // dangling in the source too; guessing a target here would be more
                        // dangerous than retaining the original runtime behavior.
                        *unresolved_legacy_pointers += 1;
                    }
                }
                pointer.source_asset = 0;
            }
        }
        _ => {}
    }
    Ok(())
}

fn type_tree_key(info: &ObjectInfo) -> i32 {
    if info.type_id != 0 {
        info.type_id
    } else {
        info.class_id
    }
}

fn load_part_assets(
    part: &[SourceBundle],
) -> Result<(crate::NativeBuildTempDir, Vec<LoadedAsset>), String> {
    let temp = crate::native_build_temp_dir("npc_layout_part")?;
    let mut assets = Vec::new();
    for (bundle_index, bundle) in part.iter().enumerate() {
        let extract_dir = temp.path().join(format!("source_{bundle_index:04}"));
        crate::extract_bundle_native_to_dir(&bundle.path, &extract_dir)?;
        let mut paths = fs::read_dir(&extract_dir)
            .map_err(|err| format!("{}: {err}", extract_dir.display()))?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            if let Ok(asset) = Asset::from_path(&path) {
                if !asset.objects.is_empty() {
                    assets.push(LoadedAsset {
                        source_bundle: bundle.name.clone(),
                        asset,
                    });
                }
            }
        }
    }
    if assets.is_empty() {
        return Err("NPC pack sources contained no readable Unity serialized assets".to_string());
    }
    Ok((temp, assets))
}

fn merge_part(
    part: &[SourceBundle],
    output_path: &Path,
    dedupe_exact: bool,
) -> Result<PartReport, String> {
    let (temp, assets) = load_part_assets(part)?;
    let primary_asset_index = assets
        .iter()
        .enumerate()
        .find_map(|(asset_index, loaded)| {
            loaded.asset.objects.values().find_map(|info| {
                (loaded.asset.object_type_name(info) == "AssetBundle").then_some(asset_index)
            })
        })
        .ok_or_else(|| "NPC pack sources contained no AssetBundle object".to_string())?;

    let internal_name = format!(
        "CustomAssetBundle-{}",
        output_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("NPC_Pack")
    );

    let mut exact_seen = BTreeMap::<ExactObjectKey, CanonicalObject>::new();
    let mut output_ids = BTreeMap::<(usize, i64), i64>::new();
    let mut canonical = Vec::<CanonicalObject>::new();
    let mut next_path_id = 2i64;
    let mut exact_objects_deduplicated = 0usize;
    let mut exact_bytes_deduplicated = 0u64;
    let mut same_name_hashes = BTreeMap::<(String, String), BTreeSet<String>>::new();
    let mut unique_audio = BTreeSet::<(String, String)>::new();
    let mut primary_assetbundle = None::<(usize, i64)>;

    for (asset_index, loaded) in assets.iter().enumerate() {
        for (path_id, info) in &loaded.asset.objects {
            let object_type = loaded.asset.object_type_name(info);
            if object_type == "AssetBundle" {
                if primary_assetbundle.is_none() || asset_index == primary_asset_index {
                    primary_assetbundle = Some((asset_index, *path_id));
                }
                continue;
            }
            let raw = loaded.asset.object_raw_data(info)?;
            let content_hash = sha1_hex(raw);
            let name = loaded
                .asset
                .read_object(asset_index, info)
                .map(|value| object_name(&value))
                .unwrap_or_default();
            same_name_hashes
                .entry((object_type.clone(), name.to_ascii_lowercase()))
                .or_default()
                .insert(content_hash.clone());
            if object_type == "AudioClip" {
                unique_audio.insert((name.to_ascii_lowercase(), content_hash.clone()));
            }
            let dedupe_key = ExactObjectKey {
                object_type: object_type.clone(),
                name: name.to_ascii_lowercase(),
                content_hash,
            };
            if dedupe_exact
                && !name.is_empty()
                && EXACT_DEDUPE_TYPES.contains(&object_type.as_str())
            {
                if let Some(existing) = exact_seen.get(&dedupe_key) {
                    output_ids.insert((asset_index, *path_id), existing.output_path_id);
                    exact_objects_deduplicated += 1;
                    exact_bytes_deduplicated =
                        exact_bytes_deduplicated.saturating_add(u64::from(info.size));
                    continue;
                }
            }
            let object = CanonicalObject {
                asset_index,
                path_id: *path_id,
                output_path_id: next_path_id,
            };
            output_ids.insert((asset_index, *path_id), next_path_id);
            if dedupe_exact
                && !name.is_empty()
                && EXACT_DEDUPE_TYPES.contains(&object_type.as_str())
            {
                exact_seen.insert(dedupe_key, object.clone());
            }
            canonical.push(object);
            next_path_id = next_path_id.saturating_add(1);
        }
    }
    let primary_assetbundle = primary_assetbundle
        .ok_or_else(|| "NPC pack primary AssetBundle object was not found".to_string())?;
    output_ids.insert(primary_assetbundle, 1);

    let mut output_refs = vec![AssetRef {
        asset_path: String::new(),
        guid: [0; 16],
        type_id: 0,
        file_path: internal_name.clone(),
    }];
    let mut ref_by_key = BTreeMap::<(String, [u8; 16], i32, String), i32>::new();
    let mut external_ref_ids = BTreeMap::<(usize, i32), i32>::new();
    for (asset_index, loaded) in assets.iter().enumerate() {
        for (old_index, reference) in loaded.asset.asset_refs.iter().enumerate().skip(1) {
            let key = asset_ref_key(reference);
            let new_index = if let Some(index) = ref_by_key.get(&key) {
                *index
            } else {
                let index = i32::try_from(output_refs.len())
                    .map_err(|_| "NPC pack has too many external asset refs".to_string())?;
                output_refs.push(reference.clone());
                ref_by_key.insert(key, index);
                index
            };
            external_ref_ids.insert(
                (
                    asset_index,
                    i32::try_from(old_index)
                        .map_err(|_| "NPC source ref index overflow".to_string())?,
                ),
                new_index,
            );
        }
    }

    let mut merged_assetbundle = assets[primary_assetbundle.0].asset.read_object(
        primary_assetbundle.0,
        &assets[primary_assetbundle.0].asset.objects[&primary_assetbundle.1],
    )?;
    let mut merged_preloads = Vec::<UnityValue>::new();
    let mut merged_container = Vec::<UnityValue>::new();
    let mut container_targets = BTreeMap::<String, (i32, i64, String)>::new();
    let mut main_asset = None::<(String, UnityValue)>;
    let mut unresolved_legacy_pointers_preserved = 0usize;
    for (asset_index, loaded) in assets.iter().enumerate() {
        for info in loaded.asset.objects.values() {
            if loaded.asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let value = loaded.asset.read_object(asset_index, info)?;
            let old_preloads = value_array(value.get("m_PreloadTable")).to_vec();
            for source_entry in value_array(value.get("m_Container")) {
                let Some((path, metadata)) = pair_name_value(source_entry) else {
                    continue;
                };
                let mut entry = source_entry.clone();
                rewrite_pointers(
                    &mut entry,
                    asset_index,
                    &output_ids,
                    &external_ref_ids,
                    &mut unresolved_legacy_pointers_preserved,
                )?;
                let target = pair_value_mut(&mut entry)
                    .and_then(|metadata| metadata.get("asset"))
                    .and_then(UnityValue::as_pointer)
                    .cloned()
                    .unwrap_or(Pointer {
                        source_asset: 0,
                        file_id: 0,
                        path_id: 0,
                    });
                let normalized = normalized_asset_path(path);
                let target_hash = if target.file_id == 0 {
                    canonical
                        .iter()
                        .find(|object| object.output_path_id == target.path_id)
                        .and_then(|object| {
                            let source = &assets[object.asset_index].asset;
                            source
                                .objects
                                .get(&object.path_id)
                                .and_then(|info| source.object_raw_data(info).ok())
                        })
                        .map(sha1_hex)
                        .unwrap_or_default()
                } else {
                    format!("external:{}:{}", target.file_id, target.path_id)
                };
                if let Some((file_id, path_id, hash)) = container_targets.get(&normalized) {
                    if *file_id == target.file_id
                        && *path_id == target.path_id
                        && *hash == target_hash
                    {
                        continue;
                    }
                    return Err(format!(
                        "container path conflict in NPC pack: '{path}' from {} resolves to different content",
                        loaded.source_bundle
                    ));
                }
                container_targets.insert(normalized, (target.file_id, target.path_id, target_hash));
                let preload_start = merged_preloads.len();
                let (old_start, old_end) = preload_range(metadata, old_preloads.len());
                for preload in &old_preloads[old_start..old_end] {
                    let mut preload = preload.clone();
                    rewrite_pointers(
                        &mut preload,
                        asset_index,
                        &output_ids,
                        &external_ref_ids,
                        &mut unresolved_legacy_pointers_preserved,
                    )?;
                    merged_preloads.push(preload);
                }
                let preload_size = merged_preloads.len().saturating_sub(preload_start);
                if let Some(metadata) = pair_value_mut(&mut entry) {
                    set_i64(metadata, "preloadIndex", preload_start as i64);
                    set_i64(metadata, "preloadSize", preload_size as i64);
                    if main_asset.is_none() {
                        if let Some(asset_pointer) = metadata.get("asset").cloned() {
                            main_asset = Some((path.to_string(), asset_pointer));
                        }
                    }
                }
                merged_container.push(entry);
            }
        }
    }
    if let Some(object) = merged_assetbundle.as_object_mut() {
        object.insert(
            "m_PreloadTable".to_string(),
            UnityValue::Array(merged_preloads),
        );
        object.insert(
            "m_Container".to_string(),
            UnityValue::Array(merged_container),
        );
        if let Some((path, pointer)) = main_asset {
            if let Some(main) = object
                .get_mut("m_MainAsset")
                .and_then(UnityValue::as_object_mut)
            {
                main.insert("name".to_string(), UnityValue::String(path));
                main.insert("asset".to_string(), pointer);
            }
        }
    }
    set_string(&mut merged_assetbundle, "m_Name", &internal_name);

    let mut output_asset = assets[primary_asset_index].asset.clone();
    output_asset.name = internal_name.clone();
    output_asset.asset_refs = output_refs;
    for loaded in &assets {
        for (key, tree) in &loaded.asset.tree.type_trees {
            output_asset
                .tree
                .type_trees
                .entry(*key)
                .or_insert_with(|| tree.clone());
        }
    }

    let mut output_objects = Vec::<(ObjectInfo, Vec<u8>)>::new();
    let primary_info = assets[primary_assetbundle.0].asset.objects[&primary_assetbundle.1].clone();
    let mut output_info = primary_info.clone();
    output_info.path_id = 1;
    output_objects.push((
        output_info,
        assets[primary_assetbundle.0].asset.serialize_object_value(
            primary_assetbundle.0,
            &primary_info,
            &merged_assetbundle,
        )?,
    ));
    for object in &canonical {
        let source = &assets[object.asset_index].asset;
        let info = source
            .objects
            .get(&object.path_id)
            .ok_or_else(|| format!("{}#{} disappeared", source.name, object.path_id))?;
        let mut value = source.read_object(object.asset_index, info)?;
        rewrite_pointers(
            &mut value,
            object.asset_index,
            &output_ids,
            &external_ref_ids,
            &mut unresolved_legacy_pointers_preserved,
        )?;
        let mut output_info = info.clone();
        output_info.path_id = object.output_path_id;
        if !output_asset
            .tree
            .type_trees
            .contains_key(&type_tree_key(&output_info))
        {
            output_asset.tree.type_trees.insert(
                type_tree_key(&output_info),
                source.object_type_tree(info)?.clone(),
            );
        }
        output_objects.push((
            output_info,
            source.serialize_object_value(object.asset_index, info, &value)?,
        ));
    }
    output_objects.sort_by_key(|(info, _)| info.path_id);
    let serialized = output_asset.rebuild_from_object_data_as_format(&output_objects, Some(6))?;
    let pack_dir = temp.path().join("packed");
    fs::create_dir_all(&pack_dir).map_err(|err| format!("{}: {err}", pack_dir.display()))?;
    let serialized_path = pack_dir.join(&internal_name);
    fs::write(&serialized_path, serialized)
        .map_err(|err| format!("{}: {err}", serialized_path.display()))?;

    // Parse the finished asset before deleting any source bundle. This catches bad metadata,
    // duplicate path IDs and translated AudioClip loss while rollback is still trivial.
    let verified = Asset::from_path(&serialized_path)?;
    let verified_audio = verified
        .objects
        .values()
        .filter(|info| verified.object_type_name(info) == "AudioClip")
        .filter_map(|info| {
            let name = verified
                .read_object(0, info)
                .ok()
                .map(|value| object_name(&value))?;
            let hash = sha1_hex(verified.object_raw_data(info).ok()?);
            Some((name.to_ascii_lowercase(), hash))
        })
        .collect::<BTreeSet<_>>();
    if verified_audio != unique_audio {
        return Err(format!(
            "{}: AudioClip verification failed: expected {} unique name+content pairs, wrote {}",
            output_path.display(),
            unique_audio.len(),
            verified_audio.len()
        ));
    }
    crate::pack_bundle_native_from_dir(&pack_dir, output_path)?;

    let same_name_different_content = same_name_hashes
        .values()
        .filter(|hashes| hashes.len() > 1)
        .count();
    Ok(PartReport {
        output: output_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        sources: part.iter().map(|bundle| bundle.name.clone()).collect(),
        source_bytes: part.iter().map(|bundle| bundle.size).sum(),
        output_bytes: output_path.metadata().map(|value| value.len()).unwrap_or(0),
        objects_written: output_objects.len(),
        exact_objects_deduplicated,
        exact_bytes_deduplicated,
        unique_audio_clips_preserved: unique_audio.len(),
        same_name_different_content,
        unresolved_legacy_pointers_preserved,
    })
}

pub(crate) fn apply_bundle_layout(
    project: &Path,
    out_dir: &Path,
    patch_config: &JsonValue,
) -> Result<BundleLayoutSummary, String> {
    let config = layout_config(project, patch_config);
    let sources = collect_character_bundles(out_dir)?;
    let source_bytes = sources.iter().map(|bundle| bundle.size).sum::<u64>();
    if !config.enabled || sources.is_empty() {
        return Ok(BundleLayoutSummary {
            enabled: config.enabled,
            source_bundle_count: sources.len(),
            output_bundle_count: 0,
            output_bundles: Vec::new(),
            source_bytes,
            output_bytes: 0,
            exact_objects_deduplicated: 0,
            exact_bytes_deduplicated: 0,
            unique_audio_clips_preserved: 0,
            same_name_different_content: 0,
            report_path: None,
        });
    }

    let parts = split_parts(
        sources.clone(),
        config.max_part_bytes,
        config.max_bundles_per_part,
    );
    let output_paths = (0..parts.len())
        .map(|index| {
            out_dir.join(format!(
                "{}_{:03}.resourceFile",
                config.npc_pack_prefix,
                index + 1
            ))
        })
        .collect::<Vec<_>>();
    for output in &output_paths {
        if output.is_file() {
            fs::remove_file(output).map_err(|err| format!("{}: {err}", output.display()))?;
        }
    }

    let mut part_reports = Vec::new();
    for (part, output) in parts.iter().zip(&output_paths) {
        match merge_part(part, output, config.dedupe_exact) {
            Ok(report) => part_reports.push(report),
            Err(err) => {
                for generated in &output_paths {
                    let _ = fs::remove_file(generated);
                }
                return Err(err);
            }
        }
    }

    // Commit the layout only after every part parsed and passed audio verification.
    for source in &sources {
        fs::remove_file(&source.path).map_err(|err| format!("{}: {err}", source.path.display()))?;
    }
    let output_bundles = part_reports
        .iter()
        .map(|part| part.output.clone())
        .collect::<Vec<_>>();
    let summary = BundleLayoutSummary {
        enabled: true,
        source_bundle_count: sources.len(),
        output_bundle_count: part_reports.len(),
        output_bundles,
        source_bytes,
        output_bytes: part_reports.iter().map(|part| part.output_bytes).sum(),
        exact_objects_deduplicated: part_reports
            .iter()
            .map(|part| part.exact_objects_deduplicated)
            .sum(),
        exact_bytes_deduplicated: part_reports
            .iter()
            .map(|part| part.exact_bytes_deduplicated)
            .sum(),
        unique_audio_clips_preserved: part_reports
            .iter()
            .map(|part| part.unique_audio_clips_preserved)
            .sum(),
        same_name_different_content: part_reports
            .iter()
            .map(|part| part.same_name_different_content)
            .sum(),
        report_path: Some(config.report_path.to_string_lossy().to_string()),
    };
    let report = LayoutReport {
        format: "ffclient.bundle-layout.v1",
        policy: serde_json::json!({
            "maxPartBytes": config.max_part_bytes,
            "maxBundlesPerPart": config.max_bundles_per_part,
            "dedupeIdentity": ["objectType", "name", "serializedContentSha1"],
            "atomicUnit": "standalone NPC dependency closure",
            "translatedAudioPolicy": "preserve patched AudioClip payloads from the already-patched standalone source bundles and verify unique name+content pairs before deleting sources",
            "implementedFamilies": [
                "NPC_Pack_*"
            ],
            "plannedFamilies": [
                "CoreShared.resourceFile",
                "TutorialAudio.resourceFile",
                "UiAudio.resourceFile",
                "NpcVoiceShared.resourceFile",
                "WorldShared_*.resourceFile",
                "DongResources_*",
                "Nano_Pack_*",
                "PlayerCharacter_Pack_*",
                "Items_Pack_*",
                "Icons_Pack_*"
            ]
        }),
        summary: summary.clone(),
        parts: part_reports,
    };
    if let Some(parent) = config.report_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let data = serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?;
    fs::write(&config.report_path, format!("{data}\n"))
        .map_err(|err| format!("{}: {err}", config.report_path.display()))?;
    Ok(summary)
}

#[cfg(test)]
mod tests;
