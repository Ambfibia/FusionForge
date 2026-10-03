use super::*;

pub const LOGICAL_MODEL_TREE_AUDIT_SCHEMA: &str = "ffone.logical-model-tree-audit.v1";

pub(super) const LOGICAL_MODEL_SCHEMA: &str = "ffone.skinned-model.v1";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelFeatureCounts {
    pub nodes: u64,
    pub mesh_parts: u64,
    pub material_slots: u64,
    pub skinned_meshes: u64,
    pub joints: u64,
    pub inverse_bind_matrices: u64,
    pub weighted_vertices: u64,
    pub animation_clips: u64,
    pub animation_channels: u64,
    pub empty_trs_bindings: u64,
    pub duplicate_trs_bindings: u64,
    pub duplicate_trs_keyframes: u64,
    pub duplicate_same_time_keys: u64,
    pub animation_time_recoveries: u64,
    pub recovered_animation_keyframes: u64,
    pub animation_curve_recoveries: u64,
    pub rejected_conflicting_trs_bindings: u64,
    pub rejected_conflicting_trs_keyframes: u64,
    pub animation_keyframes: u64,
    pub cubic_spline_keyframes: u64,
    pub animation_events: u64,
    pub animation_event_null_object_pointers: u64,
    pub object_reference_keys: u64,
    pub colliders: u64,
    pub lod_levels: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelTreeCounts {
    pub files: u64,
    pub glbs: u64,
    pub pngs: u64,
    pub publish_reports: u64,
    pub extra_model_formats: u64,
    pub logical_roots: u64,
    pub meshes: u64,
    pub materials: u64,
    pub images: u64,
    pub textures: u64,
    pub samplers: u64,
    pub skins: u64,
    pub standard_animation_clips: u64,
    pub metadata_only_animation_clips: u64,
    pub referenced_pngs: u64,
    pub orphan_pngs: u64,
    pub features: LogicalModelFeatureCounts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelTreeViolation {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelFileCounts {
    pub meshes: u64,
    pub materials: u64,
    pub images: u64,
    pub textures: u64,
    pub samplers: u64,
    pub skins: u64,
    pub standard_animation_clips: u64,
    pub metadata_only_animation_clips: u64,
    pub features: LogicalModelFeatureCounts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelFileAudit {
    pub path: String,
    pub logical_name: Option<String>,
    pub blake3: String,
    pub publish_report: Option<String>,
    pub passed: bool,
    pub counts: LogicalModelFileCounts,
    pub referenced_pngs: Vec<String>,
    pub violations: Vec<LogicalModelTreeViolation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelTreeAuditReport {
    pub schema: String,
    pub output_root: String,
    pub passed: bool,
    /// This audit never authorizes the GPU/runtime gate by itself.
    pub gpu_gate_pending: bool,
    pub counts: LogicalModelTreeCounts,
    pub models: Vec<LogicalModelFileAudit>,
    pub violations: Vec<LogicalModelTreeViolation>,
}

pub(super) struct ParsedGlb<'a> {
    pub(super) document: Value,
    pub(super) binary: &'a [u8],
}

pub(super) struct AuditedGlb {
    pub(super) logical_name: Option<String>,
    pub(super) counts: LogicalModelFileCounts,
    pub(super) referenced_pngs: BTreeSet<String>,
    pub(super) violations: Vec<LogicalModelTreeViolation>,
}

/// Recursively audits a candidate publication root without mutating it.
///
/// Content defects are accumulated in the serializable report.  `Err` is
/// reserved for failures that prevent the tree itself from being read.
pub fn audit_logical_model_tree(output_root: &Path) -> Result<LogicalModelTreeAuditReport> {
    if !output_root.is_dir() {
        return tree_error(format!(
            "logical-model output root is not a directory: {:?}",
            output_root
        ));
    }

    let mut root_violations = Vec::new();
    let files = walk_files(output_root, &mut root_violations)?;
    let mut glbs = Vec::new();
    let mut pngs = BTreeSet::new();
    let mut reports = BTreeMap::new();
    let mut counts = LogicalModelTreeCounts {
        files: to_u64(files.len()),
        ..LogicalModelTreeCounts::default()
    };

    for path in &files {
        let relative = relative_string(output_root, path)?;
        if Path::new(&relative)
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => value.to_str(),
                _ => None,
            })
            .any(has_generated_identity)
        {
            push_violation(
                &mut root_violations,
                "generated_identity_in_path",
                &relative,
                "candidate path contains a hash/PathID/generated-object fallback",
            );
        }
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let lower_name = file_name.to_ascii_lowercase();
        let lower_extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if lower_extension == "glb" {
            counts.glbs += 1;
            if path.extension().and_then(|value| value.to_str()) != Some("glb") {
                push_violation(
                    &mut root_violations,
                    "non_canonical_glb_extension",
                    &relative,
                    "logical-model extension must be exactly lowercase .glb",
                );
            }
            glbs.push((relative, path.clone()));
        } else if lower_extension == "png" {
            counts.pngs += 1;
            if path.extension().and_then(|value| value.to_str()) != Some("png") {
                push_violation(
                    &mut root_violations,
                    "non_canonical_png_extension",
                    &relative,
                    "viewable texture extension must be exactly lowercase .png",
                );
            }
            pngs.insert(relative);
        } else if lower_name.ends_with(".publish.json") {
            counts.publish_reports += 1;
            if !file_name.ends_with(".publish.json") {
                push_violation(
                    &mut root_violations,
                    "non_canonical_report_name",
                    &relative,
                    "publish report suffix must be exactly .publish.json",
                );
            }
            reports.insert(relative, path.clone());
        } else if is_extra_model_extension(&lower_extension) {
            counts.extra_model_formats += 1;
            push_violation(
                &mut root_violations,
                "extra_model_format",
                &relative,
                format!(
                    "candidate tree contains forbidden non-GLB model format .{lower_extension}"
                ),
            );
        }
    }
    glbs.sort_by(|left, right| left.0.cmp(&right.0));

    if glbs.is_empty() {
        push_violation(
            &mut root_violations,
            "no_logical_models",
            "",
            "candidate tree contains no logical-model GLB files",
        );
    }

    let mut models = Vec::with_capacity(glbs.len());
    let mut used_reports = BTreeSet::new();
    let mut referenced_pngs = BTreeSet::new();
    for (relative, path) in glbs {
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        let mut audited = audit_glb(output_root, &relative, &bytes, &pngs)?;
        let expected_report = report_sidecar(&relative);
        let report_path = reports.get(&expected_report);
        if let Some(report_path) = report_path {
            used_reports.insert(expected_report.clone());
            audit_publish_report(
                output_root,
                report_path,
                &expected_report,
                &relative,
                &hash,
                &bytes,
                audited.logical_name.as_deref(),
                &audited.counts,
                &pngs,
                &mut audited.violations,
            )?;
        } else {
            push_violation(
                &mut audited.violations,
                "missing_publish_report",
                &relative,
                format!("missing adjacent publish report {expected_report:?}"),
            );
        }

        referenced_pngs.extend(audited.referenced_pngs.iter().cloned());
        accumulate_file_counts(&mut counts, &audited.counts);
        counts.logical_roots += u64::from(audited.logical_name.is_some());
        let passed = audited.violations.is_empty();
        models.push(LogicalModelFileAudit {
            path: relative,
            logical_name: audited.logical_name,
            blake3: hash,
            publish_report: report_path.map(|_| expected_report),
            passed,
            counts: audited.counts,
            referenced_pngs: audited.referenced_pngs.into_iter().collect(),
            violations: audited.violations,
        });
    }

    for report in reports.keys() {
        if !used_reports.contains(report) {
            push_violation(
                &mut root_violations,
                "orphan_publish_report",
                report,
                "publish report has no adjacent logical-model GLB",
            );
        }
    }
    for png in pngs.difference(&referenced_pngs) {
        counts.orphan_pngs += 1;
        push_violation(
            &mut root_violations,
            "orphan_png",
            png,
            "PNG is not referenced by any logical-model image URI",
        );
    }
    counts.referenced_pngs = to_u64(referenced_pngs.len());

    let mut violations = root_violations;
    for model in &models {
        violations.extend(model.violations.iter().cloned());
    }
    violations.sort_by(|left, right| {
        (&left.path, &left.code, &left.message).cmp(&(&right.path, &right.code, &right.message))
    });
    let passed = violations.is_empty();
    Ok(LogicalModelTreeAuditReport {
        schema: LOGICAL_MODEL_TREE_AUDIT_SCHEMA.to_owned(),
        output_root: slash_path(output_root),
        passed,
        // A successful structural audit still cannot prove Bevy GPU parity.
        gpu_gate_pending: true,
        counts,
        models,
        violations,
    })
}

pub(super) fn audit_glb(
    output_root: &Path,
    relative: &str,
    bytes: &[u8],
    pngs: &BTreeSet<String>,
) -> Result<AuditedGlb> {
    let mut violations = Vec::new();
    let parsed = match parse_glb(bytes) {
        Ok(parsed) => parsed,
        Err(message) => {
            push_violation(&mut violations, "invalid_glb", relative, message);
            return Ok(AuditedGlb {
                logical_name: None,
                counts: LogicalModelFileCounts::default(),
                referenced_pngs: BTreeSet::new(),
                violations,
            });
        }
    };
    let document = &parsed.document;
    if document.pointer("/asset/version").and_then(Value::as_str) != Some("2.0") {
        push_violation(
            &mut violations,
            "invalid_gltf_version",
            relative,
            "GLB asset.version must be 2.0",
        );
    }
    if document.pointer("/extras/schema").and_then(Value::as_str) != Some(LOGICAL_MODEL_SCHEMA) {
        push_violation(
            &mut violations,
            "invalid_model_schema",
            relative,
            format!("root extras.schema must be {LOGICAL_MODEL_SCHEMA:?}"),
        );
    }
    let coordinate_contract = document
        .pointer("/extras/nativeCoordinateContract")
        .cloned()
        .and_then(|value| serde_json::from_value::<NativeCoordinateContract>(value).ok());
    if coordinate_contract.as_ref() != Some(&exact_native_coordinate_contract()) {
        push_violation(
            &mut violations,
            "invalid_native_coordinate_contract",
            relative,
            "GLB coordinate contract must preserve exact source root TRS/unit scale and forbid auto-center/auto-scale",
        );
    }

    let logical_name = document
        .pointer("/extras/logicalModelName")
        .and_then(Value::as_str)
        .map(str::to_owned);
    match logical_name.as_deref() {
        Some(name) => {
            validate_true_name(name, "logical root", relative, &mut violations);
            match minimal_windows_glb_filename(name) {
                Ok(expected) => {
                    let actual = Path::new(relative)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default();
                    if actual != expected {
                        push_violation(
                            &mut violations,
                            "logical_name_filename_mismatch",
                            relative,
                            format!(
                                "filename {actual:?} must equal minimally Windows-safe true name {expected:?}"
                            ),
                        );
                    }
                }
                Err(error) => push_violation(
                    &mut violations,
                    "invalid_logical_name",
                    relative,
                    error.to_string(),
                ),
            }
        }
        None => push_violation(
            &mut violations,
            "missing_logical_name",
            relative,
            "root extras.logicalModelName is required",
        ),
    }

    let (views, accessors) =
        audit_binary_layout(document, parsed.binary, relative, &mut violations);
    let _ = views;
    let nodes = array_or_empty(document, "nodes");
    let meshes = array_or_empty(document, "meshes");
    let skins = array_or_empty(document, "skins");
    let materials = array_or_empty(document, "materials");
    let images = array_or_empty(document, "images");
    let textures = array_or_empty(document, "textures");
    let samplers = array_or_empty(document, "samplers");
    let animations = array_or_empty(document, "animations");
    let metadata_animations = document
        .pointer("/extras/ffone/metadataOnlyAnimations")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let root_index = audit_hierarchy(
        document,
        nodes,
        meshes.len(),
        skins.len(),
        logical_name.as_deref(),
        relative,
        &mut violations,
    );
    if root_index.is_none() && logical_name.is_some() {
        push_violation(
            &mut violations,
            "logical_root_unresolved",
            relative,
            "could not establish the sole reachable true-name root",
        );
    }

    let mut counts = LogicalModelFileCounts {
        meshes: to_u64(meshes.len()),
        materials: to_u64(materials.len()),
        images: to_u64(images.len()),
        textures: to_u64(textures.len()),
        samplers: to_u64(samplers.len()),
        skins: to_u64(skins.len()),
        standard_animation_clips: to_u64(animations.len()),
        metadata_only_animation_clips: to_u64(metadata_animations.len()),
        features: LogicalModelFeatureCounts {
            nodes: to_u64(nodes.len()),
            animation_clips: to_u64(animations.len() + metadata_animations.len()),
            ..LogicalModelFeatureCounts::default()
        },
    };

    let skin_palettes = audit_skins(
        skins,
        nodes.len(),
        &accessors,
        relative,
        &mut counts.features,
        &mut violations,
    );
    audit_meshes(
        meshes,
        nodes,
        materials.len(),
        &skin_palettes,
        &accessors,
        parsed.binary,
        relative,
        &mut counts.features,
        &mut violations,
    );
    let referenced_pngs = audit_materials_and_images(
        output_root,
        relative,
        materials,
        images,
        textures,
        samplers,
        pngs,
        &mut violations,
    )?;
    audit_animations(
        animations,
        metadata_animations,
        nodes,
        &accessors,
        parsed.binary,
        relative,
        &mut counts.features,
        &mut violations,
    );

    Ok(AuditedGlb {
        logical_name,
        counts,
        referenced_pngs,
        violations,
    })
}

pub(super) fn parse_glb(bytes: &[u8]) -> std::result::Result<ParsedGlb<'_>, String> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return Err("payload is not a GLB file".to_owned());
    }
    let version = read_u32(bytes, 4).ok_or("truncated GLB version")?;
    let declared = read_u32(bytes, 8).ok_or("truncated GLB length")? as usize;
    if version != 2 || declared != bytes.len() {
        return Err("GLB version/declared length is invalid".to_owned());
    }
    let mut cursor = 12usize;
    let mut json_chunk = None;
    let mut binary = &[][..];
    let mut chunk_index = 0usize;
    while cursor < bytes.len() {
        if cursor.checked_add(8).is_none_or(|end| end > bytes.len()) {
            return Err("truncated GLB chunk header".to_owned());
        }
        let length = read_u32(bytes, cursor).ok_or("truncated GLB chunk length")? as usize;
        let kind = read_u32(bytes, cursor + 4).ok_or("truncated GLB chunk type")?;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or("GLB chunk exceeds declared payload")?;
        match (chunk_index, kind) {
            (0, 0x4e4f_534a) => json_chunk = Some(&bytes[start..end]),
            (1, 0x004e_4942) => binary = &bytes[start..end],
            _ => return Err("GLB contains an unexpected chunk/order".to_owned()),
        }
        cursor = end;
        chunk_index += 1;
    }
    let json_chunk = json_chunk.ok_or("GLB has no leading JSON chunk")?;
    let document = serde_json::from_slice(json_chunk)
        .map_err(|error| format!("invalid GLB JSON chunk: {error}"))?;
    Ok(ParsedGlb { document, binary })
}

