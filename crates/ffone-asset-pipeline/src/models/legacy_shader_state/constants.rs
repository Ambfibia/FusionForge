
pub(super) const SKINNED_TOON: &str = "SkinnedToonShading_blendSrcalphaInvsrcalpha";

// Alternate TrainingGrounds.resourceFile Shader PathID 3950161577, reached
// from the Fusion Zak extension. Its BASE pass omits `Cull`, which is the
// ShaderLab default Back state; the OUTLINE pass still declares Cull Front.
pub(super) const SKINNED_TOON_IMPLICIT_BACKFACE_SHA256: &str =
    "0dd25609fc875bc337f60027eca89973a586554a38ffb591c3778ac0a4f0e2d4";

// Retrobution 2026-08-21 common toon program with sky-colored rimlight and
// explicit Back/On/LEqual state in the BASE pass.
pub(super) const SKINNED_TOON_SKY_RIM_EXPLICIT_DEPTH_SHA256: &str =
    "7be0aa97a32e4c012f0fa2da8b1e392de8ec63d97c66e66678570e4492257fbb";

pub(super) const SKINNED_TOON_E1: &str = "SkinnedToonShading_blendSrcalphaInvsrcalpha e1";

pub(super) const SKINNED_TOON_E1_BASE_CULL_OFF_SHA256: &str =
    "bbbabcb8a4d22bf80f390224eb4cbebe97375b04f4b2df9f258b37656895e278";

pub(super) const RIGID_TOON: &str = "ToonShading_blendSrcalphaInvsrcalpha";

// Alternate TrainingGrounds.resourceFile Shader PathID 3513714044, used by
// the rigid attachments on Drew Saturday, TOM and Zak Saturday. Like the
// alternate skinned toon program above, its BASE pass relies on ShaderLab's
// implicit Back culling while OUTLINE explicitly uses Cull Front.
pub(super) const RIGID_TOON_IMPLICIT_BACKFACE_SHA256: &str =
    "0ead3d0ca2cb31ee50c4a68834de20f2d5753561097019754b8cc7a3425b6986";

pub(super) const FUSION_EFFECT: &str = "Skin_FusionEffect_blendSrcalphaInvsrcalpha";

// Retrobution 2026-08-21 Tutorial.resourceFile Shader PathID 1622. This is a
// true replacement for the older three-texture Fusion effect: one combined
// RGB texture drives the Fusion, bubble and electricity layers, and the
// program adds its own yellow rim plus a settings-controlled toon outline.
pub(super) const SKINNED_FUSION_MATTER_LIGHT_DIR: &str =
    "SkinnedFusionMatterLightDir_blendSrcalphaInvsrcalpha";

pub(super) const SKINNED_FUSION_MATTER_LIGHT_DIR_SHA256: &str =
    "a151f6c8746046984ec3fcac358b852e83e0535b7c169d10f16019ebfd02cc8d";

// Retro_shared_part2 Shader PathID 305, used by the clean-primary Time Pod.
// It is an RGB-only alpha blend in Transparent-100 with default Back culling
// and depth writing. The exact fixed-function source also has `_Speed`, but
// this material leaves it at the shader default zero.
pub(super) const DIFFUSE_FADE: &str = "retro_diffuseFade";

pub(super) const DIFFUSE_FADE_SHA256: &str =
    "0a5bdc216f56d33491301ea9f86aed21a9a476a28343d71c5f4b419154e1cf11";

pub(super) const SOURCE_ALPHA_TEST: &str = "normal_blendSrcalphaOneTest_cullOff";

pub(super) const SOURCE_ALPHA_TEST_SHA256: &str = "8fb847aba2d17c313ca47cecf7e3c0c2f4c92daffd20c2fe4bc08a49ae273537";

pub(super) const ADDITIVE_TEST: &str = "normal_blendOneOneTest_cullOff";

pub(super) const ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF: &str = "normal_blendOneOneTest_zwriteOff_cullOff";

pub(super) const ADDITIVE_TEST_ZWRITE_OFF_CULL_OFF_SHA256: &str =
    "8c774d255c18ac8ba5adcdd12096fc7be0b4c8d2b82ff63666c9f2b34985f28e";

pub(super) const CUTOUT_TWO_SIDED: &str = "normal_blendSrcalphaInvsrcalphaTest_cullOff";

pub(super) const TRANSPARENT_NORMAL: &str = "normal_blendSrcalphaInvsrcalpha_zwriteOff";

pub(super) const OPAQUE_NORMAL: &str = "normal";

pub(super) const OPAQUE_NORMAL_SHA256: &str =
    "d19398c7d5886af538a3c14a6bd29e16501e9543e6f12fce8acbefb46e6dd070";

pub(super) const ADDITIVE_ONE_ONE_CULL_OFF: &str = "normal_blendOneOne_zwriteOff_cullOff";

pub(super) const ADDITIVE_ONE_ONE_CULL_OFF_SHA256: &str =
    "94cf3af0dd3ecfe6f8f7e0f009a403dc6a8d41bf7eabbab6a0656f541b3f5d23";

