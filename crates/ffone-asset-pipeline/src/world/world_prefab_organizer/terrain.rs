use super::*;

#[derive(Default)]
pub(super) struct SharedTerrainFiles {
    pub(super) artifacts: BTreeMap<String, WorldPrefabArtifact>,
    pub(super) package_routes: BTreeMap<(String, String), String>,
    pub(super) package_variant_counts: BTreeMap<String, u64>,
    pub(super) detail_package_routes: BTreeMap<(String, String), String>,
    pub(super) detail_package_variant_counts: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SharedTerrainDetailTextureRoutes {
    pub(super) document_path: String,
    pub(super) document_blake3: String,
    pub(super) texture_path: String,
}

pub(super) fn publish_shared_terrain_layers(
    source_root: &Path,
    staging: &Path,
    terrain: &mut JsonValue,
    shared: &mut SharedTerrainFiles,
) -> Result<BTreeSet<String>> {
    let layers = terrain
        .get_mut("splat")
        .and_then(|splat| splat.get_mut("layers"))
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("terrain descriptor has no splat.layers array"))?;
    let mut skipped = BTreeSet::new();
    for layer in layers {
        let true_name = layer
            .get("trueTextureName")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error("terrain layer has no trueTextureName"))?;
        let original_albedo = layer
            .get("albedo")
            .cloned()
            .ok_or_else(|| invalid_error("terrain layer has no albedo contract"))?;
        let package_hash =
            hash_bytes(&serde_json::to_vec(&original_albedo).map_err(generated_json_error)?);
        let package_name = safe_slug(true_name, "layer", 56);
        let package_key = (package_name.clone(), package_hash.clone());
        let package_root = if let Some(existing) = shared.package_routes.get(&package_key) {
            existing.clone()
        } else {
            let variant = shared
                .package_variant_counts
                .entry(package_name.clone())
                .or_default();
            *variant += 1;
            let route = if *variant == 1 {
                format!("shared/terrain/layers/{package_name}")
            } else {
                format!("shared/terrain/layers/{package_name}_variant_{variant:02}")
            };
            shared.package_routes.insert(package_key, route.clone());
            route
        };
        let albedo = layer
            .get_mut("albedo")
            .and_then(JsonValue::as_object_mut)
            .ok_or_else(|| invalid_error("terrain layer albedo is not an object"))?;
        let original_base = albedo
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error("terrain layer albedo has no path"))?
            .to_owned();
        let base_png_hash = albedo
            .get("pngBlake3")
            .and_then(JsonValue::as_str)
            .map(str::to_owned);
        skipped.insert(original_base.clone());

        let mut first_mip_path = None::<String>;
        let mut first_mip_hash = None::<String>;
        if let Some(mips) = albedo.get_mut("mips").and_then(JsonValue::as_array_mut) {
            for mip in mips {
                let mip_object = mip
                    .as_object_mut()
                    .ok_or_else(|| invalid_error("terrain layer mip is not an object"))?;
                let level = mip_object
                    .get("level")
                    .and_then(JsonValue::as_u64)
                    .ok_or_else(|| invalid_error("terrain layer mip has no level"))?;
                let original = mip_object
                    .get("path")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| invalid_error("terrain layer mip has no path"))?
                    .to_owned();
                let output = format!("{package_root}/mips/mip_{level:02}.png");
                publish_shared_terrain_file(source_root, staging, &original, &output, shared)?;
                skipped.insert(original);
                let rooted = format!("map/{output}");
                mip_object.insert("path".to_owned(), JsonValue::String(rooted.clone()));
                if level == 0 {
                    first_mip_path = Some(rooted);
                    first_mip_hash = mip_object
                        .get("pngBlake3")
                        .and_then(JsonValue::as_str)
                        .map(str::to_owned);
                }
                if let Some(source_encoded) = mip_object
                    .get_mut("sourceEncoded")
                    .and_then(JsonValue::as_object_mut)
                {
                    let original = source_encoded
                        .get("path")
                        .and_then(JsonValue::as_str)
                        .ok_or_else(|| invalid_error("terrain source mip has no path"))?
                        .to_owned();
                    let output = format!("{package_root}/mips/mip_{level:02}.source.bin");
                    publish_shared_terrain_file(source_root, staging, &original, &output, shared)?;
                    skipped.insert(original);
                    source_encoded.insert(
                        "path".to_owned(),
                        JsonValue::String(format!("map/{output}")),
                    );
                }
            }
        }
        if first_mip_hash.is_some() && first_mip_hash == base_png_hash {
            albedo.insert(
                "path".to_owned(),
                JsonValue::String(first_mip_path.expect("matching mip hash has a mip path")),
            );
        } else {
            let output = format!("{package_root}/albedo.png");
            publish_shared_terrain_file(source_root, staging, &original_base, &output, shared)?;
            albedo.insert(
                "path".to_owned(),
                JsonValue::String(format!("map/{output}")),
            );
        }
    }
    Ok(skipped)
}

