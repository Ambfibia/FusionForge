use super::*;

pub(super) fn load_hierarchy(
    asset_root: &Path,
    tile_id: &str,
    scope: &str,
    expected_source_archive_blake3: &str,
) -> Result<HierarchyIndex> {
    let relative = match scope {
        "tutorial" => format!("world/tutorial/static/tiles/{tile_id}/hierarchy.json"),
        "worldMap" => format!("world/maps/static/tiles/{tile_id}/hierarchy.json"),
        other => return invalid(format!("unsupported world scope {other:?}")),
    };
    let path = asset_root.join(&relative);
    if !path.exists() && scope == "tutorial" {
        // The current tutorial package intentionally carries terrain-only
        // scenes after the shared asset-tree cleanup. Its behaviour export
        // retains each exact source-derived world matrix; worldMap tiles,
        // which publish static models, additionally validate those matrices
        // against their hierarchy.
        return Ok(HierarchyIndex::default());
    }
    let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
    let hierarchy: JsonValue = parse_json(&bytes, &path)?;
    if hierarchy.get("schema").and_then(JsonValue::as_str) != Some(HIERARCHY_SCHEMA) {
        return invalid(format!("{relative} has an unexpected schema"));
    }
    if hierarchy
        .get("sourceArchiveBlake3")
        .and_then(JsonValue::as_str)
        != Some(expected_source_archive_blake3)
    {
        return invalid(format!(
            "{relative} does not match the behaviour export source archive"
        ));
    }

    let mut index = HierarchyIndex::default();
    let mut parent_by_node = BTreeMap::<String, String>::new();
    for node in hierarchy
        .get("nodes")
        .and_then(JsonValue::as_array)
        .unwrap_or(&Vec::new())
    {
        let Some(id) = node.get("id").and_then(JsonValue::as_str) else {
            continue;
        };
        let parent = node
            .get("parentId")
            .and_then(JsonValue::as_str)
            .map(str::to_owned);
        if let Some(parent) = parent.as_deref() {
            parent_by_node.insert(id.to_owned(), parent.to_owned());
            index
                .children_by_node
                .entry(parent.to_owned())
                .or_default()
                .push(id.to_owned());
        }
        if let Some(matrix) = node
            .get("transform")
            .and_then(|transform| transform.get("worldNativeMatrix"))
        {
            let rows = hierarchy_matrix(matrix, &format!("{relative} node {id}"))?;
            index.world_matrix_by_node.insert(id.to_owned(), rows);
            let local = node.pointer("/transform/localNativeTrs").ok_or_else(|| {
                invalid_error(format!("{relative} node {id} has no localNativeTrs"))
            })?;
            index.nodes.insert(
                id.to_owned(),
                HierarchyNode {
                    name: node
                        .get("name")
                        .and_then(JsonValue::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    parent,
                    local_translation: hierarchy_vector(
                        local.get("translation"),
                        3,
                        &format!("{relative} node {id} translation"),
                    )?
                    .try_into()
                    .expect("three hierarchy vector entries"),
                    local_rotation: hierarchy_vector(
                        local.get("rotation"),
                        4,
                        &format!("{relative} node {id} rotation"),
                    )?
                    .try_into()
                    .expect("four hierarchy vector entries"),
                    local_scale: hierarchy_vector(
                        local.get("scale"),
                        3,
                        &format!("{relative} node {id} scale"),
                    )?
                    .try_into()
                    .expect("three hierarchy vector entries"),
                    world_matrix: rows,
                },
            );
        }
        for component in node
            .get("components")
            .and_then(JsonValue::as_array)
            .unwrap_or(&Vec::new())
        {
            if let Some(path_id) = component
                .get("source")
                .and_then(|source| source.get("pathId"))
                .and_then(JsonValue::as_i64)
            {
                index
                    .node_by_component_path_id
                    .entry(path_id)
                    .or_insert_with(|| id.to_owned());
            }
        }
    }
    for payload in hierarchy
        .get("payloads")
        .and_then(JsonValue::as_array)
        .unwrap_or(&Vec::new())
    {
        let (Some(node), Some(model)) = (
            payload.get("hierarchyNodeId").and_then(JsonValue::as_str),
            payload.get("modelId").and_then(JsonValue::as_str),
        ) else {
            continue;
        };
        if payload.get("runtimePublished").and_then(JsonValue::as_bool) != Some(true) {
            continue;
        }
        index
            .direct_model_ids_by_node
            .entry(node.to_owned())
            .or_default()
            .push(model.to_owned());
    }
    for children in index.children_by_node.values_mut() {
        children.sort();
        children.dedup();
    }
    for models in index.direct_model_ids_by_node.values_mut() {
        models.sort();
        models.dedup();
    }
    index.model_ids_by_node =
        models_in_node_subtrees(&parent_by_node, &index.direct_model_ids_by_node);
    Ok(index)
}

pub(super) fn parse_json<T: serde::de::DeserializeOwned>(bytes: &[u8], path: &Path) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })
}
