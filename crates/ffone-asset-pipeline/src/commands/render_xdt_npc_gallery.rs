//! Builds a local, git-ignored GPU gallery for every XDT NPC row that names a model.
//!
//! This is an offline audit boundary. It reads the patched XDT, the native runtime
//! character registry, and (optionally) the clean-primary logical-model plan. It
//! never makes the FFOne runtime depend on a legacy build.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
    process::{Command},
};

use ffone_skinned_model::{NativeSampler, SamplerMagFilter, SamplerMinFilter, SamplerWrapMode};
use image::{Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[cfg(test)]
mod tests;

mod constants;
mod assets;
mod types;
mod models;
mod materials;
mod textures;
mod containers;
mod operations;
mod input;
mod systems;
mod animation;
mod state;
mod output;
mod validation;

use constants::{SCHEMA, HELP};
use assets::{
    REGISTRY_SCHEMA, RuntimeRegistry, index_registry, checked_registry_output_base,
    normalized_route
};
use types::{Options, EntityRecord, LegacyEvidence};
use models::{RuntimeModel, ModelTask, build_missing_model_report, model_name};
use materials::{ModelRenderRecord, render_models};
use textures::{
    RuntimeTextureCatalog, build_texture_resolution_report, build_runtime_texture_catalog, texture_name
};
use containers::{SourceBundleProof, object_field};
pub(super) use operations::run;
use operations::{
    sha256_hex, is_sha256, is_blake3, canonical_file,
    relative_to, set_once, escape_html
};
use input::{load_legacy_evidence, load_supplemental_blockers, read_file};
use systems::apply_supplemental_blockers;
use animation::{build_animation_coverage_report, select_animation};
use state::ensure_placeholder_for_status;
use output::{write_html, link_or_copy, write_json};
use validation::{require_file, require_directory};
#[cfg(test)]
use textures::{SourceTextureMetadata, collect_runtime_texture_files, resolve_runtime_texture_path};
#[cfg(test)]
use operations::{appearance_key, is_primary_interaction_placeholder};