pub(super) fn publish_shared_terrain_detail_textures(
    source_root: &Path,
    staging: &Path,
    terrain: &mut JsonValue,
    shared: &mut SharedTerrainFiles,
) -> Result<BTreeSet<String>> {
    let Some(texture_records) = terrain
        .pointer("/detailAndTrees/textures")
        .and_then(JsonValue::as_array)
        .cloned()
    else {
        return Ok(BTreeSet::new());
    };
    let mut skipped = BTreeSet::new();
    let mut routes_by_document = BTreeMap::<String, SharedTerrainDetailTextureRoutes>::new();
    for record in &texture_records {
        let document_path = record
            .get("documentPath")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error("terrain detail texture has no documentPath"))?
            .to_owned();
        let routes = publish_shared_terrain_detail_texture_package(
            source_root,
            staging,
            record,
            shared,
            &mut skipped,
        )?;
        if let Some(existing) = routes_by_document.insert(document_path.clone(), routes.clone())
            && existing != routes
        {
            return invalid(format!(
                "terrain detail document {document_path:?} resolves to contradictory shared routes"
            ));
        }
    }

    let detail = terrain
        .get_mut("detailAndTrees")
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| invalid_error("terrain detailAndTrees is not an object"))?;
    let textures = detail
        .get_mut("textures")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("terrain detailAndTrees.textures is not an array"))?;
    for texture in textures {
        rewrite_shared_terrain_detail_texture_reference(texture, &routes_by_document)?;
    }
    if let Some(prototypes) = detail
        .get_mut("prototypes")
        .and_then(JsonValue::as_array_mut)
    {
        for prototype in prototypes {
            let Some(reference) = prototype.get_mut("prototypeTexture") else {
                continue;
            };
            rewrite_shared_terrain_detail_texture_reference(reference, &routes_by_document)?;
        }
    }
    Ok(skipped)
}

