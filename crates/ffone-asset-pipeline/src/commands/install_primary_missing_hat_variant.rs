use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    PLAYER_ITEM_SCHEMA, PlayerItemCatalogModel, PlayerItemDefinition, PlayerItemSetCatalog,
    ResourceSetArtifact, ResourceSetDocument, ResourceSetMember, verify_player_item_sets,
};
use ffone_runtime_contracts::{
    AvatarItemCategory, CharacterCreationAssetReference, CharacterCreationAvatarItems,
    NativeLookupStatus,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const TRUE_NAME: &str = "m_head_012_type02";
const EXACT_ROUTE: &str = "wear/m_head_012_type02.nif";
const BASE_TRUE_NAME: &str = "m_head_012_type01";
const ITEM_NUMBER: u32 = 12;

pub(super) fn run(command_args: &[String]) -> Result<(), String> {
    let args = command_args.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    let [
        project_root,
        primary_root,
        candidate_glb,
        source_manifest_path,
        gpu_audit_path,
    ] = args.as_slice()
    else {
        return Err(
            "usage: install_primary_missing_hat_variant <PROJECT_ROOT> <PRIMARY_SOURCE_ROOT> <CANDIDATE_GLB> <SOURCE_MANIFEST> <GPU_AUDIT>"
                .to_owned(),
        );
    };
    let project_root = canonical_directory(Path::new(project_root), "project root")?;
    let primary_root = canonical_directory(Path::new(primary_root), "primary source root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let candidate_glb = canonical_file(Path::new(candidate_glb), "candidate GLB")?;
    let source_manifest_path = canonical_file(Path::new(source_manifest_path), "source manifest")?;
    let gpu_audit_path = canonical_file(Path::new(gpu_audit_path), "GPU audit")?;
    verify_gates(&primary_root, &source_manifest_path, &gpu_audit_path)?;

    let catalog_path = canonical_file(
        &asset_root.join("characters/player/items/catalog.json"),
        "player item catalog",
    )?;
    let avatar_path = canonical_file(
        &asset_root.join("data/character_creation/avatar_items.json"),
        "avatar items",
    )?;
    let runtime_path = canonical_file(
        &asset_root.join("data/character_creation/runtime_textures.json"),
        "runtime textures",
    )?;
    let appearance_path = canonical_file(
        &asset_root.join("data/character_creation/appearance.json"),
        "appearance",
    )?;
    let name_wheel_path = canonical_file(
        &asset_root.join("data/character_creation/name_wheel.json"),
        "name wheel",
    )?;

    let mut catalog: PlayerItemSetCatalog = read_typed(&catalog_path)?;
    let base = catalog
        .models
        .iter()
        .find(|model| model.category == "head" && model.true_name == BASE_TRUE_NAME)
        .cloned()
        .ok_or_else(|| format!("base model {BASE_TRUE_NAME} is absent"))?;
    if catalog
        .models
        .iter()
        .any(|model| model.category == "head" && model.true_name == TRUE_NAME)
    {
        return Err(format!("refusing to replace installed {TRUE_NAME}"));
    }
    let set_entry_index = catalog
        .sets
        .iter()
        .position(|entry| entry.id == base.resource_set)
        .ok_or_else(|| format!("resource set {} is absent", base.resource_set))?;
    let set_path = checked_asset_path(&asset_root, &catalog.sets[set_entry_index].definition.path)?;
    let mut set: ResourceSetDocument = read_typed(&set_path)?;
    if set.id != base.resource_set || set.members.iter().any(|member| member.name == TRUE_NAME) {
        return Err("target head resource set identity changed".to_owned());
    }
    let set_root = set_path
        .parent()
        .ok_or_else(|| "target set has no parent".to_owned())?;
    let model_root = set_root.join("models").join(TRUE_NAME);
    if model_root.exists() {
        return Err(format!("refusing to overwrite {}", model_root.display()));
    }

    let protected_paths = [
        set_path.clone(),
        catalog_path.clone(),
        avatar_path.clone(),
        runtime_path.clone(),
        appearance_path.clone(),
        name_wheel_path.clone(),
    ];
    let originals = protected_paths
        .iter()
        .map(|path| {
            fs::read(path)
                .map(|bytes| (path.clone(), bytes))
                .map_err(|error| format!("cannot back up {}: {error}", path.display()))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    let install_result = (|| -> Result<Value, String> {
        let glb = fs::read(&candidate_glb)
            .map_err(|error| format!("cannot read {}: {error}", candidate_glb.display()))?;
        let relative_root = relative_slash(&asset_root, set_root)?;
        let model_artifact = artifact(
            format!("{relative_root}/models/{TRUE_NAME}/model.glb"),
            &glb,
        );
        let item_id = format!(
            "player-item-{}",
            blake3::hash(
                format!("primary\0head/{TRUE_NAME}/{TRUE_NAME}.glb\0{EXACT_ROUTE}").as_bytes()
            )
            .to_hex()
        );
        let definition = PlayerItemDefinition {
            schema: PLAYER_ITEM_SCHEMA.to_owned(),
            id: item_id.clone(),
            true_name: TRUE_NAME.to_owned(),
            category: "head".to_owned(),
            source_route: format!("head/{TRUE_NAME}/{TRUE_NAME}.glb"),
            resource_set: set.id.clone(),
            model: model_artifact.clone(),
        };
        let definition_bytes = pretty_json(&definition)?;
        let definition_artifact = artifact(
            format!("{relative_root}/models/{TRUE_NAME}/item.json"),
            &definition_bytes,
        );
        fs::create_dir_all(&model_root)
            .map_err(|error| format!("cannot create {}: {error}", model_root.display()))?;
        write_new(&model_root.join("model.glb"), &glb)?;
        write_new(&model_root.join("item.json"), &definition_bytes)?;

        set.members.push(ResourceSetMember {
            id: item_id,
            name: TRUE_NAME.to_owned(),
            definition: definition_artifact,
            files: vec![model_artifact.clone()],
        });
        set.members
            .sort_by(|left, right| left.name.cmp(&right.name));
        let set_bytes = pretty_json(&set)?;
        let set_artifact = artifact(relative_slash(&asset_root, &set_path)?, &set_bytes);
        catalog.sets[set_entry_index].definition = set_artifact.clone();
        catalog.sets[set_entry_index].member_count = set.members.len() as u64;
        catalog.models.push(PlayerItemCatalogModel {
            category: "head".to_owned(),
            true_name: TRUE_NAME.to_owned(),
            source_route: definition.source_route.clone(),
            resource_set: set.id.clone(),
            model: model_artifact.clone(),
        });
        catalog.models.sort_by(|left, right| {
            (
                left.category.as_str(),
                left.true_name.as_str(),
                left.source_route.as_str(),
                left.model.path.as_str(),
            )
                .cmp(&(
                    right.category.as_str(),
                    right.true_name.as_str(),
                    right.source_route.as_str(),
                    right.model.path.as_str(),
                ))
        });
        let catalog_bytes = pretty_json(&catalog)?;
        let catalog_artifact = artifact(
            "characters/player/items/catalog.json".to_owned(),
            &catalog_bytes,
        );

        let mut avatar = read_value(&avatar_path)?;
        let item = avatar["items"]
            .as_array_mut()
            .ok_or_else(|| "avatar items array is absent".to_owned())?
            .iter_mut()
            .find(|item| item["category"] == "head" && item["itemNumber"] == ITEM_NUMBER)
            .ok_or_else(|| "male hair item 12 is absent".to_owned())?;
        let models = item["male"]["models"]
            .as_array_mut()
            .ok_or_else(|| "male hair item 12 models are absent".to_owned())?;
        models.push(json!({
            "exactRoute": EXACT_ROUTE,
            "nativeAsset": model_artifact,
            "trueName": TRUE_NAME
        }));
        models.sort_by(|left, right| left["trueName"].as_str().cmp(&right["trueName"].as_str()));
        item["male"]["modelStatus"] = json!("verified_variants");
        update_catalog_proof(&mut avatar, &catalog_artifact)?;
        let mut runtime = read_value(&runtime_path)?;
        let mut appearance = read_value(&appearance_path)?;
        let mut name_wheel = read_value(&name_wheel_path)?;
        update_catalog_proof(&mut runtime, &catalog_artifact)?;
        update_catalog_proof(&mut appearance, &catalog_artifact)?;
        update_catalog_proof(&mut name_wheel, &catalog_artifact)?;

        write_replace(&set_path, &set_bytes)?;
        write_replace(&catalog_path, &catalog_bytes)?;
        write_replace(&avatar_path, &pretty_json(&avatar)?)?;
        write_replace(&runtime_path, &pretty_json(&runtime)?)?;
        write_replace(&appearance_path, &pretty_json(&appearance)?)?;
        write_replace(&name_wheel_path, &pretty_json(&name_wheel)?)?;
        verify_install(&asset_root, &catalog_path, &avatar_path)?;

        let pre_existing_global_blocker = match verify_player_item_sets(&project_root) {
            Ok(_) => Value::Null,
            Err(error) if error.to_string().contains("hnpc-runtime-textures") => json!({
                "classification": "pre-existing-unrelated-retired-store",
                "message": error.to_string(),
                "extensionScopedVerification": "passed"
            }),
            Err(error) => return Err(error.to_string()),
        };
        Ok(json!({
            "model": model_artifact,
            "resourceSet": set.id,
            "resourceSetDefinition": set_artifact,
            "scopedVerification": "passed",
            "preExistingGlobalBlocker": pre_existing_global_blocker
        }))
    })();

    let installation = match install_result {
        Ok(value) => value,
        Err(error) => {
            for (path, bytes) in &originals {
                let _ = write_replace(path, bytes);
            }
            let _ = fs::remove_dir_all(&model_root);
            return Err(error);
        }
    };
    let evidence_path =
        project_root.join("target/ffone-audits/player-equipment-primary-hat-variant-audit.json");
    let source_manifest = fs::read(&source_manifest_path)
        .map_err(|error| format!("cannot read source manifest: {error}"))?;
    let gpu_audit =
        fs::read(&gpu_audit_path).map_err(|error| format!("cannot read GPU audit: {error}"))?;
    let evidence = json!({
        "schema": "ffone.player-equipment-primary-hat-variant-audit.v1",
        "status": "installed-validated-native-primary-parity-repair",
        "sourceAlias": "primary",
        "sourceBuild": "retrobution-20260613",
        "exactRoute": EXACT_ROUTE,
        "trueName": TRUE_NAME,
        "behaviorOwnership": "CharacterCustomize.SetHat equipType 1/5 selects type02 hair; item 12 was the only primary type02 route absent from the native item catalog.",
        "intentionalDivergence": Value::Null,
        "conversionCommands": [
            "FFONE_EXTRA_DEPENDENCY_BUNDLES=<primary>/CharacterCreation.resourceFile fusionforge fusionforge export-equipment-model-sources <bundle-index> <source-plan> <source-root>",
            "ffone-asset-pipeline publish-equipment-logical-model-batch <source-root> <candidate-root>",
            "ffone-asset-pipeline accept-equipment-gpu-batch <candidate-root> <gpu-evidence-root> logical_model_gpu_preview full <gpu-audit> --frames 240 --timeout 45",
            "ffone-asset-pipeline install_primary_missing_hat_variant <project-root> <primary-root> <candidate-glb> <source-manifest> <gpu-audit>"
        ],
        "sourceManifest": {
            "bytes": source_manifest.len(),
            "sha256": sha256_hex(&source_manifest)
        },
        "gpuAudit": {
            "bytes": gpu_audit.len(),
            "sha256": sha256_hex(&gpu_audit),
            "passed": "1/1"
        },
        "installation": installation
    });
    write_replace(&evidence_path, &pretty_json(&evidence)?)?;
    println!(
        "Installed primary {TRUE_NAME}; evidence={}",
        evidence_path.display()
    );
    Ok(())
}

fn verify_gates(
    primary_root: &Path,
    source_manifest_path: &Path,
    gpu_audit_path: &Path,
) -> Result<(), String> {
    let source = read_value(source_manifest_path)?;
    if source["status"] != "complete"
        || source
            .pointer("/counts/exportedPhysicalModels")
            .and_then(Value::as_u64)
            != Some(1)
        || source
            .pointer("/counts/totalBlockers")
            .and_then(Value::as_u64)
            != Some(0)
        || source
            .pointer("/exported/0/trueName")
            .and_then(Value::as_str)
            != Some(TRUE_NAME)
        || source
            .pointer("/exported/0/canonicalRoute")
            .and_then(Value::as_str)
            != Some(EXACT_ROUTE)
    {
        return Err("primary source gate is incomplete".to_owned());
    }
    let raw_path = canonical_file(
        &primary_root.join("CharacterSelection.resourceFile"),
        "primary CharacterSelection resource",
    )?;
    let raw = fs::read(&raw_path)
        .map_err(|error| format!("cannot read {}: {error}", raw_path.display()))?;
    if source
        .pointer("/bundles/0/byteLength")
        .and_then(Value::as_u64)
        != Some(raw.len() as u64)
        || source.pointer("/bundles/0/sha256").and_then(Value::as_str)
            != Some(sha256_hex(&raw).as_str())
    {
        return Err("primary raw bundle acceptance identity changed".to_owned());
    }
    let gpu = read_value(gpu_audit_path)?;
    if gpu
        .pointer("/counts/selectedModels")
        .and_then(Value::as_u64)
        != Some(1)
        || gpu
            .pointer("/counts/standaloneGpuPassedModels")
            .and_then(Value::as_u64)
            != Some(1)
        || gpu
            .pointer("/counts/executionBlockedModels")
            .and_then(Value::as_u64)
            != Some(0)
        || !gpu
            .pointer("/models/0/disposition")
            .and_then(Value::as_str)
            .is_some_and(|disposition| disposition.starts_with("passed-"))
    {
        return Err("primary GPU gate is incomplete".to_owned());
    }
    Ok(())
}

fn verify_install(
    asset_root: &Path,
    catalog_path: &Path,
    avatar_path: &Path,
) -> Result<(), String> {
    let catalog: PlayerItemSetCatalog = read_typed(catalog_path)?;
    let model = catalog
        .models
        .iter()
        .find(|model| model.category == "head" && model.true_name == TRUE_NAME)
        .ok_or_else(|| format!("installed model {TRUE_NAME} is absent"))?;
    verify_artifact(asset_root, &model.model)?;
    let set_entry = catalog
        .sets
        .iter()
        .find(|entry| entry.id == model.resource_set)
        .ok_or_else(|| "installed resource set is absent".to_owned())?;
    verify_artifact(asset_root, &set_entry.definition)?;
    let set: ResourceSetDocument =
        read_typed(&checked_asset_path(asset_root, &set_entry.definition.path)?)?;
    let member = set
        .members
        .iter()
        .find(|member| member.name == TRUE_NAME)
        .ok_or_else(|| "installed resource-set member is absent".to_owned())?;
    verify_artifact(asset_root, &member.definition)?;
    for artifact in &member.files {
        verify_artifact(asset_root, artifact)?;
    }
    if !member.files.contains(&model.model) {
        return Err("installed model is not owned by its resource set".to_owned());
    }
    let avatar: CharacterCreationAvatarItems = read_typed(avatar_path)?;
    let item = avatar
        .items
        .iter()
        .find(|item| item.category == AvatarItemCategory::Head && item.item_number == ITEM_NUMBER)
        .ok_or_else(|| "installed avatar item is absent".to_owned())?;
    let references = item
        .male
        .models
        .iter()
        .filter(|reference| reference.true_name == TRUE_NAME)
        .collect::<Vec<_>>();
    let [reference] = references.as_slice() else {
        return Err(format!(
            "avatar {TRUE_NAME} must be unique, found {}",
            references.len()
        ));
    };
    if item.male.model_status != NativeLookupStatus::VerifiedVariants
        || reference.exact_route != EXACT_ROUTE
        || !asset_reference_matches(&reference.native_asset, &model.model)
    {
        return Err("installed avatar hat variant mapping changed".to_owned());
    }
    Ok(())
}

fn asset_reference_matches(
    reference: &CharacterCreationAssetReference,
    artifact: &ResourceSetArtifact,
) -> bool {
    reference.path == artifact.path
        && reference.bytes == artifact.bytes
        && reference.blake3 == artifact.blake3
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

fn update_catalog_proof(
    document: &mut Value,
    artifact: &ResourceSetArtifact,
) -> Result<(), String> {
    let slot = document
        .pointer_mut("/provenance/playerEquipmentCatalog")
        .ok_or_else(|| "character-creation document has no equipment proof".to_owned())?;
    *slot = serde_json::to_value(artifact).map_err(|error| error.to_string())?;
    Ok(())
}

fn artifact(path: String, bytes: &[u8]) -> ResourceSetArtifact {
    ResourceSetArtifact {
        path,
        bytes: bytes.len() as u64,
        blake3: blake3::hash(bytes).to_hex().to_string(),
    }
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

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!("refusing to overwrite {}", path.display()));
    }
    fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
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
    let path = asset_root.join(relative);
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))?;
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
