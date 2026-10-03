use std::{fs, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(super) fn run(command_args: &[String]) -> Result<(), String> {
    let args = command_args.iter().cloned().collect::<Vec<_>>();
    let [report_path, output_path] = args.as_slice() else {
        return Err(
            "usage: publish-exact-texture <FusionForge-exact-report.json> <output.png>"
                .to_string(),
        );
    };
    let report_path = Path::new(report_path);
    let output_path = Path::new(output_path);
    if output_path.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err(format!(
            "output texture {} is not a lowercase .png path",
            output_path.display()
        ));
    }
    if output_path.exists() {
        return Err(format!(
            "refusing to overwrite existing exact texture {}",
            output_path.display()
        ));
    }

    let report_bytes = fs::read(report_path).map_err(|error| {
        format!(
            "cannot read exact report {}: {error}",
            report_path.display()
        )
    })?;
    let report: Value = serde_json::from_slice(&report_bytes)
        .map_err(|error| format!("invalid exact report {}: {error}", report_path.display()))?;
    let payload = report
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| "exact report payload is missing".to_string())?;
    let data_url = payload
        .get("dataUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| "exact report payload.dataUrl is missing".to_string())?;
    let encoded = data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(|| "exact report payload is not a PNG data URL".to_string())?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| format!("exact report payload has invalid base64: {error}"))?;
    let expected_length = payload
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| "exact report payload.byteLength is missing".to_string())?;
    if u64::try_from(bytes.len()).ok() != Some(expected_length) {
        return Err(format!(
            "PNG length mismatch: got {}, expected {expected_length}",
            bytes.len()
        ));
    }
    let expected_sha256 = payload
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "exact report payload.sha256 is missing".to_string())?;
    let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
    if actual_sha256 != expected_sha256 {
        return Err(format!(
            "PNG SHA-256 mismatch: got {actual_sha256}, expected {expected_sha256}"
        ));
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create exact texture directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(output_path, bytes).map_err(|error| {
        format!(
            "cannot publish exact texture {}: {error}",
            output_path.display()
        )
    })?;
    println!(
        "Published {} bytes to {} from {} (SHA-256 {})",
        expected_length,
        output_path.display(),
        report_path.display(),
        expected_sha256
    );
    Ok(())
}
