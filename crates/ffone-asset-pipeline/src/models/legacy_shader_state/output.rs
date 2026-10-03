
// Clean-primary World_shared5.resourceFile Shader PathID 1855, reached by
// mob/nanomachine.kfm. This fixed-function additive program intentionally
// keeps ShaderLab's default Back culling and depth writing.
pub(super) const ADDITIVE_ONE_ONE_DEPTH_WRITE: &str = "normal_blendOneOne";

pub(super) const ADDITIVE_ONE_ONE_DEPTH_WRITE_SHA256: &str =
    "b4d0ecba4bff63754363c4af568f02d951603cd887009f9c8419fe7a93f942b4";

pub(super) const ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE: &str = "normal_blendOneOne_cullOff";

pub(super) const ADDITIVE_ONE_ONE_CULL_OFF_DEPTH_WRITE_SHA256: &str =
    "44916e5c653b2a55839e2365555b4d7b924d6dfba10f0c6d8f7043f695eec84e";

// Effects.resourceFile Shader PathID 1612, used by ES602. Unlike the sibling
// `_zwriteOff` program this exact source keeps Unity's default depth writing
// and uses the base Transparent queue.
pub(super) const ADDITIVE_TRANSPARENT_DEPTH_WRITE: &str = "normal_blendSrcalphaOne";

pub(super) const ADDITIVE_TRANSPARENT_DEPTH_WRITE_SHA256: &str =
    "abf077fb317a3c425f5aac7fad70e68dae46c4fd67bc06ccb5aa7b789720a3da";
