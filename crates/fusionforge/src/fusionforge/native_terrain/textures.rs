use super::*;

pub(super) fn write_texture_mip_chain(
    output_dir: &Path,
    relative_root: &str,
    mips: &[DecodedTextureMip],
) -> Result<Vec<JsonValue>, String> {
    if mips.is_empty() {
        return Err("texture mip chain is empty".to_string());
    }
    let root = output_dir.join(relative_root);
    fs::create_dir_all(&root).map_err(|err| {
        format!(
            "could not create texture mip directory {}: {err}",
            root.display()
        )
    })?;
    let mut documents = Vec::with_capacity(mips.len());
    for mip in mips {
        let canonical = canonicalize_rgba_flip_y(mip.width, mip.height, &mip.rgba)?;
        let png = encode_rgba8_png(mip.width, mip.height, &canonical)?;
        let png_relative = format!("{relative_root}/mip_{:02}.png", mip.level);
        let encoded_relative = format!("{relative_root}/mip_{:02}.source.bin", mip.level);
        write_new_or_verify(&output_dir.join(&png_relative), &png)?;
        write_new_or_verify(&output_dir.join(&encoded_relative), &mip.source_encoded)?;
        documents.push(json!({
            "level": mip.level,
            "width": mip.width,
            "height": mip.height,
            "path": png_relative,
            "pngColorType": "RGBA8",
            "canonicalTransform": "flipY",
            "canonicalRgbaBlake3": hash_bytes(&canonical),
            "pngBlake3": hash_bytes(&png),
            "sourceEncoded": {
                "path": encoded_relative,
                "byteLength": mip.source_encoded.len(),
                "blake3": hash_bytes(&mip.source_encoded),
            }
        }));
    }
    Ok(documents)
}

pub(super) fn texture_import_contract(
    texture: &UnityValue,
    usage_color_space: &'static str,
    decoded_mip_count: usize,
) -> Result<JsonValue, String> {
    let settings = texture
        .get("m_TextureSettings")
        .and_then(UnityValue::as_object)
        .ok_or_else(|| "Texture2D has no m_TextureSettings".to_string())?;
    let exact_setting = |key: &str| -> Result<i64, String> {
        settings
            .get(key)
            .and_then(exact_i64)
            .ok_or_else(|| format!("Texture2D m_TextureSettings.{key} is not an exact integer"))
    };
    let filter_mode = exact_setting("m_FilterMode")?;
    let aniso = exact_setting("m_Aniso")?;
    let mip_bias = settings
        .get("m_MipBias")
        .and_then(UnityValue::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| "Texture2D m_TextureSettings.m_MipBias is not finite".to_string())?;
    let wrap_mode = exact_setting("m_WrapMode")?;
    let mip_map = texture
        .get("m_MipMap")
        .and_then(UnityValue::as_i64)
        .map(|value| value != 0)
        .ok_or_else(|| "Texture2D m_MipMap is absent".to_string())?;
    let texture_format = texture
        .get("m_TextureFormat")
        .and_then(exact_i64)
        .ok_or_else(|| "Texture2D m_TextureFormat is absent".to_string())?;
    let filter_name = match filter_mode {
        0 => "Point",
        1 => "Bilinear",
        2 => "Trilinear",
        _ => "UnknownSerializedValue",
    };
    let wrap_name = match wrap_mode {
        0 => "Repeat",
        1 => "Clamp",
        _ => "UnknownSerializedValue",
    };
    Ok(json!({
        "sourceSchema": "Unity 2.5.5 Texture2D serialized type tree",
        "textureFormat": texture_format,
        "mipMap": mip_map,
        "mipCount": decoded_mip_count,
        "mipCountSource": "all serialized levels decoded from the contiguous Texture2D image payload",
        "filterMode": {
            "serializedValue": filter_mode,
            "unityName": filter_name,
        },
        "wrapMode": {
            "serializedValue": wrap_mode,
            "unityName": wrap_name,
            "u": wrap_name,
            "v": wrap_name,
            "axisSource": "legacy single m_WrapMode applies to both axes",
        },
        "anisoLevel": aniso,
        "mipBias": mip_bias,
        "usageColorSpace": usage_color_space,
        "colorSpaceFlag": {
            "serializedStatus": "notPresentInUnity2.5.5Texture2DTypeTree",
            "runtimeInterpretationSource": "terrain texture role",
        },
        "alphaIsTransparencyFlag": {
            "serializedStatus": "notPresentInUnity2.5.5Texture2DTypeTree",
            "runtimeContract": "preserve decoded RGBA alpha exactly",
        },
        "serializedAuxiliary": {
            "completeImageSize": texture.get("m_CompleteImageSize").and_then(exact_i64),
            "imageCount": texture.get("m_ImageCount").and_then(exact_i64),
            "limit": texture.get("m_Limit").and_then(exact_i64),
            "touchable": texture.get("m_Touchable").and_then(UnityValue::as_i64).map(|value| value != 0),
        }
    }))
}

