use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticCharacterModel {
    pub logical_name: String,
    pub semantic_kind: String,
    /// Exporter/source family is preserved independently from runtime taxonomy.
    pub family: String,
    pub semantic_directories: Vec<String>,
    pub glb: String,
    pub publish_report: String,
    pub glb_blake3: String,
    pub source_document: String,
    pub source_sha256: String,
    pub publish_status: String,
    pub publishable: bool,
    pub runtime_accepted: bool,
    pub production_approved: bool,
    pub authored_root: SemanticAuthoredRoot,
    pub rig: SemanticRigSummary,
    pub animations: SemanticAnimationSummary,
    pub material_shader_metadata: SemanticMaterialShaderMetadata,
}

pub(super) fn build_catalog_model(
    candidate_root: &Path,
    source_glb: &str,
    mapping: &Value,
) -> Result<SemanticCharacterModel> {
    let logical_name = required_string(mapping, "logicalName")?;
    let semantic_kind = semantic_kind(logical_name)?;
    let destination_directory = format!("characters/{semantic_kind}/{logical_name}");
    build_catalog_model_for_destination(
        candidate_root,
        source_glb,
        mapping,
        semantic_kind,
        &destination_directory,
    )
}

pub(crate) fn build_catalog_model_for_destination(
    candidate_root: &Path,
    source_glb: &str,
    mapping: &Value,
    semantic_kind: &str,
    destination_directory: &str,
) -> Result<SemanticCharacterModel> {
    validate_relative(destination_directory)?;
    if !destination_directory.starts_with("characters/") {
        return invalid("semantic character destination must live below characters/");
    }
    let source_path = join_relative(candidate_root, source_glb)?;
    let glb_bytes = fs::read(&source_path).map_err(|error| io_at(&source_path, error))?;
    validate_glb_render_contract(&glb_bytes)
        .map_err(|error| invalid_error(format!("GLB render contract failed: {error}")))?;
    validate_retrobution_fusion_eye_contract(&glb_bytes).map_err(|error| {
        invalid_error(format!("Retrobution Fusion Eyes contract failed: {error}"))
    })?;
    let source_report = source_glb
        .strip_suffix(".glb")
        .ok_or_else(|| invalid_error("structural model path is not a GLB"))?
        .to_owned()
        + ".publish.json";
    let report_path = join_relative(candidate_root, &source_report)?;
    let report_bytes = fs::read(&report_path).map_err(|error| io_at(&report_path, error))?;
    let report: Value =
        serde_json::from_slice(&report_bytes).map_err(|source| PipelineError::Json {
            path: report_path.display().to_string(),
            source,
        })?;
    let contract = report
        .get("contract")
        .ok_or_else(|| invalid_error("publish report has no contract"))?;
    let coordinate = report
        .get("coordinateAudit")
        .ok_or_else(|| invalid_error("publish report has no coordinate audit"))?;
    let materials = report
        .get("materialPublish")
        .ok_or_else(|| invalid_error("publish report has no material metadata"))?;

    validate_retrobution_fusion_eye_report(materials)?;
    let logical_name = required_string(contract, "legacy_name")?.to_owned();
    if required_string(contract, "root_node")? != logical_name
        || string(mapping, "logicalName") != Some(logical_name.as_str())
    {
        return invalid(format!(
            "exact root m_Name disagrees across model evidence for {source_glb:?}"
        ));
    }
    let expected_blake3 = required_string(contract, "glb_blake3")?;
    if blake3::hash(&glb_bytes).to_hex().as_str() != expected_blake3 {
        return invalid(format!("GLB BLAKE3 mismatch for {source_glb:?}"));
    }
    let file_name = Path::new(source_glb)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid_error("source GLB has no UTF-8 filename"))?;
    let destination_glb = format!("{destination_directory}/{file_name}");
    let destination_report = format!(
        "{destination_directory}/{}",
        Path::new(&source_report)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid_error("source report has no UTF-8 filename"))?
    );
    let published = contract
        .get("published")
        .ok_or_else(|| invalid_error("publish contract has no feature counts"))?;
    let skinned_meshes = integer(published, "skinned_meshes");
    let joints = integer(published, "joints");
    let inverse_bind_matrices = integer(published, "inverse_bind_matrices");
    let weighted_vertices = integer(published, "weighted_vertices");
    let animation_names = glb_animation_names(&glb_bytes)?;
    let model_directory = Path::new(&destination_glb)
        .parent()
        .ok_or_else(|| invalid_error("destination GLB has no parent"))?;

    let mut shaders = materials
        .get("materials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|material| SemanticShader {
            material_name: string(material, "name").unwrap_or_default().to_owned(),
            shader_name: string(material, "legacyShaderName")
                .or_else(|| string(material, "declaredShaderName"))
                .unwrap_or_default()
                .to_owned(),
            shader_sha256: string(material, "shaderSha256")
                .unwrap_or_default()
                .to_owned(),
            effective_render_queue: material
                .get("effectiveRenderQueue")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
            render_pass_count: integer(material, "renderPassCount"),
        })
        .collect::<Vec<_>>();
    shaders.sort();

    let source_model_directory = source_path
        .parent()
        .ok_or_else(|| invalid_error("source GLB has no parent"))?;
    let textures = materials
        .get("textures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|texture| {
            let uri = required_string(texture, "uri")?;
            verify_sha256_file(
                &source_model_directory.join(uri),
                required_string(texture, "sha256")?,
            )?;
            let path = slash_path(&model_directory.join(uri));
            let mip_paths = texture
                .get("mipLevels")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|mip| {
                    let uri = required_string(mip, "uri")?;
                    verify_sha256_file(
                        &source_model_directory.join(uri),
                        required_string(mip, "pngSha256")?,
                    )?;
                    Ok(slash_path(&model_directory.join(uri)))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(SemanticTexture {
                source_name: required_string(texture, "sourceName")?.to_owned(),
                path,
                source_mip_count: integer(texture, "sourceMipCount"),
                published_policy: required_string(texture, "publishedPolicy")?.to_owned(),
                png_sha256: required_string(texture, "sha256")?.to_owned(),
                mip_paths,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(SemanticCharacterModel {
        logical_name,
        semantic_kind: semantic_kind.to_owned(),
        family: required_string(contract, "family")?.to_owned(),
        semantic_directories: contract
            .get("semantic_directories")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_error("publish contract has no semantic directories"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid_error("semantic directory is not a string"))
            })
            .collect::<Result<Vec<_>>>()?,
        glb: destination_glb,
        publish_report: destination_report.clone(),
        glb_blake3: expected_blake3.to_owned(),
        source_document: required_string(mapping, "source")?.to_owned(),
        source_sha256: required_string(mapping, "sourceSha256")?.to_owned(),
        publish_status: required_string(&report, "status")?.to_owned(),
        publishable: report
            .get("publishable")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        runtime_accepted: true,
        production_approved: false,
        authored_root: SemanticAuthoredRoot {
            translation: float_array(coordinate, "rootTranslation")?,
            rotation_xyzw: float_array(coordinate, "rootRotation")?,
            scale: float_array(coordinate, "rootScale")?,
            unit_scale: coordinate
                .get("unitScale")
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid_error("coordinate audit has no unit scale"))?,
            auto_centered: coordinate
                .get("autoCentered")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            auto_scaled: coordinate
                .get("autoScaled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        rig: SemanticRigSummary {
            skinned_meshes,
            joints,
            inverse_bind_matrices,
            weighted_vertices,
            skinning_preserved: skinned_meshes > 0
                && joints > 0
                && joints == inverse_bind_matrices
                && weighted_vertices > 0,
        },
        animations: SemanticAnimationSummary {
            clips: integer(published, "animation_clips"),
            channels: integer(published, "animation_channels"),
            keyframes: integer(published, "animation_keyframes"),
            names: animation_names,
        },
        material_shader_metadata: SemanticMaterialShaderMetadata {
            publish_report: destination_report,
            material_count: integer(materials, "materialCount"),
            texture_count: integer(materials, "textureCount"),
            sampler_count: integer(materials, "samplerCount"),
            shaders,
            textures,
        },
    })
}
