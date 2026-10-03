use super::*;

pub const TUTORIAL_MODEL_ROOT: &str = "tutorial/models";

pub const TUTORIAL_MODEL_CATALOG_PATH: &str = "tutorial/models/catalog.json";

pub const TUTORIAL_MODEL_CATALOG_SCHEMA: &str = "ffone.tutorial-model-catalog.v1";

pub const TUTORIAL_MODEL_INSTALL_REPORT_SCHEMA: &str = "ffone.tutorial-model-install-report.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TutorialModelInstallOptions {
    pub candidate_root: PathBuf,
    pub asset_root: PathBuf,
    pub source_build: String,
    pub evidence_roots: Vec<PathBuf>,
    pub models: Vec<String>,
}

impl TutorialModelInstallOptions {
    pub fn new(
        candidate_root: impl Into<PathBuf>,
        asset_root: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            candidate_root: candidate_root.into(),
            asset_root: asset_root.into(),
            source_build: source_build.into(),
            evidence_roots: Vec::new(),
            models: Vec::new(),
        }
    }

    pub fn with_evidence_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.evidence_roots.push(root.into());
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.models.push(model.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialModelCatalog {
    pub schema: String,
    pub status: String,
    pub installer: String,
    pub install_prefix: String,
    pub source_build: String,
    pub candidate_batch: TutorialModelBatchProof,
    pub models: Vec<TutorialModelCatalogEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialModelBatchProof {
    pub candidate_relative_path: String,
    pub schema: String,
    pub sha256: String,
    pub selected_models: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialModelCatalogEntry {
    pub source_family: String,
    pub logical_name: String,
    pub candidate_glb: String,
    pub installed_glb: String,
    pub glb_byte_length: u64,
    pub glb_blake3: String,
    pub glb_sha256: String,
    pub source_document: String,
    pub source_sha256: String,
    pub publish_report: String,
    pub publish_report_sha256: String,
    pub semantic_roundtrip_sha256: BTreeMap<String, String>,
    pub runtime_facts: TutorialModelRuntimeFacts,
    pub gpu_evidence: TutorialModelGpuProof,
    pub closure: Vec<TutorialModelClosureFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialModelRuntimeFacts {
    pub true_name: String,
    pub standard_animation_names: Vec<String>,
    pub mesh_parts: u64,
    pub skinned_mesh_parts: u64,
    pub skin_joint_references: u64,
    pub inverse_bind_matrices: u64,
    pub materials_applied: u64,
    pub legacy_pass_companions: u64,
    pub outline_pass_companions: u64,
    pub assigned_texture_bindings: u64,
    pub exact_mip_markers: u64,
    pub exact_mip_chains: u64,
    pub exact_mip_levels: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialModelGpuProof {
    pub evidence_root: String,
    pub evidence_json: String,
    pub evidence_json_sha256: String,
    pub screenshot_png: String,
    pub screenshot_sha256: String,
    pub render_profile: String,
    pub visual_parity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialModelClosureFile {
    pub candidate_path: String,
    pub installed_path: String,
    pub kind: ProjectAssetKind,
    pub bytes: u64,
    pub blake3: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialModelInstallReport {
    pub schema: String,
    pub status: String,
    pub replaced_previous_install: bool,
    pub installed_models: u64,
    pub installed_files: u64,
    pub installed_bytes: u64,
    pub manifest_files: u64,
    pub catalog: TutorialModelCatalog,
}

pub(super) fn prepare_model(
    candidate_root: &Path,
    evidence_roots: &[EvidenceRoot],
    selection: &str,
    mapping: &LogicalModelBatchMapping,
) -> Result<(TutorialModelCatalogEntry, Vec<PreparedFile>)> {
    validate_mapping(mapping)?;
    let selected_parent = parent_slash(selection)?;
    let mapped_parent = parent_slash(&mapping.output_glb)?;
    if selected_parent != mapped_parent {
        return invalid(format!(
            "selected model parent {:?} differs from its exact batch output parent {:?}",
            selected_parent, mapped_parent
        ));
    }

    let glb_bytes = read_regular_relative(candidate_root, selection)?;
    let facts = gpu_model_facts_from_glb(&glb_bytes)
        .map_err(|error| invalid_error(format!("GLB structural facts failed: {error}")))?;
    if facts.true_name != mapping.logical_name {
        return invalid(format!(
            "GLB true name {:?} differs from batch logical name {:?}",
            facts.true_name, mapping.logical_name
        ));
    }

    let publish_relative = selection
        .strip_suffix(".glb")
        .ok_or_else(|| invalid_error("selection lost lowercase .glb suffix"))?
        .to_owned()
        + ".publish.json";
    let publish_bytes = read_regular_relative(candidate_root, &publish_relative)?;
    let publish: Value =
        serde_json::from_slice(&publish_bytes).map_err(|source| PipelineError::Json {
            path: candidate_root.join(&publish_relative).display().to_string(),
            source,
        })?;
    if publish.get("schema").and_then(Value::as_str) != Some(LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA) {
        return invalid(format!(
            "publish report for {selection:?} has an unsupported schema"
        ));
    }
    if publish
        .pointer("/semanticProof/matched")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return invalid(format!(
            "publish report for {selection:?} has no matching semantic roundtrip proof"
        ));
    }
    let semantic_roundtrip_sha256 = verify_semantic_roundtrip_hashes(&publish, selection)?;
    let contract_value = publish
        .get("contract")
        .cloned()
        .ok_or_else(|| invalid_error("publish report has no contract"))?;
    let contract: ModelPublishContract =
        serde_json::from_value(contract_value).map_err(|source| PipelineError::Json {
            path: format!("{publish_relative}#contract"),
            source,
        })?;
    validate_publish_identity(
        &publish,
        &contract,
        mapping,
        selection,
        &publish_relative,
        &glb_bytes,
    )?;

    let glb_uris = external_uri_set(&glb_bytes)?;
    let reported_external = reported_external_files(&publish, candidate_root, &selected_parent)?;
    let reported_paths = reported_external.keys().cloned().collect::<BTreeSet<_>>();
    if glb_uris != reported_paths {
        return invalid(format!(
            "GLB external URI closure and publish report differ for {selection:?}: glb={glb_uris:?}, report={reported_paths:?}"
        ));
    }

    let semantic_parent = selected_parent
        .strip_prefix("models/")
        .ok_or_else(|| invalid_error("selected model escaped the exact models/ prefix"))?;
    let destination_directory = format!("{TUTORIAL_MODEL_ROOT}/{semantic_parent}");
    validate_relative(&destination_directory)?;
    let glb_name = file_name(selection)?;
    let publish_name = file_name(&publish_relative)?;
    let installed_glb = format!("{destination_directory}/{glb_name}");
    let installed_publish = format!("{destination_directory}/{publish_name}");

    let mut prepared = vec![
        PreparedFile {
            candidate_path: selection.to_owned(),
            installed_path: installed_glb.clone(),
            kind: ProjectAssetKind::Model,
            bytes: glb_bytes.clone(),
        },
        PreparedFile {
            candidate_path: publish_relative.clone(),
            installed_path: installed_publish.clone(),
            kind: ProjectAssetKind::Data,
            bytes: publish_bytes.clone(),
        },
    ];
    for expected in reported_external.values() {
        let candidate_path = format!("{selected_parent}/{}", expected.path);
        let bytes = read_regular_relative(candidate_root, &candidate_path)?;
        if bytes.len() as u64 != expected.bytes || sha256(&bytes) != expected.sha256 {
            return invalid(format!(
                "external URI hash/length mismatch for {candidate_path:?}"
            ));
        }
        prepared.push(PreparedFile {
            candidate_path,
            installed_path: format!("{destination_directory}/{}", expected.path),
            kind: kind_for_external(&expected.path)?,
            bytes,
        });
    }
    prepared.sort_by(|left, right| left.installed_path.cmp(&right.installed_path));
    let closure = prepared
        .iter()
        .map(|file| TutorialModelClosureFile {
            candidate_path: file.candidate_path.clone(),
            installed_path: file.installed_path.clone(),
            kind: file.kind,
            bytes: file.bytes.len() as u64,
            blake3: blake3::hash(&file.bytes).to_hex().to_string(),
            sha256: sha256(&file.bytes),
        })
        .collect::<Vec<_>>();

    let evidence = find_exact_evidence(evidence_roots, selection)?;
    let gpu_proof = verify_evidence(&evidence, selection, &glb_bytes, &facts)?;

    Ok((
        TutorialModelCatalogEntry {
            source_family: mapping.family.clone(),
            logical_name: mapping.logical_name.clone(),
            candidate_glb: selection.to_owned(),
            installed_glb,
            glb_byte_length: glb_bytes.len() as u64,
            glb_blake3: contract.glb_blake3,
            glb_sha256: sha256(&glb_bytes),
            source_document: mapping.source.clone(),
            source_sha256: mapping.source_sha256.clone(),
            publish_report: installed_publish,
            publish_report_sha256: sha256(&publish_bytes),
            semantic_roundtrip_sha256,
            runtime_facts: runtime_facts(&facts),
            gpu_evidence: gpu_proof,
            closure,
        },
        prepared,
    ))
}

pub(super) fn glb_document(glb: &[u8]) -> Result<Value> {
    if glb.len() < 28 || glb.get(..4) != Some(b"glTF") {
        return invalid("logical model is not a complete GLB 2.0 file");
    }
    let version = read_u32(glb, 4)?;
    let total = read_u32(glb, 8)? as usize;
    let json_length = read_u32(glb, 12)? as usize;
    if version != 2 || total != glb.len() || read_u32(glb, 16)? != 0x4e4f_534a {
        return invalid("logical model has an invalid GLB 2.0 header");
    }
    let end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| invalid_error("GLB JSON length overflow"))?;
    if end > glb.len() {
        return invalid("logical model GLB JSON chunk is truncated");
    }
    serde_json::from_slice(&glb[20..end]).map_err(|source| PipelineError::Json {
        path: "tutorial-model.glb#JSON".to_owned(),
        source,
    })
}
