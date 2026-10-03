use super::*;

pub const PLAYER_SKELETON_SOURCE_SCHEMA: &str = "ffone.player-skeleton-source.v1";

pub(super) const SKELETON_OUTPUT: &str = "characters/player/male/base/male_skeleton.json";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeSkeletonNode {
    pub actor_bone_index: usize,
    pub source_transform_path_id: i64,
    pub source_game_object_path_id: i64,
    pub true_name: String,
    pub full_path: String,
    pub parent_actor_bone_index: Option<usize>,
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationClipIndex {
    pub order: usize,
    pub source_path_id: i64,
    pub true_name: String,
    pub position_curve_count: usize,
    pub rotation_curve_count: usize,
    pub scale_curve_count: usize,
    pub float_curve_count: usize,
    pub object_curve_count: usize,
    pub event_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSkeletonSource {
    pub schema: String,
    pub complete: bool,
    pub name: String,
    pub route: RouteOwnership,
    pub actor_skin_combiner_path_id: i64,
    pub animation_component_path_id: i64,
    pub empty_root_renderer_path_id: i64,
    pub native_coordinate_contract: NativeCoordinateContract,
    pub nodes: Vec<NativeSkeletonNode>,
    pub animation_clips: Vec<AnimationClipIndex>,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkeletonOutputEvidence {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    pub node_count: usize,
    pub indexed_animation_clip_count: usize,
    pub status: String,
}

pub(super) fn extract_male_skeleton(
    catalog: &DumpCatalog,
    ownership: RouteOwnership,
) -> Result<PlayerSkeletonSource> {
    let root = catalog.value(ownership.target_path_id)?;
    let components = component_path_ids(&root)?;
    let mut combiners = Vec::new();
    let mut animations = Vec::new();
    let mut empty_renderers = Vec::new();
    for component in components {
        let object = catalog.get(component)?;
        let value = catalog.value(component)?;
        if object.object_type == "MonoBehaviour" && value.get("actorBones").is_some() {
            combiners.push(component);
        } else if object.object_type == "Animation" {
            animations.push(component);
        } else if object.object_type == "SkinnedMeshRenderer"
            && pointer_path_id(value.get("m_Mesh").unwrap_or(&Value::Null)).unwrap_or_default() == 0
        {
            empty_renderers.push(component);
        }
    }
    let [combiner_path_id] = combiners.as_slice() else {
        return player_error(format!(
            "male root must have one ActorSkinCombiner, found {}",
            combiners.len()
        ));
    };
    let [animation_component_path_id] = animations.as_slice() else {
        return player_error(format!(
            "male root must have one Animation component, found {}",
            animations.len()
        ));
    };
    let [empty_root_renderer_path_id] = empty_renderers.as_slice() else {
        return player_error(format!(
            "male root must have one intentional empty SkinnedMeshRenderer, found {}",
            empty_renderers.len()
        ));
    };

    let combiner_value = catalog.value(*combiner_path_id)?;
    let actor_bones = array_field(&combiner_value, "actorBones")?
        .iter()
        .map(pointer_path_id)
        .collect::<Result<Vec<_>>>()?;
    if actor_bones.len() != 133 || actor_bones.iter().any(|path_id| *path_id == 0) {
        return player_error(format!(
            "male ActorSkinCombiner actorBones must contain 133 non-null transforms, found {}",
            actor_bones.len()
        ));
    }
    let bone_indices = actor_bones
        .iter()
        .enumerate()
        .map(|(index, path_id)| (*path_id, index))
        .collect::<HashMap<_, _>>();

    struct PendingNode {
        transform_path_id: i64,
        game_object_path_id: i64,
        true_name: String,
        parent_path_id: i64,
        translation: [f64; 3],
        rotation: [f64; 4],
        scale: [f64; 3],
    }
    let mut pending = Vec::with_capacity(actor_bones.len());
    for transform_path_id in &actor_bones {
        let transform_object = catalog.get(*transform_path_id)?;
        if transform_object.object_type != "Transform" {
            return player_error(format!(
                "actorBones contains {}#{}",
                transform_object.object_type, transform_path_id
            ));
        }
        let value = catalog.value(*transform_path_id)?;
        let game_object_path_id = pointer_path_id(
            value
                .get("m_GameObject")
                .ok_or_else(|| player_message("Transform has no m_GameObject"))?,
        )?;
        let game_object = catalog.get(game_object_path_id)?;
        if game_object.object_type != "GameObject" {
            return player_error(format!(
                "Transform#{transform_path_id} points to {}, not GameObject",
                game_object.object_type
            ));
        }
        validate_true_name("player skeleton node", &game_object.name)?;
        let parent_path_id = pointer_path_id(
            value
                .get("m_Father")
                .ok_or_else(|| player_message("Transform has no m_Father"))?,
        )?;
        if parent_path_id != 0 && !bone_indices.contains_key(&parent_path_id) {
            return player_error(format!(
                "actor bone Transform#{transform_path_id} parent Transform#{parent_path_id} is outside actorBones"
            ));
        }
        pending.push(PendingNode {
            transform_path_id: *transform_path_id,
            game_object_path_id,
            true_name: game_object.name.clone(),
            parent_path_id,
            translation: native_translation(vec3_field(&value, "m_LocalPosition")?),
            rotation: native_rotation(quat_field(&value, "m_LocalRotation")?)?,
            scale: vec3_field(&value, "m_LocalScale")?,
        });
    }

    fn full_path(
        index: usize,
        pending: &[PendingNode],
        indices: &HashMap<i64, usize>,
        visiting: &mut BTreeSet<usize>,
        cache: &mut [Option<String>],
    ) -> Result<String> {
        if let Some(path) = &cache[index] {
            return Ok(path.clone());
        }
        if !visiting.insert(index) {
            return player_error("actorBones hierarchy contains a cycle");
        }
        let node = &pending[index];
        let path = if node.parent_path_id == 0 {
            node.true_name.clone()
        } else {
            let parent = *indices
                .get(&node.parent_path_id)
                .ok_or_else(|| player_message("actor bone parent index disappeared"))?;
            format!(
                "{}/{}",
                full_path(parent, pending, indices, visiting, cache)?,
                node.true_name
            )
        };
        visiting.remove(&index);
        cache[index] = Some(path.clone());
        Ok(path)
    }

    let mut paths = vec![None; pending.len()];
    let mut nodes = Vec::with_capacity(pending.len());
    for index in 0..pending.len() {
        let mut visiting = BTreeSet::new();
        let path = full_path(index, &pending, &bone_indices, &mut visiting, &mut paths)?;
        let node = &pending[index];
        nodes.push(NativeSkeletonNode {
            actor_bone_index: index,
            source_transform_path_id: node.transform_path_id,
            source_game_object_path_id: node.game_object_path_id,
            true_name: node.true_name.clone(),
            full_path: path,
            parent_actor_bone_index: (node.parent_path_id != 0)
                .then(|| bone_indices[&node.parent_path_id]),
            translation: node.translation,
            rotation: node.rotation,
            scale: node.scale,
        });
    }
    if nodes
        .first()
        .is_none_or(|node| node.true_name != "m" || node.parent_actor_bone_index.is_some())
    {
        return player_error("actorBones[0] is not the parentless true root m");
    }

    let animation_value = catalog.value(*animation_component_path_id)?;
    let clip_pointers = array_field(&animation_value, "m_Animations")?;
    if clip_pointers.len() != 230 {
        return player_error(format!(
            "male Animation component expected 230 clips, found {}",
            clip_pointers.len()
        ));
    }
    let mut seen_clips = BTreeSet::new();
    let mut animation_clips = Vec::with_capacity(clip_pointers.len());
    for (order, pointer) in clip_pointers.iter().enumerate() {
        let path_id = pointer_path_id(pointer)?;
        if !seen_clips.insert(path_id) {
            return player_error(format!("Animation.m_Animations repeats PathID {path_id}"));
        }
        let object = catalog.get(path_id)?;
        if object.object_type != "AnimationClip" {
            return player_error(format!(
                "Animation.m_Animations[{order}] targets {}, not AnimationClip",
                object.object_type
            ));
        }
        validate_true_name("player animation clip", &object.name)?;
        let value = catalog.value(path_id)?;
        animation_clips.push(AnimationClipIndex {
            order,
            source_path_id: path_id,
            true_name: object.name.clone(),
            position_curve_count: optional_array_len(&value, "m_PositionCurves"),
            rotation_curve_count: optional_array_len(&value, "m_RotationCurves"),
            scale_curve_count: optional_array_len(&value, "m_ScaleCurves"),
            float_curve_count: optional_array_len(&value, "m_FloatCurves"),
            object_curve_count: optional_array_len(&value, "m_PPtrCurves"),
            event_count: optional_array_len(&value, "m_Events"),
        });
    }

    Ok(PlayerSkeletonSource {
        schema: PLAYER_SKELETON_SOURCE_SCHEMA.to_owned(),
        complete: false,
        name: "m".to_owned(),
        route: ownership,
        actor_skin_combiner_path_id: *combiner_path_id,
        animation_component_path_id: *animation_component_path_id,
        empty_root_renderer_path_id: *empty_root_renderer_path_id,
        native_coordinate_contract: exact_native_coordinate_contract(),
        nodes,
        animation_clips,
        blockers: vec![
            "AnimationClip curve payload conversion is pending; this readable file is not a runnable avatar or a GLB."
                .to_owned(),
            "No origin recentering, root scaling, or second character-facing rotation has been applied."
                .to_owned(),
        ],
    })
}
