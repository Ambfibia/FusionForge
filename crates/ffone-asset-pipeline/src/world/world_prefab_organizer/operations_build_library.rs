use super::*;

pub(super) fn build_library(
    asset_root: &Path,
    staging: &Path,
    inventory: &Inventory,
    resource_taxonomy: &BTreeMap<String, Taxonomy>,
    report: &mut WorldPrefabOrganizerReport,
) -> Result<()> {
    let resource_drafts_by_id = inventory
        .resources
        .values()
        .map(|resource| (resource.id.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    let mut object_plans = BTreeMap::<String, ObjectPlan>::new();
    let mut planned = Vec::<(&PrefabDraft, Taxonomy, String)>::new();
    for draft in inventory.prefabs.values() {
        let mut combined_aliases = draft.aliases.clone();
        for part in &draft.parts {
            let resource = resource_drafts_by_id
                .get(part.resource.as_str())
                .ok_or_else(|| invalid_error("map-object geometry is absent"))?;
            combined_aliases.extend(resource.aliases.iter().cloned());
        }
        let taxonomy = taxonomy(&combined_aliases);
        let relative_root = object_base_root(&taxonomy);
        planned.push((draft, taxonomy, relative_root));
    }
    let mut route_groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, (_, _, base)) in planned.iter().enumerate() {
        route_groups.entry(base.clone()).or_default().push(index);
    }
    let mut readable_roots = vec![String::new(); planned.len()];
    for (base, mut indexes) in route_groups {
        indexes.sort_by(|left, right| {
            planned[*right]
                .0
                .occurrences
                .cmp(&planned[*left].0.occurrences)
                .then_with(|| planned[*left].0.id.cmp(&planned[*right].0.id))
        });
        for (variant_index, index) in indexes.into_iter().enumerate() {
            readable_roots[index] = if variant_index == 0 {
                base.clone()
            } else {
                format!("{base}_variant_{:04}", variant_index + 1)
            };
        }
    }
    let mut resource_owners = BTreeMap::<String, String>::new();
    for ((draft, taxonomy, _), relative_root) in planned.into_iter().zip(readable_roots) {
        for part in &draft.parts {
            if let Some(previous) =
                resource_owners.insert(part.resource.clone(), relative_root.clone())
            {
                return invalid(format!(
                    "geometry {} is owned by two map objects: {previous:?} and {relative_root:?}",
                    part.resource
                ));
            }
        }
        object_plans.insert(
            draft.id.clone(),
            ObjectPlan {
                taxonomy,
                relative_root,
            },
        );
    }
    if resource_owners.len() != inventory.resources.len() {
        return invalid("some map geometry has no owning map object");
    }

    let mut textures = BTreeMap::<String, String>::new();
    let mut resources = Vec::<BuiltResource>::new();
    let mut maximum_position_error = 0.0_f64;
    let mut drafts = inventory.resources.values().collect::<Vec<_>>();
    drafts.sort_by(|left, right| left.id.cmp(&right.id));
    for (index, draft) in drafts.iter().enumerate() {
        let taxonomy = resource_taxonomy
            .get(&draft.id)
            .ok_or_else(|| invalid_error("resource taxonomy is absent"))?;
        let object_root = resource_owners
            .get(&draft.id)
            .ok_or_else(|| invalid_error("resource owner is absent"))?;
        let model_relative = format!("{object_root}/{}.glb", draft.key.kind.label());
        let source_relative = draft
            .representative
            .model_path
            .as_deref()
            .ok_or_else(|| invalid_error("representative resource has no baked model"))?;
        let source_path = safe_join(asset_root, source_relative)?;
        let source_bytes = read_regular_file(&source_path, "representative baked model")?;
        let source_hash = hash_bytes(&source_bytes);
        if source_hash != draft.baked_hash {
            return invalid(format!(
                "representative baked model changed during organization: {source_relative}"
            ));
        }
        let mut parsed = parse_glb(&source_bytes, source_relative)?;
        let (bounds, reconstruction_error) = unbake_glb_geometry(
            &mut parsed,
            draft.representative.world_matrix,
            source_relative,
        )?;
        maximum_position_error = maximum_position_error.max(reconstruction_error);
        relocate_glb_textures(
            &mut parsed.document,
            &source_path,
            &model_relative,
            staging,
            &mut textures,
        )?;
        set_glb_resource_metadata(
            &mut parsed.document,
            &draft.id,
            &taxonomy.name,
            source_relative,
            &source_hash,
        )?;
        let (mesh, primitive, vertex_count, index_count) =
            geometry_layout(&parsed.document, source_relative)?;
        let output_bytes = encode_glb(&parsed, &model_relative)?;
        let model_path = safe_join(staging, &model_relative)?;
        write_new(&model_path, &output_bytes)?;
        let artifact = WorldPrefabArtifact {
            path: format!("map/{model_relative}"),
            bytes: output_bytes.len() as u64,
            blake3: hash_bytes(&output_bytes),
        };
        resources.push(BuiltResource {
            record: WorldPrefabResource {
                id: draft.id.clone(),
                kind: draft.key.kind,
                name: taxonomy.name.clone(),
                category: taxonomy.category.clone(),
                prefix: taxonomy.prefix.clone(),
                family: taxonomy.family.clone(),
                aliases: ranked_aliases(&draft.aliases),
                model: artifact,
                bounds,
                scene: 0,
                mesh,
                primitive,
                vertex_count,
                index_count,
                source_mesh: draft.representative.source_mesh.clone(),
                material_ids: draft.key.material_ids.clone(),
                representative_tile: draft.representative_tile.clone(),
                representative_payload: draft.representative.id.clone(),
                representative_baked_model: source_relative.to_owned(),
                representative_baked_blake3: source_hash,
                representative_world_matrix: string_matrix(
                    draft.representative.world_matrix,
                ),
                pre_winding_model_blake3: draft.representative.model_blake3.clone(),
                occurrence_count: draft.occurrences,
                derivation: "inverse of the exact payload worldNativeMatrix applied to POSITION; transpose of its linear part applied to NORMAL; indices/UV/material state retained from the audited winding-repaired publication".to_owned(),
            },
        });
        if (index + 1) % 1_000 == 0 || index + 1 == drafts.len() {
            eprintln!(
                "map-object geometry: {}/{} local GLBs",
                index + 1,
                drafts.len()
            );
        }
    }
    report.counts.textures = textures.len() as u64;

    let resources_by_id = resources
        .iter()
        .map(|resource| (resource.record.id.as_str(), &resource.record))
        .collect::<BTreeMap<_, _>>();
    let prefab_drafts_by_id = inventory
        .prefabs
        .values()
        .map(|object| (object.id.as_str(), object))
        .collect::<BTreeMap<_, _>>();
    let mut prefab_catalog = Vec::<WorldPrefabCatalogPrefab>::new();
    let mut prefab_drafts = inventory.prefabs.values().collect::<Vec<_>>();
    prefab_drafts.sort_by(|left, right| left.id.cmp(&right.id));
    for draft in prefab_drafts {
        let mut combined_aliases = draft.aliases.clone();
        for part in &draft.parts {
            let resource = resources_by_id
                .get(part.resource.as_str())
                .ok_or_else(|| invalid_error("map-object geometry is absent"))?;
            combined_aliases.extend(resource.aliases.iter().cloned());
        }
        let plan = object_plans
            .get(&draft.id)
            .ok_or_else(|| invalid_error("map-object plan is absent"))?;
        let taxonomy = &plan.taxonomy;
        let definition = WorldPrefabDefinition {
            schema: WORLD_PREFAB_SCHEMA.to_owned(),
            id: draft.id.clone(),
            name: taxonomy.name.clone(),
            category: taxonomy.category.clone(),
            prefix: taxonomy.prefix.clone(),
            family: taxonomy.family.clone(),
            aliases: ranked_aliases(&combined_aliases),
            coordinate_space: "native local; H=diag(-1,1,1), one Unity unit equals one Bevy unit"
                .to_owned(),
            parts: draft
                .parts
                .iter()
                .map(|part| WorldPrefabPart {
                    resource: part.resource.clone(),
                    role: part.role,
                    local_matrix: string_matrix(identity_matrix()),
                    is_trigger: part.is_trigger,
                })
                .collect(),
            source_nodes: draft.source_nodes.iter().cloned().collect(),
            resource_set: None,
        };
        let relative = format!("{}/object.json", plan.relative_root);
        let bytes = pretty_json(&definition)?;
        write_new(&safe_join(staging, &relative)?, &bytes)?;
        prefab_catalog.push(WorldPrefabCatalogPrefab {
            id: draft.id.clone(),
            name: taxonomy.name.clone(),
            category: taxonomy.category.clone(),
            prefix: taxonomy.prefix.clone(),
            family: taxonomy.family.clone(),
            definition: WorldPrefabArtifact {
                path: format!("map/{relative}"),
                bytes: bytes.len() as u64,
                blake3: hash_bytes(&bytes),
            },
            part_count: draft.parts.len() as u64,
            occurrence_count: draft.occurrences,
            resource_set: None,
        });
    }
    let composite_objects = publish_composite_map_objects(asset_root, staging)?;
    report.counts.prefabs += composite_objects.len() as u64;
    *report
        .categories
        .entry("structures".to_owned())
        .or_default() += composite_objects.len() as u64;

    let mut grouped_placements = BTreeMap::<(String, String), Vec<&PlacementDraft>>::new();
    for tile in &inventory.tiles {
        grouped_placements.entry(tile.clone()).or_default();
    }
    for placement in &inventory.placements {
        grouped_placements
            .entry((placement.scope.clone(), placement.tile_id.clone()))
            .or_default()
            .push(placement);
    }
    let mut placement_sets = Vec::new();
    let mut shared_terrain = SharedTerrainFiles::default();
    for ((scope, tile_id), mut placements) in grouped_placements {
        placements.sort_by(|left, right| {
            left.source_node
                .cmp(&right.source_node)
                .then_with(|| left.prefab.cmp(&right.prefab))
        });
        let canonical_tile = canonical_tile_id(&scope, &tile_id)?;
        let legacy_id = (canonical_tile != tile_id).then_some(tile_id.clone());
        let document = WorldPrefabPlacementDocument {
            schema: WORLD_PREFAB_PLACEMENTS_SCHEMA.to_owned(),
            source_build: SOURCE_BUILD.to_owned(),
            tile_id: canonical_tile.clone(),
            legacy_id: legacy_id.clone(),
            coordinate_space: "native world; H=diag(-1,1,1), one Unity unit equals one Bevy unit"
                .to_owned(),
            instances: placements
                .iter()
                .map(|placement| WorldPrefabPlacement {
                    prefab: placement.prefab.clone(),
                    source_node: placement.source_node.clone(),
                    source_geometry: placement.source_geometry.clone(),
                    legacy_layer: placement.legacy_layer,
                    world_matrix: string_matrix(placement.world_matrix),
                })
                .collect(),
        };
        let relative = format!("tiles/{canonical_tile}/objects.json");
        let bytes = pretty_json(&document)?;
        write_new(&safe_join(staging, &relative)?, &bytes)?;
        let objects_artifact = WorldPrefabArtifact {
            path: format!("map/{relative}"),
            bytes: bytes.len() as u64,
            blake3: hash_bytes(&bytes),
        };
        let manifest = publish_map_tile(
            asset_root,
            staging,
            &scope,
            &tile_id,
            &canonical_tile,
            legacy_id.as_deref(),
            &objects_artifact,
            &placements,
            &prefab_drafts_by_id,
            &resources_by_id,
            &mut shared_terrain,
        )?;
        placement_sets.push(WorldPrefabPlacementSetReference {
            tile_id: canonical_tile,
            legacy_id,
            manifest,
            artifact: objects_artifact,
            instance_count: placements.len() as u64,
        });
    }

    publish_shared_map_runtime_assets(asset_root, staging, &mut shared_terrain)?;

    let catalog = WorldPrefabCatalog {
        schema: WORLD_PREFAB_CATALOG_SCHEMA.to_owned(),
        source_build: SOURCE_BUILD.to_owned(),
        generated_by: WORLD_PREFAB_TOOL.to_owned(),
        coordinate_contract: "map-object geometry is local native H=diag(-1,1,1); tile object placements are exact native world matrices"
            .to_owned(),
        compatibility_policy: report.compatibility_policy.clone(),
        reconstruction_proof: MapReconstructionProof {
            source_set_blake3: report.source_set_blake3.clone(),
            resources: resources.len() as u64,
            maximum_position_error,
        },
        categories: report.categories.clone(),
        prefixes: report.prefixes.clone(),
        shared_files: shared_terrain.artifacts.into_values().collect(),
        resource_sets: Vec::new(),
        resources: resources.into_iter().map(|resource| resource.record).collect(),
        prefabs: prefab_catalog,
        composite_objects,
        placement_sets,
    };
    let catalog_bytes = pretty_json(&catalog)?;
    write_new(&staging.join("catalog.json"), &catalog_bytes)?;

    let files = collect_files(staging)?;
    report.counts.output_files = files.len() as u64;
    report.counts.output_bytes = files.iter().map(|(_, bytes, _)| *bytes).sum();
    let mut result_set = blake3::Hasher::new();
    for (path, _, hash) in &files {
        append_set_hash(&mut result_set, path, hash);
    }
    report.result_set_blake3 = Some(result_set.finalize().to_hex().to_string());
    Ok(())
}

pub(super) fn authored_transform_json(matrix: [[f64; 4]; 4], context: &str) -> Result<JsonValue> {
    let matrix = dmat4(matrix);
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    if !scale.is_finite()
        || !rotation.is_finite()
        || !translation.is_finite()
        || scale.min_element() <= 0.0
    {
        return invalid(format!(
            "{context:?} has a non-finite/non-positive map transform"
        ));
    }
    let reconstructed = DMat4::from_scale_rotation_translation(scale, rotation, translation);
    let maximum = matrix
        .to_cols_array()
        .into_iter()
        .zip(reconstructed.to_cols_array())
        .map(|(expected, actual)| (expected - actual).abs())
        .fold(0.0_f64, f64::max);
    let scale_limit = matrix
        .to_cols_array()
        .into_iter()
        .map(f64::abs)
        .fold(1.0_f64, f64::max)
        * 2.0e-6;
    if maximum > scale_limit {
        return invalid(format!(
            "{context:?} contains non-TRS shear: reconstruction error {maximum}"
        ));
    }
    Ok(serde_json::json!({
        "translation": [translation.x as f32, translation.y as f32, translation.z as f32],
        "rotation": [rotation.x as f32, rotation.y as f32, rotation.z as f32, rotation.w as f32],
        "scale": [scale.x as f32, scale.y as f32, scale.z as f32],
    }))
}

pub(super) fn normalized_blake3<'a>(value: &'a str, context: &str) -> Result<&'a str> {
    let value = value.strip_prefix("blake3:").unwrap_or(value);
    value
        .parse::<blake3::Hash>()
        .map_err(|error| invalid_error(format!("invalid {context} BLAKE3: {error}")))?;
    Ok(value)
}

