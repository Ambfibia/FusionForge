use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const OVERLAY_SCHEMA: &str = "ffone.retrobution-hostile-visual-overlay.v1";
const CATALOG_SCHEMA: &str = "ffone.xdt-npc-texture-catalog.v1";
const METADATA_SCHEMA: &str = "ffone.offline.chartexture-metadata.v1";
const REPORT_SCHEMA: &str = "ffone.retrobution-hostile-xdt-texture-publication.v1";
const NPC_TEXTURE_SOURCE: &str = "CustomAssetBundle-d304c52c4bae348e38c743762c1bd818";
const RUNTIME_ROOT: &str = "characters/shared/runtime-textures";

#[derive(Debug)]
struct TexturePlan {
    true_name: String,
    recovered: PathBuf,
    route: String,
    bytes: u64,
    blake3: String,
}

pub(super) fn run(command_args: &[String]) -> Result<(), String> {
    let args = command_args.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    let (paths, apply) = match args.as_slice() {
        [overlay, catalog, metadata, recovery, asset_root, report] => (
            [overlay, catalog, metadata, recovery, asset_root, report],
            false,
        ),
        [
            overlay,
            catalog,
            metadata,
            recovery,
            asset_root,
            report,
            flag,
        ] if flag == "--apply" => (
            [overlay, catalog, metadata, recovery, asset_root, report],
            true,
        ),
        _ => {
            return Err(
                "usage: publish-retrobution-hostile-xdt-textures <OVERLAY_REPORT> \
                 <TEXTURE_CATALOG> <NPC_TEXTURE_METADATA> <RECOVERY_DIR> <ASSET_ROOT> \
                 <REPORT> [--apply]"
                    .to_owned(),
            );
        }
    };
    let [
        overlay_arg,
        catalog_arg,
        metadata_arg,
        recovery_arg,
        asset_root_arg,
        report_arg,
    ] = paths;
    let overlay_path = PathBuf::from(overlay_arg);
    let catalog_path = PathBuf::from(catalog_arg);
    let metadata_path = PathBuf::from(metadata_arg);
    let recovery_root = PathBuf::from(recovery_arg);
    let asset_root = PathBuf::from(asset_root_arg);
    let report_path = PathBuf::from(report_arg);

    let overlay = read_json(&overlay_path, "hostile overlay report")?;
    let catalog = read_json(&catalog_path, "texture catalog audit")?;
    let metadata = read_json(&metadata_path, "NpcTexture metadata")?;
    require_schema(&overlay, OVERLAY_SCHEMA, "hostile overlay report")?;
    require_schema(&catalog, CATALOG_SCHEMA, "texture catalog audit")?;
    require_schema(&metadata, METADATA_SCHEMA, "NpcTexture metadata")?;
    if metadata.get("sourceAsset").and_then(Value::as_str) != Some(NPC_TEXTURE_SOURCE) {
        return Err(format!(
            "NpcTexture metadata must name exact source asset {NPC_TEXTURE_SOURCE}"
        ));
    }
    if overlay
        .pointer("/counts/changedFriendlyOrHnpcRows")
        .and_then(Value::as_u64)
        != Some(0)
    {
        return Err("hostile overlay did not prove zero friendly/HNPC row changes".to_owned());
    }
    require_directory(&recovery_root, "recovery directory")?;
    require_directory(&asset_root, "asset root")?;
    must_not_exist(&report_path, "publication report")?;

    let changed_names = collect_changed_texture_names(&overlay)?;
    let targets = collect_missing_changed_textures(&catalog, &changed_names)?;
    if targets.is_empty() {
        return Err("texture audit has no missing changed hostile textures".to_owned());
    }
    let metadata_by_name = collect_metadata_by_name(&metadata, &targets)?;
    let runtime_root = asset_root.join(RUNTIME_ROOT);
    require_directory(&runtime_root, "runtime texture root")?;

    let mut plans = Vec::new();
    for true_name in &targets {
        let source = metadata_by_name
            .get(true_name)
            .ok_or_else(|| format!("NpcTexture metadata has no exact {true_name:?} record"))?;
        let path_id = required_i64(source, "pathId")?;
        let bytes = required_u64(source, "nativePngBytes")?;
        let expected_blake3 = required_string(source, "nativePngBlake3")?.to_lowercase();
        if path_id <= 0 || !is_hex_digest(&expected_blake3, 64) {
            return Err(format!("invalid source identity for {true_name:?}"));
        }
        let recovered = recovery_root.join(format!("{path_id}--{expected_blake3}.png"));
        let recovered_bytes = read_regular_file(&recovered, "recovered PNG")?;
        if recovered_bytes.len() as u64 != bytes {
            return Err(format!(
                "{true_name:?} recovered byte length is {}, expected {bytes}",
                recovered_bytes.len()
            ));
        }
        let actual_blake3 = blake3::hash(&recovered_bytes).to_hex().to_string();
        if actual_blake3 != expected_blake3 {
            return Err(format!(
                "{true_name:?} recovered BLAKE3 is {actual_blake3}, expected {expected_blake3}"
            ));
        }
        let file_name = format!("{true_name}.png");
        let route = format!("{RUNTIME_ROOT}/{file_name}");
        let destination = runtime_root.join(&file_name);
        must_not_exist(&destination, "runtime hostile texture")?;
        plans.push(TexturePlan {
            true_name: true_name.clone(),
            recovered,
            route,
            bytes,
            blake3: expected_blake3,
        });
    }

    let report = json!({
        "schema": REPORT_SCHEMA,
        "status": if apply { "committed" } else { "planned" },
        "sourceAlias": "retrobution",
        "sourceRole": "primary",
        "policy": {
            "selection": "changed hostile overlay texture names blocked only by a missing runtime PNG",
            "friendlyOrHnpcRowsChanged": false,
            "sourceAsset": NPC_TEXTURE_SOURCE,
            "publicationRoot": RUNTIME_ROOT,
        },
        "inputs": [
            input_proof("hostile-overlay", &overlay_path)?,
            input_proof("texture-catalog-audit", &catalog_path)?,
            input_proof("npc-texture-metadata", &metadata_path)?,
        ],
        "counts": {
            "changedTextureNames": changed_names.len(),
            "missingRuntimeTextures": plans.len(),
        },
        "textures": plans.iter().map(|plan| json!({
            "trueName": plan.true_name,
            "recovered": plan.recovered.to_string_lossy().replace('\\', "/"),
            "path": plan.route,
            "bytes": plan.bytes,
            "blake3": plan.blake3,
        })).collect::<Vec<_>>(),
    });
    if !apply {
        println!(
            "plan passed: {} exact hostile XDT textures are ready",
            plans.len()
        );
        return Ok(());
    }

    publish(&asset_root, &runtime_root, &report_path, &plans, &report)?;
    println!(
        "published {} exact hostile XDT textures to {RUNTIME_ROOT}; report={}",
        plans.len(),
        report_path.display()
    );
    Ok(())
}

