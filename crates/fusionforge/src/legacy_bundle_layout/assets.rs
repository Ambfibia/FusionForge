use super::*;

#[derive(Debug, Clone)]
pub(super) struct SourceAsset {
    pub(super) bundle_index: usize,
    pub(super) path: PathBuf,
    pub(super) name: String,
    pub(super) format: u32,
    pub(super) tree: TypeMetadata,
    pub(super) objects: BTreeMap<i64, ObjectInfo>,
    pub(super) refs: Vec<AssetRef>,
}

#[derive(Debug, Clone)]
pub(super) struct AssetBundleTemplate {
    pub(super) source_asset: usize,
    pub(super) path_id: i64,
    pub(super) value: UnityValue,
    pub(super) info: ObjectInfo,
}

pub(super) fn normalized_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

pub(super) const CACHING_MANIFEST_SECTIONS: &[&str] = &[
    "m_CharacterCreation",
    "m_CharacterSelection",
    "m_Tutorial",
    "m_FreeZone",
    "m_FreeZoneComplete",
    "m_PaidZone",
    "m_PaidZoneComplete",
];

/// Read the lifecycle contract before any legacy names are retired. A filename can occur
/// in several phases; retain every phase instead of assigning phases from semantic family.
pub(super) fn source_caching_manifest_sections(
    out_dir: &Path,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let main = out_dir.join("main.unity3d");
    if !main.is_file() {
        return Err(format!("{} was not found", main.display()));
    }
    let extracted = crate::native_build_temp_dir("legacy_layout_manifest_sections")?;
    crate::extract_bundle_native_to_dir(&main, extracted.path())?;
    let mut paths = fs::read_dir(extracted.path())
        .map_err(|err| format!("{}: {err}", extracted.path().display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && !retained_opaque_file(path))
        .collect::<Vec<_>>();
    paths.sort();

    let mut result = BTreeMap::<String, BTreeSet<String>>::new();
    let mut found = false;
    for path in paths {
        let Ok(asset) = Asset::from_path(&path) else {
            continue;
        };
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "MonoBehaviour" {
                continue;
            }
            let Ok(value) = asset.read_object(0, info) else {
                continue;
            };
            if object_name(&value) != "CachingManifest" {
                continue;
            }
            found = true;
            for section in CACHING_MANIFEST_SECTIONS {
                for entry in value_array(value.get(section)) {
                    let Some(name) = entry.get("fileName").and_then(UnityValue::as_str) else {
                        continue;
                    };
                    let name = name.trim();
                    if name.is_empty() {
                        continue;
                    }
                    result
                        .entry(name.to_ascii_lowercase())
                        .or_default()
                        .insert((*section).to_string());
                }
            }
        }
    }
    if !found {
        return Err(format!(
            "CachingManifest was not found while extracting {}",
            main.display()
        ));
    }
    Ok(result)
}

pub(super) fn path_id_candidates(path_id: i64) -> [i64; 2] {
    [path_id, i64::from(path_id as u32)]
}

pub(super) fn asset_has_path(asset: &SourceAsset, path_id: i64) -> Option<i64> {
    path_id_candidates(path_id)
        .into_iter()
        .find(|candidate| asset.objects.contains_key(candidate))
}

pub(super) fn source_asset_for_ref(
    reference: &AssetRef,
    assets: &[SourceAsset],
    by_internal_name: &BTreeMap<String, Vec<usize>>,
    path_id: i64,
) -> Result<Option<NodeKey>, String> {
    let mut candidates = BTreeSet::<NodeKey>::new();
    for value in [&reference.file_path, &reference.asset_path] {
        let name = internal_ref_name(value);
        if name.is_empty() {
            continue;
        }
        for asset_index in by_internal_name.get(&name).into_iter().flatten() {
            if let Some(path_id) = asset_has_path(&assets[*asset_index], path_id) {
                candidates.insert((*asset_index, path_id));
            }
        }
    }
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.into_iter().next()),
        _ => Err(format!(
            "ambiguous explicit asset ref '{}'/'{}' pathID {path_id}: {candidates:?}",
            reference.file_path, reference.asset_path
        )),
    }
}

pub(super) fn legacy_route_rank(path: &str, bundle: &str) -> i32 {
    let path = normalized_path(path);
    let bundle = bundle.to_ascii_lowercase();
    if bundle.starts_with("character_") || bundle.starts_with("npc_pack_") {
        return -100;
    }
    if path.starts_with("fu sound/") {
        return (bundle != "fusound.resourcefile") as i32;
    }
    if path.starts_with("cc sound")
        || path.starts_with("ui sound")
        || path.starts_with("charactercreationassets")
    {
        return (bundle != "charactercreation.resourcefile") as i32;
    }
    if path.starts_with("prefabs/particle/effectscripts") || path.contains("bullettable.asset") {
        return (bundle != "effects.resourcefile") as i32;
    }
    if path.starts_with("tutorialassets/shaders/") {
        return (bundle != "characterselection.resourcefile") as i32;
    }
    if path.starts_with("tutorialassets") || path.starts_with("tut sound") {
        return (bundle != "tutorial.resourcefile") as i32;
    }
    if path.starts_with("texture/") || path.ends_with(".dds") {
        return match bundle.as_str() {
            "chartexture.resourcefile" => 0,
            "npctexture.resourcefile" => 1,
            "retro_shared.resourcefile" => 2,
            "retro_shared_part2.resourcefile" => 3,
            _ => 100,
        };
    }
    if path.starts_with("nano/") {
        return match bundle.as_str() {
            "futurenano.resourcefile" => 0,
            "nano.resourcefile" => 1,
            _ => 100,
        };
    }
    match bundle.as_str() {
        "retro_shared.resourcefile" => 0,
        "retro_shared_part2.resourcefile" => 1,
        "icons.resourcefile" => 2,
        "tabledata.resourcefile" => 3,
        "freearea_shared.resourcefile" => 4,
        "world_shared_part1.resourcefile" => 10,
        "world_shared_part2.resourcefile" => 11,
        "world_shared_part3.resourcefile" => 12,
        "world_shared_part4.resourcefile" => 13,
        "world_shared_part5.resourcefile" => 14,
        _ if bundle.starts_with("dongresources_") => 100,
        _ => 50,
    }
}

