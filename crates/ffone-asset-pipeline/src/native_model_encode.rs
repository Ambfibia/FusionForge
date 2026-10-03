//! Encode a `ffone.skinned-model.v1` document into a published GLB.
//!
//! The logical-model publisher builds its `NativeModel` from an exact Unity
//! source. A handful of characters have no Unity source to export: the modded
//! build authored them outside the client and kept only a decomposed dump, so
//! the model arrives as an authored glTF plus its texture. This command is the
//! last step of that path -- it takes a `NativeModel` written by
//! `tools/legacy-sources/build-authored-character-model.py`, runs the same
//! validator every published model passes, and emits the GLB with
//! `ffone_skinned_model::encode_glb`, so an authored model can never reach the
//! runtime through a weaker contract than an exported one.

use std::{fs, path::PathBuf};

use ffone_skinned_model::{NativeModel, encode_glb, validate};

use crate::{PipelineError, Result, error::io_at};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeModelEncodeOptions {
    pub model: PathBuf,
    pub output_glb: PathBuf,
}

impl NativeModelEncodeOptions {
    pub fn new(model: impl Into<PathBuf>, output_glb: impl Into<PathBuf>) -> Self {
        Self {
            model: model.into(),
            output_glb: output_glb.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeModelEncodeReport {
    pub schema: String,
    pub logical_name: String,
    pub output_glb: String,
    pub glb_bytes: u64,
    pub glb_blake3: String,
    pub nodes: u64,
    pub meshes: u64,
    pub materials: u64,
    pub textures: u64,
    pub animations: u64,
    pub animation_names: Vec<String>,
}

pub const NATIVE_MODEL_ENCODE_SCHEMA: &str = "ffone.native-model-encode.v1";

pub fn encode_native_model(options: &NativeModelEncodeOptions) -> Result<NativeModelEncodeReport> {
    let bytes = fs::read(&options.model).map_err(|error| io_at(&options.model, error))?;
    let model: NativeModel =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: options.model.display().to_string(),
            source,
        })?;
    validate(&model).map_err(|error| {
        PipelineError::InvalidManifest(format!("authored native model is invalid: {error}"))
    })?;
    let glb = encode_glb(&model).map_err(|error| {
        PipelineError::InvalidManifest(format!(
            "authored native model could not be encoded: {error}"
        ))
    })?;

    if let Some(parent) = options.output_glb.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    fs::write(&options.output_glb, &glb).map_err(|error| io_at(&options.output_glb, error))?;

    Ok(NativeModelEncodeReport {
        schema: NATIVE_MODEL_ENCODE_SCHEMA.to_owned(),
        logical_name: model.name.clone(),
        output_glb: options.output_glb.display().to_string(),
        glb_bytes: glb.len() as u64,
        glb_blake3: blake3::hash(&glb).to_hex().to_string(),
        nodes: model.nodes.len() as u64,
        meshes: model.meshes.len() as u64,
        materials: model.materials.len() as u64,
        textures: model.textures.len() as u64,
        animations: model.animations.len() as u64,
        animation_names: model
            .animations
            .iter()
            .map(|clip| clip.name.clone())
            .collect(),
    })
}
