use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{PlayerItemSetCatalog, ResourceSetArtifact};
use ffone_runtime_contracts::CharacterCreationAvatarItems;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(super) fn run(command_args: &[String]) -> Result<(), String> {
    let args = command_args.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    let [project_root] = args.as_slice() else {
        return Err("usage: repair_verified_avatar_model_routes <PROJECT_ROOT>".to_owned());
    };
    let project_root = canonical_directory(Path::new(project_root), "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let avatar_path = canonical_file(
        &asset_root.join("data/character_creation/avatar_items.json"),
        "avatar items",
    )?;
    let catalog_path = canonical_file(
        &asset_root.join("characters/player/items/catalog.json"),
        "player item catalog",
    )?;
    let rig_path = canonical_file(
        &asset_root.join("characters/player/shared/player_rig_contract.json"),
        "player rig contract",
    )?;
    let catalog_bytes = fs::read(&catalog_path)
        .map_err(|error| format!("cannot read {}: {error}", catalog_path.display()))?;
    let rig_bytes = fs::read(&rig_path)
        .map_err(|error| format!("cannot read {}: {error}", rig_path.display()))?;
    let catalog: PlayerItemSetCatalog = serde_json::from_slice(&catalog_bytes)
        .map_err(|error| format!("invalid catalog: {error}"))?;
    let rig: Value =
        serde_json::from_slice(&rig_bytes).map_err(|error| format!("invalid rig: {error}"))?;
    let mut avatar: Value = read_value(&avatar_path)?;

    let mut exact_routes = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for gender in rig["genders"].as_array().into_iter().flatten() {
        for part in gender["creatorParts"].as_array().into_iter().flatten() {
            let (Some(true_name), Some(glb), Some(exact_route)) = (
                part["trueName"].as_str(),
                part["glb"].as_str(),
                part["exactRoute"].as_str(),
            ) else {
                continue;
            };
            exact_routes
                .entry((true_name.to_ascii_lowercase(), glb.to_owned()))
                .or_default()
                .insert(exact_route.to_owned());
        }
    }

    let mut repairs = Vec::new();
    for item in avatar["items"]
        .as_array_mut()
        .ok_or_else(|| "avatar items array is absent".to_owned())?
    {
        let category = item["category"]
            .as_str()
            .ok_or_else(|| "avatar item category is absent".to_owned())?
            .to_owned();
        let item_number = item["itemNumber"].as_u64().unwrap_or(0);
        for gender in ["male", "female"] {
            let visual = item
                .get_mut(gender)
                .ok_or_else(|| format!("avatar item {item_number} has no {gender} visual"))?;
            if visual["modelStatus"] != "missing" {
                continue;
            }
            let Some(source_true_name) = visual["sourceModelTrueName"].as_str().map(str::to_owned)
            else {
                continue;
            };
            let matches = catalog
                .models
                .iter()
                .filter(|model| {
                    model.category == category
                        && model.true_name.eq_ignore_ascii_case(&source_true_name)
                })
                .collect::<Vec<_>>();
            let [model] = matches.as_slice() else {
                continue;
            };
            verify_artifact(&asset_root, &model.model)?;
            let routes = exact_routes
                .get(&(
                    model.true_name.to_ascii_lowercase(),
                    model.model.path.clone(),
                ))
                .cloned()
                .unwrap_or_default();
            if routes.len() != 1 {
                continue;
            }
            let exact_route = routes
                .iter()
                .next()
                .cloned()
                .expect("one exact route after length check");
            visual["models"] = json!([{
                "exactRoute": exact_route,
                "nativeAsset": model.model,
                "trueName": model.true_name
            }]);
            visual["modelStatus"] = json!("verified_unique");
            repairs.push(json!({
                "category": category,
                "itemNumber": item_number,
                "gender": gender,
                "sourceTrueName": source_true_name,
                "exactRoute": exact_route,
                "nativeAsset": model.model,
                "resourceSet": model.resource_set
            }));
        }
    }
    let repaired_true_names = repairs
        .iter()
        .filter_map(|repair| repair["sourceTrueName"].as_str())
        .collect::<BTreeSet<_>>();
    if repairs.len() != 60
        || repaired_true_names != BTreeSet::from(["f_pants_gothgirl", "f_pants_stylistdandy"])
    {
        return Err(format!(
            "expected 60 stale avatar visual repairs for two primary models, found {} for {:?}",
            repairs.len(),
            repaired_true_names
        ));
    }
    recompute_counts(&mut avatar)?;
    let avatar_bytes = pretty_json(&avatar)?;
    let original_avatar = fs::read(&avatar_path)
        .map_err(|error| format!("cannot back up {}: {error}", avatar_path.display()))?;
    write_replace(&avatar_path, &avatar_bytes)?;
    if let Err(error) = read_typed::<CharacterCreationAvatarItems>(&avatar_path) {
        let _ = write_replace(&avatar_path, &original_avatar);
        return Err(error);
    }

    let evidence_path = project_root
        .join("target/ffone-audits/player-equipment-avatar-model-route-repair-audit.json");
    let evidence = json!({
        "schema": "ffone.player-equipment-avatar-model-route-repair.v1",
        "status": "complete",
        "sourceAlias": "primary",
        "sourceBuild": "retrobution-20260613",
        "reason": "The native GLBs and shared-rig routes were already published and verified, but avatar_items.json predated those catalog entries and still marked the two models missing.",
        "intentionalDivergence": Value::Null,
        "inputs": {
            "playerItemCatalog": {
                "path": relative_slash(&project_root, &catalog_path)?,
                "bytes": catalog_bytes.len(),
                "sha256": sha256_hex(&catalog_bytes)
            },
            "playerRigContract": {
                "path": relative_slash(&project_root, &rig_path)?,
                "bytes": rig_bytes.len(),
                "sha256": sha256_hex(&rig_bytes)
            }
        },
        "conversionCommand": "ffone-asset-pipeline repair_verified_avatar_model_routes <project-root>",
        "repairCount": repairs.len(),
        "repairedTrueNames": repaired_true_names,
        "repairs": repairs
    });
    write_replace(&evidence_path, &pretty_json(&evidence)?)?;
    println!(
        "Repaired 60 stale avatar visuals for 2 verified primary models; evidence={}",
        evidence_path.display()
    );
    Ok(())
}

