use super::*;

pub const CHARACTER_CREATION_NAME_WHEEL_PATH: &str = "data/character_creation/name_wheel.json";

pub const CHARACTER_CREATION_APPEARANCE_PATH: &str = "data/character_creation/appearance.json";

pub const CHARACTER_CREATION_AVATAR_ITEMS_PATH: &str = "data/character_creation/avatar_items.json";

pub const CHARACTER_CREATION_RUNTIME_TEXTURES_PATH: &str =
    "data/character_creation/runtime_textures.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationAssetReference {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvatarItemLookup {
    pub category: AvatarItemCategory,
    pub item_number: u32,
    pub level: u16,
    pub required_gender: u8,
    /// Legacy `m_iEquipType`. Hats use values 0..=5 to select the exact
    /// hair/face/glasses visibility policy; other categories normally use 0.
    #[serde(default)]
    pub equip_type: u8,
    pub name: String,
    pub description: String,
    pub icon: Option<AvatarIconReference>,
    pub male: AvatarItemVisual,
    pub female: AvatarItemVisual,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeLookupStatus {
    VerifiedUnique,
    VerifiedVariants,
    Missing,
    Ambiguous,
}

#[derive(Clone, Debug)]
pub(super) struct CatalogDocuments {
    pub(super) name_wheel: CharacterCreationNameWheel,
    pub(super) appearance: CharacterCreationAppearance,
    pub(super) avatar_items: CharacterCreationAvatarItems,
    pub(super) runtime_textures: CharacterCreationRuntimeTextures,
}

pub(super) fn effective_equipment_container_route(
    true_name: &str,
    source: &RuntimeTextureSourceMetadata,
    repair: Option<&EquipmentTextureRouteRepair>,
    true_name_repair: Option<&EquipmentTextureTrueNameRepair>,
) -> Option<String> {
    let expected = format!("texture/{true_name}.dds");
    let exact = source
        .container_routes
        .iter()
        .find(|route| route.eq_ignore_ascii_case(&expected))
        .cloned();
    if let Some(exact) = exact {
        return valid_equipment_true_name_resolution(true_name, source, true_name_repair, &exact)
            .then_some(exact);
    }
    if !true_name.eq_ignore_ascii_case(&source.true_name) || true_name_repair.is_some() {
        return None;
    }
    repair
        .filter(|repair| {
            repair
                .requested_container_route
                .eq_ignore_ascii_case(&expected)
                && repair.serialized_container_routes == source.container_routes
        })
        .map(|repair| repair.requested_container_route.clone())
}

