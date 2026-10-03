use super::*;

pub(super) fn is_legacy_additive_black_visual(payload: &HierarchyPayload) -> bool {
    let (mesh, material) = LEGACY_ADDITIVE_BLACK_VISUAL;
    payload.kind == "visual"
        && payload.source_mesh.asset == LEGACY_COLLISION_HELPER_ASSET
        && payload.source_mesh.object_type == "Mesh"
        && payload.source_mesh.path_id == mesh
        && payload.material_ids.as_slice()
            == [Some(format!("{LEGACY_COLLISION_HELPER_ASSET}:{material}"))]
}

pub fn organize_world_prefabs(
    options: &WorldPrefabOrganizerOptions,
) -> Result<WorldPrefabOrganizerReport> {
    let project_root = canonical_directory(&options.project_root, "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let report_path = absolute_under_project(&project_root, &options.report_path, "report path")?;
    if report_path.starts_with(&asset_root) {
        return invalid("map organizer report must remain outside the runtime asset tree");
    }
    let world_metadata_root = canonical_directory(
        &asset_root.join("world/maps/static/tiles"),
        "world static metadata root",
    )?;
    let tutorial_metadata_root = match &options.tutorial_metadata_root {
        Some(root) => canonical_directory(
            &absolute_from(&project_root, root),
            "tutorial static metadata root",
        )?,
        None => discover_tutorial_metadata_root(&project_root, &asset_root)?,
    };

    let mut inventory = Inventory::default();
    scan_scope(
        "worldMap",
        &world_metadata_root,
        &asset_root,
        &mut inventory,
    )?;
    scan_scope(
        "tutorial",
        &tutorial_metadata_root,
        &asset_root,
        &mut inventory,
    )?;
    propagate_visual_aliases_to_colliders(&mut inventory.resources);
    inventory.counts.reusable_resources = inventory.resources.len() as u64;
    inventory.counts.prefabs = inventory.prefabs.len() as u64;
    inventory.counts.placements = inventory.placements.len() as u64;
    inventory.counts.visual_resources = inventory
        .resources
        .values()
        .filter(|resource| resource.key.kind == WorldPrefabResourceKind::Visual)
        .count() as u64;
    inventory.counts.collider_resources =
        inventory.counts.reusable_resources - inventory.counts.visual_resources;

    let resource_taxonomy = inventory
        .resources
        .values()
        .map(|resource| (resource.id.clone(), taxonomy(&resource.aliases)))
        .collect::<BTreeMap<_, _>>();
    let mut categories = BTreeMap::<String, u64>::new();
    let mut prefixes = BTreeMap::<String, u64>::new();
    let mut category_samples = BTreeMap::<String, BTreeSet<String>>::new();
    let mut prefix_samples = BTreeMap::<String, BTreeSet<String>>::new();
    for taxonomy in resource_taxonomy.values() {
        *categories.entry(taxonomy.category.clone()).or_default() += 1;
        *prefixes.entry(taxonomy.prefix.clone()).or_default() += 1;
        category_samples
            .entry(taxonomy.category.clone())
            .or_default()
            .insert(taxonomy.name.clone());
        prefix_samples
            .entry(taxonomy.prefix.clone())
            .or_default()
            .insert(taxonomy.name.clone());
    }
    let category_samples = limited_samples(category_samples);
    let prefix_samples = limited_samples(prefix_samples);
    let source_set_blake3 = inventory.source_set.finalize().to_hex().to_string();
    let mut report = WorldPrefabOrganizerReport {
        schema: WORLD_PREFAB_ORGANIZER_REPORT_SCHEMA.to_owned(),
        tool: WORLD_PREFAB_TOOL.to_owned(),
        source_build: SOURCE_BUILD.to_owned(),
        mode: if options.apply {
            WorldPrefabOrganizerMode::Apply
        } else {
            WorldPrefabOrganizerMode::Plan
        },
        project_root: path_text(&project_root),
        asset_root: path_text(&asset_root),
        report_path: project_relative(&project_root, &report_path)?,
        world_metadata_root: project_relative(&project_root, &world_metadata_root)?,
        tutorial_metadata_root: project_relative(&project_root, &tutorial_metadata_root)?,
        catalog_path: WORLD_PREFAB_CATALOG_PATH.to_owned(),
        compatibility_policy: "map objects own local visual and collision geometry; map tiles own terrain, behaviours and object placements; exact primary collision-alpha helpers and the exact additive-black non-presentation visual are excluded while sibling collision geometry is retained; legacy scope names are migration aliases only".to_owned(),
        classification_policy: "ranked legacy GameObject/material aliases; unproven names remain unclassified; prefix/family preserve observed source naming rather than redefining primary ownership".to_owned(),
        counts: inventory.counts.clone(),
        categories,
        prefixes,
        category_samples,
        prefix_samples,
        source_set_blake3: source_set_blake3.clone(),
        result_set_blake3: None,
    };

    if options.apply {
        let target_root = asset_root.join("map");
        if target_root.exists() && !options.replace {
            return invalid(format!(
                "organized map root already exists at {}; replace it only through a reviewed map migration",
                target_root.display()
            ));
        }
        if target_root.exists() {
            validate_replacement_target(&target_root)?;
        }
        let staging = asset_root.join(format!(
            ".map-stage-{}-{}",
            std::process::id(),
            short_hash(source_set_blake3.as_bytes())
        ));
        if staging.exists() {
            return invalid(format!("stale map staging root: {}", staging.display()));
        }
        fs::create_dir(&staging).map_err(|error| io_at(&staging, error))?;
        let apply_result = build_library(
            &asset_root,
            &staging,
            &inventory,
            &resource_taxonomy,
            &mut report,
        );
        if let Err(error) = apply_result {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        if target_root.exists() {
            let backup = asset_root.join(format!(
                ".map-replace-backup-{}-{}",
                std::process::id(),
                short_hash(source_set_blake3.as_bytes())
            ));
            if backup.exists() {
                let _ = fs::remove_dir_all(&staging);
                return invalid(format!(
                    "stale map replacement backup: {}",
                    backup.display()
                ));
            }
            fs::rename(&target_root, &backup).map_err(|error| io_at(&target_root, error))?;
            if let Err(error) = fs::rename(&staging, &target_root) {
                let _ = fs::rename(&backup, &target_root);
                return Err(io_at(&target_root, error));
            }
            fs::remove_dir_all(&backup).map_err(|error| io_at(&backup, error))?;
        } else {
            fs::rename(&staging, &target_root).map_err(|error| io_at(&target_root, error))?;
        }
    }

    write_json_atomic(&report_path, &report)?;
    Ok(report)
}

pub(super) fn verify_artifact(
    asset_root: &Path,
    artifact: &WorldPrefabArtifact,
    label: &str,
) -> Result<Vec<u8>> {
    let path = safe_join(asset_root, &artifact.path)?;
    let bytes = read_regular_file(&path, label)?;
    if bytes.len() as u64 != artifact.bytes || hash_bytes(&bytes) != artifact.blake3 {
        return invalid(format!(
            "{label} differs from catalog proof: {}",
            artifact.path
        ));
    }
    Ok(bytes)
}

pub(super) fn verify_resource_set_artifact(
    asset_root: &Path,
    artifact: &crate::ResourceSetArtifact,
    label: &str,
) -> Result<Vec<u8>> {
    let path = safe_join(asset_root, &artifact.path)?;
    let bytes = read_regular_file(&path, label)?;
    if bytes.len() as u64 != artifact.bytes || hash_bytes(&bytes) != artifact.blake3 {
        return invalid(format!(
            "{label} differs from resource-set proof: {}",
            artifact.path
        ));
    }
    Ok(bytes)
}

pub(super) fn verify_local_resource_contract(resource: &WorldPrefabResource, local: &ParsedGlb) -> Result<()> {
    let (mesh, primitive, vertex_count, index_count) =
        geometry_layout(&local.document, &resource.model.path)?;
    validate_direct_mesh_runtime_contract(&local.document, &resource.name, &resource.model.path)?;
    if mesh != resource.mesh
        || primitive != resource.primitive
        || vertex_count != resource.vertex_count
        || index_count != resource.index_count
    {
        return invalid(format!(
            "resource {} local geometry layout differs from its catalog proof",
            resource.id
        ));
    }
    let accessors = position_accessors(&local.document, &resource.model.path)?;
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    for accessor in accessors {
        for value in read_vec3_accessor(
            &local.binary,
            &local.document,
            accessor,
            &resource.model.path,
        )? {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(value[axis]);
                maximum[axis] = maximum[axis].max(value[axis]);
            }
        }
    }
    if minimum != resource.bounds.minimum || maximum != resource.bounds.maximum {
        return invalid(format!(
            "resource {} local bounds differ from its catalog proof",
            resource.id
        ));
    }
    parse_string_matrix(&resource.representative_world_matrix)?;
    Ok(())
}

pub(super) fn position_accessors(document: &JsonValue, path: &str) -> Result<BTreeSet<usize>> {
    let meshes = document
        .get("meshes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{path:?} has no meshes")))?;
    let mut accessors = BTreeSet::new();
    for mesh in meshes {
        for primitive in mesh
            .get("primitives")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| invalid_error(format!("{path:?} mesh has no primitives")))?
        {
            let attributes = primitive
                .get("attributes")
                .and_then(JsonValue::as_object)
                .ok_or_else(|| invalid_error(format!("{path:?} primitive has no attributes")))?;
            accessors.insert(required_u64(attributes.get("POSITION"), "POSITION", path)? as usize);
        }
    }
    Ok(accessors)
}