fn recompute_counts(avatar: &mut Value) -> Result<(), String> {
    let items = avatar["items"]
        .as_array()
        .ok_or_else(|| "avatar items array is absent".to_owned())?;
    let mut model_references = 0_u64;
    let mut resolved_models = 0_u64;
    let mut texture_references = 0_u64;
    let mut resolved_textures = 0_u64;
    let mut icon_references = 0_u64;
    let mut resolved_icons = 0_u64;
    for item in items {
        if !item["icon"].is_null() {
            icon_references += 1;
            if item["icon"]["status"] == "verified_unique" {
                resolved_icons += 1;
            }
        }
        for gender in ["male", "female"] {
            let visual = &item[gender];
            if !visual["sourceModelTrueName"].is_null() {
                model_references += 1;
            }
            if matches!(
                visual["modelStatus"].as_str(),
                Some("verified_unique" | "verified_variants")
            ) {
                resolved_models += 1;
            }
            for slot in ["primaryTexture", "secondaryTexture"] {
                if !visual[slot].is_null() {
                    texture_references += 1;
                    if visual[slot]["status"] == "verified_unique" {
                        resolved_textures += 1;
                    }
                }
            }
        }
    }
    avatar["counts"]["modelReferences"] = json!(model_references);
    avatar["counts"]["resolvedModels"] = json!(resolved_models);
    avatar["counts"]["textureReferences"] = json!(texture_references);
    avatar["counts"]["resolvedTextures"] = json!(resolved_textures);
    avatar["counts"]["iconReferences"] = json!(icon_references);
    avatar["counts"]["resolvedIcons"] = json!(resolved_icons);
    avatar["lookupComplete"] = json!(
        model_references == resolved_models
            && texture_references == resolved_textures
            && icon_references == resolved_icons
    );
    Ok(())
}

fn verify_artifact(asset_root: &Path, artifact: &ResourceSetArtifact) -> Result<(), String> {
    let path = checked_asset_path(asset_root, &artifact.path)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("cannot read artifact {}: {error}", path.display()))?;
    if bytes.len() as u64 != artifact.bytes
        || blake3::hash(&bytes).to_hex().as_str() != artifact.blake3
    {
        return Err(format!(
            "artifact acceptance identity changed: {}",
            artifact.path
        ));
    }
    Ok(())
}

fn read_value(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid JSON {}: {error}", path.display()))
}

fn read_typed<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid JSON {}: {error}", path.display()))
}

fn pretty_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "output has no parent".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let temporary = parent.join(format!(
        ".{}.next-{}",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("output"),
        std::process::id()
    ));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| format!("cannot publish {}: {error}", path.display()))
}

fn checked_asset_path(asset_root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty()
        || relative.contains('\\')
        || Path::new(relative).is_absolute()
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("invalid native asset route: {relative}"));
    }
    let canonical = fs::canonicalize(asset_root.join(relative))
        .map_err(|error| format!("cannot resolve asset {relative}: {error}"))?;
    if !canonical.starts_with(asset_root) || !canonical.is_file() {
        return Err(format!("asset is outside native root: {relative}"));
    }
    Ok(canonical)
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve {label} {}: {error}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!(
            "{label} is not a directory: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve {label} {}: {error}", path.display()))?;
    if !canonical.is_file() {
        return Err(format!("{label} is not a file: {}", canonical.display()));
    }
    Ok(canonical)
}

fn relative_slash(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| format!("{} is outside {}", path.display(), root.display()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
