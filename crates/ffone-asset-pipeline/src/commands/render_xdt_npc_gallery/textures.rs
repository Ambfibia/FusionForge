use super::*;

#[derive(Clone, Debug)]
pub(super) struct TextureResolution {
    pub(super) path: Option<String>,
    pub(super) status: &'static str,
    pub(super) candidates: Vec<String>,
    pub(super) sampler: Option<NativeSampler>,
    pub(super) sampler_status: &'static str,
}

#[derive(Default)]
pub(super) struct RuntimeTextureCatalog {
    pub(super) by_true_name: BTreeMap<String, Vec<String>>,
    pub(super) by_blake3_prefix: BTreeMap<String, Vec<String>>,
    pub(super) samplers_by_true_name: BTreeMap<String, Vec<SourceTextureMetadata>>,
    pub(super) samplers_by_container_route: BTreeMap<String, Vec<SourceTextureMetadata>>,
    pub(super) source_proofs: Vec<SourceTextureMetadataProof>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceTextureMetadataDocument {
    pub(super) schema: String,
    pub(super) source_path: String,
    pub(super) source_file_bytes: u64,
    pub(super) source_file_sha256: String,
    pub(super) source_asset: String,
    pub(super) raw_bundle: SourceBundleProof,
    pub(super) textures: Vec<SourceTextureMetadata>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceTextureMetadataProof {
    pub(super) metadata_path: String,
    pub(super) metadata_bytes: u64,
    pub(super) metadata_sha256: String,
    pub(super) source_path: String,
    pub(super) source_file_bytes: u64,
    pub(super) source_file_sha256: String,
    pub(super) source_asset: String,
    pub(super) raw_bundle: SourceBundleProof,
    pub(super) texture_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceTextureMetadata {
    pub(super) true_name: String,
    pub(super) path_id: i64,
    pub(super) container_routes: Vec<String>,
    pub(super) source_chain_sha256: String,
    pub(super) native_png_blake3: String,
    pub(super) mip_map: bool,
    pub(super) source_mip_count: u32,
    pub(super) filter_mode: i32,
    pub(super) wrap_mode: i32,
    pub(super) anisotropy_level: i32,
    pub(super) mip_map_bias: f64,
}

pub(super) fn build_texture_resolution_report(
    records: &[EntityRecord],
    catalog: &RuntimeTextureCatalog,
) -> Vec<Value> {
    let mut grouped = BTreeMap::<(&'static str, String), Vec<&EntityRecord>>::new();
    for record in records {
        for (slot, hint) in [
            ("main", record.texture.as_deref()),
            ("sub", record.texture2.as_deref()),
        ] {
            let Some(hint) = hint else {
                continue;
            };
            grouped
                .entry((slot, hint.to_lowercase()))
                .or_default()
                .push(record);
        }
    }
    grouped
        .into_iter()
        .map(|((slot, texture_hint), rows)| {
            let resolution = catalog.resolve(&texture_hint);
            json!({
                "slot": slot,
                "trueName": texture_hint,
                "status": resolution.status,
                "selectedPath": resolution.path,
                "candidates": resolution.candidates,
                "samplerStatus": resolution.sampler_status,
                "sampler": resolution.sampler,
                "rowCount": rows.len(),
                "npcNumbers": rows.iter().map(|row| row.npc_number).collect::<Vec<_>>(),
                "modelStems": rows.iter().map(|row| row.model_stem.as_str()).collect::<BTreeSet<_>>(),
                "appliedBySetupNpc": rows.iter().filter(|row| !row.hnpc && row.npc_class < 100).count(),
            })
        })
        .collect()
}

#[derive(Default)]
pub(super) struct RuntimeTextureUsage {
    pub(super) slots: BTreeSet<&'static str>,
    pub(super) row_indices: BTreeSet<usize>,
    pub(super) npc_numbers: BTreeSet<i64>,
    pub(super) model_stems: BTreeSet<String>,
}

pub(super) fn build_runtime_texture_catalog(
    records: &[EntityRecord],
    catalog: &RuntimeTextureCatalog,
    asset_root: &Path,
) -> Result<Value, String> {
    let mut usages = BTreeMap::<String, RuntimeTextureUsage>::new();
    for record in records
        .iter()
        .filter(|record| !record.hnpc && record.npc_class < 100)
    {
        for (slot, hint) in [
            ("main", record.texture.as_deref()),
            ("sub", record.texture2.as_deref()),
        ] {
            let Some(hint) = hint else {
                continue;
            };
            let usage = usages.entry(hint.to_lowercase()).or_default();
            usage.slots.insert(slot);
            usage.row_indices.insert(record.row_index);
            usage.npc_numbers.insert(record.npc_number);
            usage.model_stems.insert(record.model_stem.clone());
        }
    }

    let mut textures = Vec::new();
    let mut blocked = Vec::new();
    for (true_name, usage) in usages {
        let resolution = catalog.resolve(&true_name);
        match (resolution.path, resolution.sampler) {
            (Some(path), Some(mut sampler)) => {
                let texture_path = asset_root.join(&path);
                let bytes = read_file(&texture_path)?;
                // The runtime contract is keyed by the XDT/container name. The
                // selected Texture2D may expose a different internal m_Name;
                // its exact filter/wrap state remains authoritative, while the
                // contract sampler identity must match the lookup key.
                if !sampler.name.eq_ignore_ascii_case(&true_name) {
                    sampler.name = true_name.clone();
                }
                textures.push(json!({
                    "trueName": true_name,
                    "path": path,
                    "sha256": sha256_hex(&bytes),
                    "sampler": sampler,
                }));
            }
            _ => blocked.push(json!({
                "trueName": true_name,
                "slots": usage.slots,
                "rowCount": usage.row_indices.len(),
                "modelStems": usage.model_stems,
                "npcNumbers": usage.npc_numbers,
                "status": resolution.status,
                "samplerStatus": resolution.sampler_status,
            })),
        }
    }
    Ok(json!({
        "schema": "ffone.xdt-npc-texture-catalog.v1",
        "textures": textures,
        "blocked": blocked,
    }))
}

pub(super) fn collect_runtime_texture_files(asset_root: &Path) -> Result<Vec<PathBuf>, String> {
    let texture_root = asset_root.join("characters");
    if !texture_root.is_dir() {
        return Err(format!(
            "runtime character texture root does not exist: {}",
            texture_root.display()
        ));
    }
    let mut pending = vec![texture_root];
    let mut textures = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            format!(
                "cannot enumerate runtime texture directory {}: {error}",
                directory.display()
            )
        })?;
        let mut entries = entries
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("cannot read entry below {}: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let file_type = entry.file_type().map_err(|error| {
                format!(
                    "cannot inspect runtime texture entry {}: {error}",
                    entry.path().display()
                )
            })?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            let path = entry.path();
            if file_type.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
            {
                textures.push(path);
            }
        }
    }
    textures.sort();
    Ok(textures)
}

