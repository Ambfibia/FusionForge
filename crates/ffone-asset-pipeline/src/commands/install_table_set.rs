//! Publish a rebuilt XDT table set at its one stable runtime route.
//!
//! Content identity is reported from the published bytes and captured by the
//! release dependency graph; filenames never change when the table changes.

use std::{fs, path::PathBuf};

const TABLE_ROUTE: &str = "data/tables/table-set.json";

pub(super) fn run(command_args: &[String]) -> Result<String, String> {
    let mut args = command_args.iter().map(std::ffi::OsString::from);
    let source = PathBuf::from(
        args.next()
            .ok_or("usage: install-table-set <TABLE_SET_JSON> <ASSET_ROOT>")?,
    );
    let asset_root = PathBuf::from(
        args.next()
            .ok_or("usage: install-table-set <TABLE_SET_JSON> <ASSET_ROOT>")?,
    );
    if args.next().is_some() {
        return Err("usage: install-table-set <TABLE_SET_JSON> <ASSET_ROOT>".to_owned());
    }

    let bytes = fs::read(&source).map_err(|error| format!("{}: {error}", source.display()))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", source.display()))?;
    if value.get("schema").and_then(serde_json::Value::as_str) != Some("ffone.table-set.v1") {
        return Err(format!("{} is not an ffone.table-set.v1", source.display()));
    }
    // Re-serialize through the same pretty writer the rest of the tree uses so
    // the published bytes do not depend on how the source file was produced.
    let mut published = serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?;
    published.push(b'\n');
    let blake3 = blake3::hash(&published).to_hex().to_string();
    let target = asset_root.join(TABLE_ROUTE);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let next = target.with_extension(format!("next-{}.json", std::process::id()));
    fs::write(&next, &published).map_err(|error| format!("{}: {error}", next.display()))?;
    if target.exists() {
        let backup = target.with_extension(format!("backup-{}.json", std::process::id()));
        fs::rename(&target, &backup).map_err(|error| format!("{}: {error}", target.display()))?;
        if let Err(error) = fs::rename(&next, &target) {
            let _ = fs::rename(&backup, &target);
            let _ = fs::remove_file(&next);
            return Err(format!("{}: {error}", target.display()));
        }
        fs::remove_file(&backup).map_err(|error| format!("{}: {error}", backup.display()))?;
    } else {
        fs::rename(&next, &target).map_err(|error| format!("{}: {error}", target.display()))?;
    }

    Ok(format!(
        "published {TABLE_ROUTE} ({} bytes, blake3 {blake3})",
        published.len()
    ))
}
