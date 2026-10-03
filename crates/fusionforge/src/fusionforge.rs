use std::path::{Path, PathBuf};

use ffbuildtool::bundle::AssetBundle;
use serde::Serialize;

pub mod binary;
pub(crate) mod build_patch;
mod cli;
mod inspect;
mod direct_ui;
pub(crate) mod direct_input;
mod content_pack;
pub mod coordinates;
pub mod engine;
pub mod equipment_model_batch_export;
pub mod logical_model_batch_export;
pub mod logical_model_catalog;
pub mod logical_model_export_plan;
pub mod logical_model_root_transform_audit;
pub mod logical_prop_batch_export;
pub(crate) mod managed;
pub(crate) mod managed_assembly_evidence;
pub mod modding;
mod native_terrain;
mod native_publication;
mod native_corruption_effects;
mod native_collision;
mod native_audio_paths;
mod native_texture_sharing;
mod native_nano_voice;
mod native_banker;
mod native_hnpc_clips;
mod native_player_emotes;
mod native_emote_events;
mod native_world_repairs;
mod native_repairs;
mod native_cel_shading;
mod native_darwin_normals;
mod native_json_patch;
mod native_terrain_batch;
pub mod native_world_static;
mod object_evidence;
mod preview;
pub mod reverse_engineering;
mod ui_image_mode_evidence;
mod ui_interaction_evidence;
mod unity;
pub mod unity_text_adapter;
mod world;
pub(crate) mod workspace;
pub mod world_transform_audit;

pub use cli::run_cli_from_env;
pub(crate) use cli::snapshot_npc_bundle_quiet;
pub use preview::{
    decode_texture, extract_mesh, image_to_data_url, material_preview, mesh_to_exact_source,
    mesh_to_obj, mesh_to_preview, mesh_vertex_count, terrain_to_document, texture_to_data_url,
    DecodedTexture, Matrix4, MeshData,
};
pub use unity::{
    coerce_value_for_type, empty_value_for_type, object_name, pair_name_value, read_packed_bits,
    value_array, Asset, AssetRef, ObjectInfo, ObjectKey, Pointer, TypeMetadata, TypeTree,
    UnityEnvironment, UnityValue,
};
pub use world::{inspect_world_bundles, WorldInspectOptions};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleFileSummary {
    pub name: String,
    pub size: u64,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleReadSummary {
    pub path: String,
    pub header: String,
    pub files: Vec<BundleFileSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleExtractSummary {
    pub path: String,
    pub output_dir: String,
    pub files: Vec<BundleFileSummary>,
}

fn read_bundle(path: &Path) -> Result<(String, AssetBundle), String> {
    let (header, bundle) = AssetBundle::from_file(&path.to_string_lossy())?;
    Ok((header.to_string(), bundle))
}

fn bundle_files(bundle: &AssetBundle) -> Result<Vec<BundleFileSummary>, String> {
    let mut files = bundle
        .get_uncompressed_info(0)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|(name, info)| BundleFileSummary {
            name,
            size: info.size,
            hash: info.hash,
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

pub fn inspect_bundle(path: impl AsRef<Path>) -> Result<BundleReadSummary, String> {
    let path = path.as_ref();
    let (header, bundle) = read_bundle(path)?;
    Ok(BundleReadSummary {
        path: path.to_string_lossy().to_string(),
        header,
        files: bundle_files(&bundle)?,
    })
}

pub fn extract_bundle(
    path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> Result<BundleExtractSummary, String> {
    let path = path.as_ref();
    let output_dir = output_dir.as_ref();
    let (_header, bundle) = read_bundle(path)?;
    std::fs::create_dir_all(output_dir).map_err(|err| {
        format!(
            "Could not create extract directory {}: {err}",
            output_dir.display()
        )
    })?;
    bundle.extract_files(&output_dir.to_string_lossy())?;
    Ok(BundleExtractSummary {
        path: path.to_string_lossy().to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
        files: bundle_files(&bundle)?,
    })
}

pub fn default_extract_dir(input_bundle: &Path, output_root: &Path) -> PathBuf {
    let name = input_bundle
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("bundle");
    output_root.join(name)
}
