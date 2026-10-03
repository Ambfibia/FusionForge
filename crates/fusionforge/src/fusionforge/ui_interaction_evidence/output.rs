use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldWriteEvidence {
    pub site_id: String,
    pub il_offset: u32,
    pub basic_block: u32,
    pub field: ManagedFieldIdentity,
    pub certainty: EvidenceLevel,
    pub assigned_value: EvidenceValue,
    pub runtime_execution_proven: bool,
}

pub(crate) fn export_ui_interaction_evidence_cli(args: &[String]) -> Result<(), String> {
    if args.len() != 3 || args[1] != "--out" || args[0].is_empty() || args[2].is_empty() {
        return Err(CLI_USAGE.to_string());
    }
    let request_path =
        fs::canonicalize(&args[0]).map_err(|error| format!("{}: {error}", args[0]))?;
    if !request_path.is_file() {
        return Err(format!("request is not a file: {}", request_path.display()));
    }
    let request_root = request_path
        .parent()
        .ok_or_else(|| "request path has no parent directory".to_string())?;
    let request_bytes =
        fs::read(&request_path).map_err(|error| format!("{}: {error}", request_path.display()))?;
    let mut request: UiInteractionAnalysisRequest = serde_json::from_slice(&request_bytes)
        .map_err(|error| format!("{}: {error}", request_path.display()))?;
    request.validate()?;

    request.managed.evidence_report = resolve_portable_input(
        request_root,
        &request.managed.evidence_report,
        "managed.evidenceReport",
    )?;
    if let Some(payload) = request.managed.payload.as_ref() {
        request.managed.payload = Some(resolve_portable_input(
            request_root,
            payload,
            "managed.payload",
        )?);
    }

    let output = if args[2] == "-" {
        None
    } else {
        Some(PathBuf::from(&args[2]))
    };
    preflight_output(
        output.as_deref(),
        &request_path,
        &request.managed.evidence_report,
        request.managed.payload.as_deref(),
    )?;

    let analysis = analyze_ui_interactions_with_request_bytes(&request, &request_bytes)?;
    let mut json = serde_json::to_vec_pretty(&analysis).map_err(|error| error.to_string())?;
    json.push(b'\n');
    match output.as_deref() {
        None => io::stdout()
            .write_all(&json)
            .map_err(|error| error.to_string()),
        Some(path) => {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            preflight_output(
                Some(path),
                &request_path,
                &request.managed.evidence_report,
                request.managed.payload.as_deref(),
            )?;
            fs::write(path, json).map_err(|error| format!("{}: {error}", path.display()))
        }
    }
}