pub(super) fn taxonomy(aliases: &BTreeSet<String>) -> Taxonomy {
    let ranked = ranked_aliases(aliases);
    let name = ranked
        .first()
        .cloned()
        .unwrap_or_else(|| "unnamed-static-object".to_owned());
    let searchable = ranked.join(" ").to_ascii_lowercase();
    let category = classify_category(&searchable).to_owned();
    let (prefix, family) = observed_prefix_family(&ranked);
    Taxonomy {
        name: clean_display_name(&name),
        category,
        prefix,
        family,
    }
}

pub(super) fn classify_category(text: &str) -> &'static str {
    const RULES: &[(&str, &[&str])] = &[
        (
            "effects",
            &[
                "particle",
                "effect",
                "_fx",
                "fx_",
                "blackhole",
                "attack_hit",
                "smoke",
                "flame",
                "fire",
                "glow",
                "forcefield",
                "shield",
                "portal",
                "waterfall",
            ],
        ),
        (
            "attractions",
            &[
                "merry-go-round",
                "merry_go_round",
                "carousel",
                "ferris",
                "circus",
                "bumperstage",
                "roller_coaster",
                "roller coaster",
                "amusement",
                "amusment",
                "balloon",
            ],
        ),
        (
            "nature",
            &[
                "tree", "bush", "shrub", "grass", "flower", "plant", "hedge", "rock", "stone",
                "stump", "mushroom", "vine", "root", "cactus", "fern",
            ],
        ),
        (
            "structures",
            &[
                "house",
                "building",
                "factory",
                "tower",
                "dome",
                "warehouse",
                "garage",
                "roof",
                "wall",
                "window",
                "door",
                "school",
                "lab",
                "temple",
                "castle",
                "barn",
                "cityhall",
                "city_hall",
                "pillar",
                "floor",
                "ceiling",
                "balcony",
                "arch_",
            ],
        ),
        (
            "furniture",
            &[
                "bench", "chair", "table", "desk", "couch", "sofa", "bed", "cabinet", "shelf",
                "locker", "stool",
            ],
        ),
        (
            "vehicles",
            &[
                "vehicle",
                " car",
                "car_",
                "truck",
                "bus",
                "van",
                "train",
                "ship",
                "boat",
                "plane",
                "helicopter",
                "crane",
                "wagon",
                "weagon",
            ],
        ),
        (
            "signs",
            &[
                "sign",
                "billboard",
                "poster",
                "banner",
                "logo",
                "placard",
                "notice",
            ],
        ),
        (
            "gameplay",
            &[
                "spawn",
                "launcher",
                "race",
                "ring",
                "switch",
                "warp",
                "checkpoint",
                "jump_pad",
                "jumppad",
                "trigger",
            ],
        ),
        (
            "infrastructure",
            &[
                "road",
                "street",
                "sidewalk",
                "fence",
                "rail",
                "pipe",
                "pole",
                "hydrant",
                "sewer",
                "tunnel",
                "bridge",
                "platform",
                "stairs",
                "stair",
                "cable",
                "lamp",
                "lightpost",
                "gate",
                "_net",
                "net_",
            ],
        ),
        (
            "props",
            &[
                "crate",
                "barrel",
                "trash",
                "bin",
                "mailbox",
                "vending",
                "box",
                "container",
                "cart",
                "can_",
                "bottle",
                "machine",
            ],
        ),
        ("collision", &["collision", "collider"]),
    ];
    RULES
        .iter()
        .find_map(|(category, terms)| {
            terms
                .iter()
                .any(|term| text.contains(term))
                .then_some(*category)
        })
        .unwrap_or("unclassified")
}