pub(super) fn publish_shared_terrain_detail_texture_package(
    source_root: &Path,
    staging: &Path,
    record: &JsonValue,
    shared: &mut SharedTerrainFiles,
    skipped: &mut BTreeSet<String>,
) -> Result<SharedTerrainDetailTextureRoutes> {
    let true_name = record
        .get("trueTextureName")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error("terrain detail texture has no trueTextureName"))?;
    let document_relative = record
        .get("documentPath")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error("terrain detail texture has no documentPath"))?;
    validate_relative(document_relative)?;
    let document_relative_path = Path::new(document_relative);
    if document_relative_path
        .file_name()
        .and_then(|value| value.to_str())
        != Some("texture.json")
    {
        return invalid(format!(
            "terrain detail document has a non-canonical filename: {document_relative:?}"
        ));
    }
    let source_package_relative = document_relative_path
        .parent()
        .ok_or_else(|| invalid_error("terrain detail document has no package root"))?;
    if !source_package_relative.starts_with(Path::new("details/textures")) {
        return invalid(format!(
            "terrain detail package is outside details/textures: {document_relative:?}"
        ));
    }
    let source_package = canonical_directory(
        &safe_join(source_root, source_package_relative)?,
        "terrain detail texture package",
    )?;
    if !source_package.starts_with(source_root) {
        return invalid("terrain detail texture package escaped the terrain root");
    }
    let files = collect_files(&source_package)?;
    if files.is_empty() || !files.iter().any(|(path, _, _)| path == "texture.json") {
        return invalid(format!(
            "terrain detail package {document_relative:?} has no complete closure"
        ));
    }
    let mut closure = blake3::Hasher::new();
    for (path, _, hash) in &files {
        append_set_hash(&mut closure, path, hash);
    }
    let closure_hash = closure.finalize().to_hex().to_string();
    let package_name = safe_slug(true_name, "detail", 56);
    let package_key = (package_name.clone(), closure_hash);
    let package_root = if let Some(existing) = shared.detail_package_routes.get(&package_key) {
        existing.clone()
    } else {
        let variant = shared
            .detail_package_variant_counts
            .entry(package_name.clone())
            .or_default();
        *variant += 1;
        let route = if *variant == 1 {
            format!("shared/terrain/details/{package_name}")
        } else {
            format!("shared/terrain/details/{package_name}_variant_{variant:02}")
        };
        shared
            .detail_package_routes
            .insert(package_key, route.clone());
        route
    };

    let source_document_path = safe_join(source_root, document_relative)?;
    let source_document_bytes =
        read_regular_file(&source_document_path, "terrain detail texture document")?;
    let expected_document_hash = record
        .get("documentBlake3")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error("terrain detail texture has no document BLAKE3"))?;
    if hash_bytes(&source_document_bytes)
        != normalized_blake3(expected_document_hash, "terrain detail document")?
    {
        return invalid(format!(
            "terrain detail document bytes differ from {document_relative:?} acceptance hash"
        ));
    }
    let mut document: JsonValue =
        serde_json::from_slice(&source_document_bytes).map_err(|source| PipelineError::Json {
            path: source_document_path.display().to_string(),
            source,
        })?;
    if document.get("schema").and_then(JsonValue::as_str)
        != Some("ffone.native-terrain-detail-texture.v1")
        || document.get("trueTextureName") != record.get("trueTextureName")
        || document
            .get("source")
            .and_then(|source| source.get("resolvedAssetName"))
            != record.get("resolvedAssetName")
        || document
            .get("source")
            .and_then(|source| source.get("resolvedPathId"))
            != record.get("resolvedPathId")
        || document
            .get("source")
            .and_then(|source| source.get("serializedObjectRawBlake3"))
            != record.get("serializedObjectRawBlake3")
        || document.get("path") != record.get("path")
    {
        return invalid(format!(
            "terrain detail texture document/record identity mismatch at {document_relative:?}"
        ));
    }

    let base_path = document
        .get("path")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error("terrain detail texture document has no base path"))?
        .to_owned();
    let base_hash = document
        .get("pngBlake3")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error("terrain detail texture document has no base PNG hash"))?
        .to_owned();
    let (mip_zero_path, mip_zero_hash) = document
        .get("mips")
        .and_then(JsonValue::as_array)
        .and_then(|mips| {
            mips.iter()
                .find(|mip| mip.get("level").and_then(JsonValue::as_u64) == Some(0))
        })
        .map(|mip| -> Result<(String, String)> {
            let path = mip
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| invalid_error("terrain detail mip zero has no path"))?;
            let hash = mip
                .get("pngBlake3")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| invalid_error("terrain detail mip zero has no PNG hash"))?;
            Ok((path.to_owned(), hash.to_owned()))
        })
        .transpose()?
        .ok_or_else(|| invalid_error("terrain detail texture document has no mip zero"))?;
    let base_is_mip_zero = exact_hashed_terrain_files_match(
        source_root,
        &base_path,
        &base_hash,
        &mip_zero_path,
        &mip_zero_hash,
        "terrain detail base/mip zero",
    )?;

    let rooted_member = |source_relative: &str| -> Result<String> {
        validate_relative(source_relative)?;
        let member = Path::new(source_relative)
            .strip_prefix(source_package_relative)
            .map_err(|_| {
                invalid_error(format!(
                    "terrain detail texture dependency {source_relative:?} escaped package {source_package_relative:?}"
                ))
            })?;
        validate_relative_path(member)?;
        Ok(format!("map/{package_root}/{}", slash_path(member)))
    };

    let mut rewritten_mip_zero = None;
    let document_object = document
        .as_object_mut()
        .ok_or_else(|| invalid_error("terrain detail texture document is not an object"))?;
    let mips = document_object
        .get_mut("mips")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("terrain detail texture document has no mips array"))?;
    for mip in mips {
        let mip_object = mip
            .as_object_mut()
            .ok_or_else(|| invalid_error("terrain detail texture mip is not an object"))?;
        let level = mip_object
            .get("level")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| invalid_error("terrain detail texture mip has no level"))?;
        let original = mip_object
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error("terrain detail texture mip has no path"))?
            .to_owned();
        let rooted = rooted_member(&original)?;
        mip_object.insert("path".to_owned(), JsonValue::String(rooted.clone()));
        if level == 0 {
            rewritten_mip_zero = Some(rooted);
        }
        if let Some(source_encoded) = mip_object
            .get_mut("sourceEncoded")
            .and_then(JsonValue::as_object_mut)
        {
            let original = source_encoded
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| invalid_error("terrain detail source mip has no path"))?
                .to_owned();
            source_encoded.insert(
                "path".to_owned(),
                JsonValue::String(rooted_member(&original)?),
            );
        }
    }
    let rewritten_base = if base_is_mip_zero {
        rewritten_mip_zero
            .ok_or_else(|| invalid_error("rewritten terrain detail document lost mip zero"))?
    } else {
        rooted_member(&base_path)?
    };
    document_object.insert("path".to_owned(), JsonValue::String(rewritten_base.clone()));
    let rewritten_document = pretty_json(&document)?;

    let source_package_prefix = slash_path(source_package_relative);
    for (member, _, _) in &files {
        let source_relative = format!("{source_package_prefix}/{member}");
        skipped.insert(source_relative.clone());
        if member == "texture.json"
            || (base_is_mip_zero && base_path != mip_zero_path && source_relative == base_path)
        {
            continue;
        }
        publish_shared_terrain_file(
            source_root,
            staging,
            &source_relative,
            &format!("{package_root}/{member}"),
            shared,
        )?;
    }
    let document_output = format!("{package_root}/texture.json");
    publish_shared_terrain_bytes(staging, &document_output, &rewritten_document, shared)?;
    Ok(SharedTerrainDetailTextureRoutes {
        document_path: format!("map/{document_output}"),
        document_blake3: format!("blake3:{}", hash_bytes(&rewritten_document)),
        texture_path: rewritten_base,
    })
}

