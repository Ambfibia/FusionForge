use super::*;

/// Decode a bounded raw-source clip selection in memory; no staging assets.
pub fn encode_player_rig_clip_additions(
    contract: &[u8],
    selected_gender: PlayerRigGender,
    objects: serde_json::Value,
) -> Result<Vec<u8>> {
    let contract: PlayerSharedRigContract = serde_json::from_slice(contract)
        .map_err(|error| rig_message(format!("rig contract: {error}")))?;
    let gender = contract.genders.iter().find(|g| g.gender == selected_gender)
        .ok_or_else(|| rig_message("missing gender"))?;
    let objects: Vec<DumpObject> = serde_json::from_value(objects)
        .map_err(|error| rig_message(format!("selected raw clips: {error}")))?;
    let mut by_path_id = HashMap::new();
    for (index, object) in objects.iter().enumerate() {
        if by_path_id.insert(object.path_id, index).is_some() {
            return rig_error("duplicate selected clip");
        }
    }
    let catalog = DumpCatalog { objects, by_path_id };
    let root = match selected_gender { PlayerRigGender::Male => "m", PlayerRigGender::Female => "w" };
    let mut animations = Vec::new();
    for object in &catalog.objects {
        let mut clip = decode_clip(&catalog, &gender.nodes, root, &object.name, object.path_id)?;
        rebase_legacy_additive_clip(&mut clip, &gender.nodes)?;
        animations.push(clip);
    }
    let model = NativeModel {
        schema: ffone_skinned_model::MODEL_SCHEMA.to_owned(), name: root.to_owned(),
        native_coordinate_contract: exact_native_coordinate_contract(), roots: vec![0],
        nodes: gender.nodes.iter().map(|node| ModelNode {
            name: node.true_name.clone(), legacy_name: None, legacy_sibling_ordinal: None,
            parent: node.parent_actor_bone_index, translation: node.translation,
            rotation: node.rotation, scale: node.scale, mesh: None, skin: None,
        }).collect(),
        meshes: Vec::new(), skins: Vec::new(), materials: Vec::new(), textures: Vec::new(),
        samplers: Vec::new(), animations,
    };
    encode_glb(&model).map_err(|error| rig_message(format!("selected clips: {error}")))
}
