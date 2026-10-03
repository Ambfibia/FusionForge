use super::*;

pub(super) const EXPORT_SCHEMA: &str = "ffone.native-static-world-export.v1";

#[derive(Debug, Clone)]
pub struct NativeStaticWorldExportOptions {
    pub map_bundle: PathBuf,
    pub build_root: PathBuf,
    pub native_asset_root: PathBuf,
    pub output_root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeStaticWorldExportReport {
    pub schema: String,
    pub status: String,
    pub source_build: String,
    pub tile_id: String,
    pub source_archive: String,
    pub source_archive_blake3: String,
    pub output_root: String,
    pub scene_path: String,
    pub scene_blake3: String,
    pub hierarchy_path: String,
    pub hierarchy_blake3: String,
    pub material_path: String,
    pub material_blake3: String,
    pub catalog_path: String,
    pub catalog_blake3: String,
    pub manifest_path: String,
    pub counts: ExportCounts,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCounts {
    pub scene_assets: usize,
    pub scene_nodes: usize,
    pub scene_roots: usize,
    pub source_mesh_renderers: usize,
    pub source_skinned_mesh_renderers: usize,
    pub source_mesh_colliders: usize,
    pub scripted_prefab_roots: usize,
    pub scripted_prefab_nodes: usize,
    pub exported_visual_payloads: usize,
    pub runtime_visuals: usize,
    pub exported_collider_payloads: usize,
    pub runtime_colliders: usize,
    pub empty_source_mesh_payloads: usize,
    pub exported_models: usize,
    pub unique_source_meshes: usize,
    pub vertices: usize,
    pub indices: usize,
    pub exact_materials: usize,
    pub exact_textures: usize,
    pub exact_texture_mips: usize,
    pub output_files: usize,
    pub output_bytes: u64,
}

/// Export a complete static map scene into a fresh, self-contained native
/// asset overlay. The original build and native terrain tree remain immutable.
pub fn export_native_static_world(
    options: NativeStaticWorldExportOptions,
) -> Result<NativeStaticWorldExportReport, String> {
    validate_fresh_output(&options.output_root)?;
    let build_root = canonical_directory(&options.build_root, "effective build root")?;
    let asset_root = canonical_directory(&options.native_asset_root, "native asset root")?;
    let map_bundle = canonical_file(&options.map_bundle, "map bundle")?;
    if !map_bundle.starts_with(&build_root) {
        return Err(format!(
            "map bundle {} is outside effective build root {}",
            map_bundle.display(),
            build_root.display()
        ));
    }
    let (map_id, _bundle_tile_id, tile) = parse_map_bundle_identity(&map_bundle)?;
    let layout = resolve_tile_layout(&asset_root, tile)?;
    let tile_id = layout.tile_id.clone();
    let source_build = build_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "effective build root has no UTF-8 source-build name".to_string())?
        .to_string();
    let source_archive_bytes = fs::read(&map_bundle)
        .map_err(|err| format!("could not read {}: {err}", map_bundle.display()))?;
    let source_archive_blake3 = blake3_hex(&source_archive_bytes);
    let replace_verified_static = verified_existing_static_publication(
        &asset_root,
        &layout,
        &source_build,
        &source_archive_blake3,
    )?;

    let output_parent = options
        .output_root
        .parent()
        .ok_or_else(|| "fresh output root has no parent".to_string())?;
    fs::create_dir_all(output_parent).map_err(|err| {
        format!(
            "could not create output parent {}: {err}",
            output_parent.display()
        )
    })?;
    let output_parent = canonical_directory(output_parent, "output parent")?;
    if asset_root.starts_with(&options.output_root) || options.output_root.starts_with(&asset_root)
    {
        return Err(
            "native asset root and fresh output root must not contain one another".to_string(),
        );
    }

    let extract = ScratchDirectory::fresh(&output_parent, "ffone-static-world-extract")?;
    extract_world_environment(&map_bundle, &build_root, &extract.path)?;
    let env = UnityEnvironment::from_dir(&extract.path);
    if env.assets.is_empty() {
        return Err("world extraction produced no readable Unity serialized assets".to_string());
    }

    let mut extraction = collect_scene(&env, &map_id)?;
    if extraction.counts.scene_nodes == 0
        || extraction.payloads.is_empty()
        || extraction.counts.source_mesh_colliders == 0
    {
        return Err(format!(
            "{} is not a complete static scene: nodes={}, payloads={}, colliders={}",
            map_id,
            extraction.counts.scene_nodes,
            extraction.payloads.len(),
            extraction.counts.source_mesh_colliders
        ));
    }

    let exact_materials = logical_model_material::export_exact_mesh_materials_from_selection(
        &env,
        &extraction.selected_meshes,
        Some(&extraction.selected_material_objects),
    )
    .map_err(|err| format!("exact material closure failed: {err}"))?;
    let material_slots = renderer_material_slots(&exact_materials)?;
    extraction.counts.exact_materials = exact_materials.materials.len();
    extraction.counts.exact_textures = exact_materials.textures.len();

    let staging = ScratchDirectory::fresh(&output_parent, "ffone-static-world-stage")?;
    let source_tile_relative = layout.source_tile_relative.clone();
    let source_tile = join_relative(&asset_root, &source_tile_relative)?;
    let destination_tile = join_relative(&staging.path, &source_tile_relative)?;
    copy_tree_exact(&source_tile, &destination_tile)?;

    let model_relative_root = layout.model_relative_root.clone();
    let model_root = join_relative(&staging.path, &model_relative_root)?;
    fs::create_dir_all(model_root.join("textures")).map_err(|err| {
        format!(
            "could not create static-world model directory {}: {err}",
            model_root.display()
        )
    })?;
    let texture_publication =
        publish_exact_textures(&exact_materials, &model_root, &model_relative_root)?;
    extraction.counts.exact_texture_mips = texture_publication.mip_count;

    let mut models = Vec::<JsonValue>::new();
    let mut visuals = Vec::<JsonValue>::new();
    let mut colliders = Vec::<JsonValue>::new();
    let mut published = Vec::<PublishedPayload>::new();
    let mut unique_meshes = BTreeSet::new();
    let mut used_model_paths = BTreeSet::new();
    let mut used_runtime_names = BTreeSet::new();

    extraction
        .payloads
        .sort_by(|left, right| left.id.cmp(&right.id));
    for (ordinal, payload) in extraction.payloads.iter().enumerate() {
        let mesh = read_exact_mesh(&env, payload.source_mesh, &payload.id)?;
        unique_meshes.insert((payload.source_mesh.asset, payload.source_mesh.path_id));
        let slots = payload
            .source_renderer
            .and_then(|renderer| material_slots.get(&renderer).cloned())
            .unwrap_or_default();
        let has_geometry =
            !mesh.vertices.is_empty() && mesh.triangles.iter().any(|indices| !indices.is_empty());
        let (
            model_id,
            model_path,
            model_blake3,
            root_name,
            vertex_count,
            index_count,
            primitive_count,
        ) = if has_geometry {
            let root_name = format!(
                "{}__{:05}__{}",
                match payload.kind {
                    PayloadKind::Visual => "visual",
                    PayloadKind::Collider => "collider",
                },
                ordinal,
                safe_name(&payload.name, "unnamed")
            );
            let file_name = format!(
                "{}-{:05}-{}-{}.glb",
                match payload.kind {
                    PayloadKind::Visual => "v",
                    PayloadKind::Collider => "c",
                },
                ordinal,
                payload.source_component.path_id,
                &blake3_hex(payload.id.as_bytes())[..12]
            );
            let model_path = format!("{model_relative_root}/{file_name}");
            if !used_model_paths.insert(model_path.clone()) {
                return Err(format!("duplicate generated model path {model_path}"));
            }
            let glb = build_single_root_glb(
                &root_name,
                &mesh,
                payload.world_matrix,
                payload.kind,
                &slots,
                &exact_materials.materials,
                &texture_publication.files_by_id,
                &payload.id,
            )?;
            let glb_path = join_relative(&staging.path, &model_path)?;
            write_new_file(&glb_path, &glb.bytes)?;
            validate_single_root_glb(&glb.bytes, &root_name, &model_path)?;
            let model_blake3 = blake3_hex(&glb.bytes);
            let model_id = format!("static-{tile_id}-{ordinal:05}");
            models.push(json!({
                "id": model_id,
                "path": model_path,
                "blake3": model_blake3,
                "rootName": root_name,
            }));
            (
                Some(model_id),
                Some(model_path),
                Some(model_blake3),
                Some(root_name),
                glb.vertex_count,
                glb.index_count,
                glb.primitive_count,
            )
        } else {
            extraction.counts.empty_source_mesh_payloads += 1;
            (None, None, None, None, 0, 0, 0)
        };
        let runtime_published =
            has_geometry && payload.effective_active && payload.component_enabled;
        if runtime_published {
            let runtime_model_id = model_id
                .as_deref()
                .ok_or_else(|| "internal runtime payload has no model".to_string())?;
            let runtime_name = format!(
                "{} [{}#{} payload:{ordinal:05}]",
                nonempty_name(&payload.name, "unnamed"),
                env.asset_name(payload.source_component.asset),
                payload.source_component.path_id
            );
            if !used_runtime_names.insert(runtime_name.clone()) {
                return Err(format!("duplicate runtime scene name {runtime_name:?}"));
            }
            match payload.kind {
                PayloadKind::Visual => visuals.push(json!({
                    "name": runtime_name,
                    "model": runtime_model_id,
                    "scene": 0,
                    "transform": IDENTITY_TRANSFORM,
                })),
                PayloadKind::Collider => colliders.push(json!({
                    "name": runtime_name,
                    "kind": "triangleMesh",
                    "model": runtime_model_id,
                    "mesh": 0,
                    "primitive": 0,
                    "isTrigger": payload.is_trigger,
                    "expectedVertexCount": vertex_count,
                    "expectedIndexCount": index_count,
                    "transform": IDENTITY_TRANSFORM,
                })),
            }
        }
        extraction.counts.vertices = extraction
            .counts
            .vertices
            .checked_add(vertex_count)
            .ok_or_else(|| "published vertex count overflow".to_string())?;
        extraction.counts.indices = extraction
            .counts
            .indices
            .checked_add(index_count)
            .ok_or_else(|| "published index count overflow".to_string())?;
        match payload.kind {
            PayloadKind::Visual => {
                extraction.counts.exported_visual_payloads += 1;
                if runtime_published {
                    extraction.counts.runtime_visuals += 1;
                }
            }
            PayloadKind::Collider => {
                extraction.counts.exported_collider_payloads += 1;
                if runtime_published {
                    extraction.counts.runtime_colliders += 1;
                }
            }
        }
        published.push(PublishedPayload {
            id: payload.id.clone(),
            name: payload.name.clone(),
            kind: match payload.kind {
                PayloadKind::Visual => "visual",
                PayloadKind::Collider => "collider",
            }
            .to_string(),
            hierarchy_node_id: payload.hierarchy_node_id.clone(),
            source_component: source_identity(&env, payload.source_component)?,
            source_game_object: source_identity(&env, payload.source_game_object)?,
            source_mesh: source_identity(&env, payload.source_mesh)?,
            source_mesh_filter: payload
                .source_mesh_filter
                .map(|key| source_identity(&env, key))
                .transpose()?,
            source_renderer: payload
                .source_renderer
                .map(|key| source_identity(&env, key))
                .transpose()?,
            world_matrix: payload.world_matrix,
            transform_policy: if has_geometry {
                "exact native world matrix baked once into local GLB vertices; runtime TRS is identity"
            } else {
                "source mesh has no triangle geometry; component retained as metadata with no fabricated GLB/runtime instance"
            }
            .to_string(),
            effective_active: payload.effective_active,
            component_enabled: payload.component_enabled,
            runtime_published,
            geometry_status: if has_geometry {
                "published-glb"
            } else {
                "empty-source-mesh"
            }
            .to_string(),
            model_id,
            model_path,
            model_blake3,
            root_name,
            vertex_count,
            index_count,
            primitive_count,
            material_ids: slots,
            is_trigger: payload.is_trigger,
        });
    }
    extraction.counts.unique_source_meshes = unique_meshes.len();
    extraction.counts.exported_models = models.len();

    if extraction.counts.exported_visual_payloads
        != extraction.counts.source_mesh_renderers + extraction.counts.source_skinned_mesh_renderers
    {
        return Err(format!(
            "renderer accounting mismatch: exported {}, source MeshRenderer+SkinnedMeshRenderer {}",
            extraction.counts.exported_visual_payloads,
            extraction.counts.source_mesh_renderers
                + extraction.counts.source_skinned_mesh_renderers
        ));
    }
    if extraction.counts.exported_collider_payloads != extraction.counts.source_mesh_colliders {
        return Err(format!(
            "MeshCollider accounting mismatch: exported {}, source {}",
            extraction.counts.exported_collider_payloads, extraction.counts.source_mesh_colliders
        ));
    }

    let static_relative_root = layout.static_relative_root.clone();
    let static_root = join_relative(&staging.path, &static_relative_root)?;
    fs::create_dir_all(&static_root).map_err(|err| {
        format!(
            "could not create static-world metadata directory {}: {err}",
            static_root.display()
        )
    })?;
    let material_relative = format!("{static_relative_root}/materials.json");
    let material_document = json!({
        "schema": MATERIAL_SCHEMA,
        "sourceBuild": source_build,
        "tileId": tile_id,
        "materials": exact_materials.materials,
        "textures": texture_publication.document,
        "rendererMaterialBindings": exact_materials.renderer_bindings,
        "nonUniformMeshMaterialBindings": exact_materials.non_uniform_mesh_materials,
        "contract": {
            "sourceOrderPreserved": true,
            "nullSlotsPreserved": true,
            "allMipLevelsPublished": true,
            "texturePixelTransform": "vertical-flip-only-for-png-top-left-origin",
            "previewResizeApplied": false,
            "previewColorProcessingApplied": false
        }
    });
    let material_bytes = pretty_json_bytes(&material_document)?;
    write_new_file(
        &join_relative(&staging.path, &material_relative)?,
        &material_bytes,
    )?;
    let material_blake3 = blake3_hex(&material_bytes);

    let hierarchy_relative = format!("{static_relative_root}/hierarchy.json");
    extraction
        .hierarchy_nodes
        .sort_by(|left, right| json_string(left, "id").cmp(&json_string(right, "id")));
    let hierarchy_document = json!({
        "schema": HIERARCHY_SCHEMA,
        "sourceBuild": source_build,
        "sourceArchive": slash_path(&map_bundle),
        "sourceArchiveBlake3": source_archive_blake3,
        "tileId": tile_id,
        "coordinateContract": {
            "space": "native",
            "basis": "H=diag(-1,1,1)",
            "unitScale": "1-unity-unit-equals-1-bevy-unit",
            "originPolicy": "source-trs-unchanged-no-auto-centering"
        },
        "transformContract": {
            "hierarchy": "every source Transform retained parent-relative as exact f64 native TRS and exact composed native world matrix",
            "runtimeGeometry": "world matrix is baked once into each static payload because ffone.native-world-scene.v2 instances are flat TRS records",
            "terrain": "TerrainData is excluded and remains the separately published native Gray16 terrain"
        },
        "sceneAssets": extraction.scene_asset_names,
        "nodes": extraction.hierarchy_nodes,
        "payloads": published,
        "counts": extraction.counts,
    });
    let hierarchy_bytes = pretty_json_bytes(&hierarchy_document)?;
    write_new_file(
        &join_relative(&staging.path, &hierarchy_relative)?,
        &hierarchy_bytes,
    )?;
    let hierarchy_blake3 = blake3_hex(&hierarchy_bytes);

    let scene_relative = format!("{source_tile_relative}/scene.json");
    let scene_path = join_relative(&staging.path, &scene_relative)?;
    let scene_source_bytes = fs::read(&scene_path).map_err(|err| {
        format!(
            "could not read copied scene {}: {err}",
            scene_path.display()
        )
    })?;
    let mut scene: JsonValue = serde_json::from_slice(&scene_source_bytes).map_err(|err| {
        format!(
            "invalid copied native scene {}: {err}",
            scene_path.display()
        )
    })?;
    enrich_native_scene(
        &mut scene,
        &layout.scope,
        &tile_id,
        tile,
        models,
        visuals,
        colliders,
        replace_verified_static,
    )?;
    let scene_bytes = pretty_json_bytes(&scene)?;
    replace_regular_file(&scene_path, &scene_bytes)?;
    let scene_blake3 = blake3_hex(&scene_bytes);

    let catalog_relative = format!("{static_relative_root}/catalog.json");
    let catalog_document = json!({
        "schema": CATALOG_SCHEMA,
        "status": "complete-hash-verified-no-preview-budget",
        "sourceBuild": source_build,
        "sourceArchive": slash_path(&map_bundle),
        "sourceArchiveBlake3": source_archive_blake3,
        "tileId": tile_id,
        "scene": { "path": scene_relative, "blake3": scene_blake3 },
        "hierarchy": { "path": hierarchy_relative, "blake3": hierarchy_blake3 },
        "materials": { "path": material_relative, "blake3": material_blake3 },
        "nativeTerrainCopiedFrom": source_tile_relative,
        "terrainGlbGenerated": false,
        "counts": extraction.counts,
    });
    let catalog_bytes = pretty_json_bytes(&catalog_document)?;
    write_new_file(
        &join_relative(&staging.path, &catalog_relative)?,
        &catalog_bytes,
    )?;
    let catalog_blake3 = blake3_hex(&catalog_bytes);

    let report_relative = "export-report.json".to_string();
    let manifest_relative = "export-manifest.json".to_string();
    let mut final_report = NativeStaticWorldExportReport {
        schema: EXPORT_SCHEMA.to_string(),
        status: "complete-hash-verified-no-preview-budget".to_string(),
        source_build: source_build.clone(),
        tile_id: tile_id.clone(),
        source_archive: slash_path(&map_bundle),
        source_archive_blake3: source_archive_blake3.clone(),
        output_root: slash_path(&options.output_root),
        scene_path: scene_relative.clone(),
        scene_blake3: scene_blake3.clone(),
        hierarchy_path: hierarchy_relative.clone(),
        hierarchy_blake3: hierarchy_blake3.clone(),
        material_path: material_relative.clone(),
        material_blake3: material_blake3.clone(),
        catalog_path: catalog_relative.clone(),
        catalog_blake3: catalog_blake3.clone(),
        manifest_path: manifest_relative.clone(),
        counts: extraction.counts.clone(),
    };
    finalize_report_and_manifest(
        &staging.path,
        &report_relative,
        &manifest_relative,
        &source_build,
        &tile_id,
        &mut final_report,
    )?;
    extraction.counts = final_report.counts.clone();

    verify_complete_output(
        &staging.path,
        &scene_relative,
        &catalog_relative,
        &manifest_relative,
        extraction.counts.exported_models,
        extraction.counts.runtime_visuals,
        extraction.counts.runtime_colliders,
    )?;
    let final_source_hash = blake3_hex(
        &fs::read(&map_bundle).map_err(|err| format!("could not re-read source archive: {err}"))?,
    );
    if final_source_hash != source_archive_blake3 {
        return Err("source map archive changed during static-world export".to_string());
    }
    fs::rename(&staging.path, &options.output_root).map_err(|err| {
        format!(
            "could not atomically commit static-world output {} -> {}: {err}",
            staging.path.display(),
            options.output_root.display()
        )
    })?;

    Ok(final_report)
}

pub(super) fn publish_exact_textures(
    exact: &ExactMaterialExport,
    model_root: &Path,
    model_relative_root: &str,
) -> Result<TexturePublication, String> {
    let texture_root = model_root.join("textures");
    let mut files_by_id = BTreeMap::new();
    let mut document = JsonMap::new();
    let mut used_names = BTreeSet::new();
    let mut mip_count = 0usize;

    for (texture_id, texture) in &exact.textures {
        let true_name = texture
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("texture");
        let path_id = texture
            .pointer("/source/pathId")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| format!("exact texture {texture_id:?} has no source pathId"))?;
        let stem = format!(
            "{}-{}-{}",
            safe_name(true_name, "texture"),
            path_id,
            &blake3_hex(texture_id.as_bytes())[..12]
        );
        let levels = texture
            .get("mipLevels")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("exact texture {texture_id:?} has no mipLevels array"))?;
        if levels.is_empty() {
            return Err(format!("exact texture {texture_id:?} has no mip levels"));
        }
        let mut sanitized = texture.clone();
        let sanitized_levels = sanitized
            .get_mut("mipLevels")
            .and_then(JsonValue::as_array_mut)
            .ok_or_else(|| format!("exact texture {texture_id:?} mipLevels changed shape"))?;
        for (index, (source_level, sanitized_level)) in
            levels.iter().zip(sanitized_levels.iter_mut()).enumerate()
        {
            let level = source_level
                .get("level")
                .and_then(JsonValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| format!("texture {texture_id:?} mip {index} has invalid level"))?;
            let data_url = source_level
                .pointer("/payload/dataUrl")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| {
                    format!("texture {texture_id:?} mip {level} has no exact PNG dataUrl")
                })?;
            let encoded = data_url
                .strip_prefix("data:image/png;base64,")
                .ok_or_else(|| {
                    format!("texture {texture_id:?} mip {level} payload is not PNG data")
                })?;
            let png = STANDARD.decode(encoded).map_err(|err| {
                format!("texture {texture_id:?} mip {level} base64 is invalid: {err}")
            })?;
            let file_name = if level == 0 {
                format!("{stem}.png")
            } else {
                format!("{stem}.mip-{level}.png")
            };
            if !used_names.insert(file_name.clone()) {
                return Err(format!("duplicate exact texture filename {file_name}"));
            }
            validate_png(&png, texture_id, level)?;
            write_new_file(&texture_root.join(&file_name), &png)?;
            let relative = format!("{model_relative_root}/textures/{file_name}");
            if level == 0 {
                files_by_id.insert(texture_id.clone(), relative.clone());
            }
            let payload = sanitized_level
                .get_mut("payload")
                .and_then(JsonValue::as_object_mut)
                .ok_or_else(|| format!("texture {texture_id:?} mip {level} has invalid payload"))?;
            payload.remove("dataUrl");
            payload.insert("path".to_string(), json!(relative));
            payload.insert("blake3".to_string(), json!(blake3_hex(&png)));
            mip_count += 1;
        }
        let sanitized_base_payload = sanitized_levels
            .first()
            .and_then(|level| level.get("payload"))
            .cloned()
            .ok_or_else(|| format!("texture {texture_id:?} has no sanitized base payload"))?;
        if files_by_id.get(texture_id).is_none() {
            return Err(format!("texture {texture_id:?} did not publish base mip"));
        }
        if let Some(payload) = sanitized.get_mut("payload") {
            *payload = sanitized_base_payload;
        }
        document.insert(texture_id.clone(), sanitized);
    }
    Ok(TexturePublication {
        files_by_id,
        document: JsonValue::Object(document),
        mip_count,
    })
}

