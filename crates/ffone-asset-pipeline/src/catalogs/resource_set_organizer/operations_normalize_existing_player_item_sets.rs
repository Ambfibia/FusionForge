use super::*;

pub(super) fn normalize_existing_player_item_sets(asset_root: &Path) -> Result<PlayerItemSetOrganizerReport> {
    let catalog_path = asset_root.join("characters/player/items/catalog.json");
    let original_catalog = fs::read(&catalog_path).map_err(|error| io_at(&catalog_path, error))?;
    let mut catalog: PlayerItemSetCatalog =
        serde_json::from_slice(&original_catalog).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if catalog.schema != PLAYER_ITEM_SET_CATALOG_SCHEMA || catalog.sets.is_empty() {
        return invalid("player item-set catalog has an invalid identity");
    }

    let mut original_files = BTreeMap::<PathBuf, Vec<u8>>::new();
    original_files.insert(catalog_path.clone(), original_catalog);
    let mut rewritten_sets = Vec::<(PathBuf, Vec<u8>)>::new();
    let mut removed_reports = Vec::<(PathBuf, Vec<u8>)>::new();
    let mut models = Vec::<PlayerItemCatalogModel>::new();
    let mut published_atlases = 0u64;

    for entry in &mut catalog.sets {
        let set_path = checked_asset_path(asset_root, &entry.definition.path)?;
        let original = fs::read(&set_path).map_err(|error| io_at(&set_path, error))?;
        let mut document: ResourceSetDocument =
            serde_json::from_slice(&original).map_err(|source| PipelineError::Json {
                path: set_path.display().to_string(),
                source,
            })?;
        if document.schema != RESOURCE_SET_SCHEMA
            || document.domain != "player_item"
            || document.id != entry.id
        {
            return invalid(format!("invalid player resource set {}", entry.id));
        }
        original_files.insert(set_path.clone(), original);
        published_atlases += document.textures.len() as u64;
        let set_root = set_path
            .parent()
            .ok_or_else(|| invalid_error("player set definition has no parent"))?;

        for member in &mut document.members {
            let mut payload = Vec::with_capacity(member.files.len());
            for artifact in member.files.drain(..) {
                if artifact.path.ends_with(".publish.json") {
                    let report_path = checked_asset_path(asset_root, &artifact.path)?;
                    let canonical = canonical_file(&report_path, "player conversion report")?;
                    if !canonical.starts_with(set_root) {
                        return invalid(format!(
                            "player conversion report escaped its set: {}",
                            artifact.path
                        ));
                    }
                    let bytes = fs::read(&canonical).map_err(|error| io_at(&canonical, error))?;
                    removed_reports.push((canonical, bytes));
                } else {
                    payload.push(artifact);
                }
            }
            member.files = payload;
            let definition_path = checked_asset_path(asset_root, &member.definition.path)?;
            let definition_bytes =
                fs::read(&definition_path).map_err(|error| io_at(&definition_path, error))?;
            let item: PlayerItemDefinition =
                serde_json::from_slice(&definition_bytes).map_err(|source| {
                    PipelineError::Json {
                        path: definition_path.display().to_string(),
                        source,
                    }
                })?;
            if item.id != member.id
                || item.resource_set != entry.id
                || !member.files.contains(&item.model)
            {
                return invalid(format!("player item definition mismatch for {}", member.id));
            }
            models.push(PlayerItemCatalogModel {
                category: item.category,
                true_name: item.true_name,
                source_route: item.source_route,
                resource_set: item.resource_set,
                model: item.model,
            });
        }

        let bytes = pretty_json(&document)?;
        entry.definition.bytes = bytes.len() as u64;
        entry.definition.blake3 = hash_bytes(&bytes);
        entry.member_count = document.members.len() as u64;
        entry.texture_count = document.textures.len() as u64;
        rewritten_sets.push((set_path, bytes));
    }

    sort_player_catalog_models(&mut models);
    catalog.models = models;
    let catalog_bytes = pretty_json(&catalog)?;
    let catalog_artifact = ResourceSetArtifact {
        path: "characters/player/items/catalog.json".to_owned(),
        bytes: catalog_bytes.len() as u64,
        blake3: hash_bytes(&catalog_bytes),
    };
    for path in player_catalog_provenance_files(asset_root) {
        if path.is_file() {
            original_files.insert(
                path.clone(),
                fs::read(&path).map_err(|error| io_at(&path, error))?,
            );
        }
    }

    let apply_result = (|| -> Result<()> {
        for (path, bytes) in &rewritten_sets {
            write_replace(path, bytes)?;
        }
        write_replace(&catalog_path, &catalog_bytes)?;
        for (path, _) in &removed_reports {
            fs::remove_file(path).map_err(|error| io_at(path, error))?;
        }
        update_player_catalog_provenance(asset_root, &catalog_artifact)?;
        verify_player_item_sets_at_asset_root(asset_root)
    })();
    if let Err(error) = apply_result {
        for (path, bytes) in original_files {
            let _ = write_replace(&path, &bytes);
        }
        for (path, bytes) in removed_reports {
            let _ = write_replace(&path, &bytes);
        }
        return Err(error);
    }

    Ok(PlayerItemSetOrganizerReport {
        schema: PLAYER_ITEM_SET_REPORT_SCHEMA.to_owned(),
        sets: catalog.sets.len() as u64,
        models: catalog.models.len() as u64,
        source_texture_files: published_atlases + catalog.rendering_textures.len() as u64,
        published_atlases,
        duplicate_texture_files_removed: 0,
        rendering_textures: catalog.rendering_textures.len() as u64,
        conversion_reports_removed: removed_reports.len() as u64,
        catalog: catalog_artifact,
    })
}

