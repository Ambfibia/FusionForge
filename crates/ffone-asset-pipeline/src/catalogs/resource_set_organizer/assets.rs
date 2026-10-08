use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSetCatalogEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub prefix: String,
    pub family: String,
    pub definition: ResourceSetArtifact,
    pub member_count: u64,
    pub texture_count: u64,
}

pub const PLAYER_ITEM_SET_CATALOG_SCHEMA: &str = "ffone.player-item-set-catalog.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerItemSetCatalog {
    pub schema: String,
    pub sets: Vec<ResourceSetCatalogEntry>,
    #[serde(default)]
    pub models: Vec<PlayerItemCatalogModel>,
    pub rendering_textures: Vec<ResourceSetArtifact>,
}

pub(super) fn sort_player_catalog_models(models: &mut Vec<PlayerItemCatalogModel>) {
    models.sort_by(|left, right| {
        (
            left.category.as_str(),
            left.true_name.as_str(),
            left.source_route.as_str(),
            left.model.path.as_str(),
        )
            .cmp(&(
                right.category.as_str(),
                right.true_name.as_str(),
                right.source_route.as_str(),
                right.model.path.as_str(),
            ))
    });
}

pub(super) fn player_catalog_provenance_files(asset_root: &Path) -> Vec<PathBuf> {
    [
        "data/character_creation/appearance.json",
        "data/character_creation/avatar_items.json",
        "data/character_creation/name_wheel.json",
        "data/character_creation/runtime_textures.json",
    ]
    .into_iter()
    .map(|path| asset_root.join(path))
    .collect()
}

pub(super) fn update_player_catalog_provenance(
    asset_root: &Path,
    catalog: &ResourceSetArtifact,
) -> Result<()> {
    let value = serde_json::to_value(catalog).map_err(generated_json_error)?;
    for path in player_catalog_provenance_files(asset_root) {
        if !path.is_file() {
            continue;
        }
        let mut document = read_json(&path, "character-creation catalog")?;
        let slot = document
            .pointer_mut("/provenance/playerEquipmentCatalog")
            .ok_or_else(|| {
                invalid_error(format!(
                    "character-creation catalog has no playerEquipmentCatalog proof: {path:?}"
                ))
            })?;
        *slot = value.clone();
        write_replace(&path, &pretty_json(&document)?)?;
    }
    Ok(())
}

pub(super) fn player_stage_path(stage: &Path, rooted: &str) -> Result<PathBuf> {
    let relative = rooted
        .strip_prefix("characters/player/")
        .ok_or_else(|| invalid_error("player staged path escaped player root"))?;
    Ok(stage.join(Path::new(relative)))
}

