use super::*;

pub(super) struct DumpCatalog {
    pub(super) objects: Vec<DumpObject>,
    pub(super) by_path_id: HashMap<i64, usize>,
}

impl DumpCatalog {
    pub(super) fn read(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|error| io_at(path, error))?;
        let objects: Vec<DumpObject> =
            serde_json::from_reader(BufReader::new(file)).map_err(|source| {
                PipelineError::Json {
                    path: path.display().to_string(),
                    source,
                }
            })?;
        let mut by_path_id = HashMap::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            if by_path_id.insert(object.path_id, index).is_some() {
                return rig_error(format!("object dump repeats PathID {}", object.path_id));
            }
        }
        Ok(Self {
            objects,
            by_path_id,
        })
    }

    pub(super) fn read_objects(paths: &[PathBuf]) -> Result<Self> {
        let mut objects = Vec::with_capacity(paths.len());
        for path in paths {
            let file = File::open(path).map_err(|error| io_at(path, error))?;
            let object: DumpObject =
                serde_json::from_reader(BufReader::new(file)).map_err(|source| {
                    PipelineError::Json {
                        path: path.display().to_string(),
                        source,
                    }
                })?;
            objects.push(object);
        }
        let mut by_path_id = HashMap::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            if by_path_id.insert(object.path_id, index).is_some() {
                return rig_error(format!("clip sources repeat PathID {}", object.path_id));
            }
        }
        Ok(Self {
            objects,
            by_path_id,
        })
    }

    pub(super) fn get(&self, path_id: i64) -> Result<&DumpObject> {
        self.by_path_id
            .get(&path_id)
            .and_then(|index| self.objects.get(*index))
            .ok_or_else(|| rig_message(format!("object dump has no PathID {path_id}")))
    }

    pub(super) fn value(&self, path_id: i64) -> Result<Value> {
        let object = self.get(path_id)?;
        serde_json::from_str(object.value.get()).map_err(|source| PipelineError::Json {
            path: format!("object-dump#{}", object.path_id),
            source,
        })
    }

    pub(super) fn route(&self, exact_route: &str) -> Result<(i64, String)> {
        let mut matches = Vec::new();
        for object in self
            .objects
            .iter()
            .filter(|object| object.object_type == "AssetBundle")
        {
            let body: Value =
                serde_json::from_str(object.value.get()).map_err(|source| PipelineError::Json {
                    path: format!("object-dump#{}", object.path_id),
                    source,
                })?;
            for entry in array_field(&body, "m_Container")? {
                let Some(pair) = entry.as_array() else {
                    continue;
                };
                if pair.len() != 2 || pair[0].as_str() != Some(exact_route) {
                    continue;
                }
                let path_id = pointer_path_id(
                    pair[1]
                        .get("asset")
                        .ok_or_else(|| rig_message("route metadata has no asset pointer"))?,
                )?;
                let target = self.get(path_id)?;
                if target.object_type != "GameObject" {
                    return rig_error(format!(
                        "route {exact_route:?} targets {}, not GameObject",
                        target.object_type
                    ));
                }
                matches.push((path_id, target.name.clone()));
            }
        }
        let [resolved] = matches.as_slice() else {
            return rig_error(format!(
                "route {exact_route:?} resolved {} times",
                matches.len()
            ));
        };
        Ok(resolved.clone())
    }
}

pub(super) const fn actor_skin_combiner_clothes_index(category: CharacterAppearanceCategory) -> u8 {
    // cnAvatarStatus.SetBody/SetFace/SetHead populate the five-element
    // ActorSkinCombiner.clothes array in this exact order.
    match category {
        CharacterAppearanceCategory::Shoes => 0,
        CharacterAppearanceCategory::Pants => 1,
        CharacterAppearanceCategory::Shirt => 2,
        CharacterAppearanceCategory::Face => 3,
        CharacterAppearanceCategory::Hair => 4,
    }
}