pub(super) fn geometry_layout(document: &JsonValue, path: &str) -> Result<(usize, usize, u64, u64)> {
    let meshes = document
        .get("meshes")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{path:?} has no meshes")))?;
    if meshes.len() != 1 {
        return invalid(format!(
            "{path:?} static payload has {} meshes; one payload must remain one geometry part",
            meshes.len()
        ));
    }
    let primitives = meshes[0]
        .get("primitives")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{path:?} mesh has no primitives")))?;
    if primitives.len() != 1 {
        return invalid(format!(
            "{path:?} static payload has {} primitives; exact collision routing is ambiguous",
            primitives.len()
        ));
    }
    let attributes = primitives[0]
        .get("attributes")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| invalid_error(format!("{path:?} primitive has no attributes")))?;
    let position = required_u64(attributes.get("POSITION"), "POSITION", path)? as usize;
    let indices = required_u64(primitives[0].get("indices"), "indices", path)? as usize;
    let accessors = document
        .get("accessors")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid_error(format!("{path:?} has no accessors")))?;
    let vertex_count = accessors
        .get(position)
        .and_then(|accessor| accessor.get("count"))
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid_error(format!("{path:?} POSITION has no count")))?;
    let index_count = accessors
        .get(indices)
        .and_then(|accessor| accessor.get("count"))
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid_error(format!("{path:?} indices have no count")))?;
    if vertex_count == 0 || index_count == 0 || !index_count.is_multiple_of(3) {
        return invalid(format!("{path:?} has invalid triangle cardinality"));
    }
    Ok((0, 0, vertex_count, index_count))
}

