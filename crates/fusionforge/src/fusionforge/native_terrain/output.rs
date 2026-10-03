use super::*;

pub(super) const EXPORT_SCHEMA: &str = "ffone.native-terrain-export.v1";

pub(super) fn atomic_publish_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    retry_transient_rename(
        || fs::rename(source, destination),
        |failed_attempt| {
            let milliseconds = 50_u64.saturating_mul(failed_attempt as u64);
            thread::sleep(Duration::from_millis(milliseconds));
        },
    )
}

pub(super) fn export_optional_lightmap(
    env: &UnityEnvironment,
    value: Option<&UnityValue>,
    output_dir: &Path,
    terrain_name: &str,
) -> Result<JsonValue, String> {
    let Some(pointer) = value.and_then(UnityValue::as_pointer) else {
        return Ok(json!({
            "status": "notSerialized",
        }));
    };
    if pointer.is_null() {
        return Ok(json!({
            "status": "nullPointer",
            "sourcePointer": pointer_json(pointer),
        }));
    }
    let resolved = resolve_texture(env, pointer, "terrain lightmap")?;
    let mips = decode_texture_mips_exact(env, &resolved.body).map_err(|err| {
        format!(
            "{terrain_name}: could not decode exact complete lightmap mip chain {}#{}: {err}",
            resolved.asset_name, resolved.key.path_id
        )
    })?;
    let base = mips
        .first()
        .ok_or_else(|| format!("{terrain_name}: lightmap has no mip 0"))?;
    let canonical = canonicalize_rgba_flip_y(base.width, base.height, &base.rgba)?;
    let png = encode_rgba8_png(base.width, base.height, &canonical)?;
    fs::create_dir_all(output_dir.join("lightmap"))
        .map_err(|err| format!("could not create lightmap directory: {err}"))?;
    write_new(&output_dir.join("lightmap/lightmap.png"), &png)?;
    let mip_documents = write_texture_mip_chain(output_dir, "lightmap/mips", &mips)?;
    let sampler = texture_import_contract(&resolved.body, "srgb", mip_documents.len())?;
    Ok(json!({
        "status": "exported",
        "path": "lightmap/lightmap.png",
        "width": base.width,
        "height": base.height,
        "colorSpace": "srgb",
        "source": pointer_provenance(pointer, &resolved),
        "canonicalTransform": "flipY",
        "canonicalRgbaBlake3": hash_bytes(&canonical),
        "pngBlake3": hash_bytes(&png),
        "sampler": sampler,
        "mips": mip_documents,
        "runtimeSelection": "scene Terrain component m_UseLightmap decides whether this asset participates",
    }))
}