pub(super) fn conservative_asset_ref_bytes(reference: &AssetRef) -> u64 {
    // Format 6 writes two C strings, a 16-byte GUID and a 4-byte type ID.  The writer uses
    // NUL-terminated strings, but length-prefixed/aligned strings are deliberately charged here
    // so this remains an upper estimate if the serialized-file format changes slightly.
    align4_estimate(4u64.saturating_add(reference.asset_path.len() as u64))
        .saturating_add(16 + 4)
        .saturating_add(align4_estimate(
            4u64.saturating_add(reference.file_path.len() as u64),
        ))
}

#[derive(Default)]
pub(super) struct ReadyUnitIndex {
    pub(super) by_class: BTreeMap<(Family, BTreeSet<String>), BTreeSet<(u64, usize)>>,
    pub(super) by_owner: BTreeMap<(Family, BTreeSet<String>, usize), BTreeSet<(u64, usize)>>,
}

impl ReadyUnitIndex {
    pub(super) fn class(unit: &PackingUnit) -> (Family, BTreeSet<String>) {
        (unit.family.clone(), unit.sections.clone())
    }

    pub(super) fn insert(&mut self, index: usize, unit: &PackingUnit) {
        let key = (unit.estimated_bytes, index);
        self.by_class
            .entry(Self::class(unit))
            .or_default()
            .insert(key);
        if let CompactOwner::One(owner) = unit.owner {
            self.by_owner
                .entry((unit.family.clone(), unit.sections.clone(), owner))
                .or_default()
                .insert(key);
        }
    }

    pub(super) fn remove(&mut self, index: usize, unit: &PackingUnit) {
        let key = (unit.estimated_bytes, index);
        if let Some(ready) = self.by_class.get_mut(&Self::class(unit)) {
            ready.remove(&key);
        }
        if let CompactOwner::One(owner) = unit.owner {
            if let Some(ready) =
                self.by_owner
                    .get_mut(&(unit.family.clone(), unit.sections.clone(), owner))
            {
                ready.remove(&key);
            }
        }
    }

    pub(super) fn has_class(&self, class: &(Family, BTreeSet<String>)) -> bool {
        self.by_class
            .get(class)
            .is_some_and(|ready| !ready.is_empty())
    }

    pub(super) fn first_class(&self) -> Option<(Family, BTreeSet<String>)> {
        self.by_class
            .iter()
            .find_map(|(class, ready)| (!ready.is_empty()).then(|| class.clone()))
    }

    pub(super) fn select_from(ready: &BTreeSet<(u64, usize)>, limit: Option<u64>) -> Option<usize> {
        match limit {
            Some(limit) => ready
                .range(..=(limit, usize::MAX))
                .next_back()
                .map(|(_, index)| *index),
            None => ready.iter().next_back().map(|(_, index)| *index),
        }
    }

    pub(super) fn select(
        &self,
        class: &(Family, BTreeSet<String>),
        preferred_owner: CompactOwner,
        limit: Option<u64>,
    ) -> Option<usize> {
        if let CompactOwner::One(owner) = preferred_owner {
            if let Some(ready) = self
                .by_owner
                .get(&(class.0.clone(), class.1.clone(), owner))
            {
                if let Some(index) = Self::select_from(ready, limit) {
                    return Some(index);
                }
            }
        }
        self.by_class
            .get(class)
            .and_then(|ready| Self::select_from(ready, limit))
    }
}

pub(super) fn source_name_index(assets: &[SourceAsset]) -> BTreeMap<String, Vec<usize>> {
    let mut index = BTreeMap::<String, Vec<usize>>::new();
    for (asset_index, asset) in assets.iter().enumerate() {
        index
            .entry(asset.name.to_ascii_lowercase())
            .or_default()
            .push(asset_index);
    }
    index
}

pub(super) fn asset_ref_key(reference: &AssetRef) -> (String, [u8; 16], i32, String) {
    (
        reference.asset_path.to_ascii_lowercase(),
        reference.guid,
        reference.type_id,
        reference.file_path.to_ascii_lowercase(),
    )
}

pub(super) fn extract_single_staged_asset(
    bundle: &Path,
    dir: &Path,
    output_name: &str,
) -> Result<(PathBuf, Asset), String> {
    crate::extract_bundle_native_to_dir(bundle, dir)?;
    let mut files = fs::read_dir(dir)
        .map_err(|err| format!("{}: {err}", dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    let mut parsed = files
        .into_iter()
        .filter_map(|path| Asset::from_path(&path).ok().map(|asset| (path, asset)))
        .collect::<Vec<_>>();
    if parsed.len() != 1 {
        return Err(format!(
            "{output_name} contains {} serialized assets, expected exactly one",
            parsed.len()
        ));
    }
    Ok(parsed.remove(0))
}

pub(super) fn layout_report_path(project: &Path, patch_config: &JsonValue) -> PathBuf {
    let layout_config = patch_config.get("BundleLayout").unwrap_or(&JsonValue::Null);
    let configured = json_string(layout_config, "Report", "bundle-layout-report.json");
    if Path::new(&configured).is_absolute() {
        PathBuf::from(configured)
    } else {
        project.join(configured)
    }
}