fn collect_changed_texture_names(overlay: &Value) -> Result<BTreeSet<String>, String> {
    let changes = overlay
        .get("changes")
        .and_then(Value::as_array)
        .ok_or_else(|| "hostile overlay changes are missing".to_owned())?;
    let mut names = BTreeSet::new();
    for change in changes {
        for field in ["mainTexture", "subTexture"] {
            let Some(name) = change.get(field).and_then(Value::as_str) else {
                continue;
            };
            if name.eq_ignore_ascii_case("null") || name.is_empty() {
                continue;
            }
            let key = name.to_lowercase();
            if !valid_true_name(&key) {
                return Err(format!("invalid changed texture name {name:?}"));
            }
            names.insert(key);
        }
    }
    Ok(names)
}

fn collect_missing_changed_textures(
    catalog: &Value,
    changed: &BTreeSet<String>,
) -> Result<BTreeSet<String>, String> {
    let blocked = catalog
        .get("blocked")
        .and_then(Value::as_array)
        .ok_or_else(|| "texture catalog blocked array is missing".to_owned())?;
    let mut targets = BTreeSet::new();
    for entry in blocked {
        let name = required_string(entry, "trueName")?.to_lowercase();
        if !changed.contains(&name) {
            continue;
        }
        if entry.get("status").and_then(Value::as_str) != Some("missing")
            || !entry
                .get("samplerStatus")
                .and_then(Value::as_str)
                .is_some_and(|status| status.starts_with("verified_"))
        {
            return Err(format!(
                "changed texture {name:?} is blocked for a reason other than a missing runtime PNG"
            ));
        }
        targets.insert(name);
    }
    Ok(targets)
}

