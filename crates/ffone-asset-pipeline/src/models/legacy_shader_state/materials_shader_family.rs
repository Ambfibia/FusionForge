use super::*;

pub(super) const SKIN_DIRECTIONAL_ALPHA_BLEND: &str = "Skin_DirLight_AmbLight_blendSrcalphaInvsrcalpha";

// CharacterSelection.resourceFile Shader PathID 663. The full 2,601-byte
// ShaderLab program is present in every exact Retrobution face source used by
// the player-assembly candidate. A name match without this byte hash is not
// accepted.
pub(super) const SKIN_DIRECTIONAL_ALPHA_BLEND_SHA256: &str =
    "13e643aa02a6119c0f0e54a4cc77a480b2b55d6f230ede32e382472bf8b70b2c";

pub(super) const ALPHA_BLEND_NORMAL: &str = "normal_blendSrcalphaInvsrcalpha";

pub(super) const ALPHA_BLEND_NORMAL_SHA256: &str =
    "290562bc8d56959490861ceddd9adab6f2d298c50e00a73ce4d6e356f2023c3b";

pub(super) const ALPHA_BLEND_NORMAL_CULL_OFF: &str = "normal_blendSrcalphaInvsrcalpha_cullOff";

pub(super) const ALPHA_BLEND_NORMAL_CULL_OFF_SHA256: &str =
    "9ae2d1b5254d685fb08be7f1d345dab4d3d702a843aff3fc089301708754935d";

pub(super) const ALPHA_BLEND_VERTEX_COLOR: &str = "normal_blendSrcalphaInvsrcalpha_vertexColorAD";

pub(super) const ALPHA_BLEND_VERTEX_COLOR_SHA256: &str =
    "c883ed82614a52c24ab5465dbb705e64c946fbaf1df4345ccecc41b1d59c0d10";

pub(super) const GLOW_ALPHA_BLEND: &str = "normal_glow_blendSrcalphaInvsrcalpha";

pub(super) const GLOW_ALPHA_BLEND_SHA256: &str =
    "6ebaf7007733225536951c3be40e445c643c181c65d71494a678ebb364e87ffe";