pub(super) fn propagate_visual_aliases_to_colliders(resources: &mut BTreeMap<ResourceKey, ResourceDraft>) {
    let mut visual_aliases = BTreeMap::<(String, i64), BTreeSet<String>>::new();
    for resource in resources.values() {
        if resource.key.kind == WorldPrefabResourceKind::Visual {
            visual_aliases
                .entry((
                    resource.key.source_asset.clone(),
                    resource.key.source_path_id,
                ))
                .or_default()
                .extend(resource.aliases.iter().cloned());
        }
    }
    for resource in resources.values_mut() {
        if resource.key.kind == WorldPrefabResourceKind::Collider {
            if let Some(aliases) = visual_aliases.get(&(
                resource.key.source_asset.clone(),
                resource.key.source_path_id,
            )) {
                resource.aliases.extend(aliases.iter().cloned());
            }
        }
    }
}

pub(super) fn aliases_look_character_like(aliases: &BTreeSet<String>) -> bool {
    aliases.iter().any(|alias| {
        let lower = alias.to_ascii_lowercase();
        if lower.contains("building")
            || lower.contains("house")
            || lower.contains("shop")
            || lower.contains("sign")
        {
            return false;
        }
        lower.contains("maincharacter")
            || lower.starts_with("npc_")
            || lower.contains("-npc_")
            || lower.starts_with("mob_")
            || lower.contains("-mob_")
    })
}