fn collect_metadata_by_name<'a>(
    metadata: &'a Value,
    targets: &BTreeSet<String>,
) -> Result<BTreeMap<String, &'a Value>, String> {
    let textures = metadata
        .get("textures")
        .and_then(Value::as_array)
        .ok_or_else(|| "NpcTexture metadata textures are missing".to_owned())?;
    let mut result = BTreeMap::new();
    for texture in textures {
        let name = required_string(texture, "trueName")?.to_lowercase();
        if !targets.contains(&name) {
            continue;
        }
        if result.insert(name.clone(), texture).is_some() {
            return Err(format!("NpcTexture metadata duplicates {name:?}"));
        }
    }
    Ok(result)
}

fn publish(
    asset_root: &Path,
    runtime_root: &Path,
    report_path: &Path,
    plans: &[TexturePlan],
    report: &Value,
) -> Result<(), String> {
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let token = format!("{}-{}", std::process::id(), plans.len());
    let stage = asset_root
        .join("characters/shared")
        .join(format!(".hostile-xdt-textures-stage-{token}"));
    must_not_exist(&stage, "texture stage")?;
    fs::create_dir(&stage)
        .map_err(|error| format!("cannot create {}: {error}", stage.display()))?;
    let report_next = report_path.with_extension(format!("next-{token}.json"));
    must_not_exist(&report_next, "temporary report")?;

    let result = (|| -> Result<Vec<PathBuf>, String> {
        for plan in plans {
            let staged = stage.join(format!("{}.png", plan.true_name));
            fs::copy(&plan.recovered, &staged)
                .map_err(|error| format!("cannot stage {}: {error}", staged.display()))?;
        }
        let report_bytes = serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?;
        let mut report_bytes_with_newline = report_bytes;
        report_bytes_with_newline.push(b'\n');
        fs::write(&report_next, report_bytes_with_newline)
            .map_err(|error| format!("cannot write {}: {error}", report_next.display()))?;

        let mut committed = Vec::new();
        for plan in plans {
            let staged = stage.join(format!("{}.png", plan.true_name));
            let destination = asset_root.join(&plan.route);
            if let Err(error) = fs::rename(&staged, &destination) {
                for path in committed.iter().rev() {
                    let _ = fs::remove_file(path);
                }
                return Err(format!(
                    "cannot publish {} to {}: {error}",
                    staged.display(),
                    destination.display()
                ));
            }
            committed.push(destination);
        }
        fs::remove_dir(&stage)
            .map_err(|error| format!("cannot remove empty stage {}: {error}", stage.display()))?;
        if let Err(error) = fs::rename(&report_next, report_path) {
            for path in committed.iter().rev() {
                let _ = fs::remove_file(path);
            }
            return Err(format!(
                "cannot publish report {}: {error}",
                report_path.display()
            ));
        }
        let _ = runtime_root;
        Ok(committed)
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_file(&report_next);
    }
    result.map(|_| ())
}

fn input_proof(role: &str, path: &Path) -> Result<Value, String> {
    let bytes = read_regular_file(path, role)?;
    Ok(json!({
        "role": role,
        "path": path.to_string_lossy().replace('\\', "/"),
        "bytes": bytes.len(),
        "sha256": format!("{:x}", Sha256::digest(&bytes)),
    }))
}

fn read_json(path: &Path, label: &str) -> Result<Value, String> {
    let bytes = read_regular_file(path, label)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{label} {} is invalid JSON: {error}", path.display()))
}

fn read_regular_file(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {label} {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{label} is not a regular file: {}", path.display()));
    }
    fs::read(path).map_err(|error| format!("cannot read {label} {}: {error}", path.display()))
}

fn require_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {label} {}: {error}", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "{label} is not a regular directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn must_not_exist(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(format!("{label} already exists: {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot inspect {label} {}: {error}",
            path.display()
        )),
    }
}

fn require_schema(value: &Value, schema: &str, label: &str) -> Result<(), String> {
    if value.get("schema").and_then(Value::as_str) != Some(schema) {
        return Err(format!("{label} must use schema {schema}"));
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing non-empty string {field}"))
}

fn required_u64(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing u64 {field}"))
}

fn required_i64(value: &Value, field: &str) -> Result<i64, String> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing i64 {field}"))
}

fn valid_true_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_hex_digest(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests;