pub(super) fn hierarchy_paths_from_glb_nodes(nodes: &[Value]) -> Vec<Option<String>> {
    let mut parents = vec![None; nodes.len()];
    let mut ambiguous = BTreeSet::new();
    for (parent, node) in nodes.iter().enumerate() {
        for child in node
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
            .filter_map(|child| usize::try_from(child).ok())
            .filter(|child| *child < nodes.len())
        {
            if parents[child].replace(parent).is_some() {
                ambiguous.insert(child);
            }
        }
    }
    (0..nodes.len())
        .map(|index| {
            let mut cursor = Some(index);
            let mut seen = BTreeSet::new();
            let mut segments = Vec::new();
            while let Some(node) = cursor {
                if !seen.insert(node) || ambiguous.contains(&node) {
                    return None;
                }
                segments.push(nodes.get(node)?.get("name")?.as_str()?);
                cursor = parents[node];
            }
            segments.reverse();
            Some(segments.join("/"))
        })
        .collect()
}

pub(super) fn is_extra_model_extension(extension: &str) -> bool {
    matches!(
        extension,
        "gltf"
            | "fbx"
            | "obj"
            | "dae"
            | "3ds"
            | "blend"
            | "ply"
            | "stl"
            | "x"
            | "mesh"
            | "asset"
            | "unity3d"
    )
}
