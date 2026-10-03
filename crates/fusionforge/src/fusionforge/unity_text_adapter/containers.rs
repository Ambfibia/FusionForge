use super::*;

pub const UNITY_TEXT_ADAPTER_INPUT_SCHEMA: &str = "fusionforge.unity-text-adapter-input.v2";

pub const UNITY_TEXT_ADAPTER_OUTPUT_SCHEMA: &str = "fusionforge.unity-text-replacement-adapter.v2";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct LegacyFontObjectEvidence {
    pub(super) semantic_id: String,
    pub(super) serialized_asset: String,
    pub(super) unity_type: String,
    pub(super) path_id: i64,
    pub(super) raw_object_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UnityTextStyle {
    pub(super) semantic_id: String,
    pub(super) rect: RectPx,
    pub(super) padding: InsetsPx,
    pub(super) content_offset: PointPx,
    pub(super) alignment: TextAlignment,
    pub(super) word_wrap: bool,
    pub(super) clipping: TextClipping,
}

/// Convert measured legacy Unity GUIStyle text geometry into a deterministic
/// replacement-font placement contract.
///
/// Command contract:
/// `adapt-unity-text <input.json> [--out <report.json|->] [--allow-unsatisfied]`.
/// The JSON report embeds only the input hash and portable semantic identifiers;
/// CLI filesystem paths are never copied into it.
pub fn adapt_unity_text_cli(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        println!("adapt-unity-text <input.json> [--out <report.json|->] [--allow-unsatisfied]");
        return Ok(());
    }

    let mut input = None;
    let mut output = "-".to_string();
    let mut output_seen = false;
    let mut allow_unsatisfied = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => {
                if output_seen {
                    return Err("--out may be supplied only once".to_string());
                }
                index += 1;
                output = args
                    .get(index)
                    .ok_or_else(|| "--out requires a value".to_string())?
                    .clone();
                if output.is_empty() {
                    return Err("--out requires a non-empty value".to_string());
                }
                output_seen = true;
            }
            "--allow-unsatisfied" => {
                if allow_unsatisfied {
                    return Err("--allow-unsatisfied may be supplied only once".to_string());
                }
                allow_unsatisfied = true;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown adapt-unity-text option {value}"));
            }
            value => {
                if input.replace(value.to_string()).is_some() {
                    return Err("adapt-unity-text expects exactly one input JSON".to_string());
                }
            }
        }
        index += 1;
    }

    let input = input.ok_or_else(|| "adapt-unity-text expects one input JSON".to_string())?;
    let input_path = Path::new(&input);
    let bytes = fs::read(input_path)
        .map_err(|error| format!("could not read {}: {error}", input_path.display()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{} is not UTF-8: {error}", input_path.display()))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let parsed: AdapterInput = serde_json::from_str(text)
        .map_err(|error| format!("could not parse {}: {error}", input_path.display()))?;
    let input_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let report = build_report(parsed, input_sha256, allow_unsatisfied)?;
    if report.acceptance.status != "passed" && !allow_unsatisfied {
        return Err(format!(
            "text adapter acceptance failed: {}; rerun with --allow-unsatisfied only for explicit triage",
            report.acceptance.failures.join("; ")
        ));
    }
    write_report(&output, input_path, &report)?;
    if output != "-" {
        println!(
            "Unity text adapter: style={} samples={} acceptance={} output={}",
            report.style.semantic_id,
            report.samples.len(),
            report.acceptance.status,
            output.replace('\\', "/")
        );
    }
    Ok(())
}