pub(super) fn preload_texture_atlas_contract(
    value: Option<&UnityValue>,
    prototype_texture_pointers: &[Pointer],
) -> (JsonValue, usize) {
    let Some(value) = value else {
        return (
            json!({
                "status": "notSerialized",
                "orderedDuplicatesPreserved": true,
                "entries": [],
            }),
            0,
        );
    };
    let entries = value_array(Some(value));
    let mut blocked_count = 0_usize;
    let documents = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| match entry {
            UnityValue::Pointer(pointer) if pointer.is_null() => json!({
                "index": index,
                "status": "nullPointer",
                "sourcePointer": pointer_json(pointer),
            }),
            UnityValue::Pointer(pointer) => {
                let matching_prototype_indices = prototype_texture_pointers
                    .iter()
                    .enumerate()
                    .filter_map(|(prototype_index, prototype_pointer)| {
                        (prototype_pointer == pointer).then_some(prototype_index)
                    })
                    .collect::<Vec<_>>();
                if matching_prototype_indices.is_empty() {
                    blocked_count += 1;
                    json!({
                        "index": index,
                        "status": "unmatchedPointerClosureBlocked",
                        "sourcePointer": pointer_json(pointer),
                        "matchingPrototypeIndices": [],
                        "blocker": {
                            "code": "detailPreloadTextureClosure",
                            "message": "ordered preload atlas pointer does not match any exported detail prototype texture",
                        }
                    })
                } else {
                    json!({
                        "index": index,
                        "status": "coveredByPrototypeTextureExport",
                        "sourcePointer": pointer_json(pointer),
                        "matchingPrototypeIndices": matching_prototype_indices,
                    })
                }
            }
            other => {
                blocked_count += 1;
                json!({
                    "index": index,
                    "status": "invalidSerializedEntryBlocked",
                    "serialized": unity_value_to_json(other),
                    "blocker": {
                        "code": "detailPreloadTextureClosure",
                        "message": "preload atlas entry is not a serialized pointer",
                    }
                })
            }
        })
        .collect::<Vec<_>>();
    (
        json!({
            "status": if blocked_count == 0 { "complete" } else { "blocked" },
            "orderedDuplicatesPreserved": true,
            "serializedRaw": unity_value_to_json(value),
            "entries": documents,
        }),
        blocked_count,
    )
}

pub(super) struct ResolvedTexture {
    pub(super) key: ObjectKey,
    pub(super) asset_name: String,
    pub(super) body: UnityValue,
}

#[derive(Clone, Debug)]
pub(super) struct ExportedDetailTexture {
    pub(super) true_name: String,
    pub(super) resolved_asset_name: String,
    pub(super) resolved_path_id: i64,
    pub(super) serialized_object_raw_blake3: String,
    pub(super) document_path: String,
    pub(super) document_blake3: String,
    pub(super) texture_path: String,
}

#[derive(Clone, Debug)]
pub(super) struct DetailTextureNameProof {
    pub(super) true_name: String,
    pub(super) key: ObjectKey,
    pub(super) serialized_object_raw_blake3: String,
}

