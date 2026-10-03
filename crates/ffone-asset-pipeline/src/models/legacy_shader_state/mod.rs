//! Strict translation of the audited legacy ShaderLab programs used by the
//! controlled logical-model fixtures.
//!
//! This is deliberately not a best-effort ShaderLab parser. The complete
//! source script is authoritative: the declared shader name, category state
//! and every pass in the first supported `SubShader` must match an audited
//! program exactly. Unknown programs and contradictory evidence are
//! rejected instead of silently acquiring plausible-looking render defaults.

use ffone_skinned_model::{
    MaterialAlphaReference, MaterialAlphaTestState, MaterialBlendFactor, MaterialBlendOperation,
    MaterialBlendState, MaterialColorProperty, MaterialCompareFunction, MaterialCullMode,
    MaterialFloatProperty, MaterialOutlineState, MaterialPass, ShaderLabTextureDefault,
    ShaderLabTextureDefaultProperty,
};
use sha2::{Digest, Sha256};

#[cfg(test)]
mod tests;

mod constants;
mod materials_shader_family;
mod materials_parse_blend_factor;
mod models;
mod output;
mod types;
mod state;
mod operations_expected_program;
mod operations_direct_child_blocks;
mod validation;
mod textures;
mod input;

use constants::{
    SKINNED_TOON, SKINNED_TOON_IMPLICIT_BACKFACE_SHA256,
    SKINNED_TOON_SKY_RIM_EXPLICIT_DEPTH_SHA256, SKINNED_TOON_E1,
    SKINNED_TOON_E1_BASE_CULL_OFF_SHA256, RIGID_TOON, RIGID_TOON_IMPLICIT_BACKFACE_SHA256,
    FUSION_EFFECT, SKINNED_FUSION_MATTER_LIGHT_DIR, SKINNED_FUSION_MATTER_LIGHT_DIR_SHA256,
    DIFFUSE_FADE, DIFFUSE_FADE_SHA256, SOURCE_ALPHA_TEST, SOURCE_ALPHA_TEST_SHA256,
    ADDITIVE_TEST, ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF,
    ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF_SHA256, CUTOUT_TWO_SIDED, TRANSPARENT_NORMAL,
    OPAQUE_NORMAL, OPAQUE_NORMAL_SHA256, ADDITIVE_ONE_ONE_CULL_OFF,
    ADDITIVE_ONE_ONE_CULL_OFF_SHA256, ADDITIVE_ONE_ONE_CULL_OFF_HIPPIE_HOP_SHA256,
    ADDITIVE_ONE_ONE, ADDITIVE_ONE_ONE_SHA256, TRANSPARENT_NORMAL_CULL_OFF,
    TRANSPARENT_NORMAL_CULL_OFF_SHA256, CUTOUT_DEFAULT_CULLING, CUTOUT_DEFAULT_CULLING_SHA256,
    CUTOUT_ZWRITE_OFF_DEFAULT_CULLING, CUTOUT_ZWRITE_OFF_DEFAULT_CULLING_SHA256,
    SKINNED_TOON_CULL_OFF, SKINNED_TOON_CULL_OFF_FALLBACK_SHA256,
    SKINNED_TOON_CULL_OFF_CATEGORY_SHA256, ADDITIVE_TRANSPARENT, ADDITIVE_TRANSPARENT_SHA256,
    ADDITIVE_TRANSPARENT_CULL_OFF, ADDITIVE_TRANSPARENT_CULL_OFF_SHA256,
    PARTICLE_ADDITIVE_TRANSPARENT_CULL_OFF, PARTICLE_ADDITIVE_TRANSPARENT_CULL_OFF_SHA256,
    PARTICLE_ADDITIVE_CHARCREATION, PARTICLE_ADDITIVE_CHARCREATION_SHA256,
    PARTICLE_ONE_MINUS_SOURCE_ALPHA, PARTICLE_ONE_MINUS_SOURCE_ALPHA_SHA256,
    PARTICLE_ONE_MINUS_DESTINATION_COLOR, PARTICLE_ONE_MINUS_DESTINATION_COLOR_SHA256,
    PARTICLE_ONE_MINUS_DESTINATION_COLOR_BACKFACE,
    PARTICLE_ONE_MINUS_DESTINATION_COLOR_BACKFACE_SHA256,
    ADDITIVE_ONE_ONE_CULL_OFF_EQUIPMENT_SHA256, RIM_EMISSIVE_TOON, RIM_EMISSIVE_TOON_SHA256,
    SKINNED_TOON_RIM, SKINNED_TOON_RIM_TRANSPARENT, SKINNED_TOON_RIM_TRANSPARENT_SHA256,
    SKINNED_TOON_RIM_EXPLICIT_DEPTH_SHA256, SKINNED_TOON_RIM_IMPLICIT_BACKFACE_SHA256,
    SKINNED_TOON_RIM_MATCAP, SKINNED_TOON_RIM_MATCAP_SHA256, SKINNED_TOON_FLIPPED,
    SKINNED_TOON_FLIPPED_SHA256, VFX_ROTATING_FLIPBOOK, VFX_ROTATING_FLIPBOOK_SHA256,
    VFX_SCROLL_DISTORT_ADDITIVE, VFX_SCROLL_DISTORT_ADDITIVE_SHA256,
    VFX_HOLOGRAM_SOLID_ADDITIVE, VFX_HOLOGRAM_SOLID_ADDITIVE_SHA256
};
use materials_shader_family::{
    ShaderFamily,
    RawBlend, exact_shader_float_default, exact_shader_color_default,
    exact_shader_property_default
};
pub(crate) use materials_shader_family::resolve_legacy_material;
use materials_parse_blend_factor::{
    parse_blend_factor, parse_blend_operation,
    validate_declared_shader_name
};
pub(crate) use materials_parse_blend_factor::exact_declared_shader_name;
use models::{
    ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD,
    ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD_SHA256, ADDITIVE_TRANSPARENT_CULL_OFF_VERTEX,
    ADDITIVE_TRANSPARENT_CULL_OFF_VERTEX_SHA256
};
use output::{
    ADDITIVE_ONE_ONE_DEPTH_WRITE, ADDITIVE_ONE_ONE_DEPTH_WRITE_SHA256,
    ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE, ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE_SHA256,
    ADDITIVE_TRANSPARENT_DEPTH_WRITE, ADDITIVE_TRANSPARENT_DEPTH_WRITE_SHA256
};
use types::{RawAlphaReference, RawAlphaTest, ExpectedProgram, Block};
use state::RawState;
use operations_expected_program::{
    expected_program, effective_finite_float, effective_finite_color, exact_properties_block,
    queue_from_tag, set_once, mask_shaderlab, first_identifier, block_from_keyword, brace_depth
};
use operations_direct_child_blocks::{
    direct_child_blocks, direct_lines, is_identifier_start, is_identifier_continue
};
use validation::{require_exact_state, require_token_count};
pub(crate) use textures::exact_shader_texture_defaults;
use input::{
    parse_direct_state, parse_string_after, find_keyword,
    find_block
};
#[cfg(test)]
use materials_shader_family::{SKIN_DIRECTIONAL_ALPHA_BLEND, SKIN_DIRECTIONAL_ALPHA_BLEND_SHA256, ResolvedPassState};
