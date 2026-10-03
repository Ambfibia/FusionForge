use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use image::GenericImageView;
use sha2::{Digest, Sha256};

use crate::{PROJECT_ASSET_SCHEMA, SourcePackIdentity};

use super::*;

mod assets;
mod operations;
mod projects;
mod output;
mod state;
mod textures;
mod layout;

use assets::manifest_entry;
use operations::{first_verified_bytes, sha256_hex};
use projects::workspace_root;
use output::native_ui_work_copy;
use state::runtime_tree_sha256;