pub(super) fn observed_prefix_family(aliases: &[String]) -> (String, String) {
    for alias in aliases {
        let tokens = alias
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>();
        let Some(first) = tokens.first() else {
            continue;
        };
        if (2..=4).contains(&first.len()) && first.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let prefix = first.to_ascii_uppercase();
            let family = tokens
                .get(1)
                .filter(|token| (2..=8).contains(&token.len()))
                .map(|token| token.to_ascii_uppercase())
                .unwrap_or_else(|| "MISC".to_owned());
            return (
                safe_identifier(&prefix, "MISC", 12),
                safe_identifier(&family, "MISC", 16),
            );
        }
    }
    ("MISC".to_owned(), "MISC".to_owned())
}

pub(super) fn node_aliases(node_id: &str, nodes: &BTreeMap<&str, &HierarchyNode>) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();
    let mut current = Some(node_id);
    for _ in 0..8 {
        let Some(node) = current.and_then(|id| nodes.get(id).copied()) else {
            break;
        };
        insert_alias(&mut aliases, &node.name);
        current = node.parent_id.as_deref();
    }
    aliases
}

pub(super) fn insert_alias(aliases: &mut BTreeSet<String>, alias: &str) {
    let alias = clean_display_name(alias);
    if !alias.is_empty() && !is_generic_alias(&alias) {
        aliases.insert(alias);
    }
}