pub(super) fn export_detail_database(
    env: &UnityEnvironment,
    value: Option<&UnityValue>,
    output_dir: &Path,
    terrain_name: &str,
) -> Result<JsonValue, String> {
    let Some(database) = value.and_then(UnityValue::as_object) else {
        return Ok(json!({
            "status": "notSerialized",
        }));
    };
    let detail_root = output_dir.join("details");
    fs::create_dir_all(&detail_root)
        .map_err(|err| format!("could not create detail directory: {err}"))?;
    let raw_json = serde_json::to_vec_pretty(&unity_value_to_json(
        value.expect("detail database value was checked"),
    ))
    .map_err(|err| format!("could not encode detail database JSON: {err}"))?;
    write_new(&detail_root.join("detail-database.raw.json"), &raw_json)?;

    let prototypes = value_array(database.get("m_DetailPrototypes"));
    let patch_count = required_nonnegative_usize(database, "m_PatchCount", terrain_name)?;
    let patch_samples = required_nonnegative_usize(database, "m_PatchSamples", terrain_name)?;
    if (patch_count == 0 || patch_samples == 0) && !prototypes.is_empty() {
        return Err(format!(
            "{terrain_name}: non-empty detail prototypes require positive patchCount/patchSamples"
        ));
    }
    let patches = value_array(database.get("m_Patches"));
    let expected_patch_count = patch_count
        .checked_mul(patch_count)
        .ok_or_else(|| format!("{terrain_name}: detail patch count overflows usize"))?;
    if patches.len() != expected_patch_count {
        return Err(format!(
            "{terrain_name}: detail database has {} patches, expected {patch_count}^2={expected_patch_count}",
            patches.len()
        ));
    }

    let density_resolution = patch_count
        .checked_mul(patch_samples)
        .ok_or_else(|| format!("{terrain_name}: detail density resolution overflows usize"))?;
    let density_sample_count = density_resolution
        .checked_mul(density_resolution)
        .ok_or_else(|| format!("{terrain_name}: density map sample count overflows usize"))?;
    let density_resolution_u32 = u32::try_from(density_resolution)
        .map_err(|_| format!("{terrain_name}: density resolution exceeds u32"))?;
    let mut density_maps = vec![vec![0_u8; density_sample_count]; prototypes.len()];
    let mut raw_layer_indices = Vec::new();
    let mut raw_density = Vec::new();
    let mut nonzero_samples = vec![0_usize; prototypes.len()];
    let mut density_totals = vec![0_u64; prototypes.len()];
    for (patch_index, patch) in patches.iter().enumerate() {
        let patch = patch.as_object().ok_or_else(|| {
            format!("{terrain_name}: detail patch {patch_index} is not an object")
        })?;
        let layer_indices = exact_byte_array(
            patch.get("layerIndices"),
            &format!("{terrain_name}.detail.patch[{patch_index}].layerIndices"),
        )?;
        let number_of_objects = exact_byte_array(
            patch.get("numberOfObjects"),
            &format!("{terrain_name}.detail.patch[{patch_index}].numberOfObjects"),
        )?;
        let samples_per_layer = patch_samples
            .checked_mul(patch_samples)
            .ok_or_else(|| format!("{terrain_name}: patch sample count overflows"))?;
        let expected = layer_indices
            .len()
            .checked_mul(samples_per_layer)
            .ok_or_else(|| format!("{terrain_name}: patch density byte count overflows"))?;
        if number_of_objects.len() != expected {
            return Err(format!(
                "{terrain_name}: detail patch {patch_index} has {} density bytes, expected {} layers * {patch_samples}^2 = {expected}",
                number_of_objects.len(),
                layer_indices.len()
            ));
        }
        raw_layer_indices.extend_from_slice(&layer_indices);
        raw_density.extend_from_slice(&number_of_objects);
        let patch_x = patch_index % patch_count;
        let patch_z = patch_index / patch_count;
        for (slot, layer_index) in layer_indices.iter().copied().enumerate() {
            let layer_index = layer_index as usize;
            if layer_index >= prototypes.len() {
                return Err(format!(
                    "{terrain_name}: detail patch {patch_index} slot {slot} references prototype {layer_index}, only {} exist",
                    prototypes.len()
                ));
            }
            let source_start = slot * samples_per_layer;
            for local_z in 0..patch_samples {
                for local_x in 0..patch_samples {
                    let value = number_of_objects[source_start + local_z * patch_samples + local_x];
                    let x = patch_x * patch_samples + local_x;
                    let z = patch_z * patch_samples + local_z;
                    density_maps[layer_index][z * density_resolution + x] = value;
                    if value != 0 {
                        nonzero_samples[layer_index] += 1;
                        density_totals[layer_index] += u64::from(value);
                    }
                }
            }
        }
    }
    write_new(
        &detail_root.join("patch-layer-indices.bin"),
        &raw_layer_indices,
    )?;
    write_new(&detail_root.join("patch-density.raw.bin"), &raw_density)?;

    let mut prototype_documents = Vec::with_capacity(prototypes.len());
    let mut prototype_texture_pointers = Vec::with_capacity(prototypes.len());
    let mut detail_textures_by_key = BTreeMap::<(usize, i64), ExportedDetailTexture>::new();
    let mut detail_texture_names = BTreeMap::<String, DetailTextureNameProof>::new();
    let mut unique_texture_documents = Vec::<JsonValue>::new();
    let mut blocked_graph_root_count = 0_usize;
    let density_dir = detail_root.join("density");
    fs::create_dir_all(&density_dir)
        .map_err(|err| format!("could not create detail density directory: {err}"))?;
    for (prototype_index, prototype) in prototypes.iter().enumerate() {
        let prototype_object = prototype.as_object().ok_or_else(|| {
            format!("{terrain_name}: detail prototype {prototype_index} is not an object")
        })?;
        let density_png = encode_gray8_png(
            density_resolution_u32,
            density_resolution_u32,
            &density_maps[prototype_index],
        )?;
        let density_path = format!("details/density/layer_{prototype_index:02}.png");
        write_new(&output_dir.join(&density_path), &density_png)?;

        let texture_pointer = required_pointer(
            prototype_object.get("prototypeTexture").ok_or_else(|| {
                format!("{terrain_name}: detail prototype {prototype_index} has no texture pointer")
            })?,
            &format!("{terrain_name}.detail.prototype[{prototype_index}].prototypeTexture"),
        )?;
        prototype_texture_pointers.push(texture_pointer.clone());
        let texture_document = if texture_pointer.is_null() {
            json!({ "status": "nullPointer", "sourcePointer": pointer_json(texture_pointer) })
        } else {
            let resolved = resolve_texture(env, texture_pointer, "detail prototype texture")?;
            let raw_blake3 = resolved_object_raw_blake3(env, resolved.key)?;
            let key = (resolved.key.asset, resolved.key.path_id);
            if let Some(existing) = detail_textures_by_key.get(&key) {
                if existing.serialized_object_raw_blake3 != raw_blake3 {
                    return Err(format!(
                        "{terrain_name}: resolved detail texture {}#{} changed content within one export",
                        resolved.asset_name, resolved.key.path_id
                    ));
                }
                detail_texture_reference(texture_pointer, existing)
            } else {
                let true_texture_name =
                    required_true_name(&resolved.body, "Texture2D", resolved.key.path_id)?;
                let semantic_name = semantic_component(&true_texture_name)?;
                register_unique_detail_texture_name(
                    &mut detail_texture_names,
                    &semantic_name,
                    &true_texture_name,
                    resolved.key,
                    &raw_blake3,
                )?;
                let mips = decode_texture_mips_exact(env, &resolved.body).map_err(|err| {
                    format!(
                        "{terrain_name}: could not decode exact complete detail texture {true_texture_name:?} mip chain: {err}"
                    )
                })?;
                let base = mips.first().ok_or_else(|| {
                    format!("{terrain_name}: detail texture {true_texture_name:?} has no mip 0")
                })?;
                let canonical = canonicalize_rgba_flip_y(base.width, base.height, &base.rgba)?;
                let png = encode_rgba8_png(base.width, base.height, &canonical)?;
                let root = format!("details/textures/{semantic_name}");
                fs::create_dir_all(output_dir.join(&root)).map_err(|err| {
                    format!("could not create semantic detail texture directory: {err}")
                })?;
                let texture_path = format!("{root}/texture.png");
                write_new(&output_dir.join(&texture_path), &png)?;
                let mip_documents =
                    write_texture_mip_chain(output_dir, &format!("{root}/mips"), &mips)?;
                let document_path = format!("{root}/texture.json");
                let shared_document = json!({
                    "schema": "ffone.native-terrain-detail-texture.v1",
                    "trueTextureName": true_texture_name,
                    "path": texture_path,
                    "width": base.width,
                    "height": base.height,
                    "source": {
                        "resolvedAssetName": resolved.asset_name,
                        "resolvedPathId": resolved.key.path_id,
                        "serializedObjectRawBlake3": raw_blake3,
                    },
                    "canonicalTransform": "flipY",
                    "canonicalRgbaBlake3": hash_bytes(&canonical),
                    "pngBlake3": hash_bytes(&png),
                    "sampler": texture_import_contract(&resolved.body, "srgb", mip_documents.len())?,
                    "mips": mip_documents,
                });
                let shared_bytes = serde_json::to_vec_pretty(&shared_document).map_err(|err| {
                    format!("could not encode semantic detail texture {true_texture_name:?}: {err}")
                })?;
                write_new(&output_dir.join(&document_path), &shared_bytes)?;
                let exported = ExportedDetailTexture {
                    true_name: true_texture_name,
                    resolved_asset_name: resolved.asset_name,
                    resolved_path_id: resolved.key.path_id,
                    serialized_object_raw_blake3: raw_blake3,
                    document_path,
                    document_blake3: hash_bytes(&shared_bytes),
                    texture_path,
                };
                unique_texture_documents.push(json!({
                    "trueTextureName": exported.true_name,
                    "resolvedAssetName": exported.resolved_asset_name,
                    "resolvedPathId": exported.resolved_path_id,
                    "serializedObjectRawBlake3": exported.serialized_object_raw_blake3,
                    "documentPath": exported.document_path,
                    "documentBlake3": exported.document_blake3,
                    "path": exported.texture_path,
                }));
                let reference = detail_texture_reference(texture_pointer, &exported);
                detail_textures_by_key.insert(key, exported);
                reference
            }
        };
        let mesh_pointer = prototype_object
            .get("prototype")
            .and_then(UnityValue::as_pointer);
        let mesh_closure = export_optional_pointer_root(
            env,
            mesh_pointer,
            output_dir,
            &format!("details/prototypes/prototype_{prototype_index:02}/mesh-root"),
        )?;
        if graph_closure_is_blocked(&mesh_closure) {
            blocked_graph_root_count += 1;
        }
        prototype_documents.push(json!({
            "index": prototype_index,
            "serialized": unity_value_to_json(prototype),
            "density": {
                "path": density_path,
                "width": density_resolution,
                "height": density_resolution,
                "pngColorType": "Gray8",
                "rawEncoding": "u8",
                "pixelOrder": "row-major z rows; patchIndex = patchZ * patchCount + patchX; localIndex = z * patchSamples + x",
                "layoutStatus": "typedInferenceFromSerializedPatchShapeAndUnityAPIIndexing",
                "canonicalRawBlake3": hash_bytes(&density_maps[prototype_index]),
                "pngBlake3": hash_bytes(&density_png),
                "nonzeroSampleCount": nonzero_samples[prototype_index],
                "densityTotal": density_totals[prototype_index],
            },
            "prototypeTexture": texture_document,
            "prototypeMeshRoot": mesh_closure,
        }));
    }

    let tree_instances = value_array(database.get("m_TreeInstances"));
    let tree_prototypes = value_array(database.get("m_TreePrototypes"));
    let mut tree_prototype_documents = Vec::with_capacity(tree_prototypes.len());
    for (index, prototype) in tree_prototypes.iter().enumerate() {
        let object = prototype
            .as_object()
            .ok_or_else(|| format!("{terrain_name}: tree prototype {index} is not an object"))?;
        let pointer = object
            .get("prefab")
            .or_else(|| object.get("m_Prefab"))
            .and_then(UnityValue::as_pointer);
        let closure = export_optional_pointer_root(
            env,
            pointer,
            output_dir,
            &format!("details/trees/prototype_{index:02}/prefab-root"),
        )?;
        if graph_closure_is_blocked(&closure) {
            blocked_graph_root_count += 1;
        }
        tree_prototype_documents.push(json!({
            "index": index,
            "serialized": unity_value_to_json(prototype),
            "prefabRoot": closure,
        }));
    }
    let tree_raw = json!({
        "instances": tree_instances.iter().map(|value| unity_value_to_json(value)).collect::<Vec<_>>(),
        "prototypes": tree_prototypes.iter().map(|value| unity_value_to_json(value)).collect::<Vec<_>>(),
    });
    let tree_bytes = serde_json::to_vec_pretty(&tree_raw)
        .map_err(|err| format!("could not encode tree database: {err}"))?;
    write_new(&detail_root.join("trees.raw.json"), &tree_bytes)?;
    let random_rotations = database
        .get("m_RandomRotations")
        .map(unity_value_to_json)
        .unwrap_or_else(|| json!([]));
    let (preload_atlas, blocked_preload_pointer_count) = preload_texture_atlas_contract(
        database.get("m_PreloadTextureAtlasData"),
        &prototype_texture_pointers,
    );
    let blocked_asset_closure_count = blocked_graph_root_count + blocked_preload_pointer_count;

    Ok(json!({
        "status": if blocked_asset_closure_count == 0 {
            "serializedAssetClosureExported"
        } else {
            "assetClosureBlocked"
        },
        "assetClosure": {
            "status": if blocked_asset_closure_count == 0 { "complete" } else { "blocked" },
            "blockedCount": blocked_asset_closure_count,
            "blockedGraphRootCount": blocked_graph_root_count,
            "blockedPreloadPointerCount": blocked_preload_pointer_count,
            "prototypeTextureReferenceCount": prototype_texture_pointers.len(),
            "uniqueResolvedTextureCount": detail_textures_by_key.len(),
            "contract": "nonnull detail prototype mesh and tree prefab roots require full logical object graph expansion; every ordered preload atlas pointer must exactly match an exported prototype texture or be exported independently",
        },
        "rawDocument": {
            "path": "details/detail-database.raw.json",
            "blake3": hash_bytes(&raw_json),
        },
        "patchGrid": {
            "patchCount": patch_count,
            "patchSamples": patch_samples,
            "patches": patches.len(),
            "densityResolution": density_resolution,
            "layerIndicesRaw": {
                "path": "details/patch-layer-indices.bin",
                "byteLength": raw_layer_indices.len(),
                "blake3": hash_bytes(&raw_layer_indices),
            },
            "densityRaw": {
                "path": "details/patch-density.raw.bin",
                "byteLength": raw_density.len(),
                "blake3": hash_bytes(&raw_density),
            },
            "layoutEvidence": "layer-major patchSamples*patchSamples blocks in layerIndices order; patch grid and local sample order retained verbatim in raw sidecars",
        },
        "wavingGrass": {
            "tint": database.get("WavingGrassTint").map(unity_value_to_json),
            "amount": database.get("m_WavingGrassAmount").map(unity_value_to_json),
            "speed": database.get("m_WavingGrassSpeed").map(unity_value_to_json),
            "strength": database.get("m_WavingGrassStrength").map(unity_value_to_json),
        },
        "textures": unique_texture_documents,
        "prototypes": prototype_documents,
        "randomRotations": random_rotations,
        "preloadTextureAtlasData": preload_atlas,
        "trees": {
            "instanceCount": tree_instances.len(),
            "prototypeCount": tree_prototypes.len(),
            "instances": tree_instances.iter().map(|value| unity_value_to_json(value)).collect::<Vec<_>>(),
            "prototypes": tree_prototype_documents,
            "rawDocument": {
                "path": "details/trees.raw.json",
                "blake3": hash_bytes(&tree_bytes),
            },
            "runtimeStatus": if tree_instances.is_empty() && tree_prototypes.is_empty() {
                "empty"
            } else {
                "assetDataExportedNativeRuntimePending"
            }
        }
    }))
}

