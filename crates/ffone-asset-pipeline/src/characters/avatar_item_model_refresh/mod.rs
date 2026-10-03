//! Refresh the generated avatar-item lookup from the installed player-item catalog.
//!
//! `data/character_creation/avatar_items.json` records, per item and gender, the
//! model an equipped XDT row resolves to. `ffone-client`'s `player_shared_rig`,
//! `hnpc_runtime` and `character_creation_data` all read it, so a newly installed
//! model is invisible to the runtime until this document names it.
//!
//! `install_character_creation_data` regenerates the whole document, but it
//! indexes the tree through the global `asset-manifest.json` that the asset
//! migration removed. This refresher is deliberately narrower: it only fills in
//! entries whose `modelStatus` is `missing` while the installed
//! `characters/player/items/catalog.json` — the authority for player item models
//! — actually publishes their `sourceModelTrueName`. It never edits a resolved
//! entry, never invents a texture, and never touches a payload byte.
//!
//! The legacy route is `wear/<trueName>.nif` by construction: the equipment
//! source exporter resolves every model from exactly that container route, so
//! the XDT mesh string is the NIF file stem.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::{
    PLAYER_ITEM_SET_CATALOG_SCHEMA, PipelineError, PlayerItemSetCatalog, Result, error::io_at,
};

static TRANSACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub const AVATAR_ITEM_MODEL_REFRESH_SCHEMA: &str = "ffone.avatar-item-model-refresh.v1";
const AVATAR_ITEMS_PATH: &str = "data/character_creation/avatar_items.json";
const PLAYER_ITEM_CATALOG_PATH: &str = "characters/player/items/catalog.json";
const AVATAR_ITEMS_SCHEMA: &str = "ffone.character-creation.avatar-items.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvatarItemModelRefreshOptions {
    pub asset_root: PathBuf,
    pub apply: bool,
}

