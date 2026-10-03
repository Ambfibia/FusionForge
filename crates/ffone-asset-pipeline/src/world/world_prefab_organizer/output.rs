use super::*;

pub(super) fn publish_composite_map_objects(
    asset_root: &Path,
    staging: &Path,
) -> Result<Vec<MapCompositeObjectCatalogEntry>> {
    const SPECS: &[CompositeMapObjectSpec] = &[
        CompositeMapObjectSpec {
            id: "map-object-etc-domeglass-04",
            name: "ETC dome glass 04",
            prefix: "ETC",
            family: "DOME",
            legacy_relative: "props/tutorial/structures/etc_domeglass_04",
            map_relative: "objects/structures/ETC/DOME/etc_domeglass_04",
            model_name: "ETC_domeglass_04.glb",
            visual_scenes: &[0],
            collision_meshes: &[1],
        },
        CompositeMapObjectSpec {
            id: "map-object-npc-building",
            name: "NPC building",
            prefix: "DT",
            family: "BUILDINGS",
            legacy_relative: "props/tutorial/structures/npc_building",
            map_relative: "objects/structures/DT/BUILDINGS/npc_building",
            model_name: "npc_building.glb",
            visual_scenes: &[0],
            collision_meshes: &[],
        },
    ];

    let existing_map_root = asset_root.join("map");
    let mut published = Vec::with_capacity(SPECS.len());
    for spec in SPECS {
        let legacy_source = safe_join(asset_root, spec.legacy_relative)?;
        let mapped_source = safe_join(&existing_map_root, spec.map_relative)?;
        let source = if legacy_source.is_dir() {
            canonical_directory(&legacy_source, "legacy composite map object")?
        } else if mapped_source.is_dir() {
            canonical_directory(&mapped_source, "existing composite map object")?
        } else {
            return invalid(format!(
                "composite map object source is absent for {}",
                spec.id
            ));
        };
        let mut files = Vec::new();
        for (relative, _, _) in collect_files(&source)? {
            if relative == "object.json" {
                continue;
            }
            let bytes = read_regular_file(
                &safe_join(&source, &relative)?,
                "composite map object payload",
            )?;
            let output_relative = format!("{}/{}", spec.map_relative, relative);
            write_new(&safe_join(staging, &output_relative)?, &bytes)?;
            files.push(WorldPrefabArtifact {
                path: format!("map/{output_relative}"),
                bytes: bytes.len() as u64,
                blake3: hash_bytes(&bytes),
            });
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let model_path = format!("map/{}/{}", spec.map_relative, spec.model_name);
        let model = files
            .iter()
            .find(|artifact| artifact.path == model_path)
            .cloned()
            .ok_or_else(|| invalid_error(format!("{} has no model GLB", spec.id)))?;
        let definition = MapCompositeObjectDefinition {
            schema: MAP_COMPOSITE_OBJECT_SCHEMA.to_owned(),
            id: spec.id.to_owned(),
            name: spec.name.to_owned(),
            category: "structures".to_owned(),
            prefix: spec.prefix.to_owned(),
            family: spec.family.to_owned(),
            model,
            visual_scenes: spec.visual_scenes.to_vec(),
            collision_meshes: spec.collision_meshes.to_vec(),
            collision_is_integral: !spec.collision_meshes.is_empty(),
            resource_set: None,
        };
        let definition_relative = format!("{}/object.json", spec.map_relative);
        let definition_bytes = pretty_json(&definition)?;
        write_new(
            &safe_join(staging, &definition_relative)?,
            &definition_bytes,
        )?;
        published.push(MapCompositeObjectCatalogEntry {
            id: spec.id.to_owned(),
            name: spec.name.to_owned(),
            category: "structures".to_owned(),
            prefix: spec.prefix.to_owned(),
            family: spec.family.to_owned(),
            definition: WorldPrefabArtifact {
                path: format!("map/{definition_relative}"),
                bytes: definition_bytes.len() as u64,
                blake3: hash_bytes(&definition_bytes),
            },
            files,
            resource_set: None,
        });
    }
    Ok(published)
}

pub(super) fn publish_map_tile(
    asset_root: &Path,
    staging: &Path,
    source_scope: &str,
    source_tile_id: &str,
    tile_id: &str,
    legacy_id: Option<&str>,
    objects: &WorldPrefabArtifact,
    placements: &[&PlacementDraft],
    objects_by_id: &BTreeMap<&str, &PrefabDraft>,
    resources_by_id: &BTreeMap<&str, &WorldPrefabResource>,
    shared_terrain: &mut SharedTerrainFiles,
) -> Result<WorldPrefabArtifact> {
    let (source_scene, source_terrain, source_behaviour, exclude_scene) = match source_scope {
        "worldMap" => (
            format!("world/maps/{source_tile_id}/scene.json"),
            format!("world/maps/{source_tile_id}/terrain"),
            format!("world/behaviours/worldMap/{source_tile_id}.json"),
            false,
        ),
        "tutorial" => (
            format!("world/tutorial/terrain/tiles/{source_tile_id}/scene.json"),
            format!("world/tutorial/terrain/tiles/{source_tile_id}"),
            format!("world/behaviours/tutorial/{source_tile_id}.json"),
            true,
        ),
        _ => return invalid(format!("unknown source map scope {source_scope:?}")),
    };
    let source_terrain_path = safe_join(asset_root, &source_terrain)?;
    let source_terrain_path = canonical_directory(&source_terrain_path, "source terrain root")?;
    let source_descriptor_path = source_terrain_path.join("terrain.json");
    let source_descriptor =
        read_regular_file(&source_descriptor_path, "source terrain descriptor")?;
    let mut terrain_document: JsonValue =
        serde_json::from_slice(&source_descriptor).map_err(|source| PipelineError::Json {
            path: source_descriptor_path.display().to_string(),
            source,
        })?;
    let mut skipped_terrain_paths = publish_shared_terrain_layers(
        &source_terrain_path,
        staging,
        &mut terrain_document,
        shared_terrain,
    )?;
    skipped_terrain_paths.extend(publish_shared_terrain_detail_textures(
        &source_terrain_path,
        staging,
        &mut terrain_document,
        shared_terrain,
    )?);
    collapse_exact_terrain_base_mip_zero_copies(
        &source_terrain_path,
        &mut terrain_document,
        &mut skipped_terrain_paths,
    )?;
    let mut rewritten_files = BTreeMap::<String, Vec<u8>>::new();
    let source_environment_path = source_terrain_path.join("environment/environment.json");
    if source_environment_path.is_file() {
        let source_environment =
            read_regular_file(&source_environment_path, "source map-tile environment")?;
        let mut environment: JsonValue =
            serde_json::from_slice(&source_environment).map_err(|source| PipelineError::Json {
                path: source_environment_path.display().to_string(),
                source,
            })?;
        let environment_object = environment
            .as_object_mut()
            .ok_or_else(|| invalid_error("source map-tile environment is not an object"))?;
        environment_object.insert("scope".to_owned(), JsonValue::String("worldMap".to_owned()));
        environment_object.insert(
            "tileId".to_owned(),
            JsonValue::String(
                tile_id
                    .strip_prefix("map_")
                    .ok_or_else(|| invalid_error("canonical map tile has no map_ prefix"))?
                    .to_owned(),
            ),
        );
        if let Some(application) = environment.pointer_mut(
            "/runtimeAmbienceContract/defaultAmbienceApplication/tutorialBlendAppliesToThisScope",
        ) {
            *application = JsonValue::Bool(false);
        }
        let bytes = pretty_json(&environment)?;
        if let Some(reference) = terrain_document
            .get_mut("environment")
            .and_then(JsonValue::as_object_mut)
        {
            reference.insert(
                "blake3".to_owned(),
                JsonValue::String(format!("blake3:{}", hash_bytes(&bytes))),
            );
        }
        rewritten_files.insert("environment/environment.json".to_owned(), bytes);
    }
    let rewritten_descriptor = pretty_json(&terrain_document)?;
    let mut files = Vec::<WorldPrefabArtifact>::new();
    for (relative, _, _) in collect_files(&source_terrain_path)? {
        if exclude_scene && relative == "scene.json" {
            continue;
        }
        if skipped_terrain_paths.contains(&relative) {
            continue;
        }
        let bytes = if relative == "terrain.json" {
            rewritten_descriptor.clone()
        } else if let Some(bytes) = rewritten_files.get(&relative) {
            bytes.clone()
        } else {
            let source = safe_join(&source_terrain_path, &relative)?;
            read_regular_file(&source, "source map-tile terrain file")?
        };
        let output_relative = format!("tiles/{tile_id}/terrain/{relative}");
        write_new(&safe_join(staging, &output_relative)?, &bytes)?;
        files.push(WorldPrefabArtifact {
            path: format!("map/{output_relative}"),
            bytes: bytes.len() as u64,
            blake3: hash_bytes(&bytes),
        });
    }
    let terrain_path = format!("map/tiles/{tile_id}/terrain/terrain.json");
    let terrain = files
        .iter()
        .find(|artifact| artifact.path == terrain_path)
        .cloned()
        .ok_or_else(|| invalid_error(format!("{source_terrain:?} has no terrain.json")))?;

    let source_behaviour_path = safe_join(asset_root, &source_behaviour)?;
    let behaviour = if source_behaviour_path.is_file() {
        let source_bytes = read_regular_file(&source_behaviour_path, "source map-tile behaviour")?;
        let mut document: JsonValue =
            serde_json::from_slice(&source_bytes).map_err(|source| PipelineError::Json {
                path: source_behaviour_path.display().to_string(),
                source,
            })?;
        let document_object = document
            .as_object_mut()
            .ok_or_else(|| invalid_error("source map-tile behaviour is not an object"))?;
        document_object.insert("id".to_owned(), JsonValue::String(tile_id.to_owned()));
        document_object.insert("scope".to_owned(), JsonValue::String("worldMap".to_owned()));
        let bytes = pretty_json(&document)?;
        let relative = format!("tiles/{tile_id}/behaviour.json");
        write_new(&safe_join(staging, &relative)?, &bytes)?;
        let artifact = WorldPrefabArtifact {
            path: format!("map/{relative}"),
            bytes: bytes.len() as u64,
            blake3: hash_bytes(&bytes),
        };
        files.push(artifact.clone());
        Some(artifact)
    } else {
        None
    };
    let scene = publish_map_tile_scene(
        asset_root,
        staging,
        &source_scene,
        tile_id,
        &terrain,
        &files,
        placements,
        objects_by_id,
        resources_by_id,
    )?;
    files.push(scene.clone());
    files.push(objects.clone());
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let manifest = MapTileDocument {
        schema: MAP_TILE_SCHEMA.to_owned(),
        source_build: SOURCE_BUILD.to_owned(),
        id: tile_id.to_owned(),
        legacy_aliases: legacy_id.into_iter().map(str::to_owned).collect(),
        grid: parse_tile_grid(tile_id)?,
        coordinate_space: "native world; H=diag(-1,1,1), one Unity unit equals one Bevy unit"
            .to_owned(),
        terrain,
        objects: objects.clone(),
        scene,
        behaviour,
        files,
        source_scene,
    };
    let relative = format!("tiles/{tile_id}/tile.json");
    let bytes = pretty_json(&manifest)?;
    write_new(&safe_join(staging, &relative)?, &bytes)?;
    Ok(WorldPrefabArtifact {
        path: format!("map/{relative}"),
        bytes: bytes.len() as u64,
        blake3: hash_bytes(&bytes),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn publish_map_tile_scene(
    asset_root: &Path,
    staging: &Path,
    source_scene: &str,
    tile_id: &str,
    terrain: &WorldPrefabArtifact,
    tile_files: &[WorldPrefabArtifact],
    placements: &[&PlacementDraft],
    objects_by_id: &BTreeMap<&str, &PrefabDraft>,
    resources_by_id: &BTreeMap<&str, &WorldPrefabResource>,
) -> Result<WorldPrefabArtifact> {
    let source_path = safe_join(asset_root, source_scene)?;
    let source_bytes = read_regular_file(&source_path, "source map scene")?;
    let mut scene: JsonValue =
        serde_json::from_slice(&source_bytes).map_err(|source| PipelineError::Json {
            path: source_path.display().to_string(),
            source,
        })?;
    let scene_object = scene
        .as_object_mut()
        .ok_or_else(|| invalid_error("source map scene is not an object"))?;

    let mut used_resources = BTreeMap::<String, &WorldPrefabResource>::new();
    let mut visuals = Vec::<JsonValue>::new();
    let mut colliders = Vec::<JsonValue>::new();
    for placement in placements {
        let object = objects_by_id
            .get(placement.prefab.as_str())
            .ok_or_else(|| invalid_error("tile placement references absent map object"))?;
        let transform = authored_transform_json(placement.world_matrix, &placement.source_node)?;
        for part in &object.parts {
            let resource = resources_by_id
                .get(part.resource.as_str())
                .ok_or_else(|| invalid_error("map object references absent geometry"))?;
            used_resources.insert(resource.id.clone(), resource);
            match part.role {
                WorldPrefabResourceKind::Visual => visuals.push(serde_json::json!({
                    "name": format!("{} [{} visual]", resource.name, placement.source_node),
                    "model": resource.id,
                    "scene": resource.scene,
                    "sourceModelPath": placement.source_geometry.get(&part.resource)
                        .ok_or_else(|| invalid_error("tile placement has no source geometry route"))?,
                    "legacyLayer": placement.legacy_layer,
                    "transform": transform.clone(),
                })),
                WorldPrefabResourceKind::Collider => colliders.push(serde_json::json!({
                    "name": format!("{} [{} collision]", resource.name, placement.source_node),
                    "kind": "triangleMesh",
                    "model": resource.id,
                    "mesh": resource.mesh,
                    "primitive": resource.primitive,
                    "sourceModelPath": placement.source_geometry.get(&part.resource)
                        .ok_or_else(|| invalid_error("tile placement has no source geometry route"))?,
                    "isTrigger": part.is_trigger,
                    "expectedVertexCount": resource.vertex_count,
                    "expectedIndexCount": resource.index_count,
                    "transform": transform.clone(),
                })),
            }
        }
    }
    let models = used_resources
        .into_values()
        .map(|resource| {
            serde_json::json!({
                "id": resource.id,
                "path": resource.model.path,
                "blake3": resource.model.blake3,
                "rootName": resource.name,
            })
        })
        .collect::<Vec<_>>();
    scene_object.insert("name".to_owned(), JsonValue::String(tile_id.to_owned()));
    scene_object.insert("scope".to_owned(), JsonValue::String("worldMap".to_owned()));
    scene_object.insert(
        "tile".to_owned(),
        serde_json::json!(parse_tile_grid(tile_id)?),
    );
    scene_object.insert("models".to_owned(), JsonValue::Array(models));
    scene_object.insert("visuals".to_owned(), JsonValue::Array(visuals));
    scene_object.insert("colliders".to_owned(), JsonValue::Array(colliders));

    if let Some(native_terrain) = scene_object
        .get_mut("nativeTerrain")
        .and_then(JsonValue::as_object_mut)
    {
        native_terrain.insert("path".to_owned(), JsonValue::String(terrain.path.clone()));
        native_terrain.insert(
            "blake3".to_owned(),
            JsonValue::String(terrain.blake3.clone()),
        );
        if let Some(scene_instance) = tile_files
            .iter()
            .find(|artifact| artifact.path.ends_with("/scene-instance.json"))
        {
            native_terrain.insert(
                "sceneInstancePath".to_owned(),
                JsonValue::String(scene_instance.path.clone()),
            );
            native_terrain.insert(
                "sceneInstanceBlake3".to_owned(),
                JsonValue::String(scene_instance.blake3.clone()),
            );
        }
        if let Some(environment_artifact) = tile_files
            .iter()
            .find(|artifact| artifact.path.ends_with("/environment/environment.json"))
        {
            if let Some(environment) = native_terrain
                .get_mut("environment")
                .and_then(JsonValue::as_object_mut)
            {
                environment.insert(
                    "path".to_owned(),
                    JsonValue::String(environment_artifact.path.clone()),
                );
                environment.insert(
                    "blake3".to_owned(),
                    JsonValue::String(environment_artifact.blake3.clone()),
                );
            }
        }
    }
    let relative = format!("tiles/{tile_id}/scene.json");
    let bytes = pretty_json(&scene)?;
    write_new(&safe_join(staging, &relative)?, &bytes)?;
    Ok(WorldPrefabArtifact {
        path: format!("map/{relative}"),
        bytes: bytes.len() as u64,
        blake3: hash_bytes(&bytes),
    })
}

pub(super) fn publish_shared_map_runtime_assets(
    asset_root: &Path,
    staging: &Path,
    shared: &mut SharedTerrainFiles,
) -> Result<()> {
    for (legacy_relative, output_relative) in [
        ("tutorial/effects", "shared/effects"),
        ("tutorial/projectiles", "shared/projectiles"),
        ("map/shared/environment", "shared/environment"),
    ] {
        let legacy_root = safe_join(asset_root, legacy_relative)?;
        let unified_root = safe_join(&asset_root.join("map"), output_relative)?;
        let source_root = if legacy_root.is_dir() {
            legacy_root
        } else if unified_root.is_dir() {
            unified_root
        } else {
            return invalid(format!(
                "shared map runtime package is absent at both {:?} and {:?}",
                legacy_relative,
                format!("map/{output_relative}")
            ));
        };
        let files = collect_files(&source_root)?;
        for (relative, _, _) in files {
            let source = safe_join(&source_root, &relative)?;
            let mut bytes = read_regular_file(&source, "shared map runtime asset")?;
            if source.extension().and_then(|extension| extension.to_str()) == Some("json") {
                let source_prefix = format!("{legacy_relative}/");
                let target_prefix = format!("map/{output_relative}/");
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    if text.contains(&source_prefix) {
                        bytes = text.replace(&source_prefix, &target_prefix).into_bytes();
                    }
                }
            }
            let target_relative = format!("{output_relative}/{relative}");
            let artifact = WorldPrefabArtifact {
                path: format!("map/{target_relative}"),
                bytes: bytes.len() as u64,
                blake3: hash_bytes(&bytes),
            };
            if shared.artifacts.contains_key(&artifact.path) {
                return invalid(format!(
                    "shared map runtime package collision at {:?}",
                    artifact.path
                ));
            }
            write_new(&safe_join(staging, &target_relative)?, &bytes)?;
            shared.artifacts.insert(artifact.path.clone(), artifact);
        }
    }
    Ok(())
}

pub(super) fn write_vec3_accessor(
    binary: &mut [u8],
    document: &mut JsonValue,
    accessor_index: usize,
    values: &[[f32; 3]],
    path: &str,
) -> Result<()> {
    let layout = accessor_layout(document, accessor_index, path)?;
    if layout.count != values.len() {
        return invalid(format!("{path:?} accessor count changed"));
    }
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    for (index, value) in values.iter().enumerate() {
        let offset = layout.offset + index * layout.stride;
        for axis in 0..3 {
            binary[offset + axis * 4..offset + axis * 4 + 4]
                .copy_from_slice(&value[axis].to_le_bytes());
            minimum[axis] = minimum[axis].min(value[axis]);
            maximum[axis] = maximum[axis].max(value[axis]);
        }
    }
    let accessor = document
        .get_mut("accessors")
        .and_then(JsonValue::as_array_mut)
        .and_then(|accessors| accessors.get_mut(accessor_index))
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| invalid_error(format!("{path:?} accessor is absent")))?;
    if accessor.contains_key("min") {
        accessor.insert("min".to_owned(), serde_json::json!(minimum));
    }
    if accessor.contains_key("max") {
        accessor.insert("max".to_owned(), serde_json::json!(maximum));
    }
    Ok(())
}

pub(super) fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = pretty_json(value)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let temporary = path.with_extension("json.tmp");
    if temporary.exists() {
        return invalid(format!("stale temporary report: {}", temporary.display()));
    }
    fs::write(&temporary, &bytes).map_err(|error| io_at(&temporary, error))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| io_at(path, error))?;
    }
    fs::rename(&temporary, path).map_err(|error| io_at(path, error))?;
    Ok(())
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return invalid(format!(
            "refusing to overwrite generated artifact: {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    fs::write(path, bytes).map_err(|error| io_at(path, error))
}