pub(super) fn rewrite_shared_terrain_detail_texture_reference(
    reference: &mut JsonValue,
    routes_by_document: &BTreeMap<String, SharedTerrainDetailTextureRoutes>,
) -> Result<()> {
    let object = reference
        .as_object_mut()
        .ok_or_else(|| invalid_error("terrain detail texture reference is not an object"))?;
    let original = object
        .get("documentPath")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid_error("terrain detail texture reference has no documentPath"))?;
    let routes = routes_by_document.get(original).ok_or_else(|| {
        invalid_error(format!(
            "terrain detail texture reference {original:?} has no complete shared closure"
        ))
    })?;
    object.insert(
        "documentPath".to_owned(),
        JsonValue::String(routes.document_path.clone()),
    );
    object.insert(
        "documentBlake3".to_owned(),
        JsonValue::String(routes.document_blake3.clone()),
    );
    object.insert(
        "path".to_owned(),
        JsonValue::String(routes.texture_path.clone()),
    );
    Ok(())
}

pub(super) fn collapse_exact_terrain_base_mip_zero_copies(
    source_root: &Path,
    terrain: &mut JsonValue,
    skipped: &mut BTreeSet<String>,
) -> Result<()> {
    let weights = terrain
        .pointer_mut("/splat/weightMaps")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("terrain descriptor has no splat.weightMaps array"))?;
    for weight in weights {
        collapse_exact_terrain_base_mip_zero(source_root, weight, skipped, "terrain weight map")?;
    }
    if let Some(lightmap) = terrain.get_mut("lightmap")
        && lightmap.get("status").and_then(JsonValue::as_str) == Some("exported")
    {
        collapse_exact_terrain_base_mip_zero(source_root, lightmap, skipped, "terrain lightmap")?;
    }
    Ok(())
}

