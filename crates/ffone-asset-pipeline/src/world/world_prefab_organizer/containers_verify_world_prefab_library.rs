use super::*;

pub fn verify_world_prefab_library(
    project_root: impl AsRef<Path>,
) -> Result<WorldPrefabVerification> {
    let project_root = canonical_directory(project_root.as_ref(), "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let map_root = canonical_directory(&asset_root.join("map"), "map root")?;
    let published_object_root = asset_root.join("objects");
    let intermediate_object_root = map_root.join("objects");
    let (object_root, objects_are_top_level) = if published_object_root.is_dir() {
        (
            canonical_directory(&published_object_root, "published object root")?,
            true,
        )
    } else {
        (
            canonical_directory(&intermediate_object_root, "intermediate object root")?,
            false,
        )
    };
    let catalog_path = map_root.join("catalog.json");
    let catalog_bytes = read_regular_file(&catalog_path, "map catalog")?;
    let catalog: WorldPrefabCatalog =
        serde_json::from_slice(&catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if catalog.schema != WORLD_PREFAB_CATALOG_SCHEMA
        || catalog.source_build != SOURCE_BUILD
        || catalog.generated_by != WORLD_PREFAB_TOOL
    {
        return invalid("map catalog has an unexpected identity");
    }

    let mut expected = BTreeSet::from(["map/catalog.json".to_owned()]);
    let mut resource_set_ids = BTreeSet::new();
    let mut object_resource_sets = BTreeMap::<String, String>::new();
    let mut geometry_resource_sets = BTreeMap::<String, String>::new();
    let mut resource_set_roots = BTreeMap::<String, PathBuf>::new();
    let mut texture_paths = BTreeSet::new();
    for entry in &catalog.resource_sets {
        if !resource_set_ids.insert(entry.id.clone()) {
            return invalid(format!("duplicate map resource-set id {:?}", entry.id));
        }
        let bytes = verify_resource_set_artifact(
            &asset_root,
            &entry.definition,
            "map resource-set definition",
        )?;
        expected.insert(map_closure_path(&entry.definition.path)?);
        let document: ResourceSetDocument =
            serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
                path: entry.definition.path.clone(),
                source,
            })?;
        if document.schema != RESOURCE_SET_SCHEMA
            || document.id != entry.id
            || document.name != entry.name
            || document.category != entry.category
            || document.prefix != entry.prefix
            || document.family != entry.family
            || document.domain != "map_object"
            || document.members.len() as u64 != entry.member_count
            || document.textures.len() as u64 != entry.texture_count
        {
            return invalid(format!(
                "map resource-set definition/catalog mismatch for {}",
                entry.id
            ));
        }
        let definition_path = safe_join(&asset_root, &entry.definition.path)?;
        let set_root = canonical_directory(
            definition_path
                .parent()
                .ok_or_else(|| invalid_error("resource-set definition has no parent"))?,
            "map resource-set root",
        )?;
        resource_set_roots.insert(entry.id.clone(), set_root.clone());
        let mut owned_paths = BTreeSet::new();
        for texture in &document.textures {
            if !owned_paths.insert(texture.path.clone()) {
                return invalid(format!(
                    "map resource set {} duplicates texture {:?}",
                    entry.id, texture.path
                ));
            }
            verify_resource_set_artifact(&asset_root, texture, "map resource-set texture")?;
            let relative = map_closure_path(&texture.path)?;
            expected.insert(relative.clone());
            texture_paths.insert(relative);
        }
        for member in &document.members {
            if object_resource_sets
                .insert(member.id.clone(), entry.id.clone())
                .is_some()
            {
                return invalid(format!(
                    "map object {} belongs to more than one resource set",
                    member.id
                ));
            }
            let definition_bytes = verify_resource_set_artifact(
                &asset_root,
                &member.definition,
                "resource-set member definition",
            )?;
            expected.insert(map_closure_path(&member.definition.path)?);
            let definition_json: JsonValue =
                serde_json::from_slice(&definition_bytes).map_err(|source| {
                    PipelineError::Json {
                        path: member.definition.path.clone(),
                        source,
                    }
                })?;
            if definition_json.get("id").and_then(JsonValue::as_str) != Some(member.id.as_str())
                || definition_json
                    .get("resourceSet")
                    .and_then(JsonValue::as_str)
                    != Some(entry.id.as_str())
            {
                return invalid(format!(
                    "resource-set member definition mismatch for {}",
                    member.id
                ));
            }
            if definition_json.get("schema").and_then(JsonValue::as_str)
                == Some(WORLD_PREFAB_SCHEMA)
            {
                let definition: WorldPrefabDefinition = serde_json::from_value(definition_json)
                    .map_err(|source| PipelineError::Json {
                        path: member.definition.path.clone(),
                        source,
                    })?;
                for part in definition.parts {
                    if geometry_resource_sets
                        .insert(part.resource.clone(), entry.id.clone())
                        .is_some_and(|old| old != entry.id)
                    {
                        return invalid(format!(
                            "map geometry {} belongs to multiple resource sets",
                            part.resource
                        ));
                    }
                }
            }
            for artifact in &member.files {
                if !owned_paths.insert(artifact.path.clone()) {
                    return invalid(format!(
                        "map resource set {} duplicates owned file {:?}",
                        entry.id, artifact.path
                    ));
                }
                verify_resource_set_artifact(&asset_root, artifact, "resource-set member payload")?;
                expected.insert(map_closure_path(&artifact.path)?);
            }
        }
    }
    let mut shared_paths = BTreeSet::new();
    for artifact in &catalog.shared_files {
        if !shared_paths.insert(artifact.path.clone()) {
            return invalid(format!("duplicate shared map file {:?}", artifact.path));
        }
        verify_artifact(&asset_root, artifact, "shared map file")?;
        expected.insert(map_closure_path(&artifact.path)?);
    }
    let mut resource_ids = BTreeSet::new();
    let maximum_position_error = catalog.reconstruction_proof.maximum_position_error;
    if catalog.reconstruction_proof.resources != catalog.resources.len() as u64
        || catalog.reconstruction_proof.source_set_blake3.len() != 64
        || !catalog
            .reconstruction_proof
            .source_set_blake3
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || !maximum_position_error.is_finite()
        || maximum_position_error < 0.0
        || maximum_position_error > 0.02
    {
        return invalid("map catalog has an invalid reconstruction proof");
    }
    for (index, resource) in catalog.resources.iter().enumerate() {
        if !resource_ids.insert(resource.id.clone()) {
            return invalid(format!("duplicate map geometry id {:?}", resource.id));
        }
        if aliases_look_character_like(&resource.aliases.iter().cloned().collect()) {
            return invalid(format!(
                "character-like resource escaped exclusion: {} {:?}",
                resource.id, resource.aliases
            ));
        }
        let local_bytes = verify_artifact(&asset_root, &resource.model, "map-object geometry GLB")?;
        expected.insert(map_closure_path(&resource.model.path)?);
        let local_path = safe_join(&asset_root, &resource.model.path)?;
        let parsed = parse_glb(&local_bytes, &resource.model.path)?;
        verify_local_resource_contract(resource, &parsed)?;
        let allowed_root = if catalog.resource_sets.is_empty() {
            &object_root
        } else {
            let set_id = geometry_resource_sets.get(&resource.id).ok_or_else(|| {
                invalid_error(format!(
                    "map geometry {} is not owned by a resource set",
                    resource.id
                ))
            })?;
            resource_set_roots
                .get(set_id)
                .ok_or_else(|| invalid_error("geometry resource-set root is absent"))?
        };
        for texture in
            resolved_glb_textures(&parsed.document, &local_path, &asset_root, allowed_root)?
        {
            expected.insert(texture.clone());
            texture_paths.insert(texture);
        }
        if (index + 1) % 1_000 == 0 || index + 1 == catalog.resources.len() {
            eprintln!(
                "map verification: {}/{} geometry parts",
                index + 1,
                catalog.resources.len()
            );
        }
    }

    let mut prefab_ids = BTreeSet::new();
    for prefab in &catalog.prefabs {
        if !prefab_ids.insert(prefab.id.clone()) {
            return invalid(format!("duplicate map-object id {:?}", prefab.id));
        }
        let bytes = verify_artifact(&asset_root, &prefab.definition, "map-object definition")?;
        expected.insert(map_closure_path(&prefab.definition.path)?);
        let definition: WorldPrefabDefinition =
            serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
                path: prefab.definition.path.clone(),
                source,
            })?;
        if definition.schema != WORLD_PREFAB_SCHEMA
            || definition.id != prefab.id
            || definition.parts.len() as u64 != prefab.part_count
            || definition.resource_set != prefab.resource_set
            || (!catalog.resource_sets.is_empty()
                && object_resource_sets.get(&prefab.id) != prefab.resource_set.as_ref())
        {
            return invalid(format!(
                "map-object definition/catalog mismatch for {}",
                prefab.id
            ));
        }
        for part in &definition.parts {
            if !resource_ids.contains(&part.resource) {
                return invalid(format!(
                    "map object {} references absent geometry {}",
                    prefab.id, part.resource
                ));
            }
        }
    }

    let mut composite_ids = BTreeSet::new();
    for object in &catalog.composite_objects {
        if !composite_ids.insert(object.id.clone()) || prefab_ids.contains(&object.id) {
            return invalid(format!("duplicate composite map-object id {:?}", object.id));
        }
        let definition_bytes = verify_artifact(
            &asset_root,
            &object.definition,
            "composite map-object definition",
        )?;
        expected.insert(map_closure_path(&object.definition.path)?);
        let definition: MapCompositeObjectDefinition = serde_json::from_slice(&definition_bytes)
            .map_err(|source| PipelineError::Json {
                path: object.definition.path.clone(),
                source,
            })?;
        if definition.schema != MAP_COMPOSITE_OBJECT_SCHEMA
            || definition.id != object.id
            || definition.name != object.name
            || definition.category != object.category
            || definition.prefix != object.prefix
            || definition.family != object.family
            || definition.visual_scenes.is_empty()
            || definition.collision_is_integral != !definition.collision_meshes.is_empty()
            || definition.resource_set != object.resource_set
            || (!catalog.resource_sets.is_empty()
                && object_resource_sets.get(&object.id) != object.resource_set.as_ref())
        {
            return invalid(format!(
                "composite map-object definition/catalog mismatch for {}",
                object.id
            ));
        }
        let mut paths = BTreeSet::new();
        for artifact in &object.files {
            if !paths.insert(artifact.path.clone()) {
                return invalid(format!(
                    "composite map object {} duplicates file {:?}",
                    object.id, artifact.path
                ));
            }
            verify_artifact(&asset_root, artifact, "composite map-object payload")?;
            let relative = map_closure_path(&artifact.path)?;
            if relative.to_ascii_lowercase().ends_with(".png") {
                texture_paths.insert(relative.clone());
            }
            expected.insert(relative);
        }
        if !object.files.contains(&definition.model) {
            return invalid(format!(
                "composite map object {} does not own its model",
                object.id
            ));
        }
        let model_bytes =
            verify_artifact(&asset_root, &definition.model, "composite map-object model")?;
        let parsed = parse_glb(&model_bytes, &definition.model.path)?;
        let model_path = safe_join(&asset_root, &definition.model.path)?;
        let allowed_root = if catalog.resource_sets.is_empty() {
            &object_root
        } else {
            let set_id = object
                .resource_set
                .as_ref()
                .ok_or_else(|| invalid_error("composite object has no resource set"))?;
            resource_set_roots
                .get(set_id)
                .ok_or_else(|| invalid_error("composite resource-set root is absent"))?
        };
        for texture in
            resolved_glb_textures(&parsed.document, &model_path, &asset_root, allowed_root)?
        {
            expected.insert(texture.clone());
            texture_paths.insert(texture);
        }
        let scene_count = parsed
            .document
            .get("scenes")
            .and_then(JsonValue::as_array)
            .map_or(0, Vec::len);
        let mesh_count = parsed
            .document
            .get("meshes")
            .and_then(JsonValue::as_array)
            .map_or(0, Vec::len);
        if definition
            .visual_scenes
            .iter()
            .any(|scene| *scene >= scene_count)
            || definition
                .collision_meshes
                .iter()
                .any(|mesh| *mesh >= mesh_count)
        {
            return invalid(format!(
                "composite map object {} references an absent GLB scene/mesh",
                object.id
            ));
        }
    }

    let mut placement_keys = BTreeSet::new();
    let mut placement_count = 0_u64;
    for placement_set in &catalog.placement_sets {
        if !placement_keys.insert(placement_set.tile_id.clone()) {
            return invalid(format!("duplicate map tile {}", placement_set.tile_id));
        }
        let manifest_bytes =
            verify_artifact(&asset_root, &placement_set.manifest, "map-tile manifest")?;
        expected.insert(map_closure_path(&placement_set.manifest.path)?);
        let manifest: MapTileDocument =
            serde_json::from_slice(&manifest_bytes).map_err(|source| PipelineError::Json {
                path: placement_set.manifest.path.clone(),
                source,
            })?;
        if manifest.schema != MAP_TILE_SCHEMA
            || manifest.id != placement_set.tile_id
            || manifest.objects != placement_set.artifact
            || manifest.legacy_aliases
                != placement_set.legacy_id.iter().cloned().collect::<Vec<_>>()
            || manifest.grid != parse_tile_grid(&placement_set.tile_id)?
        {
            return invalid(format!(
                "map-tile manifest/catalog mismatch for {}",
                placement_set.tile_id
            ));
        }
        if !manifest.files.contains(&manifest.terrain)
            || !manifest.files.contains(&manifest.objects)
            || !manifest.files.contains(&manifest.scene)
            || manifest
                .behaviour
                .as_ref()
                .is_some_and(|behaviour| !manifest.files.contains(behaviour))
        {
            return invalid(format!(
                "map-tile manifest has an incomplete owned-file closure for {}",
                placement_set.tile_id
            ));
        }
        let mut manifest_paths = BTreeSet::new();
        for artifact in &manifest.files {
            if !manifest_paths.insert(artifact.path.clone()) {
                return invalid(format!(
                    "map-tile manifest duplicates file {:?}",
                    artifact.path
                ));
            }
            verify_artifact(&asset_root, artifact, "map-tile owned file")?;
            expected.insert(map_closure_path(&artifact.path)?);
        }

        let bytes = verify_artifact(&asset_root, &placement_set.artifact, "map-tile objects")?;
        expected.insert(map_closure_path(&placement_set.artifact.path)?);
        let document: WorldPrefabPlacementDocument =
            serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
                path: placement_set.artifact.path.clone(),
                source,
            })?;
        if document.schema != WORLD_PREFAB_PLACEMENTS_SCHEMA
            || document.tile_id != placement_set.tile_id
            || document.legacy_id != placement_set.legacy_id
            || document.instances.len() as u64 != placement_set.instance_count
        {
            return invalid(format!(
                "tile objects/catalog mismatch for {}",
                placement_set.tile_id
            ));
        }
        for placement in &document.instances {
            if !prefab_ids.contains(&placement.prefab) {
                return invalid(format!(
                    "tile {} references absent map object {}",
                    document.tile_id, placement.prefab
                ));
            }
        }
        placement_count += document.instances.len() as u64;
    }

    let mut files = collect_files(&map_root)?
        .into_iter()
        .map(|(path, bytes, hash)| (format!("map/{path}"), bytes, hash))
        .collect::<Vec<_>>();
    if objects_are_top_level {
        files.extend(
            collect_files(&object_root)?
                .into_iter()
                .map(|(path, bytes, hash)| (format!("objects/{path}"), bytes, hash)),
        );
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let actual = files
        .iter()
        .map(|(path, _, _)| path.clone())
        .collect::<BTreeSet<_>>();
    if actual != expected {
        let missing = expected
            .difference(&actual)
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        let orphan = actual
            .difference(&expected)
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        return invalid(format!(
            "map file closure mismatch: missing={missing:?}, orphan={orphan:?}"
        ));
    }
    let mut result_set = blake3::Hasher::new();
    for (path, _, hash) in &files {
        append_set_hash(&mut result_set, path, hash);
    }
    Ok(WorldPrefabVerification {
        schema: WORLD_PREFAB_VERIFICATION_SCHEMA.to_owned(),
        catalog_blake3: hash_bytes(&catalog_bytes),
        resources: catalog.resources.len() as u64,
        prefabs: (catalog.prefabs.len() + catalog.composite_objects.len()) as u64,
        placement_sets: catalog.placement_sets.len() as u64,
        placements: placement_count,
        textures: texture_paths.len() as u64,
        files: files.len() as u64,
        bytes: files.iter().map(|(_, bytes, _)| *bytes).sum(),
        maximum_position_reconstruction_error: maximum_position_error,
        result_set_blake3: result_set.finalize().to_hex().to_string(),
    })
}

