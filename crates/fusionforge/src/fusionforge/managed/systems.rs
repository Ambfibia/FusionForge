use super::*;

#[derive(Debug, Clone)]
pub(crate) struct ManagedApplyOptions {
    pub allow_missing: bool,
    pub backup: bool,
}

pub(crate) fn apply_translations(
    root: &Path,
    patch_path: &Path,
    options: &ManagedApplyOptions,
) -> Result<usize, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }
    let spec = read_json(patch_path)?;
    let mut by_file = BTreeMap::<String, Vec<JsonValue>>::new();
    for entry in spec
        .get("entries")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        let kind = entry.get("kind").and_then(JsonValue::as_str).unwrap_or("");
        if kind != MANAGED_KIND && !kind.is_empty() {
            continue;
        }
        if !has_text(entry.get("translation")) {
            continue;
        }
        let Some(file) = entry.get("file").and_then(JsonValue::as_str) else {
            continue;
        };
        by_file
            .entry(file.replace('\\', "/"))
            .or_default()
            .push(entry.clone());
    }

    if by_file.is_empty() {
        println!("No filled managed translations to apply.");
        return Ok(0);
    }

    let mut total = 0usize;
    for (file, entries) in by_file {
        let assembly_path = root.join(file.replace('/', std::path::MAIN_SEPARATOR_STR));
        let patched = patch_assembly_by_entries(&assembly_path, &entries, options)?;
        if patched > 0 {
            println!(
                "{}: applied {patched} translated ldstr entries",
                assembly_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("assembly")
            );
        }
        total += patched;
    }

    println!("Applied {total} translated managed ldstr entries.");
    Ok(total)
}

pub(super) fn entry_apply_key(entry: &JsonValue) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        entry_file(entry),
        entry.get("type").and_then(JsonValue::as_str).unwrap_or(""),
        entry
            .get("method")
            .and_then(JsonValue::as_str)
            .unwrap_or(""),
        entry
            .get("ilOffset")
            .or_else(|| entry.get("il_offset"))
            .and_then(JsonValue::as_i64)
            .unwrap_or_default(),
        entry
            .get("sourceSha1")
            .or_else(|| entry.get("source_sha1"))
            .and_then(JsonValue::as_str)
            .unwrap_or_else(|| entry
                .get("source")
                .and_then(JsonValue::as_str)
                .unwrap_or(""))
    )
}
