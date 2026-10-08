use ffone_skinned_model::model_relative_path;
use serde::{Deserialize, Serialize};

use crate::{PipelineError, Result};

pub const MODEL_PUBLISH_SCHEMA: &str = "ffone.logical-model.v1";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFeatureCounts {
    pub nodes: u64,
    pub mesh_parts: u64,
    pub material_slots: u64,
    pub skinned_meshes: u64,
    pub joints: u64,
    pub inverse_bind_matrices: u64,
    pub weighted_vertices: u64,
    pub animation_clips: u64,
    pub animation_channels: u64,
    /// Exact non-TRS Unity float curves, including material animation.
    #[serde(default)]
    pub float_curves: u64,
    /// Serialized Unity TRS bindings with an exact empty `m_Curve`.
    pub empty_trs_bindings: u64,
    /// Identical serialized Unity TRS curves represented by canonical channels.
    pub duplicate_trs_bindings: u64,
    /// Raw keys belonging to preserved identical or serialized-overwrite TRS curves.
    pub duplicate_trs_keyframes: u64,
    /// Identical same-time keys collapsed inside otherwise canonical curves.
    pub duplicate_same_time_keys: u64,
    /// Curves whose non-strict serialized times were recovered from an exact sibling.
    pub animation_time_recoveries: u64,
    /// Raw keys covered by typed time-recovery provenance.
    pub recovered_animation_keyframes: u64,
    /// Conflicting duplicate constant TRS groups resolved by one exact sibling.
    pub animation_curve_recoveries: u64,
    /// Rejected conflicting serialized TRS bindings retained as provenance.
    pub rejected_conflicting_trs_bindings: u64,
    /// Raw keys retained for rejected conflicting serialized TRS bindings.
    pub rejected_conflicting_trs_keyframes: u64,
    pub animation_keyframes: u64,
    pub cubic_spline_keyframes: u64,
    pub animation_events: u64,
    /// Events whose serialized Unity object pointer is typed null (pathId == 0).
    pub animation_event_null_object_pointers: u64,
    pub object_reference_keys: u64,
    pub colliders: u64,
    pub lod_levels: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPublishContract {
    pub schema: String,
    /// Exact legacy `m_Name`; never a slug, hash or PathID fallback.
    pub legacy_name: String,
    /// Exact name of the sole glTF scene root node.
    pub root_node: String,
    pub family: String,
    pub semantic_directories: Vec<String>,
    pub output_glb: String,
    pub glb_blake3: String,
    pub source: ModelFeatureCounts,
    pub published: ModelFeatureCounts,
    /// Source semantics the decoder cannot yet represent. Publishing is forbidden while non-empty.
    pub unresolved_source_features: Vec<String>,
}

impl ModelPublishContract {
    pub fn expected_relative_path(&self) -> Result<String> {
        let directories = self
            .semantic_directories
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        model_relative_path(&self.family, &directories, &self.legacy_name)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .map_err(|error| invalid_error(error.to_string()))
    }

    pub fn validate(&self, glb_bytes: &[u8]) -> Result<()> {
        if self.schema != MODEL_PUBLISH_SCHEMA {
            return invalid(format!("expected schema {MODEL_PUBLISH_SCHEMA:?}"));
        }
        if self.root_node != self.legacy_name {
            return invalid("root_node must equal the exact legacy_name");
        }
        let canonical_path = self.expected_relative_path()?;
        let semantic_root_path = canonical_path
            .strip_prefix("models/")
            .unwrap_or(&canonical_path);
        if self.output_glb != canonical_path && self.output_glb != semantic_root_path {
            return invalid("output_glb is not the semantic true-name path");
        }
        if self.source != self.published {
            return invalid("source and published feature counts differ");
        }
        if self.source.nodes == 0 || self.source.mesh_parts == 0 {
            return invalid("a publishable model requires nodes and geometry");
        }
        if !self.unresolved_source_features.is_empty() {
            return invalid(format!(
                "unresolved source features: {}",
                self.unresolved_source_features.join(", ")
            ));
        }
        validate_feature_invariants(&self.source)?;
        let actual_hash = blake3::hash(glb_bytes).to_hex().to_string();
        if self.glb_blake3 != actual_hash {
            return invalid("GLB BLAKE3 does not match contract");
        }
        validate_exact_glb_root(glb_bytes, &self.legacy_name)
    }
}

fn invalid<T>(reason: impl Into<String>) -> Result<T> {
    Err(PipelineError::InvalidModelContract(reason.into()))
}

fn validate_feature_invariants(counts: &ModelFeatureCounts) -> Result<()> {
    let has_skin_data = counts.skinned_meshes > 0
        || counts.joints > 0
        || counts.inverse_bind_matrices > 0
        || counts.weighted_vertices > 0;
    if has_skin_data
        && (counts.skinned_meshes == 0
            || counts.joints == 0
            || counts.inverse_bind_matrices != counts.joints
            || counts.weighted_vertices == 0)
    {
        return invalid(
            "skin requires skinned meshes, weighted vertices and one inverse bind matrix per joint",
        );
    }
    if counts.animation_clips == 0
        && (counts.animation_channels > 0
            || counts.float_curves > 0
            || counts.empty_trs_bindings > 0
            || counts.duplicate_trs_bindings > 0
            || counts.duplicate_trs_keyframes > 0
            || counts.duplicate_same_time_keys > 0
            || counts.animation_time_recoveries > 0
            || counts.recovered_animation_keyframes > 0
            || counts.animation_curve_recoveries > 0
            || counts.rejected_conflicting_trs_bindings > 0
            || counts.rejected_conflicting_trs_keyframes > 0
            || counts.animation_keyframes > 0
            || counts.cubic_spline_keyframes > 0
            || counts.animation_events > 0
            || counts.animation_event_null_object_pointers > 0
            || counts.object_reference_keys > 0)
    {
        return invalid("animation payload exists without an animation clip");
    }
    if counts.animation_clips > 0
        && ((counts.animation_channels == 0
            && counts.float_curves == 0
            && counts.empty_trs_bindings == 0
            && counts.duplicate_trs_bindings == 0
            && counts.animation_curve_recoveries == 0
            && counts.animation_events == 0)
            || (counts.animation_channels > 0 && counts.animation_keyframes == 0))
    {
        return invalid(
            "animation clips require channels, float curves, exact TRS metadata or events",
        );
    }
    Ok(())
}

fn validate_exact_glb_root(glb: &[u8], expected_name: &str) -> Result<()> {
    if glb.len() < 20 || &glb[0..4] != b"glTF" {
        return invalid("payload is not GLB 2.0");
    }
    let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let chunk_type = u32::from_le_bytes(glb[16..20].try_into().unwrap());
    if version != 2 || chunk_type != 0x4e4f_534a || 20 + json_len > glb.len() {
        return invalid("payload has an invalid GLB 2.0 JSON chunk");
    }
    let document: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len])
        .map_err(|error| invalid_error(format!("invalid GLB JSON: {error}")))?;
    let scene_index = document
        .get("scene")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as usize;
    let roots = document
        .get("scenes")
        .and_then(|value| value.get(scene_index))
        .and_then(|value| value.get("nodes"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_error("GLB scene has no root node list"))?;
    if roots.len() != 1 {
        return invalid("logical model GLB must have exactly one scene root");
    }
    let root_index = roots[0]
        .as_u64()
        .ok_or_else(|| invalid_error("GLB root node index is not an integer"))?
        as usize;
    let actual_name = document
        .get("nodes")
        .and_then(|value| value.get(root_index))
        .and_then(|value| value.get("name"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid_error("GLB root node has no exact name"))?;
    if actual_name != expected_name {
        return invalid("GLB root node name differs from exact legacy_name");
    }
    Ok(())
}

fn invalid_error(reason: impl Into<String>) -> PipelineError {
    PipelineError::InvalidModelContract(reason.into())
}