pub(super) fn scan_scope(
    scope: &str,
    metadata_root: &Path,
    asset_root: &Path,
    inventory: &mut Inventory,
) -> Result<()> {
    let mut hierarchy_paths = fs::read_dir(metadata_root)
        .map_err(|error| io_at(metadata_root, error))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path().join("hierarchy.json")))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    hierarchy_paths.sort();
    if hierarchy_paths.is_empty() {
        return invalid(format!(
            "{scope} metadata root contains no hierarchy.json files: {}",
            metadata_root.display()
        ));
    }

    for (tile_index, hierarchy_path) in hierarchy_paths.iter().enumerate() {
        let hierarchy_bytes = read_regular_file(hierarchy_path, "static hierarchy")?;
        let hierarchy: HierarchyDocument =
            serde_json::from_slice(&hierarchy_bytes).map_err(|source| PipelineError::Json {
                path: hierarchy_path.display().to_string(),
                source,
            })?;
        if hierarchy.schema != STATIC_HIERARCHY_SCHEMA || hierarchy.source_build != SOURCE_BUILD {
            return invalid(format!(
                "{} is not the pinned primary static hierarchy",
                hierarchy_path.display()
            ));
        }
        let material_path = hierarchy_path
            .parent()
            .ok_or_else(|| invalid_error("hierarchy has no parent"))?
            .join("materials.json");
        let material_bytes = read_regular_file(&material_path, "static materials")?;
        let materials: MaterialDocument =
            serde_json::from_slice(&material_bytes).map_err(|source| PipelineError::Json {
                path: material_path.display().to_string(),
                source,
            })?;
        append_set_hash(
            &mut inventory.source_set,
            &format!("{scope}/{}/hierarchy.json", hierarchy.tile_id),
            &hash_bytes(&hierarchy_bytes),
        );
        append_set_hash(
            &mut inventory.source_set,
            &format!("{scope}/{}/materials.json", hierarchy.tile_id),
            &hash_bytes(&material_bytes),
        );

        let nodes = hierarchy
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let mut groups =
            BTreeMap::<String, Vec<(String, PrefabPartKey, [[f64; 4]; 4], String)>>::new();
        inventory
            .tiles
            .insert((scope.to_owned(), hierarchy.tile_id.clone()));
        inventory.counts.tiles += 1;
        inventory.counts.source_nodes += hierarchy.nodes.len() as u64;
        inventory.counts.source_payloads += hierarchy.payloads.len() as u64;

        let excluded_nodes = hierarchy
            .payloads
            .iter()
            .filter(|payload| payload_has_runtime_model(payload))
            .filter_map(|payload| {
                let aliases = payload_aliases(payload, &nodes, &materials.materials);
                aliases_look_character_like(&aliases).then_some(payload.hierarchy_node_id.clone())
            })
            .collect::<BTreeSet<_>>();
        inventory.counts.excluded_character_nodes += excluded_nodes.len() as u64;

        for payload in hierarchy
            .payloads
            .iter()
            .filter(|payload| payload_has_runtime_model(payload))
        {
            if excluded_nodes.contains(&payload.hierarchy_node_id) {
                inventory.counts.excluded_character_payloads += 1;
                continue;
            }
            // Primary Unity evidence identifies these exact Mesh+Material
            // pairs as transparent authoring sheets above sibling
            // MeshColliders, never presentation geometry. Filtering by the
            // serialized identities keeps legitimate visible collision props
            // while preventing 1,639 enormous alpha planes from publication.
            if is_legacy_collision_helper_payload(payload) {
                inventory.counts.excluded_collision_helper_visuals += 1;
                continue;
            }
            // Primary Mesh 439 with Material 1847 is One/One additive,
            // ZWrite Off, textureless and contributes exact RGB zero. It is
            // mathematically invisible in Unity; publishing it as ordinary
            // PBR presentation geometry creates 799 camera-angle-dependent
            // black planes. Identity matching avoids the unreliable "NO" name.
            if is_legacy_additive_black_visual(payload) {
                inventory.counts.excluded_additive_black_visuals += 1;
                continue;
            }
            let kind = match payload.kind.as_str() {
                "visual" => WorldPrefabResourceKind::Visual,
                "collider" => WorldPrefabResourceKind::Collider,
                other => {
                    return invalid(format!(
                        "{} payload {} has unknown kind {other:?}",
                        hierarchy.tile_id, payload.id
                    ));
                }
            };
            match kind {
                WorldPrefabResourceKind::Visual => inventory.counts.source_visual_instances += 1,
                WorldPrefabResourceKind::Collider => {
                    inventory.counts.source_collider_instances += 1
                }
            }
            let model_path = payload.model_path.as_deref().expect("filtered model path");
            validate_relative(model_path)?;
            let baked_path = safe_join(asset_root, model_path)?;
            let baked_bytes = read_regular_file(&baked_path, "published baked model")?;
            let baked_hash = hash_bytes(&baked_bytes);
            append_set_hash(&mut inventory.source_set, model_path, &baked_hash);

            let key = ResourceKey {
                kind,
                source_asset: payload.source_mesh.asset.clone(),
                source_path_id: payload.source_mesh.path_id,
                material_ids: if kind == WorldPrefabResourceKind::Visual {
                    payload.material_ids.clone()
                } else {
                    Vec::new()
                },
            };
            let id = resource_id(&key)?;
            let aliases = payload_aliases(payload, &nodes, &materials.materials);
            let incoming_score = representative_score(&aliases);
            let resource =
                inventory
                    .resources
                    .entry(key.clone())
                    .or_insert_with(|| ResourceDraft {
                        id: id.clone(),
                        key,
                        aliases: BTreeSet::new(),
                        representative: payload.clone(),
                        representative_tile: hierarchy.tile_id.clone(),
                        baked_hash: baked_hash.clone(),
                        occurrences: 0,
                    });
            let previous_score = representative_score(&resource.aliases);
            resource.occurrences += 1;
            resource.aliases.extend(aliases.iter().cloned());
            if incoming_score > previous_score {
                resource.representative = payload.clone();
                resource.representative_tile = hierarchy.tile_id.clone();
                resource.baked_hash = baked_hash;
            }
            groups
                .entry(payload.hierarchy_node_id.clone())
                .or_default()
                .push((
                    id.clone(),
                    PrefabPartKey {
                        resource: id,
                        role: kind,
                        is_trigger: payload.is_trigger,
                    },
                    payload.world_matrix,
                    model_path.to_owned(),
                ));
        }

        for (node_id, mut parts) in groups {
            parts.sort_by(|left, right| left.1.cmp(&right.1));
            let world_matrix = parts[0].2;
            if parts.iter().any(|part| !matrix_close(part.2, world_matrix)) {
                return invalid(format!(
                    "{} node {node_id:?} has payloads with different world matrices; compound local offsets need explicit exporter evidence",
                    hierarchy.tile_id
                ));
            }
            let part_key = parts.iter().map(|part| part.1.clone()).collect::<Vec<_>>();
            let source_geometry = parts
                .iter()
                .map(|part| (part.0.clone(), part.3.clone()))
                .collect::<BTreeMap<_, _>>();
            let prefab_id = prefab_id(&part_key)?;
            let aliases = node_aliases(&node_id, &nodes);
            let legacy_layer = nodes.get(node_id.as_str()).and_then(|node| node.layer);
            let prefab = inventory
                .prefabs
                .entry(part_key.clone())
                .or_insert_with(|| PrefabDraft {
                    id: prefab_id.clone(),
                    parts: part_key,
                    aliases: BTreeSet::new(),
                    source_nodes: BTreeSet::new(),
                    occurrences: 0,
                });
            prefab.aliases.extend(aliases);
            prefab.source_nodes.insert(node_id.clone());
            prefab.occurrences += 1;
            inventory.placements.push(PlacementDraft {
                scope: scope.to_owned(),
                tile_id: hierarchy.tile_id.clone(),
                prefab: prefab_id,
                source_node: node_id,
                source_geometry,
                legacy_layer,
                world_matrix,
            });
        }
        eprintln!(
            "map-object inventory {scope}: {}/{} tiles ({})",
            tile_index + 1,
            hierarchy_paths.len(),
            hierarchy.tile_id
        );
    }
    Ok(())
}
