use super::*;

pub(super) const TERRAIN_SCHEMA: &str = "ffone.native-terrain.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerrainSelection {
    First,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TerrainPublishMode {
    AtomicDirectoryRename,
    CallerOwnedStaging,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportedTerrain {
    pub(super) true_name: String,
    pub(super) terrain_data_path_id: i64,
    pub(super) document: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerrainVertexShift {
    pub(super) flags: u8,
    pub(super) column: u32,
    pub(super) row: u32,
}

/// Export one TerrainData object already proven by an exact AssetBundle route.
/// Tutorial.resourceFile contains nine TerrainData objects, so batch export
/// must never fall back to whichever object happens to sort first.
pub fn export_native_terrain_exact(
    env: &UnityEnvironment,
    source_path: &Path,
    output_dir: &Path,
    key: ObjectKey,
) -> Result<usize, String> {
    export_native_terrain_impl(
        env,
        &BTreeSet::new(),
        source_path,
        output_dir,
        TerrainSelection::First,
        Some(key),
        TerrainPublishMode::AtomicDirectoryRename,
    )
}

/// Batch export already owns an isolated root that is atomically committed as
/// one directory. Writing the tile directly inside that root avoids a second,
/// Windows-hostile nested directory rename while `StagingDirectory` still
/// removes every partial tile on failure.
pub fn export_native_terrain_exact_in_caller_staging(
    env: &UnityEnvironment,
    source_path: &Path,
    output_dir: &Path,
    key: ObjectKey,
) -> Result<usize, String> {
    export_native_terrain_impl(
        env,
        &BTreeSet::new(),
        source_path,
        output_dir,
        TerrainSelection::First,
        Some(key),
        TerrainPublishMode::CallerOwnedStaging,
    )
}

pub fn export_native_terrain(
    env: &UnityEnvironment,
    source_asset_names: &BTreeSet<String>,
    source_path: &Path,
    output_dir: &Path,
    selection: TerrainSelection,
) -> Result<usize, String> {
    export_native_terrain_impl(
        env,
        source_asset_names,
        source_path,
        output_dir,
        selection,
        None,
        TerrainPublishMode::AtomicDirectoryRename,
    )
}

pub(super) fn export_native_terrain_impl(
    env: &UnityEnvironment,
    source_asset_names: &BTreeSet<String>,
    source_path: &Path,
    output_dir: &Path,
    selection: TerrainSelection,
    exact_key: Option<ObjectKey>,
    publish_mode: TerrainPublishMode,
) -> Result<usize, String> {
    if output_dir.exists() {
        return Err(format!(
            "native terrain output must not already exist: {}",
            output_dir.display()
        ));
    }
    let source_bytes = fs::read(source_path).map_err(|err| {
        format!(
            "could not read native terrain source {}: {err}",
            source_path.display()
        )
    })?;
    let canonical_source = fs::canonicalize(source_path).map_err(|err| {
        format!(
            "could not canonicalize native terrain source {}: {err}",
            source_path.display()
        )
    })?;
    let source = SourceBundle {
        path: canonical_source.to_string_lossy().to_string(),
        blake3: hash_bytes(&source_bytes),
    };

    let parent = output_dir.parent().ok_or_else(|| {
        format!(
            "native terrain output has no parent directory: {}",
            output_dir.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "could not create native terrain output parent {}: {err}",
            parent.display()
        )
    })?;
    let staging_dir = match publish_mode {
        TerrainPublishMode::AtomicDirectoryRename => fresh_staging_path(output_dir)?,
        TerrainPublishMode::CallerOwnedStaging => output_dir.to_path_buf(),
    };
    fs::create_dir(&staging_dir).map_err(|err| {
        format!(
            "could not create native terrain staging directory {}: {err}",
            staging_dir.display()
        )
    })?;
    let mut staging = StagingDirectory::new(staging_dir);

    let candidates = if let Some(key) = exact_key {
        vec![terrain_candidate_exact(env, key)?]
    } else {
        terrain_candidates(env, source_asset_names)?
    };
    if candidates.is_empty() {
        return Err("source contains no readable TerrainData objects".to_string());
    }
    let selected = match selection {
        TerrainSelection::First => &candidates[..1],
        TerrainSelection::All => candidates.as_slice(),
    };

    let mut exported = Vec::with_capacity(selected.len());
    let mut semantic_names = BTreeMap::<String, String>::new();
    for candidate in selected {
        let true_name = required_true_name(&candidate.body, "TerrainData", candidate.key.path_id)?;
        let terrain_root = if selection == TerrainSelection::First {
            staging.path().to_path_buf()
        } else {
            let component = semantic_component(&true_name)?;
            register_semantic_name(&mut semantic_names, &component, &true_name)?;
            staging.path().join("terrains").join(component)
        };
        fs::create_dir_all(&terrain_root).map_err(|err| {
            format!(
                "could not create terrain directory {}: {err}",
                terrain_root.display()
            )
        })?;
        export_one_terrain(env, &source, candidate, &true_name, &terrain_root)?;
        let document = if selection == TerrainSelection::First {
            "terrain.json".to_string()
        } else {
            format!("terrains/{}/terrain.json", semantic_component(&true_name)?)
        };
        exported.push(ExportedTerrain {
            true_name,
            terrain_data_path_id: candidate.key.path_id,
            document,
        });
    }

    let manifest = ExportManifest {
        schema: EXPORT_SCHEMA,
        source,
        selection: match (exact_key, selection) {
            (Some(_), _) => "exact-container-route",
            (None, TerrainSelection::First) => "first",
            (None, TerrainSelection::All) => "all",
        },
        terrains: exported,
    };
    write_json_new(&staging.path().join("manifest.json"), &manifest)?;

    if publish_mode == TerrainPublishMode::AtomicDirectoryRename {
        if output_dir.exists() {
            return Err(format!(
                "native terrain output appeared during export: {}",
                output_dir.display()
            ));
        }
        atomic_publish_directory(staging.path(), output_dir).map_err(|err| {
            format!(
                "could not atomically publish native terrain {} -> {} after bounded transient-lock retry: {err}",
                staging.path().display(),
                output_dir.display()
            )
        })?;
    }
    staging.keep();
    Ok(selected.len())
}

pub(super) struct TerrainCandidate {
    pub(super) key: ObjectKey,
    pub(super) asset_name: String,
    pub(super) body: UnityValue,
}

pub(super) fn terrain_candidates(
    env: &UnityEnvironment,
    source_asset_names: &BTreeSet<String>,
) -> Result<Vec<TerrainCandidate>, String> {
    let mut candidates = Vec::new();
    for (asset_index, asset) in env.assets.iter().enumerate() {
        if !source_asset_names.contains(&asset.name) {
            continue;
        }
        for (path_id, info) in &asset.objects {
            if asset.object_type_name(info) != "TerrainData" {
                continue;
            }
            let body = asset.read_object(asset_index, info).map_err(|err| {
                format!(
                    "could not read TerrainData {}#{}: {err}",
                    asset.name, path_id
                )
            })?;
            candidates.push(TerrainCandidate {
                key: ObjectKey {
                    asset: asset_index,
                    path_id: *path_id,
                },
                asset_name: asset.name.clone(),
                body,
            });
        }
    }
    Ok(candidates)
}

pub(super) fn terrain_candidate_exact(
    env: &UnityEnvironment,
    key: ObjectKey,
) -> Result<TerrainCandidate, String> {
    let asset = env.assets.get(key.asset).ok_or_else(|| {
        format!(
            "exact TerrainData source asset index {} is missing",
            key.asset
        )
    })?;
    let info = asset.objects.get(&key.path_id).ok_or_else(|| {
        format!(
            "exact TerrainData object {}#{} is missing",
            asset.name, key.path_id
        )
    })?;
    if asset.object_type_name(info) != "TerrainData" {
        return Err(format!(
            "exact container target {}#{} is {}, expected TerrainData",
            asset.name,
            key.path_id,
            asset.object_type_name(info)
        ));
    }
    let body = asset.read_object(key.asset, info).map_err(|err| {
        format!(
            "could not read exact TerrainData {}#{}: {err}",
            asset.name, key.path_id
        )
    })?;
    Ok(TerrainCandidate {
        key,
        asset_name: asset.name.clone(),
        body,
    })
}

pub(super) fn export_one_terrain(
    env: &UnityEnvironment,
    source: &SourceBundle,
    candidate: &TerrainCandidate,
    true_name: &str,
    output_dir: &Path,
) -> Result<(), String> {
    let heightmap = required_object(&candidate.body, "m_Heightmap", true_name)?;
    let width = required_dimension(heightmap, "m_Width", true_name)?;
    let height = required_dimension(heightmap, "m_Height", true_name)?;
    let sample_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| format!("{true_name}: height sample count overflows usize"))?;
    let source_heights = exact_u16_array(
        heightmap.get("m_Heights"),
        sample_count,
        &format!("{true_name}.m_Heightmap.m_Heights"),
    )?;
    let canonical_heights = canonicalize_unity_heights(width, height, &source_heights)?;
    let scale = required_vec3(
        heightmap.get("m_Scale"),
        &format!("{true_name}.m_Heightmap.m_Scale"),
    )?;
    if scale
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(format!(
            "{true_name}: heightmap scale must contain three finite positive values"
        ));
    }
    let vertex_shifts = exact_vertex_shifts(
        heightmap.get("m_Shifts"),
        width,
        height,
        &format!("{true_name}.m_Heightmap.m_Shifts"),
    )?;

    let raw_min = *source_heights
        .iter()
        .min()
        .ok_or_else(|| format!("{true_name}: empty heightmap"))?;
    let raw_max = *source_heights
        .iter()
        .max()
        .ok_or_else(|| format!("{true_name}: empty heightmap"))?;
    let source_raw_bytes = u16_le_bytes(&source_heights);
    let canonical_raw_bytes = u16_le_bytes(&canonical_heights);
    let height_png = encode_gray16_png(width, height, &canonical_heights)?;
    write_new(&output_dir.join("heightmap.png"), &height_png)?;

    let splat_database = required_object(&candidate.body, "m_SplatDatabase", true_name)?;
    let alphamap_resolution =
        required_dimension(splat_database, "m_AlphamapResolution", true_name)?;
    let splats = value_array(splat_database.get("m_Splats"));
    if splats.is_empty() {
        return Err(format!("{true_name}: m_SplatDatabase.m_Splats is empty"));
    }
    let alpha_pointers = value_array(splat_database.get("m_AlphaTextures"));
    let expected_weight_maps = splats.len().div_ceil(4);
    if alpha_pointers.len() != expected_weight_maps {
        return Err(format!(
            "{true_name}: {} layers require {expected_weight_maps} RGBA weight maps, found {}",
            splats.len(),
            alpha_pointers.len()
        ));
    }

    let weights_dir = output_dir.join("weights");
    fs::create_dir(&weights_dir).map_err(|err| {
        format!(
            "could not create native terrain weights directory {}: {err}",
            weights_dir.display()
        )
    })?;
    let mut weight_documents = Vec::with_capacity(alpha_pointers.len());
    for (map_index, pointer_value) in alpha_pointers.iter().enumerate() {
        let pointer = required_pointer(
            pointer_value,
            &format!("{true_name}.m_SplatDatabase.m_AlphaTextures[{map_index}]"),
        )?;
        let resolved = resolve_texture(env, pointer, "weight map")?;
        let mip_chain = decode_texture_mips_exact(env, &resolved.body).map_err(|err| {
            format!(
                "{true_name}: could not decode exact complete weight-map mip chain {}#{}: {err}",
                resolved.asset_name, resolved.key.path_id
            )
        })?;
        let decoded = mip_chain
            .first()
            .ok_or_else(|| format!("{true_name}: weight map has no mip 0"))?;
        if decoded.width != alphamap_resolution || decoded.height != alphamap_resolution {
            return Err(format!(
                "{true_name}: weight map {}#{} is {}x{}, expected {}x{}",
                resolved.asset_name,
                resolved.key.path_id,
                decoded.width,
                decoded.height,
                alphamap_resolution,
                alphamap_resolution
            ));
        }
        let canonical = canonicalize_rgba_flip_y(decoded.width, decoded.height, &decoded.rgba)?;
        let png = encode_rgba8_png(decoded.width, decoded.height, &canonical)?;
        let relative_path = format!("weights/weights_{map_index:02}.png");
        write_new(&output_dir.join(&relative_path), &png)?;
        let mip_documents = write_texture_mip_chain(
            output_dir,
            &format!("weights/weights_{map_index:02}_mips"),
            &mip_chain,
        )?;
        let sampler = texture_import_contract(&resolved.body, "linear", mip_documents.len())?;
        weight_documents.push(json!({
            "index": map_index,
            "path": relative_path,
            "width": decoded.width,
            "height": decoded.height,
            "colorSpace": "linear",
            "source": pointer_provenance(pointer, &resolved),
            "orientation": {
                "sourceDecodedRows": "unityTextureRows",
                "canonicalTransform": "flipY",
                "canonicalUv": {
                    "u": "+heightColumn (-nativeX)",
                    "v": "+heightRow (+nativeZ)"
                }
            },
            "canonicalRgbaBlake3": hash_bytes(&canonical),
            "pngBlake3": hash_bytes(&png),
            "sampler": sampler,
            "mips": mip_documents,
        }));
    }

    let layers_dir = output_dir.join("layers");
    fs::create_dir(&layers_dir).map_err(|err| {
        format!(
            "could not create native terrain layers directory {}: {err}",
            layers_dir.display()
        )
    })?;
    let mut layer_documents = Vec::with_capacity(splats.len());
    let mut semantic_names = BTreeMap::<String, String>::new();
    let mut written_albedos = BTreeMap::<String, Vec<u8>>::new();
    for (layer_index, splat) in splats.iter().enumerate() {
        let splat_object = splat
            .as_object()
            .ok_or_else(|| format!("{true_name}: splat layer {layer_index} is not an object"))?;
        let pointer = required_pointer(
            splat_object
                .get("texture")
                .ok_or_else(|| format!("{true_name}: splat layer {layer_index} has no texture"))?,
            &format!("{true_name}.m_SplatDatabase.m_Splats[{layer_index}].texture"),
        )?;
        let resolved = resolve_texture(env, pointer, "terrain layer albedo")?;
        let texture_true_name =
            required_true_name(&resolved.body, "Texture2D", resolved.key.path_id)?;
        let semantic_name = semantic_component(&texture_true_name)?;
        register_semantic_name(&mut semantic_names, &semantic_name, &texture_true_name)?;
        let mip_chain = decode_texture_mips_exact(env, &resolved.body).map_err(|err| {
            format!(
                "{true_name}: could not decode exact complete layer-texture mip chain {}#{}: {err}",
                resolved.asset_name, resolved.key.path_id
            )
        })?;
        let decoded = mip_chain
            .first()
            .ok_or_else(|| format!("{true_name}: layer texture has no mip 0"))?;
        let canonical = canonicalize_rgba_flip_y(decoded.width, decoded.height, &decoded.rgba)?;
        let png = encode_rgba8_png(decoded.width, decoded.height, &canonical)?;
        let relative_path = format!("layers/{semantic_name}/albedo.png");
        match written_albedos.get(&relative_path) {
            Some(existing) if existing != &png => {
                return Err(format!(
                    "{true_name}: true texture name {texture_true_name:?} resolves to conflicting albedo bytes"
                ));
            }
            Some(_) => {}
            None => {
                fs::create_dir(&output_dir.join("layers").join(&semantic_name)).map_err(|err| {
                    format!(
                        "could not create semantic terrain layer directory {}: {err}",
                        output_dir.join("layers").join(&semantic_name).display()
                    )
                })?;
                write_new(&output_dir.join(&relative_path), &png)?;
                written_albedos.insert(relative_path.clone(), png.clone());
            }
        }
        let mip_documents = write_texture_mip_chain(
            output_dir,
            &format!("layers/{semantic_name}/mips"),
            &mip_chain,
        )?;
        let sampler = texture_import_contract(&resolved.body, "srgb", mip_documents.len())?;
        let tile_size = required_vec2(
            splat_object.get("tileSize"),
            &format!("{true_name}.m_SplatDatabase.m_Splats[{layer_index}].tileSize"),
        )?;
        if tile_size
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(format!(
                "{true_name}: splat layer {layer_index} tileSize must be finite and positive"
            ));
        }
        let (mode, mode_source, mode_evidence) =
            splat_mode_with_provenance(splat_object, true_name, layer_index)?;
        let weight = weight_channel(layer_index);
        layer_documents.push(json!({
            "index": layer_index,
            "trueTextureName": texture_true_name,
            "mode": mode,
            "modeSource": mode_source,
            "modeEvidence": mode_evidence,
            "tileSize": { "x": tile_size[0], "y": tile_size[1] },
            "albedo": {
                "path": relative_path,
                "width": decoded.width,
                "height": decoded.height,
                "colorSpace": "srgb",
                "source": pointer_provenance(pointer, &resolved),
                "orientation": {
                    "sourceDecodedRows": "unityTextureRows",
                    "canonicalTransform": "flipY"
                },
                "canonicalRgbaBlake3": hash_bytes(&canonical),
                "pngBlake3": hash_bytes(&png),
                "sampler": sampler,
                "mips": mip_documents,
            },
            "weight": {
                "mapIndex": weight.map_index,
                "mapPath": format!("weights/weights_{:02}.png", weight.map_index),
                "channelIndex": weight.channel_index,
                "channel": weight.channel,
            }
        }));
    }

    let lightmap_document =
        export_optional_lightmap(env, candidate.body.get("m_Lightmap"), output_dir, true_name)?;
    let detail_document = export_detail_database(
        env,
        candidate.body.get("m_DetailDatabase"),
        output_dir,
        true_name,
    )?;
    let base_map_resolution = splat_database
        .get("m_BaseMapResolution")
        .and_then(exact_i64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{true_name}: m_BaseMapResolution must be a positive u32"))?;

    let extent_x = scale[0] * f64::from(width.saturating_sub(1));
    let extent_z = scale[2] * f64::from(height.saturating_sub(1));
    let minimum_height =
        f64::from(raw_min) / f64::from(HEIGHT_NORMALIZATION_DENOMINATOR) * scale[1];
    let maximum_height =
        f64::from(raw_max) / f64::from(HEIGHT_NORMALIZATION_DENOMINATOR) * scale[1];
    let terrain_document = json!({
        "schema": TERRAIN_SCHEMA,
        "trueName": true_name,
        "source": {
            "bundle": source,
            "assetName": candidate.asset_name,
            "terrainDataPathId": candidate.key.path_id,
        },
        "dimensions": {
            "width": width,
            "height": height,
            "sampleCount": sample_count,
        },
        "scale": {
            "sampleSpacingX": scale[0],
            "heightScale": scale[1],
            "sampleSpacingZ": scale[2],
            "extentX": extent_x,
            "extentZ": extent_z,
            "heightNormalizationDenominator": HEIGHT_NORMALIZATION_DENOMINATOR,
        },
        "heightmap": {
            "path": "heightmap.png",
            "pngColorType": "Gray16",
            "rawEncoding": "u16",
            "pngSampleByteOrder": "big-endian per PNG; image decoders return host-endian u16",
            "rawHashByteOrder": "little-endian",
            "pixelOrder": "row-major: pixelIndex = row * width + column",
            "rawMin": raw_min,
            "rawMax": raw_max,
            "sourceOrderRawBlake3": hash_bytes(&source_raw_bytes),
            "canonicalOrderRawBlake3": hash_bytes(&canonical_raw_bytes),
            "pngBlake3": hash_bytes(&height_png),
            "vertexShifts": vertex_shifts,
        },
        "orientation": {
            "sourceHeightLayout": "x-major",
            "sourceHeightIndex": "x * height + z",
            "canonicalHeightLayout": "row-major-z",
            "canonicalHeightIndex": "z * width + x",
            "canonicalColumns": "-nativeX",
            "canonicalRows": "+nativeZ",
            "weightMapTransform": "flipY",
            "runtimeRequiresUnityOrientationFixup": false,
        },
        "nativeGeometry": {
            "localOrigin": [0.0, 0.0, 0.0],
            "columnStep": [-scale[0], 0.0, 0.0],
            "rowStep": [0.0, 0.0, scale[2]],
            "heightAxis": [0.0, 1.0, 0.0],
            "vertexFormula": NATIVE_VERTEX_FORMULA,
            "vertexShiftEncoding": {
                "sourceField": "m_Heightmap.m_Shifts",
                "sourceCoordinates": "column = x, row = y",
                "sourceIndex": "y + x * height",
                "canonicalIndex": "row * width + column",
                "flagBits": {
                    "negativeSourceX": 1,
                    "positiveSourceX": 2,
                    "negativeSourceZ": 4,
                    "positiveSourceZ": 8
                },
                "positionFormula": NATIVE_VERTEX_SHIFT_FORMULA,
                "uvFormula": NATIVE_UV_SHIFT_FORMULA
            },
            "localBounds": {
                "min": [-extent_x, minimum_height, 0.0],
                "max": [0.0, maximum_height, extent_z],
            },
            "frontFace": "counterClockwise",
            "cellIndices": {
                "i00": "row * width + column",
                "i10": "i00 + 1",
                "i01": "i00 + width",
                "i11": "i01 + 1",
            },
            "cellTriangleOrder": NATIVE_CELL_TRIANGLE_ORDER,
            "geometricFrontNormal": "+nativeY",
        },
        "sceneInstance": {
            "status": "notExportedFromTerrainDataBundle",
            "terrainDataLocalGeometryOnly": true,
            "requiredForWorldPlacement": true,
            "requiredSource": "map-scene TerrainCollider/Terrain owner whose m_TerrainData resolves to source.terrainDataPathId",
            "applicationOrder": "terrainDataLocalVertex -> scene owner transform -> map tile root transform",
            "heightFormula": SCENE_HEIGHT_FORMULA,
            "translationIsNotBakedIntoHeightmap": true,
        },
        "splat": {
            "resolution": alphamap_resolution,
            "baseMapResolution": base_map_resolution,
            "weightMaps": weight_documents,
            "layers": layer_documents,
        },
        "lightmap": lightmap_document,
        "detailAndTrees": detail_document,
    });
    write_json_new(&output_dir.join("terrain.json"), &terrain_document)
}

