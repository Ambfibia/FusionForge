    use super::*;
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
    };

mod operations;
mod materials;
mod state;
mod models;
mod constants;
mod textures;
mod output;

use operations::{
    float, color, simple_script, external_source_files, toon_script, toon_script_with_defaults
};
use constants::{ADDITIVE_TRANSPARENT_SCRIPT, ADDITIVE_TRANSPARENT_CULL_OFF_SCRIPT};
