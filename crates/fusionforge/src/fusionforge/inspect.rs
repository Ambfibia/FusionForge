use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;

mod font;

/// A bounded final reference view; never an extraction cache.
pub(super) fn run(args: &[String]) -> Result<(), String> {
    let path = args.first().ok_or("inspect <container> [--asset NAME] [--type TYPE] [--name TEXT] [--path-id ID] [--font-text TEXT] [--limit N] [--format json|markdown]")?;
    let mut assembly = None;
    let mut class = None;
    let mut method = None;
    let mut asset_filter = None;
    let mut type_filter = None;
    let mut name_filter = None;
    let mut path_filter = None;
    let mut font_text = None;
    let mut limit = 30usize;
    let mut format = "json";
    if (args.len() - 1) % 2 != 0 {
        return Err("inspect options require values".into());
    }
    for pair in args[1..].chunks_exact(2) {
        match pair[0].as_str() {
            "--assembly" => assembly = Some(pair[1].as_str()),
            "--class" => class = Some(pair[1].as_str()),
            "--method" => method = Some(pair[1].as_str()),
            "--asset" => asset_filter = Some(pair[1].as_str()),
            "--type" => type_filter = Some(pair[1].as_str()),
            "--name" => name_filter = Some(pair[1].to_lowercase()),
            "--path-id" => path_filter = Some(pair[1].parse::<i64>().map_err(|e| e.to_string())?),
            "--font-text" => font_text = Some(pair[1].as_str()),
            "--limit" => limit = pair[1].parse::<usize>().map_err(|e| e.to_string())?,
            "--format" => format = &pair[1],
            other => return Err(format!("unknown inspect option {other}")),
        }
    }
    if !(1..=1000).contains(&limit) {
        return Err("limit must be 1..=1000".into());
    }
    if !matches!(format, "json" | "markdown") {
        return Err("format must be json or markdown".into());
    }
    if font_text.is_some()
        && (path_filter.is_none()
            || asset_filter.is_none()
            || assembly.is_some()
            || class.is_some()
            || method.is_some()
            || format != "json")
    {
        return Err(
            "--font-text requires exact --asset and --path-id with JSON object inspection".into(),
        );
    }
    if class.is_some() || method.is_some() || assembly.is_some() {
        let name = assembly.ok_or("managed inspection requires --assembly exact-name.dll")?;
        if asset_filter.is_some()
            || type_filter.is_some()
            || path_filter.is_some()
            || name_filter.is_some()
        {
            return Err("object filters cannot be combined with managed filters".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let (_, bundle) = ffbuildtool::bundle::AssetBundle::from_bytes(&bytes)?;
        let matches = bundle
            .iter_files()
            .filter(|(_, entry, _)| *entry == name)
            .collect::<Vec<_>>();
        let [(_, _, payload)] = matches.as_slice() else {
            return Err("missing or ambiguous assembly entry".into());
        };
        let mut result = super::managed::inspect_methods(payload, class, method, limit)?;
        result["sourceContainer"] = json!(path);
        result["sourceSha256"] = json!(format!("{:x}", Sha256::digest(&bytes)));
        result["assembly"] = json!(name);
        if format == "json" {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
            );
        } else {
            println!(
                "# Focused managed reference\n\nContainer: `{path}`; assembly: `{name}`\n\nCLR metadata and IL; not original source.\n"
            );
            for row in result["methods"].as_array().unwrap() {
                println!(
                    "- `{}` — {} IL bytes; native mapping unconfirmed",
                    row["method"].as_str().unwrap_or_default(),
                    row["ilBytes"]
                );
            }
        }
        return Ok(());
    }
    let assets = super::direct_input::read_assets(Path::new(path))?;
    let recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../recipes/native/ui/quit-menu-direct.json"
    ))
    .map_err(|e| e.to_string())?;
    let source_sha = format!(
        "{:x}",
        Sha256::digest(std::fs::read(path).map_err(|e| e.to_string())?)
    );
    let accepted = recipe["sourceSha256"].as_str() == Some(source_sha.as_str());
    let mut rows = Vec::new();
    let mut total = 0;
    for (index, asset) in assets.iter().enumerate() {
        if asset_filter.is_some_and(|name| asset.name != name) {
            continue;
        }
        for (id, info) in &asset.objects {
            if path_filter.is_some_and(|wanted| *id != wanted) {
                continue;
            }
            let ty = asset.object_type_name(info);
            if type_filter.is_some_and(|wanted| ty != wanted) {
                continue;
            }
            let body = asset.read_object(index, info)?;
            let name = super::unity::object_name(&body);
            if name_filter
                .as_ref()
                .is_some_and(|wanted| !name.to_lowercase().contains(wanted))
            {
                continue;
            }
            total += 1;
            if rows.len() < limit {
                let mapping = if accepted && asset.name == "sharedassets0.assets" {
                    if recipe["component"].as_i64() == Some(*id) && ty == "MonoBehaviour" {
                        Some("ui/en/gameplay/quit-menu/menu.ffquit.json".to_owned())
                    } else if ty == "Texture2D" {
                        recipe["texturePaths"][id.to_string()]
                            .as_str()
                            .map(str::to_owned)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let mut dependencies = Vec::new();
                let mut count = 0;
                pointer_rows(&body, "", asset, &mut dependencies, &mut count);
                let mut row = json!({"serializedAsset": asset.name, "type": ty, "pathId": id, "name": name,
                    "externalFiles": asset.asset_refs.iter().skip(1).map(|r| &r.file_path).collect::<Vec<_>>(),
                    "dependencies":dependencies,"dependencyCount":count,"dependenciesTruncated":count>dependencies.len(),
                    "nativeMapping":mapping,"mappingStatus":if mapping.is_some(){"accepted-source-pinned-recipe"}else{"unconfirmed"}});
                if let Some(text) = font_text {
                    if ty != "Font" {
                        return Err("--font-text target must be a Font".into());
                    }
                    row["fontMetrics"] = font::inspect(&body, text, asset.object_raw_data(info)?)?;
                }
                rows.push(row);
            }
        }
    }
    let result = json!({"sourceContainer": path, "sourceSha256":source_sha, "matches":total,"truncated":total>limit,
        "ambiguousSelection":path_filter.is_some() && total > 1,"objects":rows});
    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
        );
    } else {
        println!(
            "# Focused Unity reference\n\nContainer: `{path}`\n\nMatches: {total}; returned: {}\n",
            rows.len()
        );
        for row in rows {
            println!(
                "- `{}` / `{}` / `{}` — {} (mapping: {})",
                row["serializedAsset"].as_str().unwrap_or_default(),
                row["type"].as_str().unwrap_or_default(),
                row["pathId"],
                row["name"].as_str().unwrap_or_default(),
                row["mappingStatus"].as_str().unwrap_or_default()
            );
        }
    }
    Ok(())
}

