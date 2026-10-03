use super::*;

/// Repackages the already-published native map. The operation stages the new
/// object tree before replacing the old one and refreshes every map-owned hash.
pub fn organize_resource_sets(
    project_root: impl AsRef<Path>,
) -> Result<ResourceSetOrganizerReport> {
    let project_root = canonical_directory(project_root.as_ref(), "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let map_root = canonical_directory(&asset_root.join("map"), "map root")?;
    let object_root = canonical_directory(&map_root.join("objects"), "map object root")?;
    let catalog_path = map_root.join("catalog.json");
    let mut catalog = read_json(&catalog_path, "map catalog")?;
    if catalog
        .get("resourceSets")
        .and_then(JsonValue::as_array)
        .is_some_and(|sets| !sets.is_empty())
    {
        return invalid("map is already organized into resource sets");
    }

    let mut objects = load_map_objects(&object_root)?;
    if objects.is_empty() {
        return invalid("map object tree contains no object.json definitions");
    }
    objects.sort_by(|left, right| left.old_relative.cmp(&right.old_relative));
    let (groups, texture_users) = texture_groups(&objects);
    let sets = plan_sets(&objects, groups)?;

    let stage = map_root.join(format!(
        ".objects-resource-sets-stage-{}",
        std::process::id()
    ));
    if fs::symlink_metadata(&stage).is_ok() {
        fs::remove_dir_all(&stage).map_err(|error| io_at(&stage, error))?;
    }
    fs::create_dir(&stage).map_err(|error| io_at(&stage, error))?;

    let mut path_map = BTreeMap::<String, String>::new();
    let mut object_set_ids = BTreeMap::<String, String>::new();
    let mut texture_destinations = BTreeMap::<String, TextureDestination>::new();
    let mut set_texture_paths = BTreeMap::<String, Vec<String>>::new();
    let mut set_member_paths = BTreeMap::<String, Vec<(usize, String)>>::new();

    for set in &sets {
        let set_rooted = format!("map/objects/{}", set.relative);
        let mut names = BTreeMap::<String, String>::new();
        let mut used_names = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        for &member in &set.members {
            for texture in &objects[member].textures {
                hashes.insert(texture.hash.clone());
            }
        }
        for hash in hashes {
            let uses = texture_users
                .get(&hash)
                .ok_or_else(|| invalid_error("texture group lost its usage records"))?;
            let base = semantic_texture_name(uses, &set.name);
            let name = allocate_name(&base, &mut used_names);
            names.insert(hash, name);
        }

        let mut texture_paths = Vec::new();
        for &member in &set.members {
            for texture in &objects[member].textures {
                let name = names
                    .get(&texture.hash)
                    .ok_or_else(|| invalid_error("texture destination name is absent"))?;
                let destination = format!("{set_rooted}/textures/{name}.png");
                texture_destinations
                    .entry(texture.hash.clone())
                    .or_insert_with(|| TextureDestination {
                        rooted_path: destination.clone(),
                        source: texture.absolute.clone(),
                    });
                path_map.insert(
                    asset_relative(&asset_root, &texture.absolute)?,
                    destination.clone(),
                );
                if !texture_paths.contains(&destination) {
                    texture_paths.push(destination);
                }
            }
        }
        texture_paths.sort();
        set_texture_paths.insert(set.id.clone(), texture_paths);
    }

    // Copy each base atlas once, then preserve any exact published mip chain
    // under the same semantic texture name.
    let destinations = texture_destinations.values().cloned().collect::<Vec<_>>();
    for destination in destinations {
        let output = stage_path_for_rooted(&stage, &destination.rooted_path)?;
        copy_new_or_equal(&destination.source, &output)?;
        copy_texture_mips(
            &asset_root,
            &stage,
            &destination.source,
            &destination.rooted_path,
            &mut path_map,
        )?;
    }

    for set in &sets {
        let mut member_names = BTreeSet::new();
        let mut members = Vec::new();
        for &index in &set.members {
            let object = &mut objects[index];
            let leaf = object
                .old_relative
                .rsplit('/')
                .next()
                .map(|value| safe_slug(value, "object"))
                .unwrap_or_else(|| "object".to_owned());
            let member_name = allocate_name(&leaf, &mut member_names);
            let member_relative = format!("{}/objects/{member_name}", set.relative);
            let member_rooted = format!("map/objects/{member_relative}");
            let old_rooted = format!("map/objects/{}", object.old_relative);
            object_set_ids.insert(object.id.clone(), set.id.clone());

            if let Some(map) = object.definition.as_object_mut() {
                map.insert("resourceSet".to_owned(), JsonValue::String(set.id.clone()));
            }
            path_map.insert(
                format!("{old_rooted}/object.json"),
                format!("{member_rooted}/object.json"),
            );

            let texture_roots = object_texture_roots(object);
            for source in &object.files {
                let relative = source
                    .strip_prefix(&object_root)
                    .map_err(|_| invalid_error("map object file escaped its root"))?;
                if relative.file_name().and_then(|name| name.to_str()) == Some("object.json")
                    || is_owned_texture_file(source, &texture_roots)
                {
                    continue;
                }
                let within = source
                    .strip_prefix(object_root.join(&object.old_relative))
                    .map_err(|_| invalid_error("map object file escaped its package"))?;
                let output = stage.join(&member_relative).join(within);
                let old_rooted_file = format!("map/objects/{}", slash_path(relative));
                let new_rooted_file = format!("{member_rooted}/{}", slash_path(within));
                if source.extension().and_then(|value| value.to_str()) == Some("glb") {
                    let bytes = rewrite_glb_uris(source, &new_rooted_file, &path_map, &asset_root)?;
                    write_new(&output, &bytes)?;
                } else {
                    copy_new_or_equal(source, &output)?;
                }
                path_map.insert(old_rooted_file, new_rooted_file);
            }
            replace_paths(&mut object.definition, &path_map);
            refresh_staged_artifact_objects(&mut object.definition, &asset_root, &stage)?;
            let definition_bytes = pretty_json(&object.definition)?;
            let definition_output = stage.join(&member_relative).join("object.json");
            write_new(&definition_output, &definition_bytes)?;
            members.push((index, member_rooted));
        }
        set_member_paths.insert(set.id.clone(), members);
    }

    let mut set_entries = Vec::<ResourceSetCatalogEntry>::new();
    for set in &sets {
        let texture_paths = set_texture_paths
            .get(&set.id)
            .ok_or_else(|| invalid_error("set texture list is absent"))?;
        let textures = texture_paths
            .iter()
            .map(|path| artifact_from_staged(&stage, path))
            .collect::<Result<Vec<_>>>()?;
        let mut members = Vec::new();
        for &(index, ref rooted) in set_member_paths
            .get(&set.id)
            .ok_or_else(|| invalid_error("set member list is absent"))?
        {
            let object = &objects[index];
            let definition = artifact_from_staged(&stage, &format!("{rooted}/object.json"))?;
            let mut files = collect_staged_artifacts(&stage, rooted)?;
            files.retain(|file| file.path != definition.path);
            members.push(ResourceSetMember {
                id: object.id.clone(),
                name: object.name.clone(),
                definition,
                files,
            });
        }
        members.sort_by(|left, right| left.id.cmp(&right.id));
        let document = ResourceSetDocument {
            schema: RESOURCE_SET_SCHEMA.to_owned(),
            id: set.id.clone(),
            name: set.name.clone(),
            domain: "map_object".to_owned(),
            category: set.category.clone(),
            prefix: set.prefix.clone(),
            family: set.family.clone(),
            textures,
            members,
        };
        let relative = format!("{}/set.json", set.relative);
        let bytes = pretty_json(&document)?;
        write_new(&stage.join(&relative), &bytes)?;
        set_entries.push(ResourceSetCatalogEntry {
            id: set.id.clone(),
            name: set.name.clone(),
            category: set.category.clone(),
            prefix: set.prefix.clone(),
            family: set.family.clone(),
            definition: ResourceSetArtifact {
                path: format!("map/objects/{relative}"),
                bytes: bytes.len() as u64,
                blake3: hash_bytes(&bytes),
            },
            member_count: set.members.len() as u64,
            texture_count: set_texture_paths.get(&set.id).map_or(0, Vec::len) as u64,
        });
    }
    set_entries.sort_by(|left, right| left.definition.path.cmp(&right.definition.path));

    // Replace only after the complete object-set tree exists.
    let backup = asset_root.join(format!(
        ".objects-before-resource-sets-{}",
        std::process::id()
    ));
    let mut json_backups = BTreeMap::<PathBuf, Vec<u8>>::new();
    json_backups.insert(
        catalog_path.clone(),
        fs::read(&catalog_path).map_err(|error| io_at(&catalog_path, error))?,
    );
    for path in collect_files(&map_root.join("tiles"))?
        .into_iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
    {
        json_backups.insert(
            path.clone(),
            fs::read(&path).map_err(|error| io_at(&path, error))?,
        );
    }
    fs::rename(&object_root, &backup).map_err(|error| io_at(&object_root, error))?;
    if let Err(error) = fs::rename(&stage, &map_root.join("objects")) {
        let _ = fs::rename(&backup, &object_root);
        return Err(io_at(map_root.join("objects"), error));
    }

    let update_result = (|| -> Result<()> {
        refresh_map_json_references(&asset_root, &map_root, &path_map)?;
        update_catalog(
            &asset_root,
            &mut catalog,
            &path_map,
            &object_set_ids,
            &set_entries,
        )?;
        let bytes = pretty_json(&catalog)?;
        write_replace(&catalog_path, &bytes)?;
        Ok(())
    })();
    if let Err(error) = update_result {
        let broken = asset_root.join(format!(
            ".objects-resource-sets-broken-{}",
            std::process::id()
        ));
        let _ = fs::rename(map_root.join("objects"), &broken);
        let _ = fs::rename(&backup, &object_root);
        for (path, bytes) in json_backups {
            let _ = write_replace(&path, &bytes);
        }
        let _ = fs::remove_dir_all(&broken);
        return Err(error);
    }

    let shared_texture_root = map_root.join("shared/textures");
    let textures_before = if shared_texture_root.is_dir() {
        collect_files(&shared_texture_root)?.len() as u64
    } else {
        0
    };
    let shared_texture_backup = asset_root.join(format!(
        ".map-shared-textures-before-resource-sets-{}",
        std::process::id()
    ));
    if shared_texture_root.is_dir() {
        fs::rename(&shared_texture_root, &shared_texture_backup)
            .map_err(|error| io_at(&shared_texture_root, error))?;
    }
    if let Err(error) = crate::verify_world_prefab_library(&project_root) {
        let broken = asset_root.join(format!(
            ".objects-resource-sets-broken-{}",
            std::process::id()
        ));
        let _ = fs::rename(map_root.join("objects"), &broken);
        let _ = fs::rename(&backup, &object_root);
        if shared_texture_backup.is_dir() {
            let _ = fs::rename(&shared_texture_backup, &shared_texture_root);
        }
        for (path, bytes) in json_backups {
            let _ = write_replace(&path, &bytes);
        }
        let _ = fs::remove_dir_all(&broken);
        return Err(error);
    }
    if shared_texture_backup.is_dir() {
        fs::remove_dir_all(&shared_texture_backup)
            .map_err(|error| io_at(&shared_texture_backup, error))?;
    }
    fs::remove_dir_all(&backup).map_err(|error| io_at(&backup, error))?;

    let catalog_bytes = fs::read(&catalog_path).map_err(|error| io_at(&catalog_path, error))?;
    let textures_after = set_entries
        .iter()
        .map(|entry| entry.texture_count)
        .sum::<u64>();
    Ok(ResourceSetOrganizerReport {
        schema: RESOURCE_SET_ORGANIZER_REPORT_SCHEMA.to_owned(),
        map_sets: set_entries.len() as u64,
        map_objects: objects.len() as u64,
        map_textures_before: textures_before,
        map_textures_after: textures_after,
        duplicate_texture_files_removed: textures_before.saturating_sub(textures_after),
        catalog: ResourceSetArtifact {
            path: "map/catalog.json".to_owned(),
            bytes: catalog_bytes.len() as u64,
            blake3: hash_bytes(&catalog_bytes),
        },
    })
}

/// Repackages player equipment and appearance textures using the same set
/// ownership rule as map objects. Main atlases drive grouping; renderer-wide
/// toon ramps are deduplicated separately and never collapse all items into a
/// single artificial set.
pub fn organize_player_item_sets(
    project_root: impl AsRef<Path>,
) -> Result<PlayerItemSetOrganizerReport> {
    let project_root = canonical_directory(project_root.as_ref(), "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let player_root = canonical_directory(&asset_root.join("characters/player"), "player root")?;
    if player_root.join("items/catalog.json").is_file() {
        return normalize_existing_player_item_sets(&asset_root);
    }
    let equipment_root = canonical_directory(&player_root.join("equipment"), "equipment root")?;

    let (mut models, mut textures) = load_player_models(&asset_root, &equipment_root)?;
    if models.is_empty() {
        return invalid("player equipment contains no GLB models");
    }
    attach_avatar_texture_routes(&asset_root, &mut models, &mut textures)?;
    let external_roots = [
        player_root.join("shared/runtime-textures"),
        player_root.join("hnpc-runtime-textures"),
    ];
    for root in &external_roots {
        if root.is_dir() {
            for file in collect_files(root)?
                .into_iter()
                .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
            {
                register_player_texture(&mut textures, &file, file_stem(&file)?, false)?;
            }
        }
    }
    models.sort_by(|left, right| left.old_rooted.cmp(&right.old_rooted));

    let mut union = UnionFind::new(models.len());
    let mut first_by_texture = BTreeMap::<String, usize>::new();
    for (index, model) in models.iter().enumerate() {
        for hash in &model.atlas_hashes {
            if let Some(other) = first_by_texture.get(hash) {
                union.join(index, *other);
            } else {
                first_by_texture.insert(hash.clone(), index);
            }
        }
    }
    let mut grouped = BTreeMap::<String, Vec<usize>>::new();
    for (index, model) in models.iter().enumerate() {
        let key = if model.atlas_hashes.is_empty() {
            format!(
                "empty/{}/{}",
                model.category,
                strip_variant(&model.true_name)
            )
        } else {
            format!("atlas/{}", union.find(index))
        };
        grouped.entry(key).or_default().push(index);
    }
    let mut groups = grouped.into_values().collect::<Vec<_>>();
    groups.sort_by(|left, right| models[left[0]].old_rooted.cmp(&models[right[0]].old_rooted));

    let mut plans = Vec::<PlayerSetPlan>::new();
    let mut allocated = BTreeMap::<String, BTreeSet<String>>::new();
    for members in groups {
        let categories = members
            .iter()
            .map(|index| models[*index].category.clone())
            .collect::<BTreeSet<_>>();
        let category = one_or(&categories, "collections");
        let names = members
            .iter()
            .map(|index| models[*index].true_name.clone())
            .collect::<Vec<_>>();
        let candidate = common_semantic_suffix(&names)
            .unwrap_or_else(|| safe_slug(&models[members[0]].true_name, "item_set"));
        let name = allocate_name(&candidate, allocated.entry(category.clone()).or_default());
        let mut ids = members
            .iter()
            .map(|index| models[*index].old_rooted.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        let id = format!("player-item-set-{}", hash_bytes(ids.join("\n").as_bytes()));
        let texture_hashes = members
            .iter()
            .flat_map(|index| models[*index].atlas_hashes.iter().cloned())
            .collect();
        plans.push(PlayerSetPlan {
            id,
            name: name.clone(),
            category: category.clone(),
            relative: format!("items/{category}/{name}"),
            members,
            texture_hashes,
        });
    }

    let mut model_set = BTreeMap::<usize, usize>::new();
    let mut texture_set = BTreeMap::<String, usize>::new();
    for (set_index, set) in plans.iter().enumerate() {
        for member in &set.members {
            model_set.insert(*member, set_index);
        }
        for hash in &set.texture_hashes {
            texture_set.insert(hash.clone(), set_index);
        }
    }
    let orphan_hashes = textures
        .iter()
        .filter(|(_, texture)| !texture.support)
        .map(|(hash, _)| hash.clone())
        .filter(|hash| !texture_set.contains_key(hash))
        .collect::<Vec<_>>();
    for hash in orphan_hashes {
        let texture = textures
            .get(&hash)
            .ok_or_else(|| invalid_error("orphan player texture disappeared"))?;
        if let Some(set_index) = best_matching_player_set(texture, &plans, &models) {
            plans[set_index].texture_hashes.insert(hash.clone());
            texture_set.insert(hash, set_index);
            continue;
        }
        let stem = texture
            .names
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| "appearance".to_owned());
        let category = appearance_category(&stem);
        let base = appearance_family(&stem);
        let name = allocate_name(
            &base,
            allocated
                .entry(format!("appearance/{category}"))
                .or_default(),
        );
        let id = format!("player-appearance-set-{}", hash_bytes(hash.as_bytes()));
        let index = plans.len();
        plans.push(PlayerSetPlan {
            id,
            name: name.clone(),
            category: category.clone(),
            relative: format!("appearance/{category}/{name}"),
            members: Vec::new(),
            texture_hashes: BTreeSet::from([hash.clone()]),
        });
        texture_set.insert(hash, index);
    }

    let stage = player_root.join(format!(".item-resource-sets-stage-{}", std::process::id()));
    if fs::symlink_metadata(&stage).is_ok() {
        fs::remove_dir_all(&stage).map_err(|error| io_at(&stage, error))?;
    }
    fs::create_dir(&stage).map_err(|error| io_at(&stage, error))?;
    let mut path_map = BTreeMap::<String, String>::new();
    let mut set_texture_artifacts = BTreeMap::<String, Vec<ResourceSetArtifact>>::new();

    for plan in &plans {
        let mut names = BTreeSet::new();
        let mut artifacts = Vec::new();
        for hash in &plan.texture_hashes {
            let texture = textures
                .get(hash)
                .ok_or_else(|| invalid_error("player set references absent texture"))?;
            let candidates = texture.names.iter().cloned().collect::<Vec<_>>();
            let base = common_semantic_suffix(&candidates)
                .unwrap_or_else(|| safe_slug(&plan.name, "atlas"));
            let name = allocate_name(&base, &mut names);
            let rooted = format!("characters/player/{}/textures/{name}.png", plan.relative);
            let source = texture
                .sources
                .iter()
                .next()
                .ok_or_else(|| invalid_error("player texture has no source file"))?;
            let output = player_stage_path(&stage, &rooted)?;
            copy_new_or_equal(source, &output)?;
            for alias in &texture.sources {
                path_map.insert(asset_relative(&asset_root, alias)?, rooted.clone());
            }
            for alias in &texture.sources {
                copy_player_texture_mips(&asset_root, &stage, alias, &rooted, &mut path_map)?;
            }
            artifacts.push(artifact_from_player_stage(&stage, &rooted)?);
        }
        artifacts.sort_by(|left, right| left.path.cmp(&right.path));
        set_texture_artifacts.insert(plan.id.clone(), artifacts);
    }

    let mut rendering_artifacts = Vec::new();
    let mut rendering_names = BTreeSet::new();
    for texture in textures.values().filter(|texture| texture.support) {
        let candidates = texture.names.iter().cloned().collect::<Vec<_>>();
        let base = common_semantic_suffix(&candidates).unwrap_or_else(|| "toon_ramp".to_owned());
        let name = allocate_name(&base, &mut rendering_names);
        let rooted = format!("characters/player/rendering/textures/{name}.png");
        let source = texture
            .sources
            .iter()
            .next()
            .ok_or_else(|| invalid_error("rendering texture has no source"))?;
        copy_new_or_equal(source, &player_stage_path(&stage, &rooted)?)?;
        for alias in &texture.sources {
            path_map.insert(asset_relative(&asset_root, alias)?, rooted.clone());
        }
        rendering_artifacts.push(artifact_from_player_stage(&stage, &rooted)?);
    }
    rendering_artifacts.sort_by(|left, right| left.path.cmp(&right.path));

    let mut member_records = BTreeMap::<String, Vec<ResourceSetMember>>::new();
    let mut catalog_models = Vec::<PlayerItemCatalogModel>::new();
    let mut member_names = BTreeMap::<String, BTreeSet<String>>::new();
    let mut conversion_reports_removed = 0u64;
    for (index, model) in models.iter().enumerate() {
        let set_index = *model_set
            .get(&index)
            .ok_or_else(|| invalid_error("player model has no resource set"))?;
        let plan = &plans[set_index];
        let model_name = allocate_name(
            &model.true_name,
            member_names.entry(plan.id.clone()).or_default(),
        );
        let member_relative = format!("{}/models/{model_name}", plan.relative);
        let model_rooted = format!("characters/player/{member_relative}/model.glb");
        let model_output = player_stage_path(&stage, &model_rooted)?;
        let bytes = rewrite_glb_uris(&model.old_glb, &model_rooted, &path_map, &asset_root)?;
        write_new(&model_output, &bytes)?;
        path_map.insert(model.old_rooted.clone(), model_rooted.clone());

        let texture_roots = player_model_texture_roots(model, &textures);
        let mut files = Vec::new();
        for source in &model.files {
            if source == &model.old_glb || texture_roots.iter().any(|root| source.starts_with(root))
            {
                continue;
            }
            if source
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.ends_with(".publish.json"))
            {
                conversion_reports_removed += 1;
                continue;
            }
            let within = source
                .strip_prefix(&model.directory)
                .map_err(|_| invalid_error("player model sidecar escaped package"))?;
            let rooted = format!("characters/player/{member_relative}/{}", slash_path(within));
            copy_new_or_equal(source, &player_stage_path(&stage, &rooted)?)?;
            path_map.insert(asset_relative(&asset_root, source)?, rooted.clone());
            let bytes = fs::read(source).map_err(|error| io_at(source, error))?;
            files.push(ResourceSetArtifact {
                path: rooted,
                bytes: bytes.len() as u64,
                blake3: hash_bytes(&bytes),
            });
        }
        let model_artifact = artifact_from_player_stage(&stage, &model_rooted)?;
        files.push(model_artifact.clone());
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let item_id = format!("player-item-{}", hash_bytes(model.old_rooted.as_bytes()));
        let definition = PlayerItemDefinition {
            schema: PLAYER_ITEM_SCHEMA.to_owned(),
            id: item_id.clone(),
            true_name: model.true_name.clone(),
            category: model.category.clone(),
            source_route: model.source_route.clone(),
            resource_set: plan.id.clone(),
            model: model_artifact,
        };
        catalog_models.push(PlayerItemCatalogModel {
            category: definition.category.clone(),
            true_name: definition.true_name.clone(),
            source_route: definition.source_route.clone(),
            resource_set: definition.resource_set.clone(),
            model: definition.model.clone(),
        });
        let definition_rooted = format!("characters/player/{member_relative}/item.json");
        let definition_bytes = pretty_json(&definition)?;
        write_new(
            &player_stage_path(&stage, &definition_rooted)?,
            &definition_bytes,
        )?;
        member_records
            .entry(plan.id.clone())
            .or_default()
            .push(ResourceSetMember {
                id: item_id,
                name: model.true_name.clone(),
                definition: ResourceSetArtifact {
                    path: definition_rooted,
                    bytes: definition_bytes.len() as u64,
                    blake3: hash_bytes(&definition_bytes),
                },
                files,
            });
    }

    let mut catalog_sets = Vec::new();
    for plan in &plans {
        let mut members = member_records.remove(&plan.id).unwrap_or_default();
        members.sort_by(|left, right| left.definition.path.cmp(&right.definition.path));
        let textures = set_texture_artifacts.remove(&plan.id).unwrap_or_default();
        let document = ResourceSetDocument {
            schema: RESOURCE_SET_SCHEMA.to_owned(),
            id: plan.id.clone(),
            name: plan.name.clone(),
            domain: "player_item".to_owned(),
            category: plan.category.clone(),
            prefix: "PLAYER".to_owned(),
            family: plan.category.to_ascii_uppercase(),
            textures,
            members,
        };
        let rooted = format!("characters/player/{}/set.json", plan.relative);
        let bytes = pretty_json(&document)?;
        write_new(&player_stage_path(&stage, &rooted)?, &bytes)?;
        catalog_sets.push(ResourceSetCatalogEntry {
            id: plan.id.clone(),
            name: plan.name.clone(),
            category: plan.category.clone(),
            prefix: "PLAYER".to_owned(),
            family: plan.category.to_ascii_uppercase(),
            definition: ResourceSetArtifact {
                path: rooted,
                bytes: bytes.len() as u64,
                blake3: hash_bytes(&bytes),
            },
            member_count: plan.members.len() as u64,
            texture_count: plan.texture_hashes.len() as u64,
        });
    }
    catalog_sets.sort_by(|left, right| left.definition.path.cmp(&right.definition.path));
    sort_player_catalog_models(&mut catalog_models);
    let catalog = PlayerItemSetCatalog {
        schema: PLAYER_ITEM_SET_CATALOG_SCHEMA.to_owned(),
        sets: catalog_sets.clone(),
        models: catalog_models,
        rendering_textures: rendering_artifacts.clone(),
    };
    let catalog_bytes = pretty_json(&catalog)?;
    let catalog_rooted = "characters/player/items/catalog.json";
    write_new(&player_stage_path(&stage, catalog_rooted)?, &catalog_bytes)?;

    let json_files = player_reference_json_files(&asset_root)?;
    let mut json_backups = BTreeMap::<PathBuf, Vec<u8>>::new();
    for path in &json_files {
        json_backups.insert(
            path.clone(),
            fs::read(path).map_err(|error| io_at(path, error))?,
        );
    }
    let backup_root = asset_root.join(format!(
        ".player-items-before-resource-sets-{}",
        std::process::id()
    ));
    fs::create_dir(&backup_root).map_err(|error| io_at(&backup_root, error))?;
    let mut moved = Vec::<(PathBuf, PathBuf)>::new();
    for source in [&equipment_root, &external_roots[0], &external_roots[1]] {
        if source.is_dir() {
            let backup = backup_root.join(
                source
                    .file_name()
                    .ok_or_else(|| invalid_error("player source has no filename"))?,
            );
            fs::rename(source, &backup).map_err(|error| io_at(source, error))?;
            moved.push((source.to_path_buf(), backup));
        }
    }
    let mut installed = Vec::new();
    for name in ["items", "appearance", "rendering"] {
        let source = stage.join(name);
        if source.is_dir() {
            let destination = player_root.join(name);
            if fs::symlink_metadata(&destination).is_ok() {
                return invalid(format!(
                    "player item destination already exists: {destination:?}"
                ));
            }
            fs::rename(&source, &destination).map_err(|error| io_at(&destination, error))?;
            installed.push(destination);
        }
    }
    let apply_result = (|| -> Result<()> {
        for path in &json_files {
            let mut value = read_json(path, "player asset reference JSON")?;
            replace_paths(&mut value, &path_map);
            refresh_artifact_objects(&mut value, &asset_root)?;
            let bytes = pretty_json(&value)?;
            write_replace(path, &bytes)?;
        }
        update_player_catalog_provenance(
            &asset_root,
            &ResourceSetArtifact {
                path: catalog_rooted.to_owned(),
                bytes: catalog_bytes.len() as u64,
                blake3: hash_bytes(&catalog_bytes),
            },
        )?;
        verify_player_item_sets_at_asset_root(&asset_root)?;
        Ok(())
    })();
    if let Err(error) = apply_result {
        for destination in installed {
            let _ = fs::remove_dir_all(destination);
        }
        for (destination, backup) in moved {
            let _ = fs::rename(backup, destination);
        }
        for (path, bytes) in json_backups {
            let _ = write_replace(&path, &bytes);
        }
        let _ = fs::remove_dir_all(&backup_root);
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    if stage.is_dir() {
        fs::remove_dir_all(&stage).map_err(|error| io_at(&stage, error))?;
    }
    fs::remove_dir_all(&backup_root).map_err(|error| io_at(&backup_root, error))?;

    let source_texture_files = textures
        .values()
        .map(|texture| texture.sources.len() as u64)
        .sum::<u64>();
    let published_atlases = textures.values().filter(|texture| !texture.support).count() as u64;
    Ok(PlayerItemSetOrganizerReport {
        schema: PLAYER_ITEM_SET_REPORT_SCHEMA.to_owned(),
        sets: catalog_sets.len() as u64,
        models: models.len() as u64,
        source_texture_files,
        published_atlases,
        duplicate_texture_files_removed: source_texture_files
            .saturating_sub(published_atlases + rendering_artifacts.len() as u64),
        rendering_textures: rendering_artifacts.len() as u64,
        conversion_reports_removed,
        catalog: ResourceSetArtifact {
            path: catalog_rooted.to_owned(),
            bytes: catalog_bytes.len() as u64,
            blake3: hash_bytes(&catalog_bytes),
        },
    })
}