#[derive(Debug, Clone)]
pub struct NativeStaticBehaviourExportOptions {
    pub map_bundle: PathBuf,
    pub build_root: PathBuf,
    pub output_file: PathBuf,
}

/// Export the exact serialized state of every non-geometry scene component of
/// one map tile.
///
/// This is deliberately separate from `export_native_static_world`: it neither
/// reads nor copies the native asset root, so it can run against a project
/// whose scenes are already enriched, and it cannot change the hash of an
/// already reviewed static-world export.
pub fn export_native_static_behaviours(
    options: NativeStaticBehaviourExportOptions,
) -> Result<JsonValue, String> {
    let build_root = canonical_directory(&options.build_root, "effective build root")?;
    let map_bundle = canonical_file(&options.map_bundle, "map bundle")?;
    if !map_bundle.starts_with(&build_root) {
        return Err(format!(
            "map bundle {} is outside effective build root {}",
            map_bundle.display(),
            build_root.display()
        ));
    }
    let (map_id, _bundle_tile_id, tile) = parse_map_bundle_identity(&map_bundle)?;
    let source_build = build_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "effective build root has no UTF-8 source-build name".to_string())?
        .to_string();
    let source_archive_blake3 = blake3_hex(
        &fs::read(&map_bundle)
            .map_err(|err| format!("could not read {}: {err}", map_bundle.display()))?,
    );

    let output_parent = options
        .output_file
        .parent()
        .ok_or_else(|| "output file has no parent".to_string())?;
    fs::create_dir_all(output_parent).map_err(|err| {
        format!(
            "could not create output parent {}: {err}",
            output_parent.display()
        )
    })?;
    let output_parent = canonical_directory(output_parent, "output parent")?;

    let extract = ScratchDirectory::fresh(&output_parent, "ffone-static-behaviour-extract")?;
    extract_world_environment(&map_bundle, &build_root, &extract.path)?;
    let env = UnityEnvironment::from_dir(&extract.path);
    if env.assets.is_empty() {
        return Err("world extraction produced no readable Unity serialized assets".to_string());
    }
    let extraction = collect_scene(&env, &map_id)?;
    let (behaviours, scripts, animation_clips, effect_prefab_closures, counts) =
        export_behaviours(&env, &extraction)?;

    let document = json!({
        "schema": BEHAVIOUR_SCHEMA,
        "status": "complete-exact-serialized-state",
        "sourceBuild": source_build,
        "sourceArchive": slash_path(&map_bundle),
        "sourceArchiveBlake3": source_archive_blake3,
        "mapId": map_id,
        "tile": tile,
        "contract": {
            "join": "every record joins the published hierarchy.json by its exact node id; its source-derived world matrix is retained and must match that hierarchy when one is published",
            "fieldPolicy": "exact serialized fields minus m_Enabled/m_GameObject/m_Name/m_Script, which are recorded as typed record keys",
            "geometryExcluded": "MeshFilter, MeshRenderer, SkinnedMeshRenderer, MeshCollider and Transform are represented by the published GLB payloads and hierarchy"
        },
        "scripts": scripts,
        "animationClips": animation_clips,
        "effectPrefabClosures": effect_prefab_closures,
        "behaviours": behaviours,
        "counts": counts,
    });
    let bytes = pretty_json_bytes(&document)?;
    write_new_file(&options.output_file, &bytes)?;
    Ok(json!({
        "schema": BEHAVIOUR_SCHEMA,
        "mapId": map_id,
        "output": slash_path(&options.output_file),
        "blake3": blake3_hex(&bytes),
        "byteLength": bytes.len(),
        "counts": counts,
    }))
}
