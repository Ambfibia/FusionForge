use super::super::*;

pub(in super::super) fn indexed_runtime_file(file: &ClientExtractedFile, cache_dir: &Path) -> ClientBundleFile {
    let path = PathBuf::from(&file.path);
    let extension = if file.name.eq_ignore_ascii_case("mainData") {
        "mainData".to_string()
    } else {
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string()
    };
    ClientBundleFile {
        path: file.path.clone(),
        name: file.name.clone(),
        extension,
        size: file.size,
        modified_ms: None,
        cache_dir: Some(cache_dir.to_string_lossy().to_string()),
        extracted_files: Vec::new(),
        assets: Vec::new(),
        errors: Vec::new(),
    }
}

pub(in super::super) fn status_patched_count(path: &Path, key: &str) -> usize {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.get(key).and_then(serde_json::Value::as_u64))
        .unwrap_or_default() as usize
}