// Clean-primary DongResources_10_10.resourceFile Shader PathID 2294,
// reached by mob/npc_hippiehop.kfm. This older 764-byte fixed-function
// program has the same declared queue/blend/depth/cull contract but omits the
// vertex-color directive present in a sibling source variant. Keep the exact
// source hash distinct instead of accepting the shader name alone.
pub(super) const ADDITIVE_ONE_ONE_CULL_OFF_HIPPIE_HOP_SHA256: &str =
    "9609d98348a9a69435c7b01485eaa12cd939b91153baeb69dfee20d28b80af79";

pub(super) const ADDITIVE_ONE_ONE: &str = "normal_blendOneOne_zwriteOff";

pub(super) const ADDITIVE_ONE_ONE_SHA256: &str =
    "62560b7eaf853158c925a3c4e207878d7a2870140452a1abf64da830a6eceb22";

pub(super) const TRANSPARENT_NORMAL_CULL_OFF: &str = "normal_blendSrcalphaInvsrcalpha_zwriteOff_cullOff";

pub(super) const TRANSPARENT_NORMAL_CULL_OFF_SHA256: &str =
    "f61ad1c94050bbba00337abe87f22340aff339dcc162d88349edad9ae785538b";

pub(super) const CUTOUT_DEFAULT_CULLING: &str = "normal_blendSrcalphaInvsrcalphaTest";

pub(super) const CUTOUT_DEFAULT_CULLING_SHA256: &str =
    "12d452e8afb0fbc67e7f85caa73c5ed6518fae0db4ff191f087928a4ffc5430a";

// Clean-primary Effects.resourceFile Shader PathID 2227, used by the ES755,
// ES790 and ES793 weapon projectiles. Despite the historical shader name, the
// source Blend command is commented out: this is one back-face-culled cutout
// pass with depth writes disabled, not the older two-pass alpha blend family.
pub(super) const CUTOUT_ZWRITE_OFF_DEFAULT_CULLING: &str = "normal_blendSrcalphaInvsrcalphaTest_zwriteOff";

pub(super) const CUTOUT_ZWRITE_OFF_DEFAULT_CULLING_SHA256: &str =
    "40c277dda0cd761f41fea088f235f513c477b806ce4f8dfabd2ca8e0cc8924b8";

pub(super) const SKINNED_TOON_CULL_OFF: &str = "SkinnedToonShading_blendSrcalphaInvsrcalpha_cullOff";

pub(super) const SKINNED_TOON_CULL_OFF_FALLBACK_SHA256: &str =
    "b7d567b18b02cb528730e5b873278dd9a66734116e03b0d7b77affbd32d36a6a";

pub(super) const SKINNED_TOON_CULL_OFF_CATEGORY_SHA256: &str =
    "3c0331a36b909638dfb3ae2aa5e02116954496de00a7e4684186560059f1fadc";

pub(super) const ADDITIVE_TRANSPARENT: &str = "normal_blendSrcalphaOne_zwriteOff";

pub(super) const ADDITIVE_TRANSPARENT_SHA256: &str =
    "71f92dd1166ea32c2b1ac5e697e0baff1685f7254cbcf03f3f247fa8bba459ff";

pub(super) const ADDITIVE_TRANSPARENT_CULL_OFF: &str = "normal_blendSrcalphaOne_zwriteOff_cullOff";

pub(super) const ADDITIVE_TRANSPARENT_CULL_OFF_SHA256: &str =
    "70fc44ad7628b4791d774f2ce72dbe8fbe72c5b643cb8676066d7466bfc0b2b1";

// Clean-primary DongResources_08_12.resourceFile Shader PathID 211, reached
// by mob/npc_bubbie.kfm for both the wave and splash meshes. This particle
// program is source-alpha additive, two-sided, depth-write disabled, and
// preserves its literal `AlphaTest Greater .01` at queue Transparent+3.
pub(super) const PARTICLE_ADDITIVE_TRANSPARENT_CULL_OFF: &str = "particle_blendSrcalphaOne_zwriteOff_cullOff";

pub(super) const PARTICLE_ADDITIVE_TRANSPARENT_CULL_OFF_SHA256: &str =
    "c92991f9071379aab384a17e9de76afdf545fb0417767277fbc095ad1c5b6fa8";

// Primary Dexter glasses: fixed-function tint * vertex color * texture DOUBLE,
// AlphaTest Greater .01, SrcAlpha One, RGB, Cull Off, ZWrite Off, queue 3003.
pub(super) const PARTICLE_ADDITIVE_CHARCREATION: &str =
    "particle_blendSrcalphaOne_zwriteOff_cullOff_charcreation";

pub(super) const PARTICLE_ADDITIVE_CHARCREATION_SHA256: &str =
    "b636aab66217f234a5fddaa619d704c3ed95a325765dcd3d2ab786011bf3fd89";