pub(super) fn extract_route_remaps(
    catalog: &DumpCatalog,
    exact_route: &str,
    spec: GenderSpec,
) -> Result<Vec<LegacySkinRemap>> {
    let (root, _) = catalog.route(exact_route)?;
    let mut transforms = Vec::new();
    collect_transforms(
        catalog,
        game_object_transform(catalog, root)?,
        &mut transforms,
    )?;
    let mut remaps = Vec::new();
    for transform in transforms {
        let body = catalog.value(transform)?;
        let game_object = pointer_path_id(
            body.get("m_GameObject")
                .ok_or_else(|| rig_message("route Transform has no GameObject"))?,
        )?;
        let game_object_identity = catalog.get(game_object)?;
        let components = component_path_ids(&catalog.value(game_object)?)?;
        let renderers = components
            .iter()
            .filter_map(|path_id| {
                catalog
                    .get(*path_id)
                    .ok()
                    .filter(|object| object.object_type == "SkinnedMeshRenderer")
                    .map(|_| *path_id)
            })
            .filter(|path_id| {
                catalog
                    .value(*path_id)
                    .ok()
                    .and_then(|value| value.get("m_Mesh").cloned())
                    .and_then(|pointer| pointer_path_id(&pointer).ok())
                    .is_some_and(|path_id| path_id != 0)
            })
            .collect::<Vec<_>>();
        if renderers.is_empty() {
            continue;
        }
        let tables = components
            .iter()
            .filter_map(|path_id| {
                catalog
                    .value(*path_id)
                    .ok()
                    .filter(|value| value.get(spec.transform_indices_field).is_some())
                    .map(|value| (*path_id, value))
            })
            .collect::<Vec<_>>();
        let [(table_path_id, table)] = tables.as_slice() else {
            return rig_error(format!(
                "{exact_route} GameObject {:?} has {} skin tables",
                game_object_identity.name,
                tables.len()
            ));
        };
        if renderers.len() != 1 {
            return rig_error(format!(
                "{exact_route} GameObject {:?} has {} skinned renderers",
                game_object_identity.name,
                renderers.len()
            ));
        }
        let actor_indices = array_field(table, spec.transform_indices_field)?
            .iter()
            .map(|value| {
                let index = value
                    .as_u64()
                    .ok_or_else(|| rig_message("transform index is not unsigned"))?;
                u32::try_from(index).map_err(|_| rig_message("transform index exceeds u32"))
            })
            .collect::<Result<Vec<_>>>()?;
        let renderer = catalog.value(renderers[0])?;
        let renderer_bones = optional_array(&renderer, "m_Bones").len();
        if actor_indices.is_empty() || actor_indices.len() != renderer_bones {
            return rig_error(format!(
                "{exact_route} renderer {:?} palette count {}/{}",
                game_object_identity.name,
                actor_indices.len(),
                renderer_bones
            ));
        }
        remaps.push(LegacySkinRemap {
            renderer_name: game_object_identity.name.clone(),
            renderer_path_id: renderers[0],
            table_path_id: *table_path_id,
            actor_indices,
        });
    }
    if remaps.is_empty() {
        return rig_error(format!("{exact_route} has no skinned renderer remaps"));
    }
    remaps.sort_by(|left, right| left.renderer_name.cmp(&right.renderer_name));
    for pair in remaps.windows(2) {
        if pair[0].renderer_name == pair[1].renderer_name {
            return rig_error(format!(
                "{exact_route} repeats renderer true name {:?}",
                pair[0].renderer_name
            ));
        }
    }
    Ok(remaps)
}

pub(super) fn component_path_ids(value: &Value) -> Result<Vec<i64>> {
    array_field(value, "m_Component")?
        .iter()
        .map(|entry| {
            let pair = entry
                .as_array()
                .ok_or_else(|| rig_message("GameObject component is not a legacy pair"))?;
            if pair.len() != 2 {
                return rig_error("GameObject component pair does not have two values");
            }
            pointer_path_id(&pair[1])
        })
        .collect()
}

pub(super) fn curve_path(binding: &Value) -> Result<String> {
    binding
        .get("path")
        .or_else(|| binding.get("m_Path"))
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| rig_message("animation curve path is empty"))
}

pub(super) fn pointer_path_id(value: &Value) -> Result<i64> {
    value
        .get("pathId")
        .and_then(Value::as_i64)
        .ok_or_else(|| rig_message("value is not a Unity pointer"))
}