pub(super) fn export_optional_pointer_root(
    env: &UnityEnvironment,
    pointer: Option<&Pointer>,
    output_dir: &Path,
    relative_stem: &str,
) -> Result<JsonValue, String> {
    let Some(pointer) = pointer else {
        return Ok(json!({ "status": "notSerialized" }));
    };
    if pointer.is_null() {
        return Ok(json!({
            "status": "nullPointer",
            "sourcePointer": pointer_json(pointer),
        }));
    }
    let key = env
        .resolve_pointer(pointer)
        .map_err(|err| format!("could not resolve prototype root: {err}"))?;
    let asset = env
        .assets
        .get(key.asset)
        .ok_or_else(|| format!("prototype root asset {} is missing", key.asset))?;
    let info = asset
        .objects
        .get(&key.path_id)
        .ok_or_else(|| format!("prototype root {}#{} is missing", asset.name, key.path_id))?;
    let raw = asset.object_raw_data(info)?;
    let parsed = asset.read_object(key.asset, info)?;
    let raw_path = format!("{relative_stem}.bin");
    let json_path = format!("{relative_stem}.json");
    if let Some(parent) = output_dir.join(&raw_path).parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("could not create prototype root directory: {err}"))?;
    }
    write_new(&output_dir.join(&raw_path), raw)?;
    let json_bytes = serde_json::to_vec_pretty(&unity_value_to_json(&parsed))
        .map_err(|err| format!("could not encode prototype root: {err}"))?;
    write_new(&output_dir.join(&json_path), &json_bytes)?;
    Ok(json!({
        "status": "rootObjectExportedGraphClosureBlocked",
        "graphClosureComplete": false,
        "blocker": {
            "code": "logicalModelGraphExpansionRequired",
            "message": "root object bytes are preserved, but child hierarchy and referenced mesh/material/texture graph are not yet exported",
        },
        "sourcePointer": pointer_json(pointer),
        "resolvedAssetName": asset.name,
        "resolvedPathId": key.path_id,
        "typeName": asset.object_type_name(info),
        "raw": {
            "path": raw_path,
            "blake3": hash_bytes(raw),
        },
        "parsed": {
            "path": json_path,
            "blake3": hash_bytes(&json_bytes),
        }
    }))
}

pub(super) fn write_json_new(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|err| format!("could not encode {}: {err}", path.display()))?;
    bytes.push(b'\n');
    write_new(path, &bytes)
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| format!("could not create {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not sync {}: {err}", path.display()))
}

pub(super) fn write_new_or_verify(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        let existing =
            fs::read(path).map_err(|err| format!("could not verify {}: {err}", path.display()))?;
        if existing == bytes {
            return Ok(());
        }
        return Err(format!(
            "conflicting texture mip payload for {}",
            path.display()
        ));
    }
    write_new(path, bytes)
}