pub(super) fn ranked_aliases(aliases: &BTreeSet<String>) -> Vec<String> {
    let mut aliases = aliases.iter().cloned().collect::<Vec<_>>();
    aliases.sort_by(|left, right| {
        alias_score(right)
            .cmp(&alias_score(left))
            .then_with(|| left.len().cmp(&right.len()))
            .then_with(|| left.cmp(right))
    });
    aliases.truncate(MAX_ALIASES);
    aliases
}

pub(super) fn alias_score(alias: &str) -> usize {
    let lower = alias.to_ascii_lowercase();
    let category_bonus = usize::from(classify_category(&lower) != "unclassified") * 10_000;
    let prefix_bonus = usize::from(observed_prefix_family(&[alias.to_owned()]).0 != "MISC") * 1_000;
    category_bonus + prefix_bonus + alias.len().min(500)
}

pub(super) fn representative_score(aliases: &BTreeSet<String>) -> usize {
    aliases
        .iter()
        .map(|alias| alias_score(alias))
        .max()
        .unwrap_or(0)
}

pub(super) fn clean_display_name(value: &str) -> String {
    let mut value = value.trim().replace('\\', "/");
    if let Some(file_name) = value.rsplit('/').next() {
        value = file_name.to_owned();
    }
    for suffix in [".dds", ".png", ".tga", ".jpg", ".jpeg"] {
        if value.to_ascii_lowercase().ends_with(suffix) {
            value.truncate(value.len() - suffix.len());
            break;
        }
    }
    value
        .trim_matches(|ch: char| ch.is_whitespace() || ch == '_' || ch == '-')
        .to_owned()
}