pub(super) fn verify_player_item_sets_at_asset_root(asset_root: &Path) -> Result<()> {
    let player_root = canonical_directory(&asset_root.join("characters/player"), "player root")?;
    for retired in [
        player_root.join("equipment"),
        player_root.join("shared/runtime-textures"),
        player_root.join("hnpc-runtime-textures"),
    ] {
        if retired.exists() {
            return invalid(format!(
                "retired player texture/model store still exists: {retired:?}"
            ));
        }
    }
    let catalog_path = player_root.join("items/catalog.json");
    let bytes = fs::read(&catalog_path).map_err(|error| io_at(&catalog_path, error))?;
    let catalog: PlayerItemSetCatalog =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if catalog.schema != PLAYER_ITEM_SET_CATALOG_SCHEMA || catalog.sets.is_empty() {
        return invalid("player item-set catalog has an invalid identity");
    }
    if catalog.models.is_empty() {
        return invalid("player item-set catalog contains no model route index");
    }
    let items_root = canonical_directory(&player_root.join("items"), "player items root")?;
    if let Some(report) = collect_files(&items_root)?.into_iter().find(|path| {
        path.file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.ends_with(".publish.json"))
    }) {
        return invalid(format!(
            "conversion report leaked into runtime player assets: {report:?}"
        ));
    }
    let rendering_root =
        canonical_directory(&player_root.join("rendering"), "player rendering root")?;
    let shared_root = fs::canonicalize(asset_root.join("textures/shared")).ok();
    let mut texture_hashes = BTreeMap::new();
    for texture in &catalog.rendering_textures {
        verify_resource_artifact(asset_root, texture, "player rendering texture")?;
        if texture_hashes.insert(texture.blake3.clone(), texture.path.clone()).is_some() {
            return invalid("player rendering textures contain duplicate bytes");
        }
    }
    let mut model_count = 0u64;
    let mut discovered_models = Vec::<PlayerItemCatalogModel>::new();
    for entry in &catalog.sets {
        let definition_bytes =
            verify_resource_artifact(asset_root, &entry.definition, "player resource-set")?;
        let document: ResourceSetDocument =
            serde_json::from_slice(&definition_bytes).map_err(|source| PipelineError::Json {
                path: entry.definition.path.clone(),
                source,
            })?;
        if document.schema != RESOURCE_SET_SCHEMA
            || document.id != entry.id
            || document.domain != "player_item"
            || document.members.len() as u64 != entry.member_count
            || document.textures.len() as u64 != entry.texture_count
        {
            return invalid(format!("invalid player resource set {}", entry.id));
        }
        let set_root = canonical_directory(
            asset_root
                .join(&entry.definition.path)
                .parent()
                .ok_or_else(|| invalid_error("player set definition has no parent"))?,
            "player item-set root",
        )?;
        for texture in &document.textures {
            verify_resource_artifact(asset_root, texture, "player item atlas")?;
            if texture_hashes.get(&texture.blake3).is_some_and(|path|path != &texture.path) {
                return invalid(format!(
                    "player atlas bytes are duplicated across sets: {}",
                    texture.path
                ));
            }
            texture_hashes.insert(texture.blake3.clone(), texture.path.clone());
        }
        let declared_texture_paths = document.textures.iter().map(|texture| {
            fs::canonicalize(asset_root.join(&texture.path)).map_err(|error|io_at(&texture.path,error))
        }).collect::<Result<BTreeSet<_>>>()?;
        for texture in &declared_texture_paths {
            if !texture.starts_with(&items_root) && !texture.starts_with(&rendering_root)
                && !shared_root.as_ref().is_some_and(|root|texture.starts_with(root)) {
                return invalid("declared player atlas escaped its native asset domain");
            }
        }
        for member in &document.members {
            let item_bytes =
                verify_resource_artifact(asset_root, &member.definition, "player item definition")?;
            let item: PlayerItemDefinition =
                serde_json::from_slice(&item_bytes).map_err(|source| PipelineError::Json {
                    path: member.definition.path.clone(),
                    source,
                })?;
            if item.id != member.id || item.resource_set != entry.id {
                return invalid(format!("player item definition mismatch for {}", member.id));
            }
            if !member.files.contains(&item.model) {
                return invalid(format!("player item {} does not own its model", member.id));
            }
            let model_bytes =
                verify_resource_artifact(asset_root, &item.model, "player item model")?;
            let document = read_glb_json_bytes(&model_bytes, &item.model.path)?;
            let model_path = asset_root.join(&item.model.path);
            for image in document
                .get("images")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
            {
                let Some(uri) = image.get("uri").and_then(JsonValue::as_str) else {
                    continue;
                };
                let resolved = fs::canonicalize(
                    model_path
                        .parent()
                        .ok_or_else(|| invalid_error("player model has no parent"))?
                        .join(uri),
                )
                .map_err(|error| io_at(&model_path, error))?;
                if !resolved.starts_with(&set_root)
                    && !resolved.starts_with(&rendering_root)
                    && !shared_root.as_ref().is_some_and(|root| resolved.starts_with(root))
                    && !declared_texture_paths.contains(&resolved)
                {
                    return invalid(format!(
                        "player model texture escaped its item set: {}",
                        item.model.path
                    ));
                }
            }
            for file in &member.files {
                verify_resource_artifact(asset_root, file, "player item payload")?;
            }
            discovered_models.push(PlayerItemCatalogModel {
                category: item.category,
                true_name: item.true_name,
                source_route: item.source_route,
                resource_set: item.resource_set,
                model: item.model,
            });
            model_count += 1;
        }
    }
    if model_count == 0 {
        return invalid("player item sets contain no models");
    }
    sort_player_catalog_models(&mut discovered_models);
    if discovered_models != catalog.models {
        return invalid("player item-set catalog model route index differs from set members");
    }
    for pair in catalog.models.windows(2) {
        if pair[0].category == pair[1].category
            && pair[0].true_name == pair[1].true_name
            && pair[0].source_route == pair[1].source_route
        {
            return invalid(format!(
                "duplicate player item model route: {}/{} ({})",
                pair[0].category, pair[0].true_name, pair[0].source_route
            ));
        }
    }
    let catalog_artifact = ResourceSetArtifact {
        path: "characters/player/items/catalog.json".to_owned(),
        bytes: bytes.len() as u64,
        blake3: hash_bytes(&bytes),
    };
    let expected_proof = serde_json::to_value(&catalog_artifact).map_err(generated_json_error)?;
    for path in player_catalog_provenance_files(asset_root) {
        if !path.is_file() {
            return invalid(format!("character-creation catalog is missing: {path:?}"));
        }
        let document = read_json(&path, "character-creation catalog")?;
        if document.pointer("/provenance/playerEquipmentCatalog") != Some(&expected_proof) {
            return invalid(format!(
                "character-creation catalog has stale player item proof: {path:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn update_catalog(
    asset_root: &Path,
    catalog: &mut JsonValue,
    path_map: &BTreeMap<String, String>,
    object_sets: &BTreeMap<String, String>,
    sets: &[ResourceSetCatalogEntry],
) -> Result<()> {
    if let Some(shared) = catalog
        .get_mut("sharedFiles")
        .and_then(JsonValue::as_array_mut)
    {
        shared.retain(|entry| {
            !entry
                .get("path")
                .and_then(JsonValue::as_str)
                .is_some_and(|path| path.starts_with("map/shared/textures/"))
        });
    }
    replace_paths(catalog, path_map);
    for key in ["objects", "compositeObjects"] {
        for object in catalog
            .get_mut(key)
            .and_then(JsonValue::as_array_mut)
            .into_iter()
            .flatten()
        {
            let Some(id) = object.get("id").and_then(JsonValue::as_str) else {
                continue;
            };
            if let Some(set) = object_sets.get(id) {
                object
                    .as_object_mut()
                    .expect("catalog object is a JSON object")
                    .insert("resourceSet".to_owned(), JsonValue::String(set.clone()));
            }
        }
    }
    let set_values = sets
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(generated_json_error)?;
    catalog
        .as_object_mut()
        .ok_or_else(|| invalid_error("map catalog root is not an object"))?
        .insert("resourceSets".to_owned(), JsonValue::Array(set_values));
    refresh_artifact_objects(catalog, asset_root)?;

    // Composite file lists are an older ownership proof. Keep them exact while
    // the set manifest becomes the primary package closure.
    if let Some(composites) = catalog
        .get_mut("compositeObjects")
        .and_then(JsonValue::as_array_mut)
    {
        for composite in composites {
            let definition_path = composite
                .get("definition")
                .and_then(|value| value.get("path"))
                .and_then(JsonValue::as_str)
                .ok_or_else(|| invalid_error("composite object has no definition path"))?;
            let member_root = Path::new(definition_path)
                .parent()
                .ok_or_else(|| invalid_error("composite definition has no parent"))?;
            let set_root = member_root
                .parent()
                .and_then(Path::parent)
                .ok_or_else(|| invalid_error("composite member has no set root"))?;
            let files = collect_files(&asset_root.join(set_root))?
                .into_iter()
                .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("set.json"))
                .map(|path| artifact_from_live(asset_root, &path))
                .collect::<Result<Vec<_>>>()?;
            composite
                .as_object_mut()
                .expect("composite is an object")
                .insert(
                    "files".to_owned(),
                    serde_json::to_value(files).map_err(generated_json_error)?,
                );
        }
    }
    Ok(())
}

pub(super) fn stage_path_for_rooted(stage: &Path, rooted: &str) -> Result<PathBuf> {
    let relative = rooted.strip_prefix("map/objects/").ok_or_else(|| {
        invalid_error(format!("staged object path escaped object root: {rooted}"))
    })?;
    Ok(stage.join(Path::new(relative)))
}

pub(super) fn asset_relative(asset_root: &Path, path: &Path) -> Result<String> {
    Ok(slash_path(path.strip_prefix(asset_root).map_err(|_| {
        invalid_error(format!("asset escaped game root: {path:?}"))
    })?))
}

pub(super) fn checked_asset_path(asset_root: &Path, rooted: &str) -> Result<PathBuf> {
    let components = normalized_components(Path::new(rooted))?;
    if components.is_empty() {
        return invalid("asset path resolved to an empty string");
    }
    let mut path = asset_root.to_path_buf();
    for component in components {
        path.push(component);
    }
    Ok(path)
}

pub(super) fn relative_path(from: &Path, to: &Path) -> Result<String> {
    let from = normalized_components(from)?;
    let to = normalized_components(to)?;
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left.eq_ignore_ascii_case(right))
        .count();
    let mut parts = Vec::new();
    parts.extend((common..from.len()).map(|_| "..".to_owned()));
    parts.extend(to[common..].iter().cloned());
    if parts.is_empty() {
        return invalid("relative asset path resolved to an empty string");
    }
    Ok(parts.join("/"))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