impl AvatarItemModelRefreshOptions {
    pub fn new(asset_root: impl Into<PathBuf>, apply: bool) -> Self {
        Self {
            asset_root: asset_root.into(),
            apply,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvatarItemModelChange {
    pub category: String,
    pub item_number: i64,
    pub item_name: String,
    pub gender: String,
    pub true_name: String,
    pub exact_route: String,
    pub native_asset_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvatarItemModelRefreshReport {
    pub schema: String,
    pub document: String,
    pub applied: bool,
    pub catalog_models: u64,
    /// Gender slots whose `modelStatus` was `missing` before the refresh.
    pub missing_slots_before: u64,
    /// Missing slots whose XDT row names no model at all; correct behaviour that
    /// this refresher must never change.
    pub slots_without_a_source_model: u64,
    pub changes: Vec<AvatarItemModelChange>,
    pub model_references_before: u64,
    pub model_references_after: u64,
    pub resolved_models_before: u64,
    pub resolved_models_after: u64,
    pub document_blake3_before: String,
    pub document_blake3_after: String,
}

pub fn refresh_avatar_item_models(
    options: &AvatarItemModelRefreshOptions,
) -> Result<AvatarItemModelRefreshReport> {
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;

    let catalog_path = asset_root.join(native_path(PLAYER_ITEM_CATALOG_PATH));
    let catalog: PlayerItemSetCatalog = serde_json::from_slice(&read_file(&catalog_path)?)
        .map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if catalog.schema != PLAYER_ITEM_SET_CATALOG_SCHEMA {
        return invalid("player item-set catalog has the wrong schema");
    }
    let by_true_name = catalog
        .models
        .iter()
        .map(|model| (model.true_name.to_ascii_lowercase(), model))
        .collect::<BTreeMap<_, _>>();

    let document_path = asset_root.join(native_path(AVATAR_ITEMS_PATH));
    let before = read_file(&document_path)?;
    let mut document: JsonValue =
        serde_json::from_slice(&before).map_err(|source| PipelineError::Json {
            path: document_path.display().to_string(),
            source,
        })?;
    if document.get("schema").and_then(JsonValue::as_str) != Some(AVATAR_ITEMS_SCHEMA) {
        return invalid(format!(
            "avatar item document must use {AVATAR_ITEMS_SCHEMA:?}"
        ));
    }
    let references_before = counts_field(&document, "modelReferences")?;
    let resolved_before = counts_field(&document, "resolvedModels")?;

    let mut changes = Vec::new();
    let mut missing_before = 0u64;
    let mut without_source = 0u64;
    let items = document
        .get_mut("items")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("avatar item document has no items array"))?;
    for item in items.iter_mut() {
        let category = required_string(item, "category")?;
        let item_number = item
            .get("itemNumber")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| invalid_error("avatar item has no itemNumber"))?;
        let item_name = item
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_owned();
        for gender in ["male", "female"] {
            let Some(slot) = item.get_mut(gender).and_then(JsonValue::as_object_mut) else {
                continue;
            };
            if slot.get("modelStatus").and_then(JsonValue::as_str) != Some("missing") {
                continue;
            }
            missing_before += 1;
            let Some(true_name) = slot
                .get("sourceModelTrueName")
                .and_then(JsonValue::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
            else {
                // The XDT row names no model for this gender.
                without_source += 1;
                continue;
            };
            let Some(model) = by_true_name.get(&true_name.to_ascii_lowercase()) else {
                continue;
            };
            if !model.category.eq_ignore_ascii_case(&category) {
                return invalid(format!(
                    "catalog model {true_name:?} is category {:?}, but the avatar item is {category:?}",
                    model.category
                ));
            }
            let exact_route = format!("wear/{true_name}.nif");
            let mut native_asset = JsonMap::new();
            native_asset.insert("path".to_owned(), JsonValue::from(model.model.path.clone()));
            native_asset.insert("bytes".to_owned(), JsonValue::from(model.model.bytes));
            native_asset.insert(
                "blake3".to_owned(),
                JsonValue::from(model.model.blake3.clone()),
            );
            let mut entry = JsonMap::new();
            entry.insert("trueName".to_owned(), JsonValue::from(true_name.clone()));
            entry.insert(
                "exactRoute".to_owned(),
                JsonValue::from(exact_route.clone()),
            );
            entry.insert("nativeAsset".to_owned(), JsonValue::Object(native_asset));

            slot.insert("modelStatus".to_owned(), JsonValue::from("verified_unique"));
            slot.insert(
                "models".to_owned(),
                JsonValue::Array(vec![JsonValue::Object(entry)]),
            );
            changes.push(AvatarItemModelChange {
                category: category.clone(),
                item_number,
                item_name: item_name.clone(),
                gender: gender.to_owned(),
                true_name,
                exact_route,
                native_asset_path: model.model.path.clone(),
            });
        }
    }

    let added = changes.len() as u64;
    set_counts_field(&mut document, "modelReferences", references_before + added)?;
    set_counts_field(&mut document, "resolvedModels", resolved_before + added)?;
    if added > 0 {
        // The document only claims completeness when nothing is left missing.
        let still_missing = missing_before - added;
        document
            .as_object_mut()
            .ok_or_else(|| invalid_error("avatar item document is not an object"))?
            .insert(
                "lookupComplete".to_owned(),
                JsonValue::Bool(still_missing == 0),
            );
    }
    let after = pretty_json(&document)?;

    let mut report = AvatarItemModelRefreshReport {
        schema: AVATAR_ITEM_MODEL_REFRESH_SCHEMA.to_owned(),
        document: AVATAR_ITEMS_PATH.to_owned(),
        applied: false,
        catalog_models: catalog.models.len() as u64,
        missing_slots_before: missing_before,
        slots_without_a_source_model: without_source,
        changes,
        model_references_before: references_before,
        model_references_after: references_before + added,
        resolved_models_before: resolved_before,
        resolved_models_after: resolved_before + added,
        document_blake3_before: blake3::hash(&before).to_hex().to_string(),
        document_blake3_after: blake3::hash(&after).to_hex().to_string(),
    };
    if report.changes.is_empty() || !options.apply {
        return Ok(report);
    }
    commit(&document_path, &before, &after)?;
    report.applied = true;
    Ok(report)
}

fn counts_field(document: &JsonValue, field: &str) -> Result<u64> {
    document
        .get("counts")
        .and_then(|counts| counts.get(field))
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| invalid_error(format!("avatar item document has no counts.{field}")))
}

fn set_counts_field(document: &mut JsonValue, field: &str, value: u64) -> Result<()> {
    let counts = document
        .get_mut("counts")
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| invalid_error("avatar item document has no counts object"))?;
    counts.insert(field.to_owned(), JsonValue::from(value));
    Ok(())
}

fn required_string(value: &JsonValue, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid_error(format!("avatar item has no {field}")))
}

fn commit(path: &Path, before: &[u8], after: &[u8]) -> Result<()> {
    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let next = sibling(path, &token, "next")?;
    let backup = sibling(path, &token, "backup")?;
    write_new(&next, after)?;
    if read_file(path)? != before {
        let _ = fs::remove_file(&next);
        return invalid("avatar item document changed while the refresh was staged");
    }
    let result = (|| {
        fs::rename(path, &backup).map_err(|error| io_at(path, error))?;
        if let Err(error) = fs::rename(&next, path) {
            let _ = fs::rename(&backup, path);
            return Err(io_at(&next, error));
        }
        Ok(())
    })();
    if result.is_ok() {
        let _ = fs::remove_file(&backup);
    }
    let _ = fs::remove_file(&next);
    result
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{} must be a plain regular file", path.display()));
    }
    fs::read(path).map_err(|error| io_at(path, error))
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a plain directory"));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

fn pretty_json(value: &JsonValue) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: AVATAR_ITEMS_PATH.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    output
        .write_all(bytes)
        .map_err(|error| io_at(path, error))?;
    output.sync_all().map_err(|error| io_at(path, error))
}

fn sibling(path: &Path, token: &str, suffix: &str) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("transaction path has no parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error("transaction filename is not UTF-8"))?;
    Ok(parent.join(format!(".{name}.avatar-item-refresh-{token}.{suffix}")))
}

fn native_path(relative: &str) -> PathBuf {
    relative.split('/').collect()
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(format!(
        "avatar item model refresh failed: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests;
