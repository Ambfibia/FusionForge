use super::*;

pub const TUTORIAL_EFFECT_CATALOG_PATH: &str = "map/shared/effects/catalog.json";

pub const TUTORIAL_PROJECTILE_CATALOG_PATH: &str = "map/shared/projectiles/catalog.json";

pub const TUTORIAL_EFFECT_CATALOG_SCHEMA: &str = "ffone.tutorial-effect-catalog.v1";

pub const TUTORIAL_PROJECTILE_CATALOG_SCHEMA: &str = "ffone.tutorial-projectile-catalog.v1";

/// `(asset name, serialized bytes, serialized BLAKE3, dump bytes, dump BLAKE3)`.
///
/// These values bind the lossless JSON evidence to the three exact serialized
/// assets audited from `retrobution-20260821`; filenames alone are not trusted.
pub const RETROBUTION_TUTORIAL_SOURCE_ASSET_PROOFS: [(&str, u64, &str, u64, &str); 3] =
    ffone_runtime_contracts::RETROBUTION_TUTORIAL_SOURCE_ASSET_PROOFS;

pub(super) const BULLET_TABLE_ROUTE: &str = "bullettable.asset";

pub(super) const PRIMARY_EFFECTS_ASSET: &str = "CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialSourceAssetProof {
    pub asset: String,
    pub serialized_asset: TutorialSourceFileProof,
    pub object_dump: TutorialSourceFileProof,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialEffectCatalog {
    pub schema: String,
    pub source_build: String,
    pub source_bundle: TutorialSourceFileProof,
    pub source_dump: TutorialSourceFileProof,
    pub source_assets: Vec<TutorialSourceAssetProof>,
    pub renderer_status: String,
    pub effects: Vec<TutorialEffectCatalogEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialEffectCatalogEntry {
    pub effect_id: i32,
    pub container_route: String,
    pub root_asset: String,
    pub root_path_id: i64,
    pub closure_path: String,
    pub closure_bytes: u64,
    pub closure_blake3: String,
    pub object_count: u64,
    pub object_types: Vec<String>,
    pub component_types: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialProjectileCatalog {
    pub schema: String,
    pub source_build: String,
    pub source_bundle: TutorialSourceFileProof,
    pub source_dump: TutorialSourceFileProof,
    pub source_assets: Vec<TutorialSourceAssetProof>,
    pub renderer_status: String,
    pub bullet_table_route: String,
    pub bullet_table_root_path_id: i64,
    pub bullet_table_closure_path: String,
    pub bullet_table_closure_bytes: u64,
    pub bullet_table_closure_blake3: String,
    pub particle_effects: Vec<TutorialEffectCatalogEntry>,
    pub rows: Vec<TutorialBulletCatalogEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TutorialBulletCatalogEntry {
    pub bullet_type: i32,
    pub row_path: String,
    pub row_bytes: u64,
    pub row_blake3: String,
    pub serialized_row_blake3: String,
    pub parameters: TutorialBulletParameters,
}

#[derive(Clone, Debug)]
pub(super) struct LoadedSourceAsset {
    pub(super) proof: TutorialSourceAssetProof,
    pub(super) objects: Vec<DumpObject>,
}

pub(super) fn load_source_asset(source_bundle: &Path, object_dump: &Path) -> Result<LoadedSourceAsset> {
    let bundle_bytes = fs::read(source_bundle).map_err(|error| io_at(source_bundle, error))?;
    let dump_bytes = fs::read(object_dump).map_err(|error| io_at(object_dump, error))?;
    let objects: Vec<DumpObject> =
        serde_json::from_slice(&dump_bytes).map_err(|source| PipelineError::Json {
            path: object_dump.display().to_string(),
            source,
        })?;
    if objects.is_empty() {
        return invalid(format!(
            "Unity object dump is empty: {}",
            object_dump.display()
        ));
    }
    let assets = objects
        .iter()
        .map(|object| object.asset.clone())
        .collect::<BTreeSet<_>>();
    if assets.len() != 1 {
        return invalid(format!(
            "Unity object dump {} contains {} asset identities: {assets:?}",
            object_dump.display(),
            assets.len()
        ));
    }
    let asset = assets.into_iter().next().unwrap_or_default();
    let file_name = source_bundle
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            PipelineError::InvalidManifest(format!(
                "serialized asset path has no UTF-8 filename: {}",
                source_bundle.display()
            ))
        })?;
    if file_name != asset {
        return invalid(format!(
            "serialized asset filename {file_name:?} disagrees with dump asset {asset:?}"
        ));
    }
    Ok(LoadedSourceAsset {
        proof: TutorialSourceAssetProof {
            asset: asset.clone(),
            serialized_asset: TutorialSourceFileProof {
                logical_name: format!("{asset}/serialized-asset"),
                bytes: bundle_bytes.len() as u64,
                blake3: hash(&bundle_bytes),
            },
            object_dump: TutorialSourceFileProof {
                logical_name: format!("{asset}/fusionforge-dump-object-all.json"),
                bytes: dump_bytes.len() as u64,
                blake3: hash(&dump_bytes),
            },
        },
        objects,
    })
}

pub(super) fn referenced_asset_name(source_asset: &str, file_id: i64) -> Option<&'static str> {
    match (source_asset, file_id) {
        (PRIMARY_EFFECTS_ASSET, 1) => Some(EFFECTS_DEPENDENCY_B4),
        (PRIMARY_EFFECTS_ASSET, 2) => Some(EFFECTS_DEPENDENCY_BD5),
        (EFFECTS_DEPENDENCY_B4, 1) => Some(EFFECTS_DEPENDENCY_BD5),
        // fileId=2 in b4 is `library/unity default resources`.  It is
        // deliberately unsupported here: a selected closure reaching it must
        // fail closed because no serialized builtin asset proof was supplied.
        _ => None,
    }
}

pub(super) fn unique_route_root(
    routes: &BTreeMap<String, Vec<UnityObjectKey>>,
    route: &str,
) -> Result<UnityObjectKey> {
    let roots = routes.get(route).ok_or_else(|| {
        PipelineError::InvalidManifest(format!("missing exact AssetBundle route {route:?}"))
    })?;
    if roots.len() != 1 {
        return invalid(format!(
            "exact AssetBundle route {route:?} resolves {} roots",
            roots.len()
        ));
    }
    Ok(roots[0].clone())
}

pub(super) fn effect_route(effect_id: i32) -> String {
    format!("prefabs/particle/effectscripts/es[{effect_id}].prefab")
}

pub(super) fn replace_manifest(path: &Path, manifest: &ProjectAssetManifest, token: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| PipelineError::InvalidOutputPath(path.to_path_buf()))?;
    let next = parent.join(format!(".asset-manifest.tutorial-effects-next-{token}"));
    let backup = parent.join(format!(".asset-manifest.tutorial-effects-backup-{token}"));
    let bytes = pretty_json(manifest)?;
    write_new(&next, &bytes)?;
    fs::rename(path, &backup).map_err(|error| io_at(path, error))?;
    if let Err(error) = fs::rename(&next, path) {
        let _ = fs::rename(&backup, path);
        let _ = fs::remove_file(&next);
        return Err(io_at(&next, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}

pub(super) fn verify_owned_catalog(path: &Path, expected_schema: &str) -> Result<()> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    let value: JsonValue =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: path.display().to_string(),
            source,
        })?;
    if value.get("schema").and_then(JsonValue::as_str) != Some(expected_schema) {
        return invalid(format!(
            "{} is not owned by the tutorial effect installer",
            path.display()
        ));
    }
    Ok(())
}