pub(super) fn best_matching_player_set(
    texture: &PlayerTexture,
    plans: &[PlayerSetPlan],
    models: &[PlayerModel],
) -> Option<usize> {
    let names = texture
        .names
        .iter()
        .map(|name| safe_slug(name, ""))
        .collect::<Vec<_>>();
    let mut best = None::<(usize, usize)>;
    for (set_index, set) in plans.iter().enumerate() {
        for member in &set.members {
            let model_name = safe_slug(&models[*member].true_name, "");
            for name in &names {
                let score = common_prefix_length(name, &model_name);
                if score >= 6 && best.is_none_or(|(_, old)| score > old) {
                    best = Some((set_index, score));
                }
            }
        }
    }
    best.map(|(index, _)| index)
}

pub(super) fn common_prefix_length(left: &str, right: &str) -> usize {
    left.bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count()
}

pub(super) fn appearance_category(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for (category, prefixes) in [
        ("face", &["f_face_", "m_face_", "face_", "glass_"][..]),
        ("hair", &["f_head_", "m_head_", "head_"][..]),
        ("body", &["f_skin", "m_skin", "f_naked", "m_naked"][..]),
        ("shirt", &["f_shirt_", "m_shirt_", "shirt_"][..]),
        ("pants", &["f_pants_", "m_pants_", "pants_"][..]),
        ("shoes", &["f_shoes_", "m_shoes_", "shoes_"][..]),
        ("back", &["back_", "f_back_", "m_back_"][..]),
        ("hat", &["hat_", "helmet_"][..]),
        ("weapon", &["rifle_", "pistol_", "sword_", "glove_"][..]),
        ("vehicle", &["vehicle_"][..]),
    ] {
        if prefixes.iter().any(|prefix| lower.starts_with(prefix)) {
            return category.to_owned();
        }
    }
    "misc".to_owned()
}

pub(super) fn appearance_family(name: &str) -> String {
    let slug = safe_slug(name, "appearance");
    if let Some((head, tail)) = slug.rsplit_once('_') {
        if tail.len() == 1 && tail.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return head.to_owned();
        }
    }
    slug
}

pub(super) fn artifact_from_player_stage(stage: &Path, rooted: &str) -> Result<ResourceSetArtifact> {
    let path = player_stage_path(stage, rooted)?;
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    Ok(ResourceSetArtifact {
        path: rooted.to_owned(),
        bytes: bytes.len() as u64,
        blake3: hash_bytes(&bytes),
    })
}

