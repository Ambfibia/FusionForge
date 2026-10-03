use super::*;

#[derive(Clone)]
pub(super) struct CreatorModelCandidate {
    pub(super) gender: Option<PlayerRigGender>,
    pub(super) exact_route: String,
    pub(super) true_name: String,
    pub(super) glb: String,
    pub(super) glb_blake3: String,
}

pub(super) fn audit_glb_remaps(
    glb: &[u8],
    legacy: &[LegacySkinRemap],
    actor_nodes: &[PlayerRigNode],
    actor_root: &str,
) -> Result<Vec<PlayerRigSkinRemap>> {
    let document = parse_glb_json(glb)?;
    let nodes = document
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| rig_message("part GLB has no nodes"))?;
    let skins = document
        .get("skins")
        .and_then(Value::as_array)
        .ok_or_else(|| rig_message("part GLB has no skins"))?;
    if skins.len() != legacy.len() {
        return rig_error(format!(
            "part GLB skin count {} differs from legacy remap count {}",
            skins.len(),
            legacy.len()
        ));
    }
    let parents = gltf_parents(nodes)?;
    let mut output = Vec::with_capacity(skins.len());
    for (skin_index, skin) in skins.iter().enumerate() {
        let skin_name = skin
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed skin>");
        // A glTF skin describes the joint palette, not the renderer entity.
        // The native equipment publisher preserves Unity's renderer GameObject
        // true name on the node that references that skin. `skin.name` may be
        // the source Mesh name (for example "Editable Poly"), so using it for
        // ActorWearIndexTable matching silently confuses two identities.
        let renderer_nodes = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.get("skin").and_then(Value::as_u64) == Some(skin_index as u64))
            .collect::<Vec<_>>();
        let [(renderer_node_index, renderer_node)] = renderer_nodes.as_slice() else {
            return rig_error(format!(
                "part GLB skin[{skin_index}] {skin_name:?} is referenced by {} renderer nodes",
                renderer_nodes.len()
            ));
        };
        let name = renderer_node
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                rig_message(format!(
                    "part GLB renderer node[{renderer_node_index}] for skin {skin_name:?} has no true name"
                ))
            })?;
        let matches = legacy
            .iter()
            .filter(|remap| remap.renderer_name == name)
            .collect::<Vec<_>>();
        let [remap] = matches.as_slice() else {
            return rig_error(format!(
                "part GLB renderer {name:?} (skin {skin_name:?}) matched {} legacy renderers",
                matches.len()
            ));
        };
        let joints = array_field(skin, "joints")?;
        if joints.len() != remap.actor_indices.len() {
            return rig_error(format!(
                "skin {name:?} joint count {} differs from transform table {}",
                joints.len(),
                remap.actor_indices.len()
            ));
        }
        let mut gltf_joint_paths = Vec::with_capacity(joints.len());
        let mut inferred = Vec::with_capacity(joints.len());
        let mut actor_bone_paths = Vec::with_capacity(joints.len());
        for (palette_index, joint) in joints.iter().enumerate() {
            let joint_index = usize_number(joint, "glTF joint index")?;
            let path = gltf_node_path(nodes, &parents, joint_index)?;
            let matches = actor_nodes
                .iter()
                .filter(|node| {
                    let relative = node
                        .full_path
                        .strip_prefix(actor_root)
                        .and_then(|path| path.strip_prefix('/'))
                        .unwrap_or("");
                    !relative.is_empty()
                        && (path == relative
                            || path
                                .strip_suffix(relative)
                                .is_some_and(|prefix| prefix.ends_with('/')))
                })
                .collect::<Vec<_>>();
            let [matched] = matches.as_slice() else {
                return rig_error(format!(
                    "skin {name:?} joint path {path:?} matched {} actor bones",
                    matches.len()
                ));
            };
            let legacy_index = remap.actor_indices[palette_index];
            if matched.actor_bone_index != legacy_index {
                return rig_error(format!(
                    "skin {name:?} palette[{palette_index}] path {path:?} resolves actor {}, transform table says {}",
                    matched.actor_bone_index, legacy_index
                ));
            }
            inferred.push(matched.actor_bone_index);
            actor_bone_paths.push(matched.full_path.clone());
            gltf_joint_paths.push(path);
        }
        if inferred != remap.actor_indices {
            return rig_error(format!(
                "skin {name:?} inferred actor palette differs from transform table"
            ));
        }
        output.push(PlayerRigSkinRemap {
            renderer_true_name: name.to_owned(),
            renderer_path_id: remap.renderer_path_id,
            actor_wear_index_table_path_id: remap.table_path_id,
            actor_bone_indices: inferred,
            actor_bone_paths,
            gltf_joint_paths,
            exact_transform_index_parity: true,
        });
    }
    output.sort_by(|left, right| left.renderer_true_name.cmp(&right.renderer_true_name));
    Ok(output)
}

pub(super) fn parse_glb_json(bytes: &[u8]) -> Result<Value> {
    if bytes.len() < 20 || bytes.get(..4) != Some(b"glTF") {
        return rig_error("invalid GLB header");
    }
    let json_len = u32::from_le_bytes(
        bytes[12..16]
            .try_into()
            .expect("validated GLB JSON length bytes"),
    ) as usize;
    let end = 20usize
        .checked_add(json_len)
        .ok_or_else(|| rig_message("GLB JSON length overflow"))?;
    if end > bytes.len() {
        return rig_error("GLB JSON chunk is truncated");
    }
    serde_json::from_slice(&bytes[20..end]).map_err(|source| PipelineError::Json {
        path: "in-memory-glb".to_owned(),
        source,
    })
}

pub(super) fn gltf_parents(nodes: &[Value]) -> Result<Vec<Option<usize>>> {
    let mut parents = vec![None; nodes.len()];
    for (parent, node) in nodes.iter().enumerate() {
        for child in optional_array(node, "children") {
            let child = usize_number(child, "glTF child index")?;
            if child >= nodes.len() || parents[child].replace(parent).is_some() {
                return rig_error("part GLB hierarchy has invalid or repeated child");
            }
        }
    }
    Ok(parents)
}

pub(super) fn gltf_node_path(nodes: &[Value], parents: &[Option<usize>], index: usize) -> Result<String> {
    if index >= nodes.len() {
        return rig_error("part GLB joint index is out of range");
    }
    let mut names = Vec::new();
    let mut current = Some(index);
    let mut seen = BTreeSet::new();
    while let Some(node) = current {
        if !seen.insert(node) {
            return rig_error("part GLB hierarchy contains a cycle");
        }
        names.push(
            nodes[node]
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| rig_message("part GLB node has no true name"))?,
        );
        current = parents[node];
    }
    names.reverse();
    Ok(names.join("/"))
}