// Clean-primary player-equipment programs recovered from the final
// CharacterSelection/Retro_shared closures.  These names are deliberately
// admitted only together with their exact ShaderLab bytes: several of them
// have serialized aliases such as `Shader #782`, so name-only matching would
// silently conflate unrelated Unity programs.
pub(super) const PARTICLE_ONE_MINUS_SOURCE_ALPHA: &str = "particle_blendInvsrcalphaOne_zwriteOff_cullOff";

pub(super) const PARTICLE_ONE_MINUS_SOURCE_ALPHA_SHA256: &str =
    "fb0b9477936acacf39c540b35c5790d0a2e857a8ec83a1d9830c3dddc30e74d1";

pub(super) const PARTICLE_ONE_MINUS_DESTINATION_COLOR: &str =
    "particle_blendInvdestcolorOne_zwriteOff_cullOff";

pub(super) const PARTICLE_ONE_MINUS_DESTINATION_COLOR_SHA256: &str =
    "5f39cc1b6aef4ff07ffdf075c7e8641ae28f5d1fc02e94d876f74928c84ac0e8";

pub(super) const PARTICLE_ONE_MINUS_DESTINATION_COLOR_BACKFACE: &str =
    "particle_blendInvdestcolorOne_zwriteOff";

pub(super) const PARTICLE_ONE_MINUS_DESTINATION_COLOR_BACKFACE_SHA256: &str =
    "71e3e9fb352eb927579228089dd36116f7ec966ab7212579e8ffaab3239a8071";

pub(super) const ADDITIVE_ONE_ONE_CULL_OFF_EQUIPMENT_SHA256: &str =
    "c2f804137f20c3f08c1c4c91fa9129ef4a29376839b66698449af96283eeba0a";

pub(super) const RIM_EMISSIVE_TOON: &str = "Custom/RimEmissiveColoredTransparentTextureShaderNoScroll";

pub(super) const RIM_EMISSIVE_TOON_SHA256: &str =
    "b8ae3f85f5cc0a3057d6035f1b00bb2da7265b8ddee493143e86c0ac2eb8f220";

pub(super) const SKINNED_TOON_RIM: &str = "Custom/SkinnedToonShading_blendSrcalphaInvsrcalpha_Rim";

pub(super) const SKINNED_TOON_RIM_TRANSPARENT: &str =
    "Custom/SkinnedToonShading_blendSrcalphaInvsrcalpha_Rim_transparent";

pub(super) const SKINNED_TOON_RIM_TRANSPARENT_SHA256: &str =
    "3f675c982916ba498cfc462b75a877b2b2e8692351e23a46592ff693602ed7cb";

pub(super) const SKINNED_TOON_RIM_EXPLICIT_DEPTH_SHA256: &str =
    "544780784ee098ca95de1a9d6d806d369e716f9f740bc3c6d74ebb1ecec3ada4";

pub(super) const SKINNED_TOON_RIM_IMPLICIT_BACKFACE_SHA256: &str =
    "a2b00dda06a70c40d0e56f30f4f819d60055f0943a0af93341fb250cbe5f452b";

pub(super) const SKINNED_TOON_RIM_MATCAP: &str =
    "Custom/SkinnedToonShading_blendSrcalphaInvsrcalpha_RimMatcap";

pub(super) const SKINNED_TOON_RIM_MATCAP_SHA256: &str =
    "19c7456ddfe373f20c1cf5088c8b916ed385f66968cced2f30d662f8a23b3ea9";

pub(super) const SKINNED_TOON_FLIPPED: &str = "SkinnedToonShading_blendSrcalphaInvsrcalphaflipped";

pub(super) const SKINNED_TOON_FLIPPED_SHA256: &str =
    "c40639607e0c959bcbec0cc8d2caf153d14647782a7c383aaf50511a34c5c6a3";

pub(super) const VFX_ROTATING_FLIPBOOK: &str = "Custom/VFX/RotatingFlipbook";

pub(super) const VFX_ROTATING_FLIPBOOK_SHA256: &str =
    "32c3babb25f725ecb09fbd53815ac8553a05039e7079598684f96aa15f48dd9b";

pub(super) const VFX_SCROLL_DISTORT_ADDITIVE: &str = "Custom/VFX/ScrollDistort_wMask_Gradient_Additive";

pub(super) const VFX_SCROLL_DISTORT_ADDITIVE_SHA256: &str =
    "0299ea6f26b054e58ae252ec224e892f8eb13329d9a6c4bab3d34e4e68bc63ad";

// Retrobution 2026-08-21 Effects.resourceFile Shader PathID 1192, used by
// the ES865/ES866 3D mission markers. The first pass is conventional alpha
// blending and the second pass adds the animated packed-mask overlays.
pub(super) const VFX_HOLOGRAM_SOLID_ADDITIVE: &str = "Custom/VFX/Unique/HologramSolid_Additive";

pub(super) const VFX_HOLOGRAM_SOLID_ADDITIVE_SHA256: &str =
    "cd13e38df6afbd45b8fef5e61dbd6c1c1d1f6fc8169e2ab40467ef8ede4f915d";
