use std::{fs, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(super) fn run(command_args: &[String]) -> Result<(), String> {
    let args = command_args.iter().cloned().collect::<Vec<_>>();
    let [report_path, base_path] = args.as_slice() else {
        return Err(
            "usage: publish-exact-texture-mips <FusionForge-exact-report.json> \
             <existing-base.png>"
                .to_string(),
        );
    };
    let report_path = Path::new(report_path);
    let base_path = Path::new(base_path);
    if base_path.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err(format!(
            "base texture {} is not a lowercase .png path",
            base_path.display()
        ));
    }
    let base_bytes = fs::read(base_path)
        .map_err(|error| format!("cannot read base texture {}: {error}", base_path.display()))?;
    let report_bytes = fs::read(report_path).map_err(|error| {
        format!(
            "cannot read exact report {}: {error}",
            report_path.display()
        )
    })?;
    let report: Value = serde_json::from_slice(&report_bytes)
        .map_err(|error| format!("invalid exact report {}: {error}", report_path.display()))?;
    let levels = report
        .get("mipLevels")
        .and_then(Value::as_array)
        .filter(|levels| levels.len() >= 2)
        .ok_or_else(|| "exact report must contain at least two mipLevels".to_string())?;

    let mut decoded_levels = Vec::with_capacity(levels.len());
    for (index, level) in levels.iter().enumerate() {
        let actual_level = level
            .get("level")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("mipLevels[{index}].level is missing"))?;
        if actual_level != index as u64 {
            return Err(format!(
                "mipLevels[{index}] has non-contiguous level {actual_level}"
            ));
        }
        let png = level
            .get("payload")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("mipLevels[{index}].payload is missing"))?;
        let data_url = png
            .get("dataUrl")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("mipLevels[{index}].payload.dataUrl is missing"))?;
        let encoded = data_url
            .strip_prefix("data:image/png;base64,")
            .ok_or_else(|| format!("mipLevels[{index}] is not a PNG data URL"))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| format!("mipLevels[{index}] has invalid base64: {error}"))?;
        let expected_length = png
            .get("byteLength")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("mipLevels[{index}].payload.byteLength is missing"))?;
        if u64::try_from(bytes.len()).ok() != Some(expected_length) {
            return Err(format!(
                "mipLevels[{index}] PNG length mismatch: got {}, expected {expected_length}",
                bytes.len()
            ));
        }
        let expected_sha256 = png
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("mipLevels[{index}].payload.sha256 is missing"))?;
        let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
        if actual_sha256 != expected_sha256 {
            return Err(format!(
                "mipLevels[{index}] PNG SHA-256 mismatch: got {actual_sha256}, expected {expected_sha256}"
            ));
        }
        decoded_levels.push(bytes);
    }

    if decoded_levels[0] != base_bytes {
        return Err(format!(
            "existing base texture {} is not byte-identical to exact mip level zero",
            base_path.display()
        ));
    }

    let stem = base_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("base texture {} has no UTF-8 stem", base_path.display()))?;
    let mip_dir = base_path.with_file_name(format!("{stem}.mips"));
    if mip_dir.exists() {
        return Err(format!(
            "refusing to overwrite existing exact mip directory {}",
            mip_dir.display()
        ));
    }
    fs::create_dir(&mip_dir).map_err(|error| {
        format!(
            "cannot create exact mip directory {}: {error}",
            mip_dir.display()
        )
    })?;
    for (index, bytes) in decoded_levels.iter().enumerate().skip(1) {
        let output = mip_dir.join(format!("mip-{index:02}.png"));
        fs::write(&output, bytes)
            .map_err(|error| format!("cannot write exact mip {}: {error}", output.display()))?;
    }
    println!(
        "Published {} lower exact mip levels beside {} from {}",
        decoded_levels.len() - 1,
        base_path.display(),
        report_path.display()
    );
    Ok(())
}
