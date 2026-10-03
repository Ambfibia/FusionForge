use super::*;

pub(super) fn run_native_model_encode(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 3 {
        return Err(format!(
            "encode-native-model requires <MODEL_JSON> <OUT_GLB> [REPORT_JSON]\n\n{USAGE}"
        ));
    }
    let report = encode_native_model(&NativeModelEncodeOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
    ))
    .map_err(|error| error.to_string())?;
    if let Some(path) = args.get(3) {
        let report_path = PathBuf::from(path);
        let serialized = serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("failed to serialize encode report: {error}"))?;
        if let Some(parent) = report_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        std::fs::write(&report_path, serialized)
            .map_err(|error| format!("could not write {}: {error}", report_path.display()))?;
    }
    Ok(format!(
        "encoded native model {}: {} bytes, nodes={}, meshes={}, materials={}, textures={}, animations={:?}",
        report.logical_name,
        report.glb_bytes,
        report.nodes,
        report.meshes,
        report.materials,
        report.textures,
        report.animation_names
    ))
}