fn pointer_rows(
    value: &super::unity::UnityValue,
    field: &str,
    asset: &super::unity::Asset,
    rows: &mut Vec<serde_json::Value>,
    count: &mut usize,
) {
    use super::unity::UnityValue;
    match value {
        UnityValue::Pointer(p) if !p.is_null() => {
            *count += 1;
            if rows.len() < 100 {
                let owner = if p.file_id == 0 {
                    Some(asset.name.as_str())
                } else {
                    usize::try_from(p.file_id)
                        .ok()
                        .and_then(|i| asset.asset_refs.get(i))
                        .map(|r| r.file_path.as_str())
                };
                let status = if p.file_id == 0 && asset.objects.contains_key(&p.path_id) {
                    "resolved-local"
                } else if p.file_id > 0 && owner.is_some() {
                    "external-not-loaded"
                } else {
                    "unresolved"
                };
                rows.push(json!({"field":field,"fileId":p.file_id,"pathId":p.path_id,"targetAsset":owner,"status":status}));
            }
        }
        UnityValue::Object(fields) => {
            for (key, value) in fields {
                pointer_rows(value, &format!("{field}/{key}"), asset, rows, count);
            }
        }
        UnityValue::Array(values) => {
            for (i, value) in values.iter().enumerate() {
                pointer_rows(value, &format!("{field}/{i}"), asset, rows, count);
            }
        }
        UnityValue::Pair(a, b) => {
            pointer_rows(a, &format!("{field}/first"), asset, rows, count);
            pointer_rows(b, &format!("{field}/second"), asset, rows, count);
        }
        _ => {}
    }
}
