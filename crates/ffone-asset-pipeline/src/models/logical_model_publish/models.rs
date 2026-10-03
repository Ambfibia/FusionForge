use super::*;

pub const LOGICAL_MODEL_SOURCE_SCHEMA: &str = "ffone.logical-model-source.v1";

pub const LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA: &str = "ffone.logical-model-publish-report.v2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalModelPublishOptions {
    pub source: PathBuf,
    pub family: String,
    pub semantic_directories: Vec<String>,
    pub output_root: PathBuf,
    pub reviewed_texture_rebinds: Option<PathBuf>,
    pub reuse_texture_index: Option<PathBuf>,
    pub(super) output_layout: LogicalModelOutputLayout,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum LogicalModelOutputLayout {
    #[default]
    ModelsRoot,
    SemanticRoot,
}

impl LogicalModelPublishOptions {
    pub fn new(
        source: impl Into<PathBuf>,
        family: impl Into<String>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            source: source.into(),
            family: family.into(),
            semantic_directories: Vec::new(),
            output_root: output_root.into(),
            reviewed_texture_rebinds: None,
            reuse_texture_index: None,
            output_layout: LogicalModelOutputLayout::ModelsRoot,
        }
    }

    pub fn with_semantic_directories(
        mut self,
        semantic_directories: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.semantic_directories = semantic_directories.into_iter().map(Into::into).collect();
        self
    }

    /// Writes the canonical family path directly below `output_root`, without
    /// the diagnostic `models/` prefix. Installers use this for taxonomy-owned
    /// asset trees such as `characters/player/equipment/...`.
    pub fn with_semantic_root_layout(mut self) -> Self {
        self.output_layout = LogicalModelOutputLayout::SemanticRoot;
        self
    }

    pub fn with_reviewed_texture_rebinds(mut self, path: impl Into<PathBuf>) -> Self {
        self.reviewed_texture_rebinds = Some(path.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalModelPublishReport {
    pub schema: String,
    pub status: String,
    pub publishable: bool,
    pub contract: ModelPublishContract,
    /// Source authoring tools can serialize their viewport-only helper meshes
    /// beside the actual game renderer. Those helpers are not runtime
    /// geometry, but their Transform nodes remain part of the exact hierarchy.
    pub source_geometry_filter: SourceGeometryFilterReport,
    /// Value-level NativeModel -> emitted GLB proof. Counts alone are not
    /// accepted as evidence that geometry, skin or animation values survived.
    pub semantic_proof: SemanticRoundtripProof,
    pub coordinate_contract: NativeCoordinateContract,
    pub coordinate_audit: CoordinateAuditReport,
    pub material_publish: MaterialPublishReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_texture_rebinds: Option<ReviewedTextureRebindReport>,
    pub report_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceExactMeshSelectionProof {
    pub(super) schema: String,
    pub(super) policy: String,
    pub(super) excluded_candidate_meshes: usize,
    pub(super) selected_meshes: usize,
    pub(super) source_warning: String,
    pub(super) warning_disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceMesh {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) positions: Vec<f64>,
    #[serde(default)]
    pub(super) normals: Vec<f64>,
    #[serde(default)]
    pub(super) uvs: Vec<f64>,
    pub(super) indices: Vec<u32>,
    pub(super) groups: Vec<SourceGroup>,
    #[serde(default, rename = "materialIds")]
    pub(super) _material_ids: Vec<String>,
    pub(super) source_bindings: Vec<SourceBinding>,
    pub(super) skin: Option<SourceSkin>,
}

pub(super) struct ConvertedModel {
    pub(super) model: NativeModel,
    pub(super) source_counts: ModelFeatureCounts,
    pub(super) source_geometry_filter: SourceGeometryFilterReport,
    pub(super) material_slots: Vec<String>,
    pub(super) explicit_null_slots: u64,
    pub(super) texture_files: Vec<TexturePublication>,
    pub(super) material_reports: Vec<PublishedMaterialReport>,
    pub(super) texture_reports: Vec<PublishedTextureReport>,
}

pub(crate) struct PreparedLogicalModelPublication {
    pub report: LogicalModelPublishReport,
    pub(super) files: Vec<PreparedPublicationFile>,
}

impl PreparedLogicalModelPublication {
    pub(crate) fn relative_files(&self) -> impl Iterator<Item = &Path> {
        self.files.iter().map(|file| file.relative_path.as_path())
    }
}

/// Convert entirely in memory and install only final GLB/PNG files.
pub fn convert_logical_model_bytes(options: &LogicalModelPublishOptions, source: &[u8]) -> Result<LogicalModelPublishReport> {
    let (report, files)=prepare_direct_model(options, source)?;
    crate::direct_output::install(&options.output_root,&files).map_err(invalid_error)?;
    Ok(report)
}

/// Shared direct converter for multi-step recipes; no filesystem writes.
pub fn prepare_direct_model(options: &LogicalModelPublishOptions, source: &[u8]) -> Result<(LogicalModelPublishReport, Vec<(PathBuf,Vec<u8>)>)> {
    let prepared=prepare_logical_model(options,source)?;
    let files=prepared.files.into_iter().filter(|file|file.relative_path != Path::new(&prepared.report.report_path))
        .map(|file|(file.relative_path,file.bytes)).collect();
    Ok((prepared.report,files))
}

pub fn publish_logical_model(
    options: &LogicalModelPublishOptions,
) -> Result<LogicalModelPublishReport> {
    let source_bytes = fs::read(&options.source).map_err(|error| io_at(&options.source, error))?;
    let prepared = prepare_logical_model(options, &source_bytes)?;
    write_prepared_logical_model(&options.output_root, &prepared)?;
    Ok(prepared.report)
}

pub(crate) fn prepare_logical_model(
    options: &LogicalModelPublishOptions,
    source_bytes: &[u8],
) -> Result<PreparedLogicalModelPublication> {
    let mut source: SourceDocument =
        serde_json::from_slice(&source_bytes).map_err(|source_error| PipelineError::Json {
            path: options.source.display().to_string(),
            source: source_error,
        })?;
    let reviewed_texture_rebinds = apply_reviewed_texture_rebinds(options, &mut source)?;
    let mut converted = convert_source(&source)?;
    if logical_model_uses_legacy_npc_additive_deltas(&options.family, &source.logical_name) {
        rebase_legacy_npc_additive_clips(&mut converted.model.animations)?;
    }
    let semantic_directories = options
        .semantic_directories
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let canonical_relative_glb = model_relative_path(
        &options.family,
        &semantic_directories,
        &converted.model.name,
    )
    .map_err(|error| invalid_error(error.to_string()))?;
    let relative_glb = match options.output_layout {
        LogicalModelOutputLayout::ModelsRoot => canonical_relative_glb,
        LogicalModelOutputLayout::SemanticRoot => canonical_relative_glb
            .strip_prefix("models")
            .map(Path::to_path_buf)
            .map_err(|_| invalid_error("logical-model path has no canonical models/ prefix"))?,
    };
    if let Some(index) = &options.reuse_texture_index {
        native_texture_reuse::reuse_textures(index, &relative_glb, &mut converted)?;
    }
    reuse_identical_static_materials(&mut converted)?;
    validate(&converted.model).map_err(|error| invalid_error(error.to_string()))?;
    let glb = encode_glb(&converted.model).map_err(|error| invalid_error(error.to_string()))?;
    let semantic_proof = prove_semantic_roundtrip(&converted.model, &glb)
        .map_err(|error| invalid_error(error.to_string()))?;
    let coordinate_audit = coordinate_audit(&converted.model)?;
    let published_counts = feature_counts(&converted.model)?;
    if converted.source_counts != published_counts {
        return invalid(format!(
            "source/published feature count mismatch: source={:?}, published={published_counts:?}",
            converted.source_counts
        ));
    }

    let relative_glb_string = slash_path(&relative_glb);
    let contract = ModelPublishContract {
        schema: MODEL_PUBLISH_SCHEMA.to_string(),
        legacy_name: converted.model.name.clone(),
        root_node: converted.model.name.clone(),
        family: options.family.clone(),
        semantic_directories: options.semantic_directories.clone(),
        output_glb: relative_glb_string,
        glb_blake3: blake3::hash(&glb).to_hex().to_string(),
        source: converted.source_counts,
        published: published_counts,
        unresolved_source_features: Vec::new(),
    };
    contract.validate(&glb)?;

    let safe_filename = minimal_windows_glb_filename(&converted.model.name)
        .map_err(|error| invalid_error(error.to_string()))?;
    let safe_stem = safe_filename
        .strip_suffix(".glb")
        .ok_or_else(|| invalid_error("native model filename has no .glb suffix"))?;
    let relative_report = relative_glb.with_file_name(format!("{safe_stem}.publish.json"));
    let report = LogicalModelPublishReport {
        schema: LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA.to_string(),
        status: "staged-incomplete".to_string(),
        publishable: false,
        contract,
        source_geometry_filter: converted.source_geometry_filter,
        semantic_proof,
        coordinate_contract: converted.model.native_coordinate_contract.clone(),
        coordinate_audit,
        material_publish: MaterialPublishReport {
            status: "native-data-complete-runtime-validation-pending".to_string(),
            reason: "Exact renderer slot bindings, ordered material properties, effective legacy render states and external PNG textures are published natively. GPU visual parity is a separate runtime acceptance gate and is not asserted by this per-model publication report."
                .to_string(),
            preserved_slot_names: converted.material_slots,
            explicit_null_slots: converted.explicit_null_slots,
            material_count: u64_count(converted.model.materials.len(), "material count")?,
            texture_count: u64_count(converted.model.textures.len(), "texture count")?,
            sampler_count: u64_count(converted.model.samplers.len(), "sampler count")?,
            materials: converted.material_reports,
            textures: converted.texture_reports,
        },
        reviewed_texture_rebinds,
        report_path: slash_path(&relative_report),
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report).map_err(|error| {
        invalid_error(format!(
            "could not serialize logical-model publish report: {error}"
        ))
    })?;
    report_bytes.push(b'\n');
    let glb_parent = relative_glb
        .parent()
        .ok_or_else(|| invalid_error("logical-model output has no parent directory"))?;
    let mut files = converted
        .texture_files
        .into_iter()
        .map(|texture| PreparedPublicationFile {
            relative_path: native_texture_reuse::normalize_relative(&glb_parent.join(texture.uri))
                .expect("texture URI was validated before publication"),
            bytes: texture.bytes,
        })
        .collect::<Vec<_>>();
    files.push(PreparedPublicationFile {
        relative_path: relative_glb,
        bytes: glb,
    });
    // The adjacent report remains the per-model commit marker and is written last.
    files.push(PreparedPublicationFile {
        relative_path: relative_report,
        bytes: report_bytes,
    });
    Ok(PreparedLogicalModelPublication { report, files })
}

pub(crate) fn write_prepared_logical_model(
    output_root: &Path,
    prepared: &PreparedLogicalModelPublication,
) -> Result<()> {
    write_publication(output_root, &prepared.files)
}

/// Publishes a prepared model once, or proves that a prior publication is
/// byte-for-byte identical. A partial or drifted output remains a hard error;
/// this never overwrites an existing artifact.
pub(crate) fn verify_or_write_prepared_logical_model(
    output_root: &Path,
    prepared: &PreparedLogicalModelPublication,
) -> Result<()> {
    let mut existing = 0_usize;
    for file in &prepared.files {
        let path = output_root.join(&file.relative_path);
        if !path.exists() {
            continue;
        }
        existing += 1;
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        if bytes != file.bytes {
            return invalid(format!(
                "existing logical-model output differs from deterministic publication: {:?}",
                path
            ));
        }
    }
    match existing {
        0 => write_publication(output_root, &prepared.files),
        count if count == prepared.files.len() => Ok(()),
        count => invalid(format!(
            "logical-model publication is partial: {count}/{} exact outputs already exist",
            prepared.files.len()
        )),
    }
}

#[must_use]
pub(super) fn logical_model_uses_legacy_npc_additive_deltas(family: &str, logical_name: &str) -> bool {
    // Publication and runtime opt in together. Extending this list requires
    // adding the matching installed path to the client's delta-ready policy;
    // otherwise a newly rebased GLB would be treated as an absolute fallback.
    family == "mob"
        && matches!(
            logical_name,
            "mob_cerberus" | "mob_oilmonster" | "mob_sneakyspawn"
        )
}

pub(super) fn model_global_transforms(model: &NativeModel) -> Result<Vec<Mat4>> {
    let mut globals = vec![None; model.nodes.len()];
    for index in 0..model.nodes.len() {
        resolve_global_transform(model, index, &mut globals, &mut BTreeSet::new())?;
    }
    globals
        .into_iter()
        .map(|matrix| matrix.ok_or_else(|| invalid_error("coordinate global transform is missing")))
        .collect()
}

pub(super) fn model_node_paths(model: &NativeModel) -> Result<Vec<String>> {
    let mut paths = vec![None; model.nodes.len()];
    for index in 0..model.nodes.len() {
        resolve_model_node_path(model, index, &mut paths, &mut BTreeSet::new())?;
    }
    paths
        .into_iter()
        .map(|path| path.ok_or_else(|| invalid_error("coordinate node path is missing")))
        .collect()
}

pub(super) fn resolve_model_node_path(
    model: &NativeModel,
    index: usize,
    paths: &mut [Option<String>],
    visiting: &mut BTreeSet<usize>,
) -> Result<String> {
    if let Some(path) = &paths[index] {
        return Ok(path.clone());
    }
    if !visiting.insert(index) {
        return invalid("coordinate node path hierarchy contains a cycle");
    }
    let node = &model.nodes[index];
    let path = if let Some(parent) = node.parent {
        format!(
            "{}/{}",
            resolve_model_node_path(model, parent as usize, paths, visiting)?,
            node.name
        )
    } else {
        node.name.clone()
    };
    visiting.remove(&index);
    paths[index] = Some(path.clone());
    Ok(path)
}

#[cfg(test)]
pub(crate) fn logical_model_source_test_fixture() -> serde_json::Value {
    tests::fixture()
}
