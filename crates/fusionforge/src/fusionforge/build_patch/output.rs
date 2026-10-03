use super::*;

pub fn export_managed_strings(args: &[String]) -> Result<(), String> {
    let usage = "export-managed-strings <extracted-unityweb-dir> <output-json> [--assembly <name>...] [--all-assemblies] [--ui-only] [--merge <json>] [--min-length n] [--include-empty] [--include-control-chars]";
    let root = required_path(args, 0, usage)?;
    let output = required_path(args, 1, usage)?;
    let mut options = managed_export_options(&args[2..])?;
    if options.ui_only {
        options.min_length = options.min_length.max(2);
    }
    let count = managed::export_translation_index(&root, &output, &options)?;
    println!(
        "Exported {count} managed ldstr entries to {}",
        output.display()
    );
    Ok(())
}

pub(super) fn write_status(path: Option<&Path>, value: JsonValue) -> Result<(), String> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(&value).map_err(|err| err.to_string())?;
        fs::write(path, format!("{text}\n")).map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(())
}

pub(super) fn managed_export_options(args: &[String]) -> Result<managed::ManagedExportOptions, String> {
    let mut options = managed::ManagedExportOptions::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--assembly" | "--assemblies" => {
                index += 1;
                while index < args.len() && !args[index].starts_with("--") {
                    options.assemblies.push(args[index].clone());
                    index += 1;
                }
                continue;
            }
            "--all-assemblies" => options.all_assemblies = true,
            "--ui-only" => options.ui_only = true,
            "--merge" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--merge requires a value".to_string());
                };
                options.merge_paths.push(PathBuf::from(path));
            }
            "--min-length" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--min-length requires a value".to_string());
                };
                options.min_length = value
                    .parse::<usize>()
                    .map_err(|err| format!("invalid --min-length value {value:?}: {err}"))?;
            }
            "--include-empty" => options.include_empty = true,
            "--include-control-chars" => options.include_control_chars = true,
            other => return Err(format!("Unknown export option: {other}")),
        }
        index += 1;
    }
    Ok(options)
}
