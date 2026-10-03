use super::*;

pub(super) fn convert_tile(
    tile_id: &str,
    scope: &str,
    export: &BehaviourExport,
    hierarchy: &HierarchyIndex,
    totals: &mut Totals,
) -> Result<JsonValue> {
    let scripts = export
        .scripts
        .iter()
        .map(|script| (script.id.as_str(), script.class_name.as_str()))
        .collect::<BTreeMap<_, _>>();
    let billboard_nodes = export
        .behaviours
        .iter()
        .filter(|record| {
            record.kind == "MonoBehaviour"
                && record
                    .script
                    .as_deref()
                    .and_then(|id| scripts.get(id))
                    .is_some_and(|class_name| *class_name == "BillboardNode")
        })
        .map(|record| record.node.as_str())
        .collect::<Vec<_>>();
    let animation_clips = export
        .animation_clips
        .iter()
        .map(decode_world_animation_clip)
        .collect::<Result<Vec<_>>>()?;
    let animation_clip_ids = export
        .animation_clips
        .iter()
        .map(|clip| clip.id.as_str())
        .collect::<BTreeSet<_>>();
    let animation_clips_by_id = animation_clips
        .iter()
        .filter_map(|clip| {
            clip.get("id")
                .and_then(JsonValue::as_str)
                .map(|id| (id, clip))
        })
        .collect::<BTreeMap<_, _>>();
    totals.animation_clips += animation_clips.len() as u64;
    let mut effect_prefab_closure_ids = BTreeSet::new();
    for closure in &export.effect_prefab_closures {
        if closure.schema != WORLD_EFFECT_PREFAB_CLOSURE_SCHEMA {
            return invalid(format!(
                "world effect prefab closure {} uses unsupported schema {:?}",
                closure.id, closure.schema
            ));
        }
        if closure.id.is_empty() || closure.objects.is_empty() {
            return invalid(format!(
                "world effect prefab closure {:?} has no id or serialized objects",
                closure.id
            ));
        }
        if !effect_prefab_closure_ids.insert(closure.id.as_str()) {
            return invalid(format!(
                "world effect prefab closure id {:?} is duplicated",
                closure.id
            ));
        }
    }

    let mut billboards = Vec::new();
    let mut visibility_switches = Vec::new();
    let mut effect_emitters = Vec::new();
    let mut animations = Vec::new();
    let mut triggers = Vec::new();
    let mut waypoints = Vec::new();
    let mut trigger_volumes = Vec::new();
    let mut rigid_bodies = Vec::new();
    let mut blockers = Vec::new();

    for record in &export.behaviours {
        let placement = if let Some(published) = hierarchy.world_matrix_by_node.get(&record.node) {
            if published != &record.world_matrix {
                return invalid(format!(
                    "behaviour {} world matrix disagrees with the published hierarchy",
                    record.id
                ));
            }
            published
        } else {
            &record.world_matrix
        };
        let models = hierarchy
            .model_ids_by_node
            .get(&record.node)
            .cloned()
            .unwrap_or_default();
        let base = |value: &mut JsonMap<String, JsonValue>| {
            value.insert("node".to_owned(), JsonValue::String(record.node.clone()));
            value.insert("enabled".to_owned(), JsonValue::Bool(record.enabled));
            value.insert("worldMatrix".to_owned(), matrix_json(placement));
            if !models.is_empty() {
                value.insert(
                    "models".to_owned(),
                    JsonValue::Array(models.iter().cloned().map(JsonValue::String).collect()),
                );
            }
        };

        match record.kind.as_str() {
            "SphereCollider" => {
                let mut value = JsonMap::new();
                base(&mut value);
                value.insert("kind".to_owned(), JsonValue::String("sphere".to_owned()));
                value.insert(
                    "center".to_owned(),
                    vector_json(record.fields.get("m_Center")),
                );
                value.insert(
                    "radius".to_owned(),
                    number_json(record.fields.get("m_Radius")),
                );
                value.insert(
                    "isTrigger".to_owned(),
                    JsonValue::Bool(
                        record
                            .fields
                            .get("m_IsTrigger")
                            .and_then(JsonValue::as_bool)
                            .unwrap_or(false),
                    ),
                );
                trigger_volumes.push(JsonValue::Object(value));
                totals.trigger_volumes += 1;
            }
            "BoxCollider" | "CapsuleCollider" => {
                let mut value = JsonMap::new();
                base(&mut value);
                value.insert(
                    "kind".to_owned(),
                    JsonValue::String(
                        if record.kind == "BoxCollider" {
                            "box"
                        } else {
                            "capsule"
                        }
                        .to_owned(),
                    ),
                );
                value.insert(
                    "center".to_owned(),
                    vector_json(record.fields.get("m_Center")),
                );
                if let Some(size) = record.fields.get("m_Size") {
                    value.insert("size".to_owned(), vector_json(Some(size)));
                }
                if let Some(radius) = record.fields.get("m_Radius") {
                    value.insert("radius".to_owned(), number_json(Some(radius)));
                }
                if let Some(height) = record.fields.get("m_Height") {
                    value.insert("height".to_owned(), number_json(Some(height)));
                }
                if let Some(direction) = record.fields.get("m_Direction") {
                    value.insert("direction".to_owned(), number_json(Some(direction)));
                }
                value.insert(
                    "isTrigger".to_owned(),
                    JsonValue::Bool(
                        record
                            .fields
                            .get("m_IsTrigger")
                            .and_then(JsonValue::as_bool)
                            .unwrap_or(false),
                    ),
                );
                trigger_volumes.push(JsonValue::Object(value));
                totals.trigger_volumes += 1;
            }
            "Rigidbody" => {
                let mut value = JsonMap::new();
                base(&mut value);
                for (field, key) in [
                    ("m_Mass", "mass"),
                    ("m_Drag", "drag"),
                    ("m_AngularDrag", "angularDrag"),
                ] {
                    value.insert(key.to_owned(), number_json(record.fields.get(field)));
                }
                for (field, key) in [
                    ("m_UseGravity", "useGravity"),
                    ("m_IsKinematic", "isKinematic"),
                ] {
                    value.insert(
                        key.to_owned(),
                        JsonValue::Bool(
                            record
                                .fields
                                .get(field)
                                .and_then(JsonValue::as_bool)
                                .unwrap_or(false),
                        ),
                    );
                }
                rigid_bodies.push(JsonValue::Object(value));
                totals.rigid_bodies += 1;
            }
            "Animation" => {
                let mut value = JsonMap::new();
                base(&mut value);
                value.insert(
                    "playAutomatically".to_owned(),
                    JsonValue::Bool(
                        record
                            .fields
                            .get("m_PlayAutomatically")
                            .and_then(JsonValue::as_bool)
                            .unwrap_or(false),
                    ),
                );
                value.insert(
                    "animatePhysics".to_owned(),
                    JsonValue::Bool(
                        record
                            .fields
                            .get("m_AnimatePhysics")
                            .and_then(JsonValue::as_bool)
                            .unwrap_or(false),
                    ),
                );
                value.insert(
                    "animateOnlyIfVisible".to_owned(),
                    JsonValue::Bool(
                        record
                            .fields
                            .get("m_AnimateIfVisible")
                            .and_then(JsonValue::as_bool)
                            .unwrap_or(true),
                    ),
                );
                value.insert(
                    "wrapMode".to_owned(),
                    number_json(record.fields.get("m_WrapMode")),
                );
                let clip_count = record
                    .fields
                    .get("m_Animations")
                    .and_then(JsonValue::as_array)
                    .map(|clips| clips.len())
                    .unwrap_or(0);
                value.insert("clipCount".to_owned(), JsonValue::from(clip_count));
                if let Some(default_clip) = record.fields.get("m_Animation") {
                    value.insert("defaultClip".to_owned(), default_clip.clone());
                }
                if let Some(clips) = record.fields.get("m_Animations") {
                    value.insert("clipRefs".to_owned(), clips.clone());
                }
                if let Some(default_clip) = record.resolved_default_clip.as_deref() {
                    if !animation_clip_ids.contains(default_clip) {
                        return invalid(format!(
                            "animation {} resolves default clip {default_clip:?} outside the tile clip closure",
                            record.id
                        ));
                    }
                    value.insert(
                        "defaultClipId".to_owned(),
                        JsonValue::String(default_clip.to_owned()),
                    );
                }
                if !record.resolved_clips.is_empty() {
                    for clip in &record.resolved_clips {
                        if !animation_clip_ids.contains(clip.as_str()) {
                            return invalid(format!(
                                "animation {} resolves clip {clip:?} outside the tile clip closure",
                                record.id
                            ));
                        }
                    }
                    value.insert(
                        "clipIds".to_owned(),
                        JsonValue::Array(
                            record
                                .resolved_clips
                                .iter()
                                .cloned()
                                .map(JsonValue::String)
                                .collect(),
                        ),
                    );
                }
                let mut target_paths = BTreeSet::new();
                for clip_id in record
                    .resolved_default_clip
                    .iter()
                    .chain(record.resolved_clips.iter())
                {
                    let clip = animation_clips_by_id.get(clip_id.as_str()).ok_or_else(|| {
                        invalid_error(format!(
                            "animation {} has no decoded clip {clip_id:?}",
                            record.id
                        ))
                    })?;
                    collect_animation_target_paths(clip, &mut target_paths)?;
                }
                if !target_paths.is_empty() && hierarchy.nodes.contains_key(&record.node) {
                    let model_set = models.iter().map(String::as_str).collect::<BTreeSet<_>>();
                    let mut billboard_paths = BTreeSet::new();
                    for billboard_node in &billboard_nodes {
                        let overlaps_animation = hierarchy
                            .model_ids_by_node
                            .get(*billboard_node)
                            .is_some_and(|billboard_models| {
                                billboard_models
                                    .iter()
                                    .any(|model| model_set.contains(model.as_str()))
                            });
                        if overlaps_animation {
                            if let Some(path) =
                                relative_hierarchy_path(&record.node, billboard_node, hierarchy)?
                            {
                                billboard_paths.insert(path);
                            }
                        }
                    }
                    value.insert(
                        "targets".to_owned(),
                        JsonValue::Array(build_animation_targets(
                            &record.node,
                            &target_paths,
                            &billboard_paths,
                            &models,
                            hierarchy,
                        )?),
                    );
                }
                animations.push(JsonValue::Object(value));
                totals.animations += 1;
            }
            "MonoBehaviour" => {
                if is_builtin_terrain_component(record) {
                    continue;
                }
                let class_name = record.script.as_deref().and_then(|id| scripts.get(id));
                let Some(class_name) = class_name.copied() else {
                    blockers.push(blocker(
                        &record.node,
                        &record.id,
                        "unresolved-script",
                        "the MonoBehaviour script pointer does not resolve to a MonoScript",
                    ));
                    totals.blockers += 1;
                    continue;
                };
                if let Some(reason) = superseded_script(class_name) {
                    let _ = reason;
                    continue;
                }
                match class_name {
                    "BillboardNode" => {
                        let mut value = JsonMap::new();
                        base(&mut value);
                        value.insert("mode".to_owned(), number_json(record.fields.get("mode")));
                        billboards.push(JsonValue::Object(value));
                        totals.billboards += 1;
                    }
                    "VisibleSwitch" => {
                        let mut value = JsonMap::new();
                        base(&mut value);
                        let target = record
                            .fields
                            .get("switchable")
                            .and_then(|pointer| pointer.get("pathId"))
                            .and_then(JsonValue::as_i64);
                        match target.and_then(|path_id| {
                            hierarchy.node_by_component_path_id.get(&path_id).cloned()
                        }) {
                            Some(node) => {
                                value.insert("switches".to_owned(), JsonValue::String(node));
                            }
                            None => {
                                blockers.push(blocker(
                                    &record.node,
                                    &record.id,
                                    "unresolved-visible-switch-target",
                                    "the switchable pointer does not resolve to a published node",
                                ));
                                totals.blockers += 1;
                                continue;
                            }
                        };
                        visibility_switches.push(JsonValue::Object(value));
                        totals.visibility_switches += 1;
                    }
                    "EffectEmitterController" | "EPElementController" => {
                        let mut value = JsonMap::new();
                        base(&mut value);
                        value.insert(
                            "controller".to_owned(),
                            JsonValue::String(class_name.to_owned()),
                        );
                        if let Some(name) =
                            record.fields.get("effectName").and_then(JsonValue::as_str)
                        {
                            value.insert(
                                "effectName".to_owned(),
                                JsonValue::String(name.to_owned()),
                            );
                        }
                        for (field, key) in [
                            ("priority", "priority"),
                            ("maxTimer", "maxTimer"),
                            ("longestLifeTime", "longestLifeTime"),
                            ("TrailTimes", "trailTimes"),
                        ] {
                            if let Some(raw) = record.fields.get(field) {
                                value.insert(key.to_owned(), number_json(Some(raw)));
                            }
                        }
                        if let Some(conform) = bool_value(record.fields.get("conformToScale")) {
                            value.insert("conformToScale".to_owned(), JsonValue::Bool(conform));
                        }
                        if let Some(disable) = bool_value(record.fields.get("disableUpdate")) {
                            value.insert("disableUpdate".to_owned(), JsonValue::Bool(disable));
                        }
                        let elements = record
                            .fields
                            .get("particleElements")
                            .and_then(JsonValue::as_array)
                            .map(|values| values.len())
                            .unwrap_or(0);
                        value.insert("particleElementCount".to_owned(), JsonValue::from(elements));
                        for (field, key) in [
                            ("nifObject", "nifObject"),
                            ("particles", "particles"),
                            ("particleElements", "particleElements"),
                        ] {
                            if let Some(raw) = record.fields.get(field) {
                                value.insert(key.to_owned(), raw.clone());
                            }
                        }
                        if class_name == "EffectEmitterController" {
                            let particle_count = record
                                .fields
                                .get("particles")
                                .and_then(JsonValue::as_array)
                                .map(Vec::len)
                                .unwrap_or_default();
                            if record.resolved_particle_prefabs.len() != particle_count {
                                return invalid(format!(
                                    "EffectEmitterController {} has {particle_count} particles but {} resolved prefab references",
                                    record.id,
                                    record.resolved_particle_prefabs.len()
                                ));
                            }
                            for closure_id in record.resolved_particle_prefabs.iter().flatten() {
                                if !effect_prefab_closure_ids.contains(closure_id.as_str()) {
                                    return invalid(format!(
                                        "EffectEmitterController {} references absent prefab closure {closure_id:?}",
                                        record.id
                                    ));
                                }
                            }
                            value.insert(
                                "resolvedParticlePrefabs".to_owned(),
                                JsonValue::Array(
                                    record
                                        .resolved_particle_prefabs
                                        .iter()
                                        .map(|id| {
                                            id.as_ref()
                                                .map(|id| JsonValue::String(id.clone()))
                                                .unwrap_or(JsonValue::Null)
                                        })
                                        .collect(),
                                ),
                            );
                        }
                        effect_emitters.push(JsonValue::Object(value));
                        totals.effect_emitters += 1;
                    }
                    "Waypoint" => {
                        let mut value = JsonMap::new();
                        base(&mut value);
                        value.insert("point".to_owned(), vector_json(record.fields.get("point")));
                        for (field, key) in [("next", "next"), ("prev", "previous")] {
                            let node = record
                                .fields
                                .get(field)
                                .and_then(|pointer| pointer.get("pathId"))
                                .and_then(JsonValue::as_i64)
                                .filter(|path_id| *path_id != 0)
                                .and_then(|path_id| {
                                    hierarchy.node_by_component_path_id.get(&path_id).cloned()
                                });
                            value.insert(
                                key.to_owned(),
                                node.map(JsonValue::String).unwrap_or(JsonValue::Null),
                            );
                        }
                        waypoints.push(JsonValue::Object(value));
                        totals.waypoints += 1;
                    }
                    "EpSynchronizer" => {
                        let mut value = JsonMap::new();
                        base(&mut value);
                        value.insert(
                            "kind".to_owned(),
                            JsonValue::String("synchronizer".to_owned()),
                        );
                        triggers.push(JsonValue::Object(value));
                        totals.triggers += 1;
                    }
                    other if other.starts_with("Ep") && other.ends_with("Trigger") => {
                        let mut value = JsonMap::new();
                        base(&mut value);
                        let kind = other
                            .trim_start_matches("Ep")
                            .trim_end_matches("Trigger")
                            .to_ascii_lowercase();
                        value.insert("kind".to_owned(), JsonValue::String(kind));
                        for (field, key) in [
                            ("serverId", "serverId"),
                            ("objectId", "objectId"),
                            ("cneId", "cneId"),
                            ("triggerType", "triggerType"),
                            ("radius", "radius"),
                            ("velocity", "velocity"),
                            ("speed", "speed"),
                            ("spinvelocity", "spinVelocity"),
                            ("addPower", "addPower"),
                            ("minPower", "minPower"),
                            ("maxPower", "maxPower"),
                            ("moveType", "moveType"),
                            ("waypointCount", "waypointCount"),
                            ("targetElementId", "targetElementId"),
                            ("targetElementTrigger", "targetElementTrigger"),
                        ] {
                            if let Some(raw) = record.fields.get(field) {
                                value.insert(key.to_owned(), number_json(Some(raw)));
                            }
                        }
                        for (field, key) in [
                            ("startPosition", "startPosition"),
                            ("from", "from"),
                            ("to", "to"),
                            ("initRotate", "initialRotation"),
                            ("maxRotate", "maxRotation"),
                        ] {
                            if let Some(raw) = record.fields.get(field) {
                                value.insert(key.to_owned(), vector_json(Some(raw)));
                            }
                        }
                        for (field, key) in [("head", "head"), ("tail", "tail")] {
                            if let Some(node) = record
                                .fields
                                .get(field)
                                .and_then(|pointer| pointer.get("pathId"))
                                .and_then(JsonValue::as_i64)
                                .filter(|path_id| *path_id != 0)
                                .and_then(|path_id| {
                                    hierarchy.node_by_component_path_id.get(&path_id).cloned()
                                })
                            {
                                value.insert(key.to_owned(), JsonValue::String(node));
                            }
                        }
                        triggers.push(JsonValue::Object(value));
                        totals.triggers += 1;
                    }
                    other => {
                        blockers.push(blocker(
                            &record.node,
                            &record.id,
                            "unclassified-script",
                            &format!("script class {other:?} has no runtime contract"),
                        ));
                        totals.blockers += 1;
                    }
                }
            }
            other => {
                blockers.push(blocker(
                    &record.node,
                    &record.id,
                    "unclassified-component",
                    &format!("component type {other:?} has no runtime contract"),
                ));
                totals.blockers += 1;
            }
        }
    }

    let mut document = JsonMap::new();
    document.insert(
        "schema".to_owned(),
        JsonValue::String(WORLD_BEHAVIOUR_DOCUMENT_SCHEMA.to_owned()),
    );
    document.insert("id".to_owned(), JsonValue::String(tile_id.to_owned()));
    document.insert("scope".to_owned(), JsonValue::String(scope.to_owned()));
    document.insert(
        "tile".to_owned(),
        JsonValue::Array(vec![
            JsonValue::from(export.tile[0]),
            JsonValue::from(export.tile[1]),
        ]),
    );
    document.insert(
        "coordinateContract".to_owned(),
        serde_json::json!({
            "space": "native",
            "basis": "H=diag(-1,1,1)",
            "unitScale": "1-unity-unit-equals-1-bevy-unit"
        }),
    );
    for (key, value) in [
        ("billboards", billboards),
        ("visibilitySwitches", visibility_switches),
        ("effectEmitters", effect_emitters),
        ("animations", animations),
        ("triggers", triggers),
        ("waypoints", waypoints),
        ("triggerVolumes", trigger_volumes),
        ("rigidBodies", rigid_bodies),
        ("blockers", blockers),
    ] {
        document.insert(key.to_owned(), JsonValue::Array(value));
    }
    document.insert(
        "animationClips".to_owned(),
        JsonValue::Array(animation_clips),
    );
    document.insert(
        "effectPrefabClosures".to_owned(),
        JsonValue::Array(
            export
                .effect_prefab_closures
                .iter()
                .map(serde_json::to_value)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| {
                    invalid_error(format!(
                        "could not serialize world effect closures: {error}"
                    ))
                })?,
        ),
    );
    Ok(JsonValue::Object(document))
}

pub(super) fn load_previous_install(asset_root: &Path) -> Result<Option<WorldBehaviourOwnership>> {
    let path = asset_root.join(WORLD_BEHAVIOUR_OWNERSHIP_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    let ownership: WorldBehaviourOwnership = parse_json(&bytes, &path)?;
    if ownership.schema != WORLD_BEHAVIOUR_OWNERSHIP_SCHEMA || ownership.installer != INSTALLER_ID {
        return invalid("existing world behaviour ownership has the wrong identity");
    }
    for tile in &ownership.tiles {
        let tile_path = asset_root.join(&tile.path);
        let tile_bytes = fs::read(&tile_path).map_err(|error| io_at(&tile_path, error))?;
        if !matches_owned_bytes(&tile_bytes, tile.bytes, &tile.blake3) {
            return invalid(format!(
                "{} differs from its world behaviour ownership proof",
                tile.path
            ));
        }
    }
    Ok(Some(ownership))
}