pub(super) fn player_reference_json_files(asset_root: &Path) -> Result<Vec<PathBuf>> {
    let needles = [
        b"characters/player/equipment/".as_slice(),
        b"characters/player/shared/runtime-textures/".as_slice(),
        b"characters/player/hnpc-runtime-textures/".as_slice(),
    ];
    let equipment = asset_root.join("characters/player/equipment");
    let mut files = Vec::new();
    for path in collect_files(asset_root)?
        .into_iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .filter(|path| !path.starts_with(&equipment))
        .filter(|path| !slash_path(path).contains("/.item-resource-sets-stage-"))
    {
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        if needles
            .iter()
            .any(|needle| bytes.windows(needle.len()).any(|window| window == *needle))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Verifies the published player item sets without consulting legacy sources.
pub fn verify_player_item_sets(project_root: impl AsRef<Path>) -> Result<()> {
    let project_root = canonical_directory(project_root.as_ref(), "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    verify_player_item_sets_at_asset_root(&asset_root)
}

pub(super) fn verify_resource_artifact(
    asset_root: &Path,
    artifact: &ResourceSetArtifact,
    label: &str,
) -> Result<Vec<u8>> {
    let path = asset_root.join(Path::new(&artifact.path));
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    if bytes.len() as u64 != artifact.bytes || hash_bytes(&bytes) != artifact.blake3 {
        return invalid(format!("{label} differs from proof: {}", artifact.path));
    }
    Ok(bytes)
}

pub(super) fn file_stem(path: &Path) -> Result<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .ok_or_else(|| invalid_error(format!("file has no UTF-8 stem: {path:?}")))
}

pub(super) fn plan_sets(objects: &[MapObject], mut groups: Vec<Vec<usize>>) -> Result<Vec<PlannedSet>> {
    groups.sort_by(|left, right| {
        objects[left[0]]
            .old_relative
            .cmp(&objects[right[0]].old_relative)
    });
    let mut allocated = BTreeMap::<String, BTreeSet<String>>::new();
    let mut sets = Vec::with_capacity(groups.len());
    for members in groups {
        let categories = members
            .iter()
            .map(|index| objects[*index].category.clone())
            .collect::<BTreeSet<_>>();
        let prefixes = members
            .iter()
            .map(|index| objects[*index].prefix.clone())
            .collect::<BTreeSet<_>>();
        let families = members
            .iter()
            .map(|index| objects[*index].family.clone())
            .collect::<BTreeSet<_>>();
        let category = one_or(&categories, "collections");
        let prefix = one_or(&prefixes, "MIXED");
        let family = one_or(&families, "MIXED");
        let material_names = members
            .iter()
            .flat_map(|index| &objects[*index].textures)
            .flat_map(|texture| texture.material_names.iter().cloned())
            .collect::<Vec<_>>();
        let fallback = strip_variant(
            objects[members[0]]
                .old_relative
                .rsplit('/')
                .next()
                .unwrap_or("object_set"),
        );
        let candidate = common_semantic_suffix(&material_names)
            .filter(|value| value.len() >= 3)
            .unwrap_or_else(|| safe_slug(&fallback, "object_set"));
        let parent = format!("{category}/{prefix}/{family}");
        let name = allocate_name(&candidate, allocated.entry(parent.clone()).or_default());
        let mut id_source = members
            .iter()
            .map(|index| objects[*index].id.as_str())
            .collect::<Vec<_>>();
        id_source.sort_unstable();
        let id = format!(
            "map-resource-set-{}",
            &hash_bytes(id_source.join("\n").as_bytes())
        );
        sets.push(PlannedSet {
            id,
            name: name.clone(),
            category,
            prefix,
            family,
            relative: format!("{parent}/{name}"),
            members,
        });
    }
    Ok(sets)
}

pub(super) fn common_semantic_suffix(names: &[String]) -> Option<String> {
    let mut tokenized = names
        .iter()
        .map(|name| {
            safe_slug(name, "")
                .trim_end_matches("_dds")
                .trim_end_matches("_tif")
                .trim_end_matches("_png")
                .split('_')
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|parts| !parts.is_empty())
        .collect::<Vec<_>>();
    if tokenized.is_empty() {
        return None;
    }
    tokenized.sort();
    tokenized.dedup();
    if tokenized.len() == 1 {
        return Some(tokenized.remove(0).join("_").chars().take(112).collect());
    }
    let shortest = tokenized.iter().map(Vec::len).min().unwrap_or(0);
    let mut count = 0;
    for offset in 0..shortest {
        let expected = &tokenized[0][tokenized[0].len() - 1 - offset];
        if tokenized
            .iter()
            .all(|parts| &parts[parts.len() - 1 - offset] == expected)
        {
            count += 1;
        } else {
            break;
        }
    }
    if count == 0 {
        return None;
    }
    let first = &tokenized[0];
    Some(first[first.len() - count..].join("_"))
}

pub(super) fn rewrite_json_file_strings(
    value: &mut JsonValue,
    old_parent: &Path,
    new_parent: &Path,
    path_map: &BTreeMap<String, String>,
    asset_root: &Path,
) -> Result<()> {
    match value {
        JsonValue::String(text) => {
            if !text.to_ascii_lowercase().contains(".png") || text.starts_with("data:") {
                return Ok(());
            }
            let candidate = old_parent.join(Path::new(text.as_str()));
            let Ok(absolute) = fs::canonicalize(&candidate) else {
                return Ok(());
            };
            let old_rooted = asset_relative(asset_root, &absolute)?;
            let Some(new_rooted) = path_map.get(&old_rooted) else {
                return Ok(());
            };
            *text = relative_path(new_parent, Path::new(new_rooted))?;
        }
        JsonValue::Array(values) => {
            for value in values {
                rewrite_json_file_strings(value, old_parent, new_parent, path_map, asset_root)?;
            }
        }
        JsonValue::Object(values) => {
            for value in values.values_mut() {
                rewrite_json_file_strings(value, old_parent, new_parent, path_map, asset_root)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn replace_paths(value: &mut JsonValue, path_map: &BTreeMap<String, String>) {
    match value {
        JsonValue::String(text) => {
            if let Some(replacement) = path_map.get(text) {
                *text = replacement.clone();
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                replace_paths(value, path_map);
            }
        }
        JsonValue::Object(values) => {
            for value in values.values_mut() {
                replace_paths(value, path_map);
            }
        }
        _ => {}
    }
}

pub(super) fn artifact_from_staged(stage: &Path, rooted: &str) -> Result<ResourceSetArtifact> {
    let path = stage_path_for_rooted(stage, rooted)?;
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    Ok(ResourceSetArtifact {
        path: rooted.to_owned(),
        bytes: bytes.len() as u64,
        blake3: hash_bytes(&bytes),
    })
}

pub(super) fn artifact_from_live(asset_root: &Path, path: &Path) -> Result<ResourceSetArtifact> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    Ok(ResourceSetArtifact {
        path: asset_relative(asset_root, path)?,
        bytes: bytes.len() as u64,
        blake3: hash_bytes(&bytes),
    })
}

pub(super) fn meaningful_image_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.is_empty()
        && !lower.starts_with("customassetbundle-")
        && !lower.chars().all(|character| character.is_ascii_hexdigit())
}

pub(super) fn one_or(values: &BTreeSet<String>, fallback: &str) -> String {
    if values.len() == 1 {
        values.iter().next().cloned().unwrap_or_default()
    } else {
        fallback.to_owned()
    }
}

pub(super) fn strip_variant(value: &str) -> String {
    let Some((head, tail)) = value.rsplit_once("_variant_") else {
        return value.to_owned();
    };
    if !tail.is_empty() && tail.bytes().all(|byte| byte.is_ascii_digit()) {
        head.to_owned()
    } else {
        value.to_owned()
    }
}

pub(super) fn allocate_name(candidate: &str, used: &mut BTreeSet<String>) -> String {
    let base = safe_slug(candidate, "resource");
    if used.insert(base.clone()) {
        return base;
    }
    for variant in 2u64.. {
        let name = format!("{base}_variant_{variant:04}");
        if used.insert(name.clone()) {
            return name;
        }
    }
    unreachable!()
}

pub(super) fn safe_slug(value: &str, fallback: &str) -> String {
    let mut output = String::new();
    let mut underscore = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            underscore = false;
        } else if !underscore && !output.is_empty() {
            output.push('_');
            underscore = true;
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    if output.is_empty() {
        output.push_str(fallback);
    }
    output.chars().take(112).collect()
}

pub(super) fn normalized_components(path: &Path) -> Result<Vec<String>> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => components.push(
                value
                    .to_str()
                    .ok_or_else(|| invalid_error("asset path is not UTF-8"))?
                    .to_owned(),
            ),
            Component::ParentDir => {
                if components.pop().is_none() {
                    return invalid("relative asset path escaped its root");
                }
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir => {
                return invalid("resource-set path must be relative");
            }
        }
    }
    Ok(components)
}

pub(super) fn pretty_json(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(generated_json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    if !canonical.is_dir() {
        return invalid(format!("{label} is not a directory: {path:?}"));
    }
    Ok(canonical)
}

pub(super) fn canonical_file(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| io_at(path, error))?;
    if !canonical.is_file() {
        return invalid(format!("{label} is not a file: {path:?}"));
    }
    Ok(canonical)
}

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}
