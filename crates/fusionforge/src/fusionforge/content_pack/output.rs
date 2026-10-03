use super::*;

pub(crate) fn export_native_pack_cli(args: &[String]) -> Result<(), String> {
    if !(2..=3).contains(&args.len()) {
        return Err("export-native-pack <source-manifest.json|source-build-dir> <fresh-output-dir> [locale]".into());
    }
    let output = cook_pack(
        None,
        Path::new(&args[0]),
        Path::new(&args[1]),
        args.get(2).map(String::as_str).unwrap_or(DEFAULT_LOCALE),
    )?;
    let report_path = cook_report_path(&output)?;
    let report = read_json_value(&report_path)?;
    let complete = report.get("complete").and_then(JsonValue::as_bool) == Some(true);
    println!(
        "{}",
        json!({"outputDir": output, "report": report_path, "complete": complete})
    );
    if !complete {
        return Err(format!(
            "native pack is incomplete; review {} before publication",
            report_path.display()
        ));
    }
    Ok(())
}
