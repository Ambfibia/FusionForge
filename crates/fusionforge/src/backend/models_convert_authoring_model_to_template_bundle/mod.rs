use super::super::*;

pub(in super::super) fn convert_authoring_model_to_template_bundle(
    authoring_model: &Path,
    template_bundle: &Path,
    template_model_paths: &BTreeSet<String>,
    target_model_path: &str,
    imported_texture: Option<(fusionforge::modding::ImportedTexture, String)>,
    output_bundle: &Path,
) -> Result<(usize, usize, usize, usize, Vec<String>, Vec<String>), String> {
    let target_object_name = asset_stem_from_container_path(target_model_path)
        .unwrap_or_else(|| "imported_model".to_string());
    let force_static_mesh_import = false;
    let target_bundle_name = output_bundle
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Character_Imported")
        .trim_end_matches("__authoring")
        .to_string();
    let target_character_name = imported_npc_character_name(&target_object_name);
    let mut imported_mesh = fusionforge::modding::ImportedMesh::from_model_path(
        authoring_model,
        Some(target_object_name.clone()),
    )?;
    let imported_animations = if force_static_mesh_import {
        Vec::new()
    } else {
        fusionforge::modding::ImportedAnimationClip::from_gltf_path(authoring_model, 30.0)?
    };
    let animation_names = imported_animations
        .iter()
        .map(|clip| clip.name.clone())
        .collect::<Vec<_>>();

    let temp = native_build_temp_dir("npc_model_convert")?;
    extract_bundle_native_to_dir(template_bundle, temp.path())?;
    let env = fusionforge::UnityEnvironment::from_dir(temp.path());
    if env.assets.is_empty() {
        return Err(format!(
            "{} did not contain serialized Unity assets",
            template_bundle.display()
        ));
    }

    let mut selected = BTreeSet::<(usize, i64)>::new();
    let mut queue = VecDeque::<(usize, i64)>::new();
    let mut matched_paths = Vec::<String>::new();
    let mut target_model_root_keys = BTreeSet::<(usize, i64)>::new();
    let mut assetbundle_replacements = BTreeMap::<(usize, i64), fusionforge::UnityValue>::new();
    let mut object_name_overrides = BTreeMap::<(usize, i64), String>::new();
    let mut renamed_paths = 0usize;

    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            if asset.object_type_name(info) != "AssetBundle" {
                continue;
            }
            let body = asset.read_object(asset_index, info)?;
            let preload_table = fusionforge::value_array(body.get("m_PreloadTable")).to_vec();
            let mut body_replacement = body.clone();
            let renamed = rename_assetbundle_container_paths(
                &mut body_replacement,
                template_model_paths,
                target_model_path,
            );
            set_unity_object_string(&mut body_replacement, "m_Name", &target_bundle_name);
            let removed = remove_assetbundle_container_prefix(&mut body_replacement, "icons/")
                + remove_assetbundle_container_prefix(&mut body_replacement, "vo/")
                + remove_assetbundle_container_prefix(&mut body_replacement, "ui sound/");
            if renamed > 0 || removed > 0 {
                renamed_paths += renamed;
                assetbundle_replacements.insert((asset_index, info.path_id), body_replacement);
            }

            for entry in fusionforge::value_array(body.get("m_Container")) {
                let Some((path, metadata)) = fusionforge::pair_name_value(entry) else {
                    continue;
                };
                if !model_container_path_matches(path, template_model_paths) {
                    continue;
                }
                matched_paths.push(path.to_string());
                let Some(pointer) = metadata
                    .get("asset")
                    .and_then(fusionforge::UnityValue::as_pointer)
                else {
                    continue;
                };
                for key in pointer_candidate_keys(&env, pointer) {
                    object_name_overrides.insert(key, target_object_name.clone());
                    if env
                        .assets
                        .get(key.0)
                        .and_then(|asset| asset.objects.get(&key.1).map(|info| (asset, info)))
                        .is_some_and(|(asset, info)| asset.object_type_name(info) == "GameObject")
                    {
                        target_model_root_keys.insert(key);
                    }
                }
                add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
                let (start, end) = metadata_preload_range(metadata, preload_table.len());
                for preload in &preload_table[start..end] {
                    if let Some(pointer) = preload.as_pointer() {
                        add_resolved_pointer_root(&env, pointer, &mut selected, &mut queue);
                    }
                }
            }
        }
    }

    if matched_paths.is_empty() {
        return Err(format!(
            "{} did not contain template model path(s): {}",
            template_bundle.display(),
            template_model_paths
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    while let Some((asset_index, path_id)) = queue.pop_front() {
        let Some(asset) = env.assets.get(asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(&path_id) else {
            continue;
        };
        let Ok(body) = asset.read_object(asset_index, info) else {
            continue;
        };
        let mut pointers = Vec::new();
        collect_value_pointers(&body, &mut pointers);
        for pointer in pointers {
            add_resolved_pointer_root(&env, &pointer, &mut selected, &mut queue);
        }
    }

    if !selected_contains_mesh(&env, &selected) {
        let tokens = preview_name_tokens(template_model_paths, &matched_paths);
        add_fuzzy_named_meshes(&env, &tokens, &mut selected);
    }

    let texture_setup = if let Some((texture, target_texture_path)) = imported_texture {
        let mut candidates = selected
            .iter()
            .filter_map(|(asset_index, path_id)| {
                let asset = env.assets.get(*asset_index)?;
                let info = asset.objects.get(path_id)?;
                (asset.object_type_name(info) == "Texture2D").then_some((*asset_index, *path_id))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(asset_index, path_id)| {
            let Some(asset) = env.assets.get(*asset_index) else {
                return 0i64;
            };
            let Some(info) = asset.objects.get(path_id) else {
                return 0i64;
            };
            asset
                .read_object(*asset_index, info)
                .ok()
                .and_then(|body| {
                    let width = body
                        .get("m_Width")
                        .and_then(fusionforge::UnityValue::as_i64)
                        .unwrap_or_default();
                    let height = body
                        .get("m_Height")
                        .and_then(fusionforge::UnityValue::as_i64)
                        .unwrap_or_default();
                    Some(width.saturating_mul(height))
                })
                .unwrap_or_default()
        });
        let Some(key) = candidates.pop() else {
            return Err(format!(
                "{} has a sidecar texture, but the template model dependency closure had no Texture2D object to replace.",
                authoring_model.display()
            ));
        };
        Some((key, texture, target_texture_path))
    } else {
        None
    };
    if let Some((texture_key, _, target_texture_path)) = texture_setup.as_ref() {
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                if asset.object_type_name(info) != "AssetBundle" {
                    continue;
                }
                let body = asset.read_object(asset_index, info)?;
                let mut body_replacement = assetbundle_replacements
                    .get(&(asset_index, info.path_id))
                    .cloned()
                    .unwrap_or(body);
                let renamed = rename_assetbundle_texture_container_path(
                    &env,
                    &mut body_replacement,
                    *texture_key,
                    target_texture_path,
                );
                if renamed > 0 {
                    renamed_paths += renamed;
                    assetbundle_replacements.insert((asset_index, info.path_id), body_replacement);
                    if let Some(target_name) = asset_stem_from_container_path(target_texture_path) {
                        object_name_overrides.insert(*texture_key, target_name);
                    }
                }
            }
        }
    }
    let target_material_name = texture_setup
        .as_ref()
        .and_then(|(_, _, target_texture_path)| asset_stem_from_container_path(target_texture_path))
        .unwrap_or_else(|| target_object_name.clone());
    let mut imported_material_keys = BTreeSet::<(usize, i64)>::new();
    let mut disabled_renderer_keys = BTreeSet::<(usize, i64)>::new();
    if let Some((texture_key, _, _)) = texture_setup.as_ref() {
        let mut glass_material_keys = BTreeSet::<(usize, i64)>::new();
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                if asset.object_type_name(info) != "Material" {
                    continue;
                }
                let body = asset.read_object(asset_index, info)?;
                let key = (asset_index, info.path_id);
                if material_main_texture_matches(&env, &body, *texture_key) {
                    imported_material_keys.insert(key);
                    object_name_overrides.insert(key, target_material_name.clone());
                } else if material_name_suggests_glass(&body) {
                    glass_material_keys.insert(key);
                }
            }
        }
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                let object_type = asset.object_type_name(info);
                if object_type != "MeshRenderer" && object_type != "SkinnedMeshRenderer" {
                    continue;
                }
                let body = asset.read_object(asset_index, info)?;
                if glass_material_keys
                    .iter()
                    .any(|material_key| renderer_uses_material_key(&env, &body, *material_key))
                {
                    disabled_renderer_keys.insert((asset_index, info.path_id));
                }
            }
        }
    }
    for (asset_index, path_id) in &selected {
        let Some(asset) = env.assets.get(*asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(path_id) else {
            continue;
        };
        let object_type = asset.object_type_name(info);
        if object_type == "MeshRenderer" || object_type == "SkinnedMeshRenderer" {
            disabled_renderer_keys.insert((*asset_index, *path_id));
        }
    }
    let active_gameobject_keys = selected
        .iter()
        .filter_map(|(asset_index, path_id)| {
            let asset = env.assets.get(*asset_index)?;
            let info = asset.objects.get(path_id)?;
            (asset.object_type_name(info) == "GameObject").then_some((*asset_index, *path_id))
        })
        .collect::<BTreeSet<_>>();

    let mut mesh_candidates = selected
        .iter()
        .filter_map(|(asset_index, path_id)| {
            let asset = env.assets.get(*asset_index)?;
            let info = asset.objects.get(path_id)?;
            (asset.object_type_name(info) == "Mesh").then_some((*asset_index, *path_id))
        })
        .collect::<Vec<_>>();
    mesh_candidates.sort_by_key(|(asset_index, path_id)| {
        let Some(asset) = env.assets.get(*asset_index) else {
            return 0usize;
        };
        let Some(info) = asset.objects.get(path_id) else {
            return 0usize;
        };
        asset
            .read_object(*asset_index, info)
            .ok()
            .and_then(|body| fusionforge::extract_mesh(&body).map(|mesh| mesh.vertices.len()))
            .unwrap_or_default()
    });
    mesh_candidates.reverse();
    let Some(mesh_key) = mesh_candidates.first().copied() else {
        return Err(format!(
            "{} matched {} model container path(s), but no Unity Mesh was found to replace. \
             This template is probably pure KFM/NIF or uses a mesh layout the native converter cannot rewrite yet.",
            template_bundle.display(),
            matched_paths.len()
        ));
    };
    let static_renderer_setup = if let Some(material_key) =
        imported_material_keys.iter().next().copied()
    {
        let target_gameobject_key = target_model_root_keys
            .iter()
            .copied()
            .find(|key| {
                env.assets
                    .get(key.0)
                    .and_then(|asset| asset.objects.get(&key.1).map(|info| (asset, info)))
                    .is_some_and(|(asset, info)| asset.object_type_name(info) == "GameObject")
            })
            .or_else(|| {
                object_name_overrides.iter().find_map(|(key, name)| {
                    if name != &target_object_name {
                        return None;
                    }
                    env.assets
                        .get(key.0)
                        .and_then(|asset| asset.objects.get(&key.1).map(|info| (asset, info)))
                        .is_some_and(|(asset, info)| asset.object_type_name(info) == "GameObject")
                        .then_some(*key)
                })
            })
            .or_else(|| {
                selected.iter().copied().find(|(asset_index, path_id)| {
                    let Some(asset) = env.assets.get(*asset_index) else {
                        return false;
                    };
                    let Some(info) = asset.objects.get(path_id) else {
                        return false;
                    };
                    asset.object_type_name(info) == "GameObject"
                        && asset
                            .read_object(*asset_index, info)
                            .ok()
                            .map(|body| fusionforge::object_name(&body))
                            .is_some_and(|name| name == target_object_name)
                })
            })
            .or_else(|| {
                env.assets
                    .iter()
                    .enumerate()
                    .find_map(|(asset_index, asset)| {
                        asset.objects.values().find_map(|info| {
                            if asset.object_type_name(info) != "GameObject" {
                                return None;
                            }
                            let body = asset.read_object(asset_index, info).ok()?;
                            (fusionforge::object_name(&body) == target_object_name)
                                .then_some((asset_index, info.path_id))
                        })
                    })
            });
        let mut skinned_candidates = selected
            .iter()
            .filter_map(|(asset_index, path_id)| {
                let asset = env.assets.get(*asset_index)?;
                let info = asset.objects.get(path_id)?;
                (asset.object_type_name(info) == "SkinnedMeshRenderer")
                    .then_some((*asset_index, *path_id))
            })
            .collect::<Vec<_>>();
        skinned_candidates.sort_by_key(|(asset_index, path_id)| {
            let Some(asset) = env.assets.get(*asset_index) else {
                return 1i32;
            };
            let Some(info) = asset.objects.get(path_id) else {
                return 1i32;
            };
            let Ok(body) = asset.read_object(*asset_index, info) else {
                return 1i32;
            };
            if renderer_mesh_key(&env, &body) == Some(mesh_key) {
                0
            } else {
                1
            }
        });
        let skinned_key = skinned_candidates.first().copied();
        let mesh_renderer_key =
            disabled_renderer_keys
                .iter()
                .copied()
                .find(|(asset_index, path_id)| {
                    env.assets
                        .get(*asset_index)
                        .and_then(|asset| {
                            asset
                                .objects
                                .get(path_id)
                                .map(|info| asset.object_type_name(info) == "MeshRenderer")
                        })
                        .unwrap_or(false)
                });
        let mesh_filter_key = mesh_renderer_key.and_then(|renderer_key| {
            let renderer_asset = env.assets.get(renderer_key.0)?;
            let renderer_info = renderer_asset.objects.get(&renderer_key.1)?;
            let renderer_body = renderer_asset
                .read_object(renderer_key.0, renderer_info)
                .ok()?;
            let renderer_gameobject_key = renderer_gameobject_key(&env, &renderer_body)?;
            let gameobject_asset = env.assets.get(renderer_gameobject_key.0)?;
            let gameobject_info = gameobject_asset.objects.get(&renderer_gameobject_key.1)?;
            let gameobject_body = gameobject_asset
                .read_object(renderer_gameobject_key.0, gameobject_info)
                .ok()?;
            component_path_id_from_gameobject(&gameobject_body, 33)
                .map(|path_id| (renderer_gameobject_key.0, path_id))
        });
        if let (
            Some(skinned_key),
            Some(mesh_filter_key),
            Some(mesh_renderer_key),
            Some(gameobject_key),
        ) = (
            skinned_key,
            mesh_filter_key,
            mesh_renderer_key,
            target_gameobject_key,
        ) {
            let gameobject_asset = env.assets.get(gameobject_key.0).ok_or_else(|| {
                format!(
                    "Renderer GameObject asset index {} is outside the template asset list",
                    gameobject_key.0
                )
            })?;
            let gameobject_info =
                gameobject_asset
                    .objects
                    .get(&gameobject_key.1)
                    .ok_or_else(|| {
                        format!("{}#{} not found", gameobject_asset.name, gameobject_key.1)
                    })?;
            let gameobject_body =
                gameobject_asset.read_object(gameobject_key.0, gameobject_info)?;
            transform_path_id_from_gameobject(&gameobject_body).map(|transform_path_id| {
                let mesh_transform_path_id =
                    transform_path_id_from_gameobject_key(&env, gameobject_key)
                        .unwrap_or(transform_path_id);
                (
                    skinned_key,
                    mesh_filter_key,
                    mesh_renderer_key,
                    gameobject_key,
                    transform_path_id,
                    mesh_transform_path_id,
                    material_key,
                )
            })
        } else {
            None
        }
    } else {
        None
    }
    .or_else(|| {
        let material_key = imported_material_keys.iter().next().copied()?;
        env.assets
            .iter()
            .enumerate()
            .find_map(|(asset_index, asset)| {
                let has_template_static = asset.objects.contains_key(&928)
                    && asset.objects.contains_key(&1113)
                    && asset.objects.contains_key(&1114)
                    && asset.objects.contains_key(&1115);
                if has_template_static {
                    let gameobject_key = target_model_root_keys
                        .iter()
                        .copied()
                        .find(|key| {
                            key.0 == asset_index
                                && asset.objects.get(&key.1).is_some_and(|info| {
                                    asset.object_type_name(info) == "GameObject"
                                })
                        })
                        .or_else(|| {
                            object_name_overrides.iter().find_map(|(key, name)| {
                                if name != &target_object_name || key.0 != asset_index {
                                    return None;
                                }
                                asset
                                    .objects
                                    .get(&key.1)
                                    .is_some_and(|info| {
                                        asset.object_type_name(info) == "GameObject"
                                    })
                                    .then_some(*key)
                            })
                        })
                        .unwrap_or((asset_index, 928));
                    let transform_path_id = asset
                        .objects
                        .get(&gameobject_key.1)
                        .and_then(|info| asset.read_object(gameobject_key.0, info).ok())
                        .and_then(|body| transform_path_id_from_gameobject(&body))
                        .unwrap_or(884);
                    Some((
                        (asset_index, 1113),
                        (asset_index, 1114),
                        (asset_index, 1115),
                        gameobject_key,
                        transform_path_id,
                        transform_path_id_from_gameobject_key(&env, gameobject_key)
                            .unwrap_or(transform_path_id),
                        material_key,
                    ))
                } else {
                    None
                }
            })
    });
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            let object_type = asset.object_type_name(info);
            if object_type == "MeshRenderer" || object_type == "SkinnedMeshRenderer" {
                disabled_renderer_keys.insert((asset_index, info.path_id));
            }
        }
    }
    if let Some((_, _, mesh_renderer_key, _, _, _, _)) = static_renderer_setup {
        disabled_renderer_keys.remove(&mesh_renderer_key);
    }
    let skinned_renderer_setup = (!force_static_mesh_import)
        .then(|| {
            imported_material_keys
                .iter()
                .next()
                .copied()
                .and_then(|material_key| {
                    selected.iter().copied().find_map(|(asset_index, path_id)| {
                        let asset = env.assets.get(asset_index)?;
                        let info = asset.objects.get(&path_id)?;
                        (asset.object_type_name(info) == "SkinnedMeshRenderer")
                            .then_some(((asset_index, path_id), material_key))
                    })
                })
        })
        .flatten();
    if let Some((skinned_key, _)) = skinned_renderer_setup {
        disabled_renderer_keys.remove(&skinned_key);
    }
    if let Some((skinned_key, _)) = skinned_renderer_setup {
        let bone_names = skinned_renderer_bone_names(&env, skinned_key);
        if !bone_names.is_empty() {
            let _remap_warnings = imported_mesh.remap_skin_to_bones(&bone_names);
        }
    }
    if let Some((_, _, _, gameobject_key, _, mesh_transform_path_id, _)) = static_renderer_setup {
        let z_offset =
            template_transform_z_offset(&env, (gameobject_key.0, mesh_transform_path_id));
        compensate_static_template_transform(&mut imported_mesh, z_offset);
    } else if target_object_name.eq_ignore_ascii_case("npc_otto") {
        compensate_static_template_transform(&mut imported_mesh, 0.3847014605998993);
    }
    let imported_mesh_transform_keys = if imported_material_keys.is_empty() {
        BTreeSet::new()
    } else {
        let mut keys = BTreeSet::new();
        for (asset_index, asset) in env.assets.iter().enumerate() {
            for info in asset.objects.values() {
                let object_type = asset.object_type_name(info);
                if object_type != "MeshRenderer" && object_type != "SkinnedMeshRenderer" {
                    continue;
                }
                let Ok(body) = asset.read_object(asset_index, info) else {
                    continue;
                };
                let uses_imported_material = imported_material_keys
                    .iter()
                    .any(|material_key| renderer_uses_material_key(&env, &body, *material_key));
                if !uses_imported_material {
                    continue;
                }
                let Some(gameobject_key) = renderer_gameobject_key(&env, &body) else {
                    continue;
                };
                if let Some(path_id) = transform_path_id_from_gameobject_key(&env, gameobject_key) {
                    keys.insert((gameobject_key.0, path_id));
                }
            }
        }
        keys
    };

    let animation_setup = if imported_animations.is_empty() {
        None
    } else {
        let animation_key = find_animation_component_key(&env, &selected).ok_or_else(|| {
            format!(
                "{} contains GLTF animation clips, but the template bundle has no Unity Animation component to attach them to.",
                authoring_model.display()
            )
        })?;
        let animation_asset = env.assets.get(animation_key.0).ok_or_else(|| {
            format!(
                "Animation component asset index {} is outside the template asset list",
                animation_key.0
            )
        })?;
        let clip_template = first_animation_clip_info(animation_asset)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "{} has an Animation component but no AnimationClip type template in asset {}; cannot create native clip objects.",
                    template_bundle.display(),
                    animation_asset.name
                )
            })?;
        let first_path_id = asset_max_path_id(animation_asset).saturating_add(1);
        let path_ids = (0..imported_animations.len())
            .map(|offset| first_path_id.saturating_add(offset as i64))
            .collect::<Vec<_>>();
        Some((animation_key, clip_template, path_ids))
    };

    for (asset_index, path_id) in &selected {
        let Some(asset) = env.assets.get(*asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(path_id) else {
            continue;
        };
        let object_type = asset.object_type_name(info);
        let Ok(body) = asset.read_object(*asset_index, info) else {
            continue;
        };
        let name = fusionforge::object_name(&body);
        let replacement = match (object_type.as_str(), name.as_str()) {
            ("GameObject", "Stanley") => Some(target_character_name.clone()),
            ("GameObject", "Stanley NonAccum") => Some(format!("{target_character_name} NonAccum")),
            ("GameObject", "GameIcon") => Some(format!("{target_object_name}_icon")),
            ("GameObject", "Editable Poly") => Some(format!("{target_object_name}_mesh")),
            ("Mesh", "GameIcon") => Some(format!("{target_object_name}_icon")),
            _ => None,
        }
        .or_else(|| {
            let lower = name.to_ascii_lowercase();
            if lower.contains("johnnytest") {
                if object_type == "GameObject" {
                    Some(target_object_name.clone())
                } else if object_type == "Mesh"
                    || object_type == "Material"
                    || object_type == "Texture2D"
                {
                    Some(target_object_name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });
        if let Some(replacement) = replacement {
            object_name_overrides.insert((*asset_index, *path_id), replacement);
        }
    }

    let mut replaced_meshes = 0usize;
    let mut replaced_textures = 0usize;
    let mut created_animation_clips = 0usize;
    for (asset_index, asset) in env.assets.iter().enumerate() {
        let mut replacements = BTreeMap::<i64, fusionforge::UnityValue>::new();
        for ((replacement_asset_index, path_id), value) in &assetbundle_replacements {
            if *replacement_asset_index == asset_index {
                replacements.insert(*path_id, value.clone());
            }
        }
        if mesh_key.0 == asset_index {
            let info = asset
                .objects
                .get(&mesh_key.1)
                .ok_or_else(|| format!("{}#{} not found", asset.name, mesh_key.1))?;
            let mut value = asset.read_object(asset_index, info)?;
            let mut mesh_for_import = imported_mesh.clone();
            if skinned_renderer_setup.is_some()
                && !target_object_name.eq_ignore_ascii_case("npc_otto")
            {
                let template_bind_poses = mesh_bind_poses_from_value(&value);
                if !template_bind_poses.is_empty() {
                    mesh_for_import.bind_poses = template_bind_poses;
                }
            }
            fusionforge::modding::apply_mesh_import_with_options(
                &mut value,
                mesh_for_import,
                skinned_renderer_setup.is_some() && !force_static_mesh_import,
            )?;
            replacements.insert(mesh_key.1, value);
            replaced_meshes += 1;
        }
        if let Some((texture_key, texture, _target_texture_path)) = texture_setup.as_ref() {
            if texture_key.0 == asset_index {
                let info = asset
                    .objects
                    .get(&texture_key.1)
                    .ok_or_else(|| format!("{}#{} not found", asset.name, texture_key.1))?;
                let mut value = asset.read_object(asset_index, info)?;
                fusionforge::modding::apply_texture_import(&mut value, texture.clone())?;
                replacements.insert(texture_key.1, value);
                replaced_textures += 1;
            }
        }
        for (material_asset_index, path_id) in &imported_material_keys {
            if *material_asset_index != asset_index {
                continue;
            }
            let Some(info) = asset.objects.get(path_id) else {
                continue;
            };
            let mut value = if let Some(value) = replacements.remove(path_id) {
                value
            } else {
                asset.read_object(asset_index, info)?
            };
            retarget_imported_material(&mut value, &target_material_name);
            replacements.insert(*path_id, value);
        }
        if let Some((
            skinned_key,
            mesh_filter_key,
            mesh_renderer_key,
            gameobject_key,
            transform_path_id,
            mesh_transform_path_id,
            material_key,
        )) = static_renderer_setup
        {
            if skinned_key.0 == asset_index {
                if let Some(info) = asset.objects.get(&skinned_key.1) {
                    let mut value = if let Some(value) = replacements.remove(&skinned_key.1) {
                        value
                    } else {
                        asset.read_object(asset_index, info)?
                    };
                    disable_renderer(&mut value);
                    replacements.insert(skinned_key.1, value);
                }
            }
            if mesh_filter_key.0 == asset_index {
                if let Some(info) = asset.objects.get(&mesh_filter_key.1) {
                    let mut value = if let Some(value) = replacements.remove(&mesh_filter_key.1) {
                        value
                    } else {
                        asset.read_object(asset_index, info)?
                    };
                    retarget_mesh_filter(&mut value, asset_index, gameobject_key.1, mesh_key.1);
                    replacements.insert(mesh_filter_key.1, value);
                }
            }
            if mesh_renderer_key.0 == asset_index {
                if let Some(info) = asset.objects.get(&mesh_renderer_key.1) {
                    let mut value = if let Some(value) = replacements.remove(&mesh_renderer_key.1) {
                        value
                    } else {
                        asset.read_object(asset_index, info)?
                    };
                    retarget_mesh_renderer(
                        &mut value,
                        asset_index,
                        gameobject_key.1,
                        material_key.1,
                    );
                    replacements.insert(mesh_renderer_key.1, value);
                }
            }
            if gameobject_key.0 == asset_index {
                if let Some(info) = asset.objects.get(&gameobject_key.1) {
                    let mut value = if let Some(value) = replacements.remove(&gameobject_key.1) {
                        value
                    } else {
                        asset.read_object(asset_index, info)?
                    };
                    set_gameobject_static_mesh_components(
                        &mut value,
                        asset_index,
                        transform_path_id,
                        mesh_filter_key.1,
                        mesh_renderer_key.1,
                    );
                    replacements.insert(gameobject_key.1, value);
                }
            }
            if gameobject_key.0 == asset_index {
                if let Some(info) = asset.objects.get(&mesh_transform_path_id) {
                    let mut value =
                        if let Some(value) = replacements.remove(&mesh_transform_path_id) {
                            value
                        } else {
                            asset.read_object(asset_index, info)?
                        };
                    reset_imported_mesh_transform(&mut value);
                    replacements.insert(mesh_transform_path_id, value);
                }
            }
        }
        if let Some((skinned_key, material_key)) = skinned_renderer_setup {
            if skinned_key.0 == asset_index {
                if let Some(info) = asset.objects.get(&skinned_key.1) {
                    let mut value = if let Some(value) = replacements.remove(&skinned_key.1) {
                        value
                    } else {
                        asset.read_object(asset_index, info)?
                    };
                    retarget_skinned_mesh_renderer(
                        &mut value,
                        asset_index,
                        mesh_key.1,
                        material_key.1,
                    );
                    replacements.insert(skinned_key.1, value);
                }
            }
        }
        for (gameobject_asset_index, path_id) in &active_gameobject_keys {
            if *gameobject_asset_index != asset_index {
                continue;
            }
            let Some(info) = asset.objects.get(path_id) else {
                continue;
            };
            let mut value = if let Some(value) = replacements.remove(path_id) {
                value
            } else {
                asset.read_object(asset_index, info)?
            };
            set_gameobject_active(&mut value, true);
            replacements.insert(*path_id, value);
        }
        for (renderer_asset_index, path_id) in &disabled_renderer_keys {
            if *renderer_asset_index != asset_index {
                continue;
            }
            if static_renderer_setup.is_some_and(|(_, _, mesh_renderer_key, _, _, _, _)| {
                mesh_renderer_key == (*renderer_asset_index, *path_id)
            }) {
                continue;
            }
            let Some(info) = asset.objects.get(path_id) else {
                continue;
            };
            let mut value = if let Some(value) = replacements.remove(path_id) {
                value
            } else {
                asset.read_object(asset_index, info)?
            };
            disable_renderer(&mut value);
            replacements.insert(*path_id, value);
        }
        if let Some((_, _, mesh_renderer_key, gameobject_key, _, _, material_key)) =
            static_renderer_setup
        {
            if mesh_renderer_key.0 == asset_index {
                if let Some(info) = asset.objects.get(&mesh_renderer_key.1) {
                    let mut value = if let Some(value) = replacements.remove(&mesh_renderer_key.1) {
                        value
                    } else {
                        asset.read_object(asset_index, info)?
                    };
                    retarget_mesh_renderer(
                        &mut value,
                        asset_index,
                        gameobject_key.1,
                        material_key.1,
                    );
                    replacements.insert(mesh_renderer_key.1, value);
                }
            }
        }
        if !imported_material_keys.is_empty() {
            for info in asset.objects.values() {
                let object_type = asset.object_type_name(info);
                if object_type != "MeshRenderer" && object_type != "SkinnedMeshRenderer" {
                    continue;
                }
                let mut value = if let Some(value) = replacements.remove(&info.path_id) {
                    value
                } else {
                    asset.read_object(asset_index, info)?
                };
                let uses_imported_material = imported_material_keys
                    .iter()
                    .any(|material_key| renderer_uses_material_key(&env, &value, *material_key));
                if uses_imported_material {
                    enable_renderer(&mut value);
                }
                replacements.insert(info.path_id, value);
            }
        }
        if let Some((_, _, _, gameobject_key, _, _, _)) = static_renderer_setup {
            if gameobject_key.0 == asset_index {
                for info in asset.objects.values() {
                    if asset.object_type_name(info) != "Transform" {
                        continue;
                    }
                    let mut value = if let Some(value) = replacements.remove(&info.path_id) {
                        value
                    } else {
                        asset.read_object(asset_index, info)?
                    };
                    if transform_gameobject_key(&env, &value) == Some(gameobject_key) {
                        reset_imported_mesh_transform(&mut value);
                    }
                    replacements.insert(info.path_id, value);
                }
            }
        }
        for (transform_asset_index, path_id) in &imported_mesh_transform_keys {
            if *transform_asset_index != asset_index {
                continue;
            }
            let Some(info) = asset.objects.get(path_id) else {
                continue;
            };
            let mut value = if let Some(value) = replacements.remove(path_id) {
                value
            } else {
                asset.read_object(asset_index, info)?
            };
            reset_imported_mesh_transform(&mut value);
            replacements.insert(*path_id, value);
        }
        for ((override_asset_index, path_id), name) in &object_name_overrides {
            if *override_asset_index != asset_index || *path_id == mesh_key.1 {
                continue;
            }
            if imported_material_keys.contains(&(*override_asset_index, *path_id)) {
                continue;
            }
            let Some(info) = asset.objects.get(path_id) else {
                continue;
            };
            let mut value = if let Some(value) = replacements.remove(path_id) {
                value
            } else {
                asset.read_object(asset_index, info)?
            };
            set_unity_object_string(&mut value, "m_Name", name);
            replacements.insert(*path_id, value);
        }

        let mut extra_objects = Vec::<(fusionforge::ObjectInfo, Vec<u8>)>::new();
        if let Some((animation_key, clip_template, clip_path_ids)) = &animation_setup {
            if animation_key.0 == asset_index {
                let info = asset
                    .objects
                    .get(&animation_key.1)
                    .ok_or_else(|| format!("{}#{} not found", asset.name, animation_key.1))?;
                let mut value = if let Some(value) = replacements.remove(&animation_key.1) {
                    value
                } else {
                    asset.read_object(asset_index, info)?
                };
                append_animation_clip_pointers(&mut value, asset_index, clip_path_ids)?;
                replacements.insert(animation_key.1, value);

                for (clip, path_id) in imported_animations.iter().cloned().zip(clip_path_ids) {
                    let mut value = asset.read_object(asset_index, clip_template)?;
                    fusionforge::modding::apply_animation_clip_import(&mut value, clip)?;
                    let data = asset.serialize_object_value(asset_index, clip_template, &value)?;
                    let mut info = clip_template.clone();
                    info.path_id = *path_id;
                    info.data_offset = 0;
                    info.size = data.len() as u32;
                    extra_objects.push((info, data));
                    created_animation_clips += 1;
                }
            }
        }

        let mut keep_ids = selected
            .iter()
            .filter_map(|(selected_asset_index, path_id)| {
                (*selected_asset_index == asset_index).then_some(*path_id)
            })
            .collect::<BTreeSet<_>>();
        keep_ids.extend(replacements.keys().copied());
        for (transform_asset_index, path_id) in &imported_mesh_transform_keys {
            if *transform_asset_index == asset_index {
                keep_ids.insert(*path_id);
            }
        }
        for info in asset.objects.values() {
            if asset.object_type_name(info) == "AssetBundle" {
                keep_ids.insert(info.path_id);
            }
        }
        if let Some((
            skinned_key,
            mesh_filter_key,
            mesh_renderer_key,
            gameobject_key,
            transform_path_id,
            mesh_transform_path_id,
            material_key,
        )) = static_renderer_setup
        {
            for key in [
                skinned_key,
                mesh_filter_key,
                mesh_renderer_key,
                gameobject_key,
                (gameobject_key.0, transform_path_id),
                (gameobject_key.0, mesh_transform_path_id),
                material_key,
                mesh_key,
            ] {
                if key.0 == asset_index {
                    keep_ids.insert(key.1);
                }
            }
        }
        if let Some((skinned_key, material_key)) = skinned_renderer_setup {
            for key in [skinned_key, material_key, mesh_key] {
                if key.0 == asset_index {
                    keep_ids.insert(key.1);
                }
            }
        }
        if let Some((texture_key, _, _)) = texture_setup.as_ref() {
            if texture_key.0 == asset_index {
                keep_ids.insert(texture_key.1);
            }
        }
        if let Some((animation_key, clip_template, _)) = &animation_setup {
            if animation_key.0 == asset_index {
                keep_ids.insert(animation_key.1);
                keep_ids.insert(clip_template.path_id);
            }
        }

        if replacements.is_empty() && extra_objects.is_empty() && keep_ids.is_empty() {
            continue;
        }
        let asset_path = extracted_asset_path(temp.path(), &asset.name)?;
        let mut data_replacements = BTreeMap::<i64, Vec<u8>>::new();
        for (path_id, value) in &replacements {
            let info = asset
                .objects
                .get(path_id)
                .ok_or_else(|| format!("{}#{} not found", asset.name, path_id))?;
            data_replacements.insert(
                *path_id,
                asset.serialize_object_value(asset_index, info, value)?,
            );
        }
        let data = asset.rebuild_with_object_data_filtered_and_extra(
            Some(&keep_ids),
            &data_replacements,
            &extra_objects,
        )?;
        fs::write(&asset_path, data).map_err(|err| format!("{}: {err}", asset_path.display()))?;
    }

    if let Some(parent) = output_bundle.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    pack_bundle_native_from_dir(temp.path(), output_bundle)?;
    matched_paths.sort();
    matched_paths.dedup();
    Ok((
        replaced_meshes,
        renamed_paths,
        replaced_textures,
        created_animation_clips,
        matched_paths,
        animation_names,
    ))
}