pub fn verify_world_prefab_library_to_report(
    project_root: impl AsRef<Path>,
    report_path: impl AsRef<Path>,
) -> Result<WorldPrefabVerification> {
    let project_root = canonical_directory(project_root.as_ref(), "project root")?;
    let verification = verify_world_prefab_library(&project_root)?;
    let report_path = absolute_under_project(
        &project_root,
        report_path.as_ref(),
        "map verification report",
    )?;
    if report_path.starts_with(project_root.join("assets/game")) {
        return invalid("map verification report must remain outside runtime assets");
    }
    write_json_atomic(&report_path, &verification)?;
    Ok(verification)
}

#[derive(Clone, Copy)]
pub(super) struct CompositeMapObjectSpec {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) prefix: &'static str,
    pub(super) family: &'static str,
    pub(super) legacy_relative: &'static str,
    pub(super) map_relative: &'static str,
    pub(super) model_name: &'static str,
    pub(super) visual_scenes: &'static [usize],
    pub(super) collision_meshes: &'static [usize],
}

pub(super) fn prefab_id(parts: &[PrefabPartKey]) -> Result<String> {
    let value = parts
        .iter()
        .map(|part| {
            serde_json::json!({
                "resource": part.resource,
                "role": part.role.label(),
                "isTrigger": part.is_trigger,
            })
        })
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&value).map_err(generated_json_error)?;
    Ok(format!("map-object-{}", hash_bytes(&bytes)))
}

pub(super) fn object_base_root(taxonomy: &Taxonomy) -> String {
    format!(
        "objects/{}/{}/{}/{}",
        taxonomy.category,
        taxonomy.prefix,
        taxonomy.family,
        safe_slug(&taxonomy.name, "unnamed", 56)
    )
}