/// Resolve exact legacy material state for the audited fixture shaders.
///
/// `material_render_queue` is a real Unity material custom-queue override. A
/// missing override derives the effective queue from the exact ShaderLab tag.
pub(crate) fn resolve_legacy_material(
    shader_name: &str,
    script: &str,
    material_render_queue: Option<i32>,
    floats: &[MaterialFloatProperty],
    colors: &[MaterialColorProperty],
) -> Result<(i32, Vec<MaterialPass>), String> {
    let family = ShaderFamily::from_exact_program(shader_name, script)?;
    let masked = mask_shaderlab(script)?;
    validate_declared_shader_name(family.declared_name(shader_name), script, &masked)?;

    let (category_state, first_subshader) = if !family.uses_category() {
        if find_keyword(&masked, "Category", 0, script.len()).is_some() {
            return Err(format!(
                "shader {shader_name:?} exact category-less program unexpectedly contains Category"
            ));
        }
        let subshader_keyword = find_keyword(&masked, "SubShader", 0, script.len())
            .ok_or_else(|| format!("shader {shader_name:?} has no top-level SubShader"))?;
        if brace_depth(&masked, 0, subshader_keyword) != 1 {
            return Err(format!(
                "shader {shader_name:?} first SubShader is not a direct Shader child"
            ));
        }
        let subshader = block_from_keyword(
            script,
            &masked,
            "SubShader",
            subshader_keyword,
            script.len(),
        )?;
        let first_pass_keyword = find_keyword(&masked, "Pass", subshader.open + 1, subshader.close)
            .ok_or_else(|| format!("shader {shader_name:?} first SubShader has no Pass"))?;
        (
            parse_direct_state(
                shader_name,
                "first SubShader",
                script,
                &masked,
                subshader.open + 1,
                first_pass_keyword,
                false,
                false,
            )?,
            subshader,
        )
    } else {
        let category = find_block(script, &masked, "Category", 0, script.len())?;
        if find_keyword(&masked, "Category", category.close + 1, script.len()).is_some() {
            return Err(format!(
                "shader {shader_name:?} contains more than one Category block"
            ));
        }

        let subshader_keyword =
            find_keyword(&masked, "SubShader", category.open + 1, category.close).ok_or_else(
                || format!("shader {shader_name:?} has no SubShader in its Category"),
            )?;
        if brace_depth(&masked, category.open + 1, subshader_keyword) != 0 {
            return Err(format!(
                "shader {shader_name:?} first SubShader is not a direct Category child"
            ));
        }
        (
            parse_direct_state(
                shader_name,
                "Category",
                script,
                &masked,
                category.open + 1,
                subshader_keyword,
                family == ShaderFamily::AdditiveTestZWriteOffCullOff,
                family.uses_compound_particle_fixed_state(),
            )?,
            block_from_keyword(
                script,
                &masked,
                "SubShader",
                subshader_keyword,
                category.close,
            )?,
        )
    };
    let pass_blocks = direct_child_blocks(
        script,
        &masked,
        "Pass",
        first_subshader.open + 1,
        first_subshader.close,
    )?;
    let mut pass_states = Vec::with_capacity(pass_blocks.len());
    for (index, pass) in pass_blocks.iter().enumerate() {
        pass_states.push(parse_direct_state(
            shader_name,
            &format!("first SubShader pass {index}"),
            script,
            &masked,
            pass.open + 1,
            pass.close,
            family == ShaderFamily::AdditiveTestZWriteOffCullOff,
            false,
        )?);
    }

    let expected = expected_program(family);
    require_exact_state(shader_name, "Category", &category_state, &expected.category)?;
    if pass_states.len() != expected.passes.len() {
        return Err(format!(
            "shader {shader_name:?} first SubShader has {} passes, expected {}",
            pass_states.len(),
            expected.passes.len()
        ));
    }
    for (index, (actual, required)) in pass_states.iter().zip(&expected.passes).enumerate() {
        require_exact_state(
            shader_name,
            &format!("first SubShader pass {index}"),
            actual,
            required,
        )?;
    }

    if family.requires_standard_outline_program_evidence() {
        let subshader_source = &script[first_subshader.open + 1..first_subshader.close];
        for evidence in [
            "Local 1, ([_Outline],0,0,0)",
            "Local 2, [_OutlineColor]",
            "Local 3, ([_FatFactor],0,0,0)",
        ] {
            if !subshader_source.contains(evidence) {
                return Err(format!(
                    "shader {shader_name:?} is missing exact outline evidence {evidence:?}"
                ));
            }
        }
    }

    if family.requires_sky_rim_outline_program_evidence() {
        let subshader_source = &script[first_subshader.open + 1..first_subshader.close];
        for evidence in [
            "Local 1, [_OutlineColor]",
            "Local 2, ([_FatFactor],0,0,0)",
            "Local 3, ([_Outline],0,0,0)",
        ] {
            if !subshader_source.contains(evidence) {
                return Err(format!(
                    "shader {shader_name:?} is missing exact sky-rim outline evidence {evidence:?}"
                ));
            }
        }
    }
    if family.requires_fusion_matter_outline_program_evidence() {
        let subshader_source = &script[first_subshader.open + 1..first_subshader.close];
        for evidence in ["Local 1, [_OutlineColor]", "Local 2, ([_Outline],0,0,0)"] {
            if !subshader_source.contains(evidence) {
                return Err(format!(
                    "shader {shader_name:?} is missing exact Fusion Matter outline evidence {evidence:?}"
                ));
            }
        }
    }

    let shader_queue = match category_state.queue.as_deref() {
        Some(tag) => queue_from_tag(tag)?,
        None if family == ShaderFamily::OpaqueNormal => 2_000,
        None => return Err(format!("shader {shader_name:?} has no Queue tag")),
    };
    let render_queue = match material_render_queue {
        Some(queue) if (0..=5_000).contains(&queue) => queue,
        Some(queue) => {
            return Err(format!(
                "shader {shader_name:?} custom render queue {queue} is outside 0..=5000"
            ));
        }
        None => shader_queue,
    };

    let mut inherited = ResolvedPassState::default();
    inherited.apply(&category_state, floats, script, &masked)?;

    let outline = if family.is_toon() {
        Some((
            effective_finite_float(floats, script, &masked, "_Outline")?,
            effective_finite_color(colors, script, &masked, "_OutlineColor")?,
        ))
    } else {
        None
    };
    if let Some((width, _)) = outline {
        if width < 0.0 {
            return Err(format!(
                "shader {shader_name:?} material _Outline is negative ({width})"
            ));
        }
    }

    let mut passes = Vec::with_capacity(pass_states.len());
    for (index, raw) in pass_states.iter().enumerate() {
        let mut state = inherited.clone();
        state.apply(raw, floats, script, &masked)?;
        let outline_state = match (outline, family.is_toon(), index) {
            (Some((width, color)), true, 1) => {
                // `_FatFactor` is a per-character runtime input, not a saved
                // material property. The material-local contribution remains
                // exact here; the native runtime adds `_FatFactor` per entity.
                MaterialOutlineState::WorldSpace { width, color }
            }
            _ => MaterialOutlineState::Disabled,
        };
        passes.push(state.into_material_pass(raw.name.clone(), outline_state));
    }

    Ok((render_queue, passes))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ShaderFamily {
    Toon,
    ToonImplicitBackface,
    ToonBaseCullOff,
    ToonCullOffFallback,
    ToonCullOffCategory,
    ToonExplicitDepth,
    ToonSkyRimExplicitDepth,
    ToonFlipped,
    ToonRimImplicitBackface,
    ToonRimTransparent,
    RimEmissiveToon,
    SkinDirectionalAlphaBlend,
    FusionEffect,
    SkinnedFusionMatterLightDir,
    DiffuseFade,
    AdditiveTest,
    SourceAlphaTest,
    AdditiveTestZWriteOffCullOff,
    AdditiveOneOne,
    AdditiveOneOneDepthWrite,
    AdditiveOneOneCullOff,
    AdditiveOneOneCullOffVertexColorAd,
    AdditiveOneOneCullOffDepthWrite,
    CutoutTwoSided,
    CutoutDefaultCulling,
    CutoutZWriteOffDefaultCulling,
    OpaqueNormal,
    AlphaBlendNormal,
    AlphaBlendNormalCullOff,
    AlphaBlendVertexColor,
    TransparentNormal,
    TransparentNormalCullOff,
    GlowAlphaBlend,
    AdditiveTransparent,
    AdditiveTransparentDepthWrite,
    AdditiveTransparentCullOff,
    ParticleAdditiveTransparentCullOff,
    ParticleOneMinusSourceAlpha,
    ParticleOneMinusDestinationColor,
    ParticleOneMinusDestinationColorBackface,
    VfxRotatingFlipbook,
    VfxScrollDistortAdditive,
    VfxHologramSolidAdditive,
}

impl ShaderFamily {
    pub(super) fn from_exact_program(name: &str, script: &str) -> Result<Self, String> {
        let hash = format!("{:x}", Sha256::digest(script.as_bytes()));
        Self::from_exact_name_and_hash(name, &hash)
    }

    pub(super) fn from_exact_name_and_hash(name: &str, hash: &str) -> Result<Self, String> {
        let family = match name {
            SKINNED_TOON_E1 if hash == SKINNED_TOON_E1_BASE_CULL_OFF_SHA256 => {
                Self::ToonBaseCullOff
            }
            SKINNED_TOON if hash == SKINNED_TOON_SKY_RIM_EXPLICIT_DEPTH_SHA256 => {
                Self::ToonSkyRimExplicitDepth
            }
            SKINNED_TOON if hash == SKINNED_TOON_IMPLICIT_BACKFACE_SHA256 => {
                Self::ToonImplicitBackface
            }
            RIGID_TOON if hash == RIGID_TOON_IMPLICIT_BACKFACE_SHA256 => Self::ToonImplicitBackface,
            SKINNED_TOON | SKINNED_TOON_E1 | RIGID_TOON => Self::Toon,
            SKINNED_TOON_CULL_OFF if hash == SKINNED_TOON_CULL_OFF_FALLBACK_SHA256 => {
                Self::ToonCullOffFallback
            }
            SKINNED_TOON_CULL_OFF if hash == SKINNED_TOON_CULL_OFF_CATEGORY_SHA256 => {
                Self::ToonCullOffCategory
            }
            SKIN_DIRECTIONAL_ALPHA_BLEND if hash == SKIN_DIRECTIONAL_ALPHA_BLEND_SHA256 => {
                Self::SkinDirectionalAlphaBlend
            }
            FUSION_EFFECT => Self::FusionEffect,
            "Skin_FusionEffect_blendSrcalphaInvsrcalpha_tutorial"
                if hash == "8e826e84ccc9d38379299fb8f6fec0506b4e7585dd651a166f99e290ac73c680" => Self::FusionEffect,
            SKINNED_FUSION_MATTER_LIGHT_DIR if hash == SKINNED_FUSION_MATTER_LIGHT_DIR_SHA256 => {
                Self::SkinnedFusionMatterLightDir
            }
            DIFFUSE_FADE if hash == DIFFUSE_FADE_SHA256 => Self::DiffuseFade,
            ADDITIVE_TEST => Self::AdditiveTest,
            ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF
                if hash == ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF_SHA256 =>
            {
                Self::AdditiveTestZWriteOffCullOff
            }
            ADDITIVE_ONE_ONE if hash == ADDITIVE_ONE_ONE_SHA256 => Self::AdditiveOneOne,
            ADDITIVE_ONE_ONE_DEPTH_WRITE if hash == ADDITIVE_ONE_ONE_DEPTH_WRITE_SHA256 => {
                Self::AdditiveOneOneDepthWrite
            }
            ADDITIVE_ONE_ONE_CULL_OFF
                if hash == ADDITIVE_ONE_ONE_CULL_OFF_SHA256
                    || hash == ADDITIVE_ONE_ONE_CULL_OFF_HIPPIE_HOP_SHA256
                    || hash == ADDITIVE_ONE_ONE_CULL_OFF_EQUIPMENT_SHA256 =>
            {
                Self::AdditiveOneOneCullOff
            }
            ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD
                if hash == ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD_SHA256 =>
            {
                Self::AdditiveOneOneCullOffVertexColorAd
            }
            ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE
                if hash == ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE_SHA256 =>
            {
                Self::AdditiveOneOneCullOffDepthWrite
            }
            CUTOUT_TWO_SIDED => Self::CutoutTwoSided,
            CUTOUT_DEFAULT_CULLING if hash == CUTOUT_DEFAULT_CULLING_SHA256 => {
                Self::CutoutDefaultCulling
            }
            CUTOUT_ZWRITE_OFF_DEFAULT_CULLING
                if hash == CUTOUT_ZWRITE_OFF_DEFAULT_CULLING_SHA256 =>
            {
                Self::CutoutZWriteOffDefaultCulling
            }
            OPAQUE_NORMAL if hash == OPAQUE_NORMAL_SHA256 => Self::OpaqueNormal,
            ALPHA_BLEND_NORMAL if hash == ALPHA_BLEND_NORMAL_SHA256 => Self::AlphaBlendNormal,
            ALPHA_BLEND_NORMAL_CULL_OFF if hash == ALPHA_BLEND_NORMAL_CULL_OFF_SHA256 => {
                Self::AlphaBlendNormalCullOff
            }
            ALPHA_BLEND_VERTEX_COLOR if hash == ALPHA_BLEND_VERTEX_COLOR_SHA256 => {
                Self::AlphaBlendVertexColor
            }
            TRANSPARENT_NORMAL => Self::TransparentNormal,
            TRANSPARENT_NORMAL_CULL_OFF if hash == TRANSPARENT_NORMAL_CULL_OFF_SHA256 => {
                Self::TransparentNormalCullOff
            }
            GLOW_ALPHA_BLEND if hash == GLOW_ALPHA_BLEND_SHA256 => Self::GlowAlphaBlend,
            ADDITIVE_TRANSPARENT if hash == ADDITIVE_TRANSPARENT_SHA256 => {
                Self::AdditiveTransparent
            }
            ADDITIVE_TRANSPARENT_DEPTH_WRITE if hash == ADDITIVE_TRANSPARENT_DEPTH_WRITE_SHA256 => {
                Self::AdditiveTransparentDepthWrite
            }
            SOURCE_ALPHA_TEST if hash == SOURCE_ALPHA_TEST_SHA256 => Self::SourceAlphaTest,
            ADDITIVE_TRANSPARENT_CULL_OFF_VERTEX if hash == ADDITIVE_TRANSPARENT_CULL_OFF_VERTEX_SHA256 => Self::AdditiveTransparentCullOff,
            ADDITIVE_TRANSPARENT_CULL_OFF if hash == ADDITIVE_TRANSPARENT_CULL_OFF_SHA256 => {
                Self::AdditiveTransparentCullOff
            }
            PARTICLE_ADDITIVE_TRANSPARENT_CULL_OFF
                if hash == PARTICLE_ADDITIVE_TRANSPARENT_CULL_OFF_SHA256 =>
            {
                Self::ParticleAdditiveTransparentCullOff
            }
            PARTICLE_ADDITIVE_CHARCREATION if hash == PARTICLE_ADDITIVE_CHARCREATION_SHA256 => {
                Self::ParticleAdditiveTransparentCullOff
            }
            PARTICLE_ONE_MINUS_SOURCE_ALPHA if hash == PARTICLE_ONE_MINUS_SOURCE_ALPHA_SHA256 => {
                Self::ParticleOneMinusSourceAlpha
            }
            PARTICLE_ONE_MINUS_DESTINATION_COLOR
                if hash == PARTICLE_ONE_MINUS_DESTINATION_COLOR_SHA256 =>
            {
                Self::ParticleOneMinusDestinationColor
            }
            PARTICLE_ONE_MINUS_DESTINATION_COLOR_BACKFACE
                if hash == PARTICLE_ONE_MINUS_DESTINATION_COLOR_BACKFACE_SHA256 =>
            {
                Self::ParticleOneMinusDestinationColorBackface
            }
            RIM_EMISSIVE_TOON if hash == RIM_EMISSIVE_TOON_SHA256 => Self::RimEmissiveToon,
            SKINNED_TOON_RIM_TRANSPARENT if hash == SKINNED_TOON_RIM_TRANSPARENT_SHA256 => {
                Self::ToonRimTransparent
            }
            SKINNED_TOON_RIM if hash == SKINNED_TOON_RIM_EXPLICIT_DEPTH_SHA256 => {
                Self::ToonExplicitDepth
            }
            SKINNED_TOON_RIM if hash == SKINNED_TOON_RIM_IMPLICIT_BACKFACE_SHA256 => {
                Self::ToonRimImplicitBackface
            }
            SKINNED_TOON_RIM_MATCAP if hash == SKINNED_TOON_RIM_MATCAP_SHA256 => {
                Self::ToonExplicitDepth
            }
            SKINNED_TOON_FLIPPED if hash == SKINNED_TOON_FLIPPED_SHA256 => Self::ToonFlipped,
            VFX_ROTATING_FLIPBOOK if hash == VFX_ROTATING_FLIPBOOK_SHA256 => {
                Self::VfxRotatingFlipbook
            }
            VFX_SCROLL_DISTORT_ADDITIVE if hash == VFX_SCROLL_DISTORT_ADDITIVE_SHA256 => {
                Self::VfxScrollDistortAdditive
            }
            VFX_HOLOGRAM_SOLID_ADDITIVE if hash == VFX_HOLOGRAM_SOLID_ADDITIVE_SHA256 => {
                Self::VfxHologramSolidAdditive
            }
            SKINNED_TOON_CULL_OFF
            | SKINNED_FUSION_MATTER_LIGHT_DIR
            | SKIN_DIRECTIONAL_ALPHA_BLEND
            | ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF
            | ADDITIVE_ONE_ONE
            | ADDITIVE_ONE_ONE_DEPTH_WRITE
            | ADDITIVE_ONE_ONE_CULL_OFF
            | ADDITIVE_ONE_ONE_CULL_OFF_VERTEX_COLOR_AD
            | ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE
            | CUTOUT_DEFAULT_CULLING
            | CUTOUT_ZWRITE_OFF_DEFAULT_CULLING
            | OPAQUE_NORMAL
            | ALPHA_BLEND_NORMAL
            | ALPHA_BLEND_NORMAL_CULL_OFF
            | ALPHA_BLEND_VERTEX_COLOR
            | TRANSPARENT_NORMAL_CULL_OFF
            | GLOW_ALPHA_BLEND
            | ADDITIVE_TRANSPARENT
            | ADDITIVE_TRANSPARENT_DEPTH_WRITE
            | ADDITIVE_TRANSPARENT_CULL_OFF
            | ADDITIVE_TRANSPARENT_CULL_OFF_VERTEX
            | SOURCE_ALPHA_TEST
            | PARTICLE_ADDITIVE_TRANSPARENT_CULL_OFF
            | PARTICLE_ONE_MINUS_SOURCE_ALPHA
            | PARTICLE_ONE_MINUS_DESTINATION_COLOR
            | PARTICLE_ONE_MINUS_DESTINATION_COLOR_BACKFACE
            | RIM_EMISSIVE_TOON
            | SKINNED_TOON_RIM
            | SKINNED_TOON_RIM_TRANSPARENT
            | SKINNED_TOON_RIM_MATCAP
            | SKINNED_TOON_FLIPPED
            | VFX_ROTATING_FLIPBOOK
            | VFX_SCROLL_DISTORT_ADDITIVE
            | VFX_HOLOGRAM_SOLID_ADDITIVE => {
                return Err(format!(
                    "shader {name:?} script SHA-256 {hash} does not match any audited exact program"
                ));
            }
            DIFFUSE_FADE => {
                return Err(format!(
                    "shader {name:?} script SHA-256 {hash} does not match any audited exact program"
                ));
            }
            _ => {
                return Err(format!(
                    "unsupported exact legacy shader name {name:?}; refusing fuzzy resolution"
                ));
            }
        };
        Ok(family)
    }

    pub(super) fn is_toon(self) -> bool {
        matches!(
            self,
            Self::Toon
                | Self::ToonImplicitBackface
                | Self::ToonBaseCullOff
                | Self::ToonCullOffFallback
                | Self::ToonCullOffCategory
                | Self::ToonExplicitDepth
                | Self::ToonSkyRimExplicitDepth
                | Self::ToonFlipped
                | Self::ToonRimImplicitBackface
                | Self::ToonRimTransparent
                | Self::RimEmissiveToon
                | Self::SkinnedFusionMatterLightDir
        )
    }

    pub(super) fn requires_standard_outline_program_evidence(self) -> bool {
        matches!(
            self,
            Self::Toon
                | Self::ToonImplicitBackface
                | Self::ToonBaseCullOff
                | Self::ToonCullOffFallback
                | Self::ToonCullOffCategory
        )
    }

    pub(super) fn requires_sky_rim_outline_program_evidence(self) -> bool {
        matches!(
            self,
            Self::ToonSkyRimExplicitDepth | Self::ToonRimTransparent
        )
    }

    pub(super) fn requires_fusion_matter_outline_program_evidence(self) -> bool {
        self == Self::SkinnedFusionMatterLightDir
    }

    pub(super) fn uses_category(self) -> bool {
        !matches!(self, Self::OpaqueNormal | Self::RimEmissiveToon)
    }

    pub(super) fn uses_compound_particle_fixed_state(self) -> bool {
        matches!(
            self,
            Self::ParticleAdditiveTransparentCullOff
                | Self::ParticleOneMinusSourceAlpha
                | Self::ParticleOneMinusDestinationColor
                | Self::ParticleOneMinusDestinationColorBackface
        )
    }

    pub(super) fn declared_name<'a>(self, supplied_name: &'a str) -> &'a str {
        match self {
            // All currently material-bound programs use the exact declared
            // name. This method keeps the alias boundary explicit if a future
            // serialized-only name needs to be admitted.
            _ => supplied_name,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum RawBlend {
    Disabled,
    Enabled {
        source: MaterialBlendFactor,
        destination: MaterialBlendFactor,
    },
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedPassState {
    pub(super) blend: MaterialBlendState,
    pub(super) cull: MaterialCullMode,
    pub(super) z_write: bool,
    pub(super) z_test: MaterialCompareFunction,
    pub(super) alpha_test: MaterialAlphaTestState,
    pub(super) color_mask: u8,
}

impl Default for ResolvedPassState {
    fn default() -> Self {
        Self {
            blend: MaterialBlendState {
                enabled: false,
                source_color: MaterialBlendFactor::One,
                destination_color: MaterialBlendFactor::Zero,
                color_operation: MaterialBlendOperation::Add,
                source_alpha: MaterialBlendFactor::One,
                destination_alpha: MaterialBlendFactor::Zero,
                alpha_operation: MaterialBlendOperation::Add,
            },
            cull: MaterialCullMode::Back,
            z_write: true,
            z_test: MaterialCompareFunction::LessEqual,
            alpha_test: MaterialAlphaTestState::Disabled,
            color_mask: 0b1111,
        }
    }
}

impl ResolvedPassState {
    pub(super) fn apply(
        &mut self,
        raw: &RawState,
        floats: &[MaterialFloatProperty],
        script: &str,
        masked: &[u8],
    ) -> Result<(), String> {
        if let Some(blend) = &raw.blend {
            match blend {
                RawBlend::Disabled => {
                    self.blend.enabled = false;
                    self.blend.source_color = MaterialBlendFactor::One;
                    self.blend.destination_color = MaterialBlendFactor::Zero;
                    self.blend.source_alpha = MaterialBlendFactor::One;
                    self.blend.destination_alpha = MaterialBlendFactor::Zero;
                }
                RawBlend::Enabled {
                    source,
                    destination,
                } => {
                    self.blend.enabled = true;
                    self.blend.source_color = *source;
                    self.blend.destination_color = *destination;
                    self.blend.source_alpha = *source;
                    self.blend.destination_alpha = *destination;
                }
            }
        }
        if let Some(operation) = raw.blend_operation {
            self.blend.color_operation = operation;
            self.blend.alpha_operation = operation;
        }
        if let Some(cull) = raw.cull {
            self.cull = cull;
        }
        if let Some(z_write) = raw.z_write {
            self.z_write = z_write;
        }
        if let Some(z_test) = raw.z_test {
            self.z_test = z_test;
        }
        if let Some(alpha_test) = &raw.alpha_test {
            self.alpha_test = match alpha_test {
                RawAlphaTest::Disabled => MaterialAlphaTestState::Disabled,
                RawAlphaTest::Enabled { compare, reference } => {
                    let reference = match reference {
                        RawAlphaReference::Literal(value) => {
                            MaterialAlphaReference::Literal { value: *value }
                        }
                        RawAlphaReference::FloatProperty(name) => {
                            MaterialAlphaReference::FloatProperty {
                                name: name.clone(),
                                resolved_value: effective_finite_float(
                                    floats, script, masked, name,
                                )?,
                            }
                        }
                    };
                    MaterialAlphaTestState::Enabled {
                        compare: *compare,
                        reference,
                    }
                }
            };
        }
        if let Some(color_mask) = raw.color_mask {
            self.color_mask = color_mask;
        }
        Ok(())
    }

    pub(super) fn into_material_pass(
        self,
        name: Option<String>,
        outline: MaterialOutlineState,
    ) -> MaterialPass {
        MaterialPass {
            name,
            blend: self.blend,
            cull: self.cull,
            z_write: self.z_write,
            z_test: self.z_test,
            alpha_test: self.alpha_test,
            color_mask: self.color_mask,
            outline,
        }
    }
}

pub(super) fn exact_shader_float_default(script: &str, masked: &[u8], name: &str) -> Result<f64, String> {
    // This exact reward-boost program references an undeclared float. Unity
    // initializes that uniform to zero; retain the Greater test itself.
    if name == "_Cutoff" && format!("{:x}", Sha256::digest(script.as_bytes())) == SOURCE_ALPHA_TEST_SHA256 {
        return Ok(0.0);
    }
    let value = exact_shader_property_default(script, masked, name)?;
    let parsed = value.parse::<f64>().map_err(|_| {
        format!("required exact ShaderLab float default {name:?} is ambiguous: {value:?}")
    })?;
    if !parsed.is_finite() {
        return Err(format!(
            "required exact ShaderLab float default {name:?} is not finite"
        ));
    }
    Ok(parsed)
}

pub(super) fn exact_shader_color_default(script: &str, masked: &[u8], name: &str) -> Result<[f64; 4], String> {
    let value = exact_shader_property_default(script, masked, name)?;
    let inner = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| {
            format!("required exact ShaderLab color default {name:?} is ambiguous: {value:?}")
        })?;
    let components = inner
        .split(',')
        .map(|component| component.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            format!("required exact ShaderLab color default {name:?} is ambiguous: {value:?}")
        })?;
    let components: [f64; 4] = components.try_into().map_err(|components: Vec<f64>| {
        format!(
            "required exact ShaderLab color default {name:?} has {} components",
            components.len()
        )
    })?;
    if components.iter().any(|component| !component.is_finite()) {
        return Err(format!(
            "required exact ShaderLab color default {name:?} contains a non-finite component"
        ));
    }
    Ok(components)
}

pub(super) fn exact_shader_property_default(
    script: &str,
    masked: &[u8],
    name: &str,
) -> Result<String, String> {
    let (properties_open, properties_close) = exact_properties_block(script, masked)
        .map_err(|error| format!("required exact ShaderLab property default {name:?}: {error}"))?;

    let mut matches = Vec::new();
    for line in direct_lines(script, masked, properties_open + 1, properties_close) {
        let trimmed = line.trim_start();
        let Some(remainder) = trimmed.strip_prefix(name) else {
            continue;
        };
        if remainder
            .as_bytes()
            .first()
            .is_some_and(|byte| is_identifier_continue(*byte))
        {
            continue;
        }
        let masked_line = mask_shaderlab(line)?;
        let equals = masked_line
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'=')
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if equals.len() != 1 {
            return Err(format!(
                "required exact ShaderLab property {name:?} has an ambiguous default assignment"
            ));
        }
        let rhs_start = equals[0] + 1;
        // Strings and comments are intentionally masked for structural
        // parsing. Recover the exact source RHS through the last unmasked code
        // byte: this retains `"white"`/`""` while excluding inline comments.
        let rhs_masked = &masked_line[rhs_start..];
        let rhs_end = rhs_masked
            .iter()
            .rposition(|byte| !byte.is_ascii_whitespace())
            .map_or(rhs_start, |offset| rhs_start + offset + 1);
        matches.push(line[rhs_start..rhs_end].trim().to_owned());
    }
    match matches.as_slice() {
        [value] if !value.is_empty() => Ok(value.clone()),
        [] => Err(format!(
            "required exact ShaderLab property default {name:?} is missing"
        )),
        [_] => Err(format!(
            "required exact ShaderLab property default {name:?} is empty"
        )),
        _ => Err(format!(
            "required exact ShaderLab property default {name:?} occurs more than once"
        )),
    }
}
