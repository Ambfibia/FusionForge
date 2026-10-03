use super::super::*;

pub(in super::super) fn write_clean_gltf_npc_resource_file(
    model_path: &Path,
    output_path: &Path,
    bundle_name: &str,
    internal_name: &str,
) -> Result<(), String> {
    let imported_material =
        fusionforge::modding::ImportedMaterial::primary_from_model_path(model_path)?;
    let asset_name = bundle_name.to_string();
    let layout_path = default_repo_root()
        .join("builds")
        .join("FusionFallClient-ru")
        .join("Character_Johnny_Test.resourceFile");
    let mut template_mesh_value = None;
    let mut asset = if layout_path.is_file() {
        let temp = native_build_temp_dir("clean_npc_layout")?;
        extract_bundle_native_to_dir(&layout_path, temp.path())?;
        let mut layout = None;
        for file in extracted_files_in_dir(temp.path()) {
            let path = PathBuf::from(file.path);
            if path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.ends_with(".json"))
            {
                continue;
            }
            if let Ok(asset) = fusionforge::Asset::from_path(&path) {
                layout = Some(asset);
                break;
            }
        }
        let mut layout = layout.ok_or_else(|| {
            format!(
                "{} did not contain a readable Unity serialized asset for TypeTree layout",
                layout_path.display()
            )
        })?;
        template_mesh_value = first_object_value_for_class(&layout, 43)?;
        layout.name = asset_name;
        layout.data = Vec::new();
        layout.objects.clear();
        layout
    } else {
        fusionforge::Asset::empty_with_metadata(asset_name, 6)?
    };
    let mut asset_refs = asset.asset_refs.clone();
    if asset_refs.is_empty() {
        asset_refs.push(fusionforge::AssetRef {
            asset_path: String::new(),
            guid: [0; 16],
            type_id: 0,
            file_path: asset.name.clone(),
        });
    } else if let Some(first) = asset_refs.first_mut() {
        first.file_path = asset.name.clone();
    }
    if asset_refs.len() == 1 {
        asset_refs.push(fusionforge::AssetRef {
            asset_path: String::new(),
            guid: [0; 16],
            type_id: 0,
            file_path: "sharedassets0.assets".to_string(),
        });
    }
    // The Unity 2.x web player resolves external PPtr references by the
    // *internal* serialized file name of the dependency bundle (for example
    // "customassetbundle-b4f543c102ded400fbc6f1da25d9679a"), never by the
    // .resourceFile name on disk. The layout template may still carry
    // bundle-file names (e.g. "Tutorial.resourceFile"), which the runtime can
    // never resolve - that silently breaks the shared shader reference and the
    // NPC renders without material/texture. Remap such names here.
    remap_bundle_file_external_refs(&mut asset_refs, layout_path.parent())?;
    // Cross-bundle shader references proved unreliable at runtime: the
    // dependency bundle must be loaded for the PPtr to resolve, and the
    // launcher-cached big bundles (Tutorial, CharacterCreation) regularly
    // fail reads with "[Position out of bounds]" in the web player log. Any
    // unresolved shader leaves the material blank and the NPC invisible.
    // Instead, copy the toon shader source (and the ToonRamp9 _ShaderMap
    // texture) out of a client bundle at build time and inline them as local
    // objects, so the generated NPC bundle is fully self-contained.
    let inline_toon = {
        let mut search_dirs = Vec::<PathBuf>::new();
        if let Some(dir) = layout_path.parent() {
            search_dirs.push(dir.to_path_buf());
        }
        let builds_root = default_repo_root().join("builds");
        if let Ok(entries) = fs::read_dir(&builds_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    search_dirs.push(path);
                }
            }
        }
        let shader_bundle = ["Tutorial.resourceFile", "CharacterCreation.resourceFile"]
            .iter()
            .find_map(|name| {
                search_dirs
                    .iter()
                    .map(|dir| dir.join(name))
                    .find(|candidate| candidate.is_file())
            })
            .ok_or_else(|| {
                format!(
                    "Neither Tutorial.resourceFile nor CharacterCreation.resourceFile was found near {} or under {}",
                    layout_path.display(),
                    builds_root.display()
                )
            })?;
        let shader_name_priority = if imported_material
            .as_ref()
            .is_some_and(|material| material.double_sided)
        {
            [
                "toonshading_blendsrcalphainvsrcalpha_culloff",
                "skinnedtoonshading_blendsrcalphainvsrcalpha_culloff",
                NPC_WORLD_TOON_SHADER_NAME,
                NPC_SKINNED_TOON_SHADER_NAME,
            ]
        } else {
            [
                NPC_WORLD_TOON_SHADER_NAME,
                NPC_SKINNED_TOON_SHADER_NAME,
                "toonshading_blendsrcalphainvsrcalpha_culloff",
                "skinnedtoonshading_blendsrcalphainvsrcalpha_culloff",
            ]
        };
        find_npc_inline_toon_shader_assets(&shader_bundle, &shader_name_priority)?
    };
    asset.asset_refs = asset_refs;
    asset
        .tree
        .type_trees
        .insert(48, inline_toon.shader_type_tree.clone());
    let inline_shader_path_id = 15_i64;
    let inline_ramp_path_id = 16_i64;
    let shader_file_id = 0_i32;
    let shader_path_id = inline_shader_path_id;
    let shader_map_pointer = inline_toon
        .ramp_value
        .is_some()
        .then_some((0_i32, inline_ramp_path_id));

    let root_go = 2_i64;
    let root_transform = 3_i64;
    let skinned_renderer = 4_i64;
    let animation = 5_i64;
    let skinned_mesh_go = 9_i64;
    let skinned_mesh_transform = 10_i64;
    let static_animation = 4_i64;
    let static_mesh_go = 5_i64;
    let static_mesh_transform = 6_i64;
    let mesh_filter = 7_i64;
    let mesh_renderer = 8_i64;
    let skinned_mesh_path_id = 6_i64;
    let skinned_texture_path_id = 7_i64;
    let skinned_material_path_id = 8_i64;
    let static_mesh_path_id = 9_i64;
    let static_texture_path_id = 10_i64;
    // 11/12 are the Bip01 wrapper GameObject/Transform, which the joint
    // emitters still write in static-mesh debug mode; 15/16 hold the inlined
    // shader/ToonRamp9.
    let static_material_path_id = 17_i64;
    let bip01_wrapper_go = 11_i64;
    let bip01_wrapper_transform = 12_i64;
    let bip01_footsteps_go = 13_i64;
    let bip01_footsteps_transform = 14_i64;
    let first_clip_path_id = 20_i64;
    let first_joint_go_path_id = 100_i64;
    let first_joint_transform_path_id = 1000_i64;

    let raw_joints = read_clean_gltf_joints(model_path)?;
    let mut joints = legacy_safe_clean_gltf_joints(&raw_joints);
    collapse_secondary_iktarget_roots_under_primary(&mut joints);
    let needs_bip01_wrapper = clean_gltf_needs_bip01_wrapper(&raw_joints);
    // The "Bip01" wrapper holds ONLY the constant Z-up bind space -> Y-up
    // world basis rotation. The joints below it (Bip01 NonAccum, ...) keep
    // their GLB rest transforms, which is also exactly what the animation
    // clips drive at runtime — so the composed skeleton stands upright both
    // in rest pose and during playback. (Hoisting the NonAccum rest into the
    // wrapper instead used to leave the runtime skeleton lying flat along Z
    // and half-sunk into the ground, because the clip curves restored the
    // NonAccum rest and zeroed the wrapper.)
    let (bip01_wrapper_translation, bip01_wrapper_rotation, bip01_wrapper_scale) =
        if needs_bip01_wrapper {
            (
                (0.0, 0.0, 0.0),
                legacy_bip01_wrapper_rotation(),
                (1.0, 1.0, 1.0),
            )
        } else {
            ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 1.0), (1.0, 1.0, 1.0))
        };
    let (mesh_local_translation, mesh_local_rotation, mesh_local_scale) =
        read_clean_gltf_mesh_node_transform(model_path)?;
    let (skinned_mesh_local_translation, skinned_mesh_local_rotation, skinned_mesh_local_scale) =
        if needs_bip01_wrapper {
            (
                mesh_local_translation,
                clean_quat_mul(legacy_skinned_mesh_basis_rotation(), mesh_local_rotation),
                mesh_local_scale,
            )
        } else {
            (
                mesh_local_translation,
                mesh_local_rotation,
                mesh_local_scale,
            )
        };
    let joint_transform_path_by_node = joints
        .iter()
        .enumerate()
        .map(|(index, joint)| {
            (
                joint.node_index,
                first_joint_transform_path_id + index as i64,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let joint_go_path_by_node = joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.node_index, first_joint_go_path_id + index as i64))
        .collect::<BTreeMap<_, _>>();
    let root_joint_transforms = joints
        .iter()
        .filter(|joint| joint.parent_node_index.is_none())
        .filter_map(|joint| joint_transform_path_by_node.get(&joint.node_index).copied())
        .collect::<Vec<_>>();
    let first_nonaccum_root_transform = joints
        .iter()
        .find(|joint| joint.parent_node_index.is_none() && joint.name == "Bip01 NonAccum")
        .and_then(|joint| joint_transform_path_by_node.get(&joint.node_index).copied());

    let mut imported_mesh = fusionforge::modding::ImportedMesh::from_model_path(
        model_path,
        Some(internal_name.to_string()),
    )?;
    imported_mesh.name = Some(internal_name.to_string());
    let texture_name = Some(internal_name.to_string());
    let imported_texture = if let Some(mut texture) = imported_material
        .as_ref()
        .and_then(|material| material.base_color_texture.clone())
    {
        texture.name = texture_name.clone();
        texture
    } else {
        let sidecar = model_path.with_extension("png");
        if !sidecar.is_file() {
            return Err(format!(
                "{} has no embedded baseColorTexture and {} does not exist",
                model_path.display(),
                sidecar.display()
            ));
        }
        fusionforge::modding::ImportedTexture::from_png_path(&sidecar, texture_name, false)?
    };
    let disable_animations = std::env::var("FF_CLEAN_NPC_DISABLE_ANIMATIONS")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let disable_skin = std::env::var("FF_CLEAN_NPC_DISABLE_SKIN")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let static_mesh = std::env::var("FF_CLEAN_NPC_STATIC_MESH")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let weighted_joint_indices = if !static_mesh && !disable_skin {
        imported_mesh.used_joint_indices()
    } else {
        BTreeSet::new()
    };
    let palette_joint_indices = if !static_mesh && !disable_skin {
        clean_gltf_palette_joint_indices(&joints, &weighted_joint_indices)
    } else {
        Vec::new()
    };
    if !static_mesh && !disable_skin {
        let target_bones = palette_joint_indices
            .iter()
            .filter_map(|index| joints.get(*index))
            .map(|joint| joint.name.clone())
            .collect::<Vec<_>>();
        let remap_warnings = imported_mesh.remap_skin_to_bones(&target_bones);
        if !remap_warnings.is_empty() {
            eprintln!(
                "Clean GLB NPC skin remap warnings for {}:\n{}",
                model_path.display(),
                remap_warnings.join("\n")
            );
        }
    }
    let mesh_path_id = if static_mesh {
        static_mesh_path_id
    } else {
        skinned_mesh_path_id
    };
    let texture_path_id = if static_mesh {
        static_texture_path_id
    } else {
        skinned_texture_path_id
    };
    let material_path_id = if static_mesh {
        static_material_path_id
    } else {
        skinned_material_path_id
    };
    let mut imported_clips = if static_mesh || disable_animations {
        Vec::new()
    } else {
        let clips = fusionforge::modding::ImportedAnimationClip::from_gltf_path(model_path, 30.0)?;
        if clips.is_empty() {
            vec![fusionforge::modding::ImportedAnimationClip {
                name: "nif-default".to_string(),
                sample_rate: 30.0,
                duration: 0.0,
                translations: Vec::new(),
                rotations: Vec::new(),
                scales: Vec::new(),
            }]
        } else {
            clips
        }
    };
    if !static_mesh && !disable_animations {
        remap_imported_clip_paths_for_runtime(&mut imported_clips, &raw_joints, &joints)?;
    }
    normalize_clean_gltf_npc_clip_names(&mut imported_clips);
    expand_single_idle_clip_aliases(&mut imported_clips);
    if std::env::var("FF_DEBUG_CLEAN_NPC_CLIPS")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        eprintln!(
            "clean gltf npc clips for {}: {}",
            model_path.display(),
            imported_clips
                .iter()
                .map(|clip| clip.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if needs_bip01_wrapper {
        ensure_legacy_bip01_root_curves(&mut imported_clips);
    }
    let missing_animation_paths = if static_mesh || disable_animations {
        Vec::new()
    } else {
        missing_clean_gltf_animation_paths(&joints, &imported_clips)
    };
    if !missing_animation_paths.is_empty() {
        let preview = missing_animation_paths
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "{} animation path(s) do not exist in the generated runtime hierarchy for {}: {}",
            missing_animation_paths.len(),
            model_path.display(),
            preview
        ));
    }
    let clip_path_ids = (0..imported_clips.len())
        .map(|index| first_clip_path_id + index as i64)
        .collect::<Vec<_>>();
    let force_single_idle_clip_name = imported_clips.len() == 1
        && imported_clips
            .first()
            .is_some_and(|clip| clip.name == "stand1");

    let mut values = Vec::<(fusionforge::ObjectInfo, fusionforge::UnityValue)>::new();
    let root_child_transforms = if static_mesh {
        vec![static_mesh_transform]
    } else if needs_bip01_wrapper {
        vec![skinned_mesh_transform, bip01_wrapper_transform]
    } else {
        std::iter::once(skinned_mesh_transform)
            .chain(root_joint_transforms.iter().copied())
            .collect()
    };
    let bone_transform_path_ids = if disable_skin || static_mesh {
        joints
            .iter()
            .filter_map(|joint| joint_transform_path_by_node.get(&joint.node_index).copied())
            .collect::<Vec<_>>()
    } else {
        palette_joint_indices
            .iter()
            .filter_map(|index| joints.get(*index))
            .filter_map(|joint| joint_transform_path_by_node.get(&joint.node_index).copied())
            .collect::<Vec<_>>()
    };
    let skinned_renderer_bind_poses = if disable_skin {
        Vec::new()
    } else {
        imported_mesh.bind_poses.clone()
    };
    values.push((
        clean_object_info(root_go, 1),
        if static_mesh {
            clean_game_object_with_active(
                &asset,
                &clean_gltf_root_gameobject_name(internal_name, true),
                &[(4, root_transform), (111, static_animation)],
                true,
            )?
        } else {
            clean_game_object_with_active(
                &asset,
                &clean_gltf_root_gameobject_name(internal_name, false),
                &[(4, root_transform), (111, animation)],
                true,
            )?
        },
    ));
    values.push((
        clean_object_info(root_transform, 4),
        clean_transform(
            &asset,
            root_go,
            None,
            &root_child_transforms,
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0, 1.0),
            (1.0, 1.0, 1.0),
        )?,
    ));
    if static_mesh {
        values.push((
            clean_object_info(static_animation, 111),
            clean_animation(&asset, root_go, &[])?,
        ));
        values.push((
            clean_object_info(static_mesh_go, 1),
            clean_game_object_with_active(
                &asset,
                "Editable Poly",
                &[
                    (4, static_mesh_transform),
                    (33, mesh_filter),
                    (23, mesh_renderer),
                ],
                true,
            )?,
        ));
        values.push((
            clean_object_info(static_mesh_transform, 4),
            clean_transform(
                &asset,
                static_mesh_go,
                Some(root_transform),
                &[],
                mesh_local_translation,
                mesh_local_rotation,
                mesh_local_scale,
            )?,
        ));
        values.push((
            clean_object_info(mesh_filter, 33),
            clean_mesh_filter(&asset, static_mesh_go, mesh_path_id)?,
        ));
        values.push((
            clean_object_info(mesh_renderer, 23),
            clean_mesh_renderer(&asset, static_mesh_go, material_path_id)?,
        ));
    } else {
        values.push((
            clean_object_info(skinned_mesh_go, 1),
            clean_game_object_with_active(
                &asset,
                internal_name,
                &[(4, skinned_mesh_transform), (137, skinned_renderer)],
                true,
            )?,
        ));
        values.push((
            clean_object_info(skinned_mesh_transform, 4),
            clean_transform(
                &asset,
                skinned_mesh_go,
                Some(root_transform),
                &[],
                skinned_mesh_local_translation,
                skinned_mesh_local_rotation,
                skinned_mesh_local_scale,
            )?,
        ));
        values.push((
            clean_object_info(skinned_renderer, 137),
            clean_skinned_renderer(
                &asset,
                skinned_mesh_go,
                mesh_path_id,
                material_path_id,
                if disable_skin {
                    &[]
                } else {
                    &bone_transform_path_ids
                },
                &skinned_renderer_bind_poses,
            )?,
        ));
        values.push((
            clean_object_info(animation, 111),
            clean_animation(&asset, root_go, &clip_path_ids)?,
        ));
    }

    if needs_bip01_wrapper && !static_mesh {
        values.push((
            clean_object_info(bip01_wrapper_go, 1),
            clean_game_object_with_active(&asset, "Bip01", &[(4, bip01_wrapper_transform)], true)?,
        ));
        values.push((
            clean_object_info(bip01_wrapper_transform, 4),
            clean_transform(
                &asset,
                bip01_wrapper_go,
                Some(root_transform),
                &root_joint_transforms,
                bip01_wrapper_translation,
                bip01_wrapper_rotation,
                bip01_wrapper_scale,
            )?,
        ));
        values.push((
            clean_object_info(bip01_footsteps_go, 1),
            clean_game_object_with_active(
                &asset,
                "Bip01 Footsteps",
                &[(4, bip01_footsteps_transform)],
                true,
            )?,
        ));
        values.push((
            clean_object_info(bip01_footsteps_transform, 4),
            clean_transform(
                &asset,
                bip01_footsteps_go,
                first_nonaccum_root_transform.or(Some(bip01_wrapper_transform)),
                &[],
                (0.0, 0.0, 0.0),
                (0.0, 0.0, 0.0, 1.0),
                (1.0, 1.0, 1.0),
            )?,
        ));
    }

    if !static_mesh {
        for (index, joint) in joints.iter().enumerate() {
            let go_path_id = joint_go_path_by_node[&joint.node_index];
            let transform_path_id = joint_transform_path_by_node[&joint.node_index];
            let mut child_transform_ids = joint
                .children_node_indices
                .iter()
                .filter_map(|node| joint_transform_path_by_node.get(node).copied())
                .collect::<Vec<_>>();
            if needs_bip01_wrapper && joint.name == "Bip01 NonAccum" {
                child_transform_ids.push(bip01_footsteps_transform);
            }
            let father = joint
                .parent_node_index
                .and_then(|node| joint_transform_path_by_node.get(&node).copied())
                .or_else(|| {
                    if needs_bip01_wrapper {
                        Some(bip01_wrapper_transform)
                    } else {
                        Some(root_transform)
                    }
                });
            values.push((
                clean_object_info(go_path_id, 1),
                clean_game_object_with_active(
                    &asset,
                    &joint.name,
                    &[(4, transform_path_id)],
                    true,
                )?,
            ));
            values.push((
                clean_object_info(transform_path_id, 4),
                clean_transform(
                    &asset,
                    go_path_id,
                    father,
                    &child_transform_ids,
                    joint.translation,
                    joint.rotation,
                    joint.scale,
                )?,
            ));
            debug_assert_eq!(
                transform_path_id,
                first_joint_transform_path_id + index as i64
            );
        }
    }

    let mut mesh_value = asset.empty_object_value_for_class(43)?;
    if static_mesh || disable_skin {
        fusionforge::modding::apply_mesh_import_uncompressed_with_options(
            &mut mesh_value,
            imported_mesh,
            false,
        )?;
    } else {
        fusionforge::modding::apply_mesh_import_with_options(&mut mesh_value, imported_mesh, true)?;
        if let Some(template_mesh_value) = template_mesh_value.as_ref() {
            preserve_legacy_mesh_compressed_defaults(&mut mesh_value, template_mesh_value);
        }
    }
    set_object_field(&mut mesh_value, "topology", fusionforge::UnityValue::Int(0));
    values.push((clean_object_info(mesh_path_id, 43), mesh_value));

    let mut texture_value = asset.empty_object_value_for_class(28)?;
    fusionforge::modding::apply_texture_import(&mut texture_value, imported_texture)?;
    rewrite_texture_to_unity25_dxt3(&mut texture_value)?;
    values.push((clean_object_info(texture_path_id, 28), texture_value));

    values.push((
        clean_object_info(inline_shader_path_id, 48),
        inline_toon.shader_value,
    ));
    if let Some(mut ramp_value) = inline_toon.ramp_value {
        set_object_field(
            &mut ramp_value,
            "m_MipCount",
            fusionforge::UnityValue::Int(1),
        );
        set_object_field(
            &mut ramp_value,
            "m_IsReadable",
            fusionforge::UnityValue::Bool(false),
        );
        set_object_field(
            &mut ramp_value,
            "m_LightmapFormat",
            fusionforge::UnityValue::Int(0),
        );
        set_object_field(
            &mut ramp_value,
            "m_ColorSpace",
            fusionforge::UnityValue::Int(0),
        );
        set_object_field(
            &mut ramp_value,
            "m_StreamData",
            fusionforge::UnityValue::Object(BTreeMap::from([
                ("offset".to_string(), fusionforge::UnityValue::Int(0)),
                ("size".to_string(), fusionforge::UnityValue::Int(0)),
                (
                    "path".to_string(),
                    fusionforge::UnityValue::String(String::new()),
                ),
            ])),
        );
        values.push((clean_object_info(inline_ramp_path_id, 28), ramp_value));
    }

    values.push((
        clean_object_info(material_path_id, 21),
        clean_material(
            &asset,
            &format!("{internal_name}_material"),
            texture_path_id,
            shader_file_id,
            shader_path_id,
            shader_map_pointer,
            imported_material.as_ref(),
        )?,
    ));

    if !static_mesh {
        for (clip, path_id) in imported_clips.into_iter().zip(&clip_path_ids) {
            let mut clip_value = asset.empty_object_value_for_class(74)?;
            fusionforge::modding::apply_animation_clip_import(&mut clip_value, clip)?;
            if force_single_idle_clip_name {
                set_unity_object_string(&mut clip_value, "m_Name", "stand1");
            }
            values.push((clean_object_info(*path_id, 74), clip_value));
        }
    }

    let mut preload_path_ids = values
        .iter()
        .map(|(info, _)| info.path_id)
        .collect::<Vec<_>>();
    preload_path_ids.sort_unstable();
    values.push((
        clean_object_info(1, 142),
        clean_asset_bundle(
            &asset,
            bundle_name,
            &format!("mob/{internal_name}.kfm"),
            root_go,
            &format!("texture/{internal_name}.dds"),
            texture_path_id,
            &[],
            &preload_path_ids,
        )?,
    ));

    let mut object_data = Vec::<(fusionforge::ObjectInfo, Vec<u8>)>::new();
    for (info, value) in values {
        let data = asset
            .serialize_object_value_for_class(0, info.class_id, &value)
            .map_err(|err| {
                format!(
                    "clean object pathId {} classId {} value {} serialization failed: {err}",
                    info.path_id,
                    info.class_id,
                    unity_value_shape(&value)
                )
            })?;
        object_data.push((info, data));
    }
    let bytes = asset.rebuild_from_object_data_as_format(&object_data, Some(6))?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let output_temp = native_build_temp_dir("clean_npc_bundle")?;
    let raw_asset_path = output_temp.path().join(&asset.name);
    fs::write(&raw_asset_path, bytes)
        .map_err(|err| format!("{}: {err}", raw_asset_path.display()))?;
    let packed =
        ffbuildtool::bundle::AssetBundle::from_directory(&output_temp.path().to_string_lossy())?;
    packed.to_file(&output_path.to_string_lossy(), 4, None)?;
    sync_build_manifests_for_bundle(output_path)?;
    Ok(())
}
