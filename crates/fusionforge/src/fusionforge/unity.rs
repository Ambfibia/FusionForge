use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde_json::{json, Value as JsonValue};

use super::binary::{BinaryReader, BinaryWriter, BitReader, Endian};

#[cfg(test)]
mod tests;

mod constants;
mod types;
mod operations;
mod assets;
mod containers;
mod input;
mod output;

use constants::{STRINGS_DAT, STRUCTS_DAT, CLASSES_JSON};
pub use types::{TypeTree, TypeMetadata, Pointer};
use operations::blob_string;
pub use operations::{
    empty_value_for_type, coerce_value_for_type, pointer_summary, vector, vector2, class_name,
    archive_name_from_ref, value_array, pair_name_value
};
pub use assets::{AssetRef, Asset, archive_ref_asset_name};
use assets::{
    write_path_id, write_pointer_path_id,
    read_pointer_path_id
};
pub use containers::{
    ObjectInfo, ReadObjectResult, UnityValue, ObjectKey, UnityEnvironment, object_name,
    normalize_bundle_name
};
use containers::write_object_metadata;
use input::read_value;
pub use input::{collect_archive_dependencies, read_packed_bits, read_packed_floats};
use output::write_value;
#[cfg(test)]
use assets::stored_path_id_for_asset;