impl RuntimeTextureCatalog {
    pub(super) fn load(asset_root: &Path, texture_metadata: &[PathBuf]) -> Result<Self, String> {
        let mut catalog = Self::default();
        for path in collect_runtime_texture_files(asset_root)? {
            if !path.is_file()
                || !path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
            {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let runtime_path = path
                .strip_prefix(asset_root)
                .map_err(|error| {
                    format!(
                        "runtime texture {} is outside {}: {error}",
                        path.display(),
                        asset_root.display()
                    )
                })?
                .to_string_lossy()
                .replace('\\', "/");
            let texture_bytes = read_file(&path)?;
            let digest = blake3::hash(&texture_bytes).to_hex();
            let digest_prefix = &digest[..16];
            catalog
                .by_true_name
                .entry(stem.to_lowercase())
                .or_default()
                .push(runtime_path.clone());
            catalog
                .by_blake3_prefix
                .entry(digest_prefix.to_owned())
                .or_default()
                .push(runtime_path);
        }
        for candidates in catalog.by_true_name.values_mut() {
            candidates.sort();
            candidates.dedup();
        }
        for candidates in catalog.by_blake3_prefix.values_mut() {
            candidates.sort();
            candidates.dedup();
        }
        for metadata_path in texture_metadata {
            let metadata_bytes = read_file(metadata_path)?;
            let document: SourceTextureMetadataDocument = serde_json::from_slice(&metadata_bytes)
                .map_err(|error| {
                format!(
                    "cannot parse primary Texture2D metadata {}: {error}",
                    metadata_path.display()
                )
            })?;
            if document.schema != "ffone.offline.chartexture-metadata.v1" {
                return Err(format!(
                    "Texture2D metadata {} has schema {:?}",
                    metadata_path.display(),
                    document.schema
                ));
            }
            if !matches!(
                document.source_asset.as_str(),
                "CustomAssetBundle-d304c52c4bae348e38c743762c1bd818"
                    | "CustomAssetBundle-b35a71b799a32429bbe08591cf8fa254"
                    | "CustomAssetBundle-aa120043d3c634fe9adfb5cbe08e6970"
                    | "CustomAssetBundle-bd5f53480423447d7bcaed95cb2a96c8"
                    | "CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a"
                    | "CustomAssetBundle-Retro_shared"
                    | "CustomAssetBundle-Retro_shared_part2"
            ) {
                return Err(format!(
                    "Texture2D metadata {} names unexpected primary source asset {:?}",
                    metadata_path.display(),
                    document.source_asset
                ));
            }
            if document.source_path.trim().is_empty()
                || document.source_file_bytes == 0
                || document.source_file_sha256.len() != 64
                || !document
                    .source_file_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
                || document.raw_bundle.source_path.trim().is_empty()
                || document.raw_bundle.byte_length == 0
                || !is_sha256(&document.raw_bundle.sha256)
            {
                return Err(format!(
                    "Texture2D metadata {} has incomplete source provenance",
                    metadata_path.display()
                ));
            }
            catalog.source_proofs.push(SourceTextureMetadataProof {
                metadata_path: metadata_path.to_string_lossy().replace('\\', "/"),
                metadata_bytes: metadata_bytes.len() as u64,
                metadata_sha256: sha256_hex(&metadata_bytes),
                source_path: document.source_path.clone(),
                source_file_bytes: document.source_file_bytes,
                source_file_sha256: document.source_file_sha256.clone(),
                source_asset: document.source_asset.clone(),
                raw_bundle: document.raw_bundle.clone(),
                texture_count: document.textures.len(),
            });
            for texture in document.textures {
                if texture.true_name.trim().is_empty()
                    || texture.path_id <= 0
                    || !is_sha256(&texture.source_chain_sha256)
                    || !is_blake3(&texture.native_png_blake3)
                {
                    return Err(format!(
                        "Texture2D metadata {} contains an invalid Texture2D record",
                        metadata_path.display()
                    ));
                }
                catalog
                    .samplers_by_true_name
                    .entry(texture.true_name.to_lowercase())
                    .or_default()
                    .push(texture.clone());
                for route in &texture.container_routes {
                    catalog
                        .samplers_by_container_route
                        .entry(route.replace('\\', "/").to_lowercase())
                        .or_default()
                        .push(texture.clone());
                }
            }
            for sources in catalog.samplers_by_true_name.values_mut() {
                sources.sort_by_key(|source| source.path_id);
            }
            for sources in catalog.samplers_by_container_route.values_mut() {
                sources.sort_by_key(|source| source.path_id);
            }
        }
        Ok(catalog)
    }

    pub(super) fn resolve(&self, true_name: &str) -> TextureResolution {
        let key = true_name.to_lowercase();
        let expected_routes = expected_texture_routes(&key);
        let exact_sources = expected_routes
            .iter()
            .flat_map(|route| {
                self.samplers_by_container_route
                    .get(route)
                    .into_iter()
                    .flatten()
            })
            .collect::<Vec<_>>();
        let all_sources = self
            .samplers_by_true_name
            .get(&key)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let direct_exact_selection = match all_sources.as_slice() {
            [source]
                if source.container_routes.iter().any(|route| {
                    expected_routes.contains(&route.replace('\\', "/").to_lowercase())
                }) =>
            {
                Some((*source, "verified_exact_true_name_source"))
            }
            [first, rest @ ..]
                if rest
                    .iter()
                    .all(|source| source_texture_equivalent(first, source))
                    && all_sources.iter().any(|source| {
                        source.container_routes.iter().any(|route| {
                            expected_routes.contains(&route.replace('\\', "/").to_lowercase())
                        })
                    }) =>
            {
                Some((*first, "verified_identical_exact_true_name_sources"))
            }
            _ => None,
        };
        let (selected_source, source_status) = if let Some(selection) = direct_exact_selection {
            (Some(selection.0), selection.1)
        } else {
            match exact_sources.as_slice() {
                [source] => (Some(*source), "verified_exact_container_route"),
                [] => match all_sources.as_slice() {
                    [source] => (Some(*source), "verified_unique_true_name_source"),
                    [first, rest @ ..]
                        if rest
                            .iter()
                            .all(|source| source_texture_equivalent(first, source)) =>
                    {
                        (Some(*first), "verified_identical_true_name_sources")
                    }
                    [] => (None, "source_texture_missing"),
                    _ => (None, "source_true_name_ambiguous"),
                },
                [first, rest @ ..]
                    if rest
                        .iter()
                        .all(|source| source_texture_equivalent(first, source)) =>
                {
                    (Some(*first), "verified_identical_exact_container_sources")
                }
                _ => (None, "exact_container_route_ambiguous"),
            }
        };
        let (sampler, sampler_status) = match selected_source {
            Some(source) => match source_texture_sampler(source) {
                Ok(sampler) => (Some(sampler), source_status),
                Err(()) => (None, "unsupported_source_sampler"),
            },
            None => (None, source_status),
        };
        let mut candidates = self.by_true_name.get(&key).cloned().unwrap_or_default();
        let hash_candidates = selected_source
            .and_then(|source| {
                let digest_prefix = source.native_png_blake3[..16].to_lowercase();
                self.by_blake3_prefix.get(&digest_prefix)
            })
            .cloned()
            .unwrap_or_default();
        candidates.extend(hash_candidates.iter().cloned());
        candidates.sort();
        candidates.dedup();
        let (path, status) = resolve_runtime_texture_path(&key, &candidates, &hash_candidates);
        TextureResolution {
            path,
            status,
            candidates,
            sampler,
            sampler_status,
        }
    }
}

pub(super) fn expected_texture_routes(key: &str) -> BTreeSet<String> {
    [
        format!("texture/{key}.dds"),
        format!("textures/{key}.dds"),
        format!("texture/{key}.dds.asset"),
        format!("textures/{key}.dds.asset"),
    ]
    .into_iter()
    .collect()
}

pub(super) fn resolve_runtime_texture_path(
    key: &str,
    candidates: &[String],
    hash_candidates: &[String],
) -> (Option<String>, &'static str) {
    if !hash_candidates.is_empty() {
        let canonical_suffix = format!("/{key}.png");
        let primary_spawn = if key == "spawn11_green" {
            hash_candidates.iter().find(|candidate| {
                candidate
                    .to_lowercase()
                    .contains("/spawn11_green/primary.png")
            })
        } else {
            None
        };
        let selected = primary_spawn
            .or_else(|| {
                hash_candidates
                    .iter()
                    .find(|candidate| candidate.to_lowercase().ends_with(&canonical_suffix))
            })
            .unwrap_or(&hash_candidates[0]);
        return (
            Some(selected.clone()),
            if primary_spawn.is_some() {
                "primary_selected"
            } else {
                "primary_source_hash_selected"
            },
        );
    }
    match candidates {
        [] => (None, "missing"),
        [only] => (Some(only.clone()), "verified_unique"),
        _ => (None, "ambiguous"),
    }
}

pub(super) fn source_texture_equivalent(left: &SourceTextureMetadata, right: &SourceTextureMetadata) -> bool {
    left.source_chain_sha256 == right.source_chain_sha256
        && left.native_png_blake3 == right.native_png_blake3
        && source_texture_sampler(left).ok() == source_texture_sampler(right).ok()
}

pub(super) fn source_texture_sampler(source: &SourceTextureMetadata) -> Result<NativeSampler, ()> {
    if source.source_mip_count == 0
        || (!source.mip_map && source.source_mip_count != 1)
        || !source.mip_map_bias.is_finite()
        || source.mip_map_bias != 0.0
        || source.anisotropy_level < 0
        || source.anisotropy_level > i32::from(u16::MAX)
    {
        return Err(());
    }
    let has_mips = source.source_mip_count > 1;
    let mag_filter = if source.filter_mode == 0 {
        SamplerMagFilter::Nearest
    } else {
        SamplerMagFilter::Linear
    };
    let min_filter = match (source.filter_mode, has_mips) {
        (0, false) => SamplerMinFilter::Nearest,
        (0, true) => SamplerMinFilter::NearestMipmapNearest,
        (1, false) | (2, false) => SamplerMinFilter::Linear,
        (1, true) => SamplerMinFilter::LinearMipmapNearest,
        (2, true) => SamplerMinFilter::LinearMipmapLinear,
        _ => return Err(()),
    };
    let wrap = match source.wrap_mode {
        0 => SamplerWrapMode::Repeat,
        1 => SamplerWrapMode::ClampToEdge,
        _ => return Err(()),
    };
    Ok(NativeSampler {
        name: source.true_name.clone(),
        mag_filter,
        min_filter,
        wrap_s: wrap,
        wrap_t: wrap,
        legacy_filter_mode: source.filter_mode,
        legacy_wrap_mode: source.wrap_mode,
        anisotropy_level: source.anisotropy_level as u32,
        mip_map_bias: source.mip_map_bias,
    })
}

pub(super) fn texture_name(value: Option<&Value>) -> Option<String> {
    model_name(value)
}