pub(super) fn valid_equipment_route_resolution(entry: &EquipmentTextureSourceMetadata) -> bool {
    let expected = format!("texture/{}.dds", entry.true_name);
    if let Some(exact) = entry
        .texture
        .container_routes
        .iter()
        .find(|route| route.eq_ignore_ascii_case(&expected))
    {
        return entry.route_repair.is_none()
            && valid_equipment_true_name_resolution(
                &entry.true_name,
                &entry.texture,
                entry.true_name_repair.as_ref(),
                exact,
            );
    }
    if !entry
        .true_name
        .eq_ignore_ascii_case(&entry.texture.true_name)
        || entry.true_name_repair.is_some()
    {
        return false;
    }
    let Some(repair) = entry.route_repair.as_ref() else {
        return false;
    };
    let (kind, routes, path_id, source_asset): (&str, &[&str], i64, &str) =
        match entry.true_name.to_ascii_lowercase().as_str() {
            "back_buzzshockbmo" => (
                "primary_container_alias",
                &["texture/back_buzzshock.dds"],
                1_889,
                "CustomAssetBundle-aa120043d3c634fe9adfb5cbe08e6970",
            ),
            "back_leggarnets1" => (
                "primary_container_alias",
                &["texture/back_leggarnet.dds"],
                1_888,
                "CustomAssetBundle-aa120043d3c634fe9adfb5cbe08e6970",
            ),
            "hatglassmelee_pochitabullfragbananaguard" => (
                "primary_container_alias",
                &["texture/glass_bullfrag.dds"],
                1_887,
                "CustomAssetBundle-aa120043d3c634fe9adfb5cbe08e6970",
            ),
            "head_puckerberryhead" => (
                "primary_unrouted_object",
                &[],
                1_639,
                "CustomAssetBundle-Retro_shared",
            ),
            "mob_bat" => (
                "primary_external_container_route",
                &[],
                277,
                "CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a",
            ),
            "shirt_gunter" => (
                "primary_cross_routed_object",
                &["texture/shirt_rainicorn.dds"],
                30_064,
                "CustomAssetBundle-Retro_shared",
            ),
            "shirt_rainicorn" => (
                "primary_cross_routed_object",
                &["texture/shirt_gunter.dds"],
                30_063,
                "CustomAssetBundle-Retro_shared",
            ),
            _ => return false,
        };
    if repair.kind != kind
        || repair.requested_container_route != expected
        || repair.reason.trim().is_empty()
        || repair.serialized_container_routes != entry.texture.container_routes
        || !repair
            .serialized_container_routes
            .iter()
            .map(String::as_str)
            .eq(routes.iter().copied())
        || entry.texture.path_id != path_id
        || entry.source.source_asset != source_asset
    {
        return false;
    }
    if entry.true_name.eq_ignore_ascii_case("mob_bat") {
        repair.route_owner.as_ref().is_some_and(|owner| {
            owner.source_alias == "primary"
                && owner.raw_resource_file == "builds/retrobution-20260613/NpcTexture.resourceFile"
                && owner.raw_resource_file_bytes == 10_670_086
                && owner.raw_resource_file_sha256
                    == "557eb191a8b6c60e535a0d66363dfc994a60d1c2218b31bbdb5fec9133a5d101"
                && owner.source_asset == "CustomAssetBundle-d304c52c4bae348e38c743762c1bd818"
                && owner.source_file_bytes == 26_851_393
                && owner.source_file_sha256
                    == "883659790a9cbc8562f4ad8122066df3e94e55d7a48b36b2b5222e5e05760b94"
                && owner.asset_bundle_path_id == 1
                && owner.external_file_id == 1
                && owner.external_path_id == 277
                && owner.target_source_asset == "CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a"
                && owner.target_path_id == 277
        })
    } else {
        repair.route_owner.is_none()
    }
}

pub(super) fn index_manifest(files: &[ProjectAssetFile]) -> Result<BTreeMap<String, ProjectAssetFile>> {
    let mut output = BTreeMap::new();
    for entry in files {
        validate_relative(&entry.path)?;
        if output.insert(entry.path.clone(), entry.clone()).is_some() {
            return invalid(format!("duplicate project manifest path {:?}", entry.path));
        }
    }
    Ok(output)
}

pub(super) fn upsert_manifest_entry(manifest: &mut ProjectAssetManifest, entry: ProjectAssetFile) {
    if let Some(existing) = manifest
        .files
        .iter_mut()
        .find(|existing| existing.path == entry.path)
    {
        *existing = entry;
    } else {
        manifest.files.push(entry);
    }
}

pub(super) fn replace_manifest(asset_root: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let path = asset_root.join(ASSET_MANIFEST_FILE);
    let next = asset_root.join(".asset-manifest.character-creation.next");
    let backup = asset_root.join(".asset-manifest.character-creation.backup");
    if fs::symlink_metadata(&next).is_ok() || fs::symlink_metadata(&backup).is_ok() {
        return invalid("stale character-creation manifest transaction files exist");
    }
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)
        .map_err(|error| io_at(&next, error))?;
    output
        .write_all(&bytes)
        .map_err(|error| io_at(&next, error))?;
    output.sync_all().map_err(|error| io_at(&next, error))?;
    fs::rename(&path, &backup).map_err(|error| io_at(&path, error))?;
    if let Err(error) = fs::rename(&next, &path) {
        let _ = fs::rename(&backup, &path);
        return Err(io_at(&path, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}

pub(super) fn reserve_path(
    existing: &BTreeSet<String>,
    planned: &mut BTreeSet<String>,
    path: &str,
) -> Result<()> {
    validate_relative(path)?;
    let portable = path.to_ascii_lowercase();
    if existing.contains(&portable) || !planned.insert(portable) {
        return invalid(format!("project-asset path collision: {path:?}"));
    }
    Ok(())
}

pub(super) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