pub(super) fn splat_mode_with_provenance(
    splat: &BTreeMap<String, UnityValue>,
    terrain_name: &str,
    layer_index: usize,
) -> Result<(i64, &'static str, JsonValue), String> {
    match splat.get("mode") {
        Some(value) => {
            let mode = exact_i64(value).ok_or_else(|| {
                format!(
                    "{terrain_name}: splat layer {layer_index} mode is present but not an exact integer"
                )
            })?;
            Ok((
                mode,
                "serializedField",
                json!({
                    "field": "m_SplatDatabase.m_Splats[].mode",
                    "fieldPresence": "present",
                }),
            ))
        }
        None => Ok((
            0,
            "implicitSchemaDefault",
            json!({
                "field": "m_SplatDatabase.m_Splats[].mode",
                "fieldPresence": "absent",
                "resolvedValue": 0,
                "reason": "CLR zero-initialization: SplatPrototype constructor does not assign m_Mode",
                "enum": {
                    "type": "UnityEngine.TerrainTextureMode",
                    "Splat": 0,
                    "Blend": 1,
                },
                "decompiledEvidence": [
                    {
                        "workspacePath": "work/map-zero-quaternion-investigation/unityengine-decompiled/UnityEngine/SplatPrototype.cs",
                        "sha256": "a52bfb67e53ef0248f8e152fc6adfa61ad4a29c2cd5b494794dbb6c1f9b22d7a",
                        "fact": "constructor initializes only m_TileSize; m_Mode is never assigned",
                    },
                    {
                        "workspacePath": "work/map-zero-quaternion-investigation/unityengine-decompiled/UnityEngine/TerrainTextureMode.cs",
                        "sha256": "be99cff10d79db03e49aae28698bf332095cefe2f3f9444cc640617494ca697b",
                        "fact": "implicit enum values are Splat=0 and Blend=1",
                    }
                ],
            }),
        )),
    }
}
