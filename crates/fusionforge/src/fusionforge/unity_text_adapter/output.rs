use super::*;

pub(super) fn write_report(output: &str, input_path: &Path, report: &AdapterReport) -> Result<(), String> {
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(report).map_err(|error| error.to_string())?
    );
    if output == "-" {
        print!("{json}");
        return Ok(());
    }
    let output_path = Path::new(output);
    if same_path(output_path, input_path)? {
        return Err("report output must not overwrite the input JSON".to_string());
    }
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    fs::write(output_path, json)
        .map_err(|error| format!("could not write {}: {error}", output_path.display()))
}
