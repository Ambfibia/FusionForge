
pub(super) const HEIGHT_NORMALIZATION_DENOMINATOR: u16 = 32_767;

pub(super) const NATIVE_UV_SHIFT_FORMULA: &str =
    "u += (positiveSourceX - negativeSourceX) / (2 * (width - 1)); v -= (positiveSourceZ - negativeSourceZ) / (2 * (height - 1))";

pub(super) const SCENE_HEIGHT_FORMULA: &str = "sceneOwnerLocalTranslationY + rawU16 / 32767 * heightScale";

pub(super) const ATOMIC_RENAME_ATTEMPTS: usize = 8;