pub(super) fn is_generic_alias(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.is_empty()
        || lower == "unnamed"
        || lower == "gameobject"
        || lower == "mesh"
        || lower == "map"
        || lower.starts_with("map_")
        || lower.starts_with("tile_")
        || lower.starts_with("buildplayer-map_")
        || lower.starts_with("visual__")
        || lower.starts_with("collider__")
}

pub(super) fn limited_samples(samples: BTreeMap<String, BTreeSet<String>>) -> BTreeMap<String, Vec<String>> {
    samples
        .into_iter()
        .map(|(key, values)| (key, values.into_iter().take(24).collect()))
        .collect()
}

pub(super) fn resource_id(key: &ResourceKey) -> Result<String> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "kind": key.kind.label(),
        "sourceMesh": {"asset": key.source_asset, "pathId": key.source_path_id},
        "materialIds": key.material_ids,
    }))
    .map_err(generated_json_error)?;
    Ok(format!("map-geometry-{}", hash_bytes(&bytes)))
}

pub(super) fn canonical_tile_id(scope: &str, tile_id: &str) -> Result<String> {
    match scope {
        "worldMap" if tile_id.starts_with("map_") => Ok(tile_id.to_owned()),
        "tutorial" if tile_id.starts_with("tile_") => Ok(format!(
            "map_{}",
            tile_id
                .strip_prefix("tile_")
                .ok_or_else(|| invalid_error("legacy tile id has no tile_ prefix"))?
        )),
        _ => invalid(format!(
            "cannot normalize map tile identity {scope:?}/{tile_id:?}"
        )),
    }
}