pub(super) fn collapse_exact_terrain_base_mip_zero(
    source_root: &Path,
    texture: &mut JsonValue,
    skipped: &mut BTreeSet<String>,
    context: &str,
) -> Result<()> {
    let (base_path, base_hash, mip_zero_index, mip_zero_path, mip_zero_hash) = {
        let base_path = texture
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error(format!("{context} has no base path")))?
            .to_owned();
        let base_hash = texture
            .get("pngBlake3")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error(format!("{context} has no base PNG hash")))?
            .to_owned();
        let mips = texture
            .get("mips")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| invalid_error(format!("{context} has no mips array")))?;
        let (index, mip) = mips
            .iter()
            .enumerate()
            .find(|(_, mip)| mip.get("level").and_then(JsonValue::as_u64) == Some(0))
            .ok_or_else(|| invalid_error(format!("{context} has no mip zero")))?;
        let mip_path = mip
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error(format!("{context} mip zero has no path")))?
            .to_owned();
        let mip_hash = mip
            .get("pngBlake3")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error(format!("{context} mip zero has no PNG hash")))?
            .to_owned();
        (base_path, base_hash, index, mip_path, mip_hash)
    };
    if base_path == mip_zero_path
        || !exact_hashed_terrain_files_match(
            source_root,
            &base_path,
            &base_hash,
            &mip_zero_path,
            &mip_zero_hash,
            context,
        )?
    {
        return Ok(());
    }
    texture
        .get_mut("mips")
        .and_then(JsonValue::as_array_mut)
        .and_then(|mips| mips.get_mut(mip_zero_index))
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| invalid_error(format!("{context} mip zero disappeared")))?
        .insert("path".to_owned(), JsonValue::String(base_path));
    skipped.insert(mip_zero_path);
    Ok(())
}

pub(super) fn exact_hashed_terrain_files_match(
    source_root: &Path,
    left_path: &str,
    left_hash: &str,
    right_path: &str,
    right_hash: &str,
    context: &str,
) -> Result<bool> {
    let left_expected = normalized_blake3(left_hash, context)?;
    let right_expected = normalized_blake3(right_hash, context)?;
    if left_expected != right_expected {
        return Ok(false);
    }
    let left = read_regular_file(&safe_join(source_root, left_path)?, context)?;
    let right = read_regular_file(&safe_join(source_root, right_path)?, context)?;
    let actual = hash_bytes(&left);
    if actual != left_expected || hash_bytes(&right) != right_expected {
        return invalid(format!(
            "{context} base/mip-zero bytes differ from their published BLAKE3"
        ));
    }
    Ok(left == right)
}

pub(super) fn publish_shared_terrain_file(
    source_root: &Path,
    staging: &Path,
    source_relative: &str,
    output_relative: &str,
    shared: &mut SharedTerrainFiles,
) -> Result<()> {
    validate_relative(source_relative)?;
    validate_relative(output_relative)?;
    let bytes = read_regular_file(
        &safe_join(source_root, source_relative)?,
        "shared terrain-layer source",
    )?;
    publish_shared_terrain_bytes(staging, output_relative, &bytes, shared)
}

pub(super) fn publish_shared_terrain_bytes(
    staging: &Path,
    output_relative: &str,
    bytes: &[u8],
    shared: &mut SharedTerrainFiles,
) -> Result<()> {
    validate_relative(output_relative)?;
    let artifact = WorldPrefabArtifact {
        path: format!("map/{output_relative}"),
        bytes: bytes.len() as u64,
        blake3: hash_bytes(&bytes),
    };
    if let Some(existing) = shared.artifacts.get(&artifact.path) {
        if existing != &artifact {
            return invalid(format!(
                "shared terrain-layer package collision at {:?}",
                artifact.path
            ));
        }
        return Ok(());
    }
    write_new(&safe_join(staging, output_relative)?, &bytes)?;
    shared.artifacts.insert(artifact.path.clone(), artifact);
    Ok(())
}
