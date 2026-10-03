use super::*;

pub(crate) const MANAGED_TRANSLATION_GENERATOR: &str = "fusionforge.rust-managed-ldstr.v3";

#[derive(Default)]
pub(super) struct TranslationMerge {
    pub(super) by_id: HashMap<String, String>,
    pub(super) by_file_and_source: HashMap<String, String>,
    pub(super) by_source: HashMap<String, Option<String>>,
}

pub(crate) fn export_translation_index(
    root: &Path,
    output_path: &Path,
    options: &ManagedExportOptions,
) -> Result<usize, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }

    let assemblies = resolve_assemblies(root, options)?;
    if assemblies.is_empty() {
        return Err("No managed game assemblies were found to export.".to_string());
    }

    let merge = TranslationMerge::load(&options.merge_paths);
    let mut entries = Vec::new();
    for assembly in &assemblies {
        entries.extend(export_assembly(root, assembly, options, &merge)?);
    }
    entries.sort_by(|left, right| managed_entry_sort_key(left).cmp(&managed_entry_sort_key(right)));

    let document = json!({
        "format": "fftools.translation.v1",
        "generatedBy": MANAGED_TRANSLATION_GENERATOR,
        "root": root.to_string_lossy(),
        "assemblies": assemblies
            .iter()
            .filter_map(|path| path.file_name().and_then(|value| value.to_str()))
            .collect::<Vec<_>>(),
        "entries": entries,
    });
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&document).map_err(|err| err.to_string())?;
    fs::write(output_path, format!("{text}\n"))
        .map_err(|err| format!("{}: {err}", output_path.display()))?;
    Ok(document
        .get("entries")
        .and_then(JsonValue::as_array)
        .map(Vec::len)
        .unwrap_or_default())
}

impl TranslationMerge {
    pub(super) fn load(paths: &[PathBuf]) -> Self {
        let mut merge = Self::default();
        for path in paths.iter().filter(|path| path.exists()) {
            merge.load_path(path);
        }
        merge
    }

    pub(super) fn find(&self, entry: &JsonValue) -> Option<String> {
        let id = entry.get("id").and_then(JsonValue::as_str).unwrap_or("");
        if let Some(translation) = self.by_id.get(id) {
            return Some(translation.clone());
        }
        let file = entry.get("file").and_then(JsonValue::as_str).unwrap_or("");
        let source = entry
            .get("source")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        if let Some(translation) = self.by_file_and_source.get(&file_source_key(file, source)) {
            return Some(translation.clone());
        }
        self.by_source.get(source).and_then(Clone::clone)
    }

    pub(super) fn load_path(&mut self, path: &Path) {
        let Ok(value) = read_json(path) else {
            return;
        };
        if let Some(entries) = value.get("entries").and_then(JsonValue::as_array) {
            for entry in entries {
                self.add_entry(entry);
            }
            return;
        }
        if let Some(files) = value.get("files").and_then(JsonValue::as_array) {
            for file in files {
                let path = file.get("path").and_then(JsonValue::as_str).unwrap_or("");
                for replacement in file
                    .get("replacements")
                    .and_then(JsonValue::as_array)
                    .into_iter()
                    .flatten()
                {
                    let source = replacement
                        .get("old")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("");
                    let translation = replacement
                        .get("new")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("");
                    self.add(None, path, source, translation);
                }
            }
        }
    }

    pub(super) fn add_entry(&mut self, entry: &JsonValue) {
        let id = entry.get("id").and_then(JsonValue::as_str);
        let file = entry.get("file").and_then(JsonValue::as_str).unwrap_or("");
        let source = entry
            .get("source")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        let translation = entry
            .get("translation")
            .or_else(|| entry.get("Translation"))
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        self.add(id, file, source, translation);
    }

    pub(super) fn add(&mut self, id: Option<&str>, file: &str, source: &str, translation: &str) {
        if translation.is_empty() {
            return;
        }
        let translation =
            repair_cp1251_mojibake(translation).unwrap_or_else(|| translation.to_string());
        if let Some(id) = id.filter(|value| !value.is_empty()) {
            self.by_id.insert(id.to_string(), translation.clone());
        }
        self.by_file_and_source
            .insert(file_source_key(file, source), translation.clone());
        match self.by_source.entry(source.to_string()) {
            Entry::Occupied(mut current) => {
                if current.get().as_deref() != Some(translation.as_str()) {
                    current.insert(None);
                }
            }
            Entry::Vacant(current) => {
                current.insert(Some(translation));
            }
        }
    }
}
