use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PipelineError, Result, error::io_at};

#[cfg(test)]
mod tests;

mod constants;
mod types;
mod state;
mod containers;
mod assets;
mod output;
mod validation;
mod operations;

pub use constants::{LEGACY_GUI_SKIN_SCHEMA, LEGACY_GUI_SKIN_EVIDENCE_LEVEL};
use constants::{LEGACY_GUI_SKIN_LIMITATIONS, TARGET_GUI_SKINS, BUILTIN_STYLES, STYLE_STATES};
pub use types::{
    LegacyGuiSkinConversionOptions, LegacyGuiSkinConversionReport, LegacyGuiSkinCandidate,
    LegacyGuiSkinContract, LegacyGuiSkinCandidateDiagnostic, LegacyGuiSkin, LegacyGuiStyle,
    LegacyGuiInsets, LegacyGuiVector2, LegacyGuiColor
};
pub use state::LegacyGuiStyleState;
pub use containers::LegacyGuiObjectPointer;
use containers::{ObjectIdentityMatch, object_identities, object_name};
use assets::{ObjectIdentityIndex, referenced_asset_counts, normalize_path};
pub use output::convert_legacy_gui_skins;
use validation::require_gui_skin_type;
use operations::{
    diagnose_style_fields, diagnose_pointer_field, diagnose_expected_field, is_integer,
    push_candidate_diagnostic, pointer, insets, vector2, color,
    append_unresolved_pointer_diagnostics, string_field, integer_field, number_field
};
#[cfg(test)]
use containers::ObjectIdentity;
#[cfg(test)]
use output::convert_style;
#[cfg(test)]
use operations::append_unresolved_pointer_diagnostic;