pub(super) fn resolve_texture(
    env: &UnityEnvironment,
    pointer: &Pointer,
    label: &str,
) -> Result<ResolvedTexture, String> {
    let key = env
        .resolve_pointer(pointer)
        .map_err(|err| format!("could not resolve {label} pointer: {err}"))?;
    let asset = env
        .assets
        .get(key.asset)
        .ok_or_else(|| format!("resolved {label} asset index {} is missing", key.asset))?;
    let info = asset.objects.get(&key.path_id).ok_or_else(|| {
        format!(
            "resolved {label} object {}#{} is missing",
            asset.name, key.path_id
        )
    })?;
    if asset.object_type_name(info) != "Texture2D" {
        return Err(format!(
            "resolved {label} {}#{} is {}, expected Texture2D",
            asset.name,
            key.path_id,
            asset.object_type_name(info)
        ));
    }
    let body = asset.read_object(key.asset, info).map_err(|err| {
        format!(
            "could not read resolved {label} {}#{}: {err}",
            asset.name, key.path_id
        )
    })?;
    Ok(ResolvedTexture {
        key,
        asset_name: asset.name.clone(),
        body,
    })
}

pub(super) fn register_unique_detail_texture_name(
    names: &mut BTreeMap<String, DetailTextureNameProof>,
    semantic_name: &str,
    true_name: &str,
    key: ObjectKey,
    serialized_object_raw_blake3: &str,
) -> Result<(), String> {
    let folded = semantic_name.to_lowercase();
    if let Some(existing) = names.get(&folded) {
        if existing.key != key
            || existing.serialized_object_raw_blake3 != serialized_object_raw_blake3
            || existing.true_name != true_name
        {
            return Err(format!(
                "case-insensitive detail texture semantic-path collision: {:?} {}#{} ({}) and {true_name:?} {}#{} ({serialized_object_raw_blake3})",
                existing.true_name,
                existing.key.asset,
                existing.key.path_id,
                existing.serialized_object_raw_blake3,
                key.asset,
                key.path_id,
            ));
        }
        return Ok(());
    }
    names.insert(
        folded,
        DetailTextureNameProof {
            true_name: true_name.to_string(),
            key,
            serialized_object_raw_blake3: serialized_object_raw_blake3.to_string(),
        },
    );
    Ok(())
}

pub(super) fn detail_texture_reference(pointer: &Pointer, texture: &ExportedDetailTexture) -> JsonValue {
    json!({
        "status": "sharedTextureReference",
        "trueTextureName": texture.true_name,
        "sourcePointer": pointer_json(pointer),
        "resolvedAssetName": texture.resolved_asset_name,
        "resolvedPathId": texture.resolved_path_id,
        "serializedObjectRawBlake3": texture.serialized_object_raw_blake3,
        "documentPath": texture.document_path,
        "documentBlake3": texture.document_blake3,
        "path": texture.texture_path,
    })
}

pub(super) fn canonicalize_rgba_flip_y(width: u32, height: u32, source: &[u8]) -> Result<Vec<u8>, String> {
    let row_bytes = (width as usize)
        .checked_mul(4)
        .ok_or_else(|| "RGBA row byte count overflows usize".to_string())?;
    let expected = row_bytes
        .checked_mul(height as usize)
        .ok_or_else(|| "RGBA byte count overflows usize".to_string())?;
    if source.len() != expected {
        return Err(format!(
            "RGBA source has {} bytes, expected {expected}",
            source.len()
        ));
    }
    let mut canonical = vec![0; expected];
    for destination_y in 0..height as usize {
        let source_y = height as usize - 1 - destination_y;
        let destination_start = destination_y * row_bytes;
        let source_start = source_y * row_bytes;
        canonical[destination_start..destination_start + row_bytes]
            .copy_from_slice(&source[source_start..source_start + row_bytes]);
    }
    Ok(canonical)
}
