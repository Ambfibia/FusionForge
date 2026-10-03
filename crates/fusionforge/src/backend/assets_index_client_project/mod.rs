use super::super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ClientFileIndex {
    pub(in super::super) source_dir: String,
    pub(in super::super) project_dir: String,
    pub(in super::super) bundles: Vec<ClientBundleFile>,
    pub(in super::super) unity_files: Vec<ClientBundleFile>,
    pub(in super::super) fonts: Vec<ClientFontRecord>,
    pub(in super::super) audio_clips: Vec<ClientAudioRecord>,
    pub(in super::super) cache_index_path: String,
    pub(in super::super) translation_index_path: String,
    pub(in super::super) translation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ClientAssetSummary {
    pub(in super::super) name: String,
    pub(in super::super) object_count: usize,
    pub(in super::super) type_counts: std::collections::BTreeMap<String, usize>,
    pub(in super::super) container_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ScriptPatchManifest {
    pub(in super::super) format: String,
    pub(in super::super) generator: String,
    pub(in super::super) entries: Vec<ScriptPatchEntry>,
}

impl Default for ScriptPatchManifest {
    fn default() -> Self {
        Self {
            format: "ffclient.script-patches.v1".to_string(),
            generator: "fusionforge.ffspy-backend.v1".to_string(),
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TableDataAssetSummary {
    pub(in super::super) asset: String,
    pub(in super::super) objects: Vec<TableDataObjectSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcCatalogInspection {
    pub(in super::super) bundle_path: String,
    pub(in super::super) cache_dir: String,
    pub(in super::super) asset: String,
    pub(in super::super) path_id: i64,
    pub(in super::super) object_name: String,
    pub(in super::super) records: Vec<NpcCatalogRecord>,
    pub(in super::super) sections: Vec<TableDataSectionSummary>,
    pub(in super::super) staged_ids: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcCatalogRecord {
    pub(in super::super) id: usize,
    pub(in super::super) name: String,
    #[serde(default)]
    pub(in super::super) internal_name: Option<String>,
    #[serde(default)]
    pub(in super::super) links: NpcCatalogLinks,
    #[serde(default)]
    pub(in super::super) icon: Option<NpcIconRecord>,
    pub(in super::super) comment: Option<String>,
    pub(in super::super) comment1: Option<String>,
    pub(in super::super) comment2: Option<String>,
    pub(in super::super) barker: Option<NpcBarkerRecord>,
    pub(in super::super) string_row: JsonValue,
    pub(in super::super) barker_row: Option<JsonValue>,
    pub(in super::super) related_rows: BTreeMap<String, JsonValue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcCatalogLinks {
    pub(in super::super) npc_name: Option<usize>,
    pub(in super::super) comment: Option<usize>,
    pub(in super::super) mesh: Option<usize>,
    pub(in super::super) icon1: Option<usize>,
    pub(in super::super) service_number: Option<usize>,
    pub(in super::super) barker_number: Option<usize>,
    pub(in super::super) active_skill1_string: Option<usize>,
    pub(in super::super) active_skill2_string: Option<usize>,
    pub(in super::super) support_skill_string: Option<usize>,
    pub(in super::super) corruption_string: Option<usize>,
    pub(in super::super) mega_string: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub(in super::super) struct NpcImportManifestContext {
    pub(in super::super) source_table_data_bundle: Option<PathBuf>,
    pub(in super::super) source_asset: Option<String>,
    pub(in super::super) source_path_id: Option<i64>,
    pub(in super::super) source_npc_id: Option<usize>,
    pub(in super::super) table_data_patch_path: Option<PathBuf>,
    pub(in super::super) source_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub(in super::super) struct NpcAssetHints {
    pub(in super::super) tokens: BTreeSet<String>,
    pub(in super::super) exact_paths: BTreeSet<String>,
    pub(in super::super) model_paths: BTreeSet<String>,
    pub(in super::super) model_names: BTreeSet<String>,
    pub(in super::super) texture_paths: BTreeSet<String>,
    pub(in super::super) icon_paths: BTreeSet<String>,
    pub(in super::super) audio_paths: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
pub(in super::super) struct WorldManifestSummary {
    pub(in super::super) staged_changes: usize,
}

pub(in super::super) const BUILD_SOURCE_INDEX_FILE: &str = ".ffclient-build-source-index.json";

pub(in super::super) const PROJECT_SCRIPT_MANIFEST_RELATIVE: &str = "scripts/manifest.json";

pub(in super::super) fn path_hash(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().to_lowercase().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(in super::super) fn normalized_server_tdata_path(value: Option<&str>) -> Option<String> {
    let value = value.map(str::trim).filter(|value| !value.is_empty())?;
    if value.starts_with('-') {
        return None;
    }
    Some(value.replace('\\', "/"))
}

pub(in super::super) fn default_server_tdata_path(project: &Path) -> Option<String> {
    if let Some(root) = workspace_root_dir() {
        let target = root.join("OpenFusion").join("bin").join("tdata");
        if target.is_dir() {
            return Some(target.to_string_lossy().replace('\\', "/"));
        }
    }
    let relative = "OpenFusion/bin/tdata";
    project
        .join("OpenFusion")
        .join("bin")
        .join("tdata")
        .is_dir()
        .then(|| relative.to_string())
}

pub(in super::super) fn resolve_server_tdata_path(project: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        return candidate;
    }
    if let Some(root) = workspace_root_dir() {
        let rooted = root.join(&candidate);
        if rooted.exists() || rooted.parent().is_some_and(|parent| parent.exists()) {
            return rooted;
        }
    }
    resolve_project_path(project, path)
}

pub(in super::super) fn sanitize_patch_config_server_tdata_path(project: &Path, patch_config: &mut serde_json::Value) {
    let current = json_string(patch_config, "ServerTdataPath");
    if let Some(path) = normalized_server_tdata_path(current.as_deref()) {
        patch_config["ServerTdataPath"] = json!(path);
        return;
    }
    if let Some(default) = default_server_tdata_path(project) {
        patch_config["ServerTdataPath"] = json!(default);
        return;
    }
    if let Some(object) = patch_config.as_object_mut() {
        object.remove("ServerTdataPath");
    }
}

pub(in super::super) fn project_patch_config_path(project: &Path) -> PathBuf {
    project.join("ffpatch.json")
}

pub(in super::super) fn resolve_project_path(project: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project.join(path)
    }
}

pub(in super::super) fn project_script_manifest_path(project: &Path) -> PathBuf {
    resolve_project_path(project, PROJECT_SCRIPT_MANIFEST_RELATIVE)
}

pub(in super::super) fn path_relative_to_project(project: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(project)
        .ok()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
}

pub(in super::super) fn load_script_patch_manifest(project_dir: String) -> EditorResult<ScriptPatchManifest> {
    let project = PathBuf::from(project_dir);
    let manifest_path = project_script_manifest_path(&project);
    load_script_patch_manifest_from_path(&manifest_path)
}

pub(in super::super) fn load_script_patch_manifest_from_path(manifest_path: &Path) -> EditorResult<ScriptPatchManifest> {
    if !manifest_path.exists() {
        return Ok(ScriptPatchManifest::default());
    }
    let data = fs::read_to_string(&manifest_path)
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    serde_json::from_str::<ScriptPatchManifest>(&data)
        .map_err(|err| format!("{}: {err}", manifest_path.display()))
}

pub(in super::super) fn write_script_patch_manifest(project: &Path, manifest: &ScriptPatchManifest) -> EditorResult<()> {
    let manifest_path = project_script_manifest_path(project);
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let data = serde_json::to_string_pretty(manifest).map_err(|err| err.to_string())?;
    fs::write(&manifest_path, format!("{data}\n"))
        .map_err(|err| format!("{}: {err}", manifest_path.display()))
}

pub(in super::super) fn load_reusable_client_index(source_path: &Path, project_path: &Path) -> Option<ClientFileIndex> {
    let index_path = project_path.join("cache").join("bundle-index.json");
    let mut index =
        serde_json::from_str::<ClientFileIndex>(&fs::read_to_string(index_path).ok()?).ok()?;
    if PathBuf::from(&index.source_dir) != source_path {
        return None;
    }

    let mut current_bundles = Vec::new();
    let mut current_unity_files = Vec::new();
    scan_client_files(source_path, &mut current_bundles, &mut current_unity_files).ok()?;
    current_bundles.sort_by(|a, b| a.path.cmp(&b.path));
    let current_signatures = source_bundle_signatures(&current_bundles)?;
    let cached_signatures = source_bundle_signatures(&index.bundles)?;
    if current_signatures != cached_signatures {
        return None;
    }
    if !cached_bundle_payloads_are_current(&index) {
        return None;
    }

    index.project_dir = project_path.to_string_lossy().to_string();
    index.cache_index_path = project_path
        .join("cache")
        .join("bundle-index.json")
        .to_string_lossy()
        .to_string();
    index.translation_index_path.clear();
    index.translation_count = 0;
    Some(index)
}

pub(in super::super) const LOCALIZABLE_PATH_HINTS: &[&str] = &[
    "MissionStringData",
    "NpcStringData",
    "NpcBarkerData",
    "ItemStringData",
    "NanoStringData",
    "NanoTuneStringData",
    "ShinyStringData",
    "SkillStringData",
    "ChatStringData",
    "MessageData",
    "GuideStringData",
    "WarpNameData",
    "WorldNameData",
    "ClassString",
    "SkillBookString",
    "SceneData",
];

pub(in super::super) fn is_asset_filename_source(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    let Some((name, extension)) = lower.rsplit_once('.') else {
        return false;
    };
    if name.is_empty()
        || name.contains(['/', '\\'])
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return false;
    }
    matches!(
        extension,
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tga" | "dds" | "psd" | "swf" | "xml"
    )
}

pub(in super::super) fn should_export_unity_field_path(field_path: &str) -> bool {
    if field_path == "m_Name" {
        return false;
    }
    if is_unsupported_unity_translation_field_path(field_path) {
        return false;
    }
    if is_explicit_localizable_unity_field_path(field_path) {
        return true;
    }
    LOCALIZABLE_PATH_HINTS
        .iter()
        .any(|hint| field_path.contains(hint))
}

pub(in super::super) fn is_explicit_localizable_unity_field_path(field_path: &str) -> bool {
    is_name_wheel_text_field_path(field_path)
        || is_transportation_location_text_field_path(field_path)
        || is_help_string_text_field_path(field_path)
        || is_help_page_text_field_path(field_path)
        || is_first_use_text_field_path(field_path)
        || is_rules_text_field_path(field_path)
        || is_npc_service_text_field_path(field_path)
}

pub(in super::super) fn is_internal_npc_alias_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pNpcTable.m_pNpcStringData[")
        && field_path.ends_with(".m_strComment2")
}

pub(in super::super) fn is_shiny_internal_name_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pShinyTable.m_pShinyStringData[")
        && field_path.ends_with(".m_strName")
}

pub(in super::super) fn is_filter_dictionary_field_path(field_path: &str) -> bool {
    (field_path.starts_with("m_pFilterTable.m_pWhiteFilterData[")
        || field_path.starts_with("m_pFilterTable.m_pBlackFilterData[")
        || field_path.starts_with("m_pFilterTable.m_pNameFilterData["))
        && field_path.ends_with(".m_strText")
}

pub(in super::super) fn is_internal_help_page_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pHelpTable.m_pHelpPageString[")
        && (field_path.ends_with(".m_strComment")
            || field_path.ends_with(".m_strComment1")
            || field_path.ends_with(".m_strComment2"))
}

pub(in super::super) fn is_internal_first_use_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pFirstUseTable.m_pFirstUseString[")
        && (field_path.ends_with(".m_strComment1") || field_path.ends_with(".m_strComment2"))
}

pub(in super::super) fn is_internal_rules_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pRulesTable.m_pRulesString[")
        && (field_path.ends_with(".m_strComment")
            || field_path.ends_with(".m_strComment1")
            || field_path.ends_with(".m_strComment2"))
}

pub(in super::super) fn is_name_wheel_text_field_path(field_path: &str) -> bool {
    (field_path.starts_with("m_pNameTable.m_pFirstName[")
        || field_path.starts_with("m_pNameTable.m_pMiddleName[")
        || field_path.starts_with("m_pNameTable.m_pLastName["))
        && field_path.ends_with(".m_pstrNameString")
}

pub(in super::super) fn is_transportation_location_text_field_path(field_path: &str) -> bool {
    (field_path.starts_with("m_pTransportationTable.m_pBroomstickString[")
        || field_path.starts_with("m_pTransportationTable.m_pTransportationWarpString["))
        && (field_path.ends_with(".m_pstrLocationName")
            || field_path.ends_with(".m_pstrLocationInfo"))
}

pub(in super::super) fn is_help_string_text_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pHelpTable.m_pHelpString[")
        && (field_path.ends_with(".m_strName") || field_path.ends_with(".m_strComment"))
}

pub(in super::super) fn is_help_page_text_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pHelpTable.m_pHelpPageString[") && field_path.ends_with(".m_strName")
}

pub(in super::super) fn is_first_use_text_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pFirstUseTable.m_pFirstUseString[")
        && (field_path.ends_with(".m_strName") || field_path.ends_with(".m_strComment"))
}

pub(in super::super) fn is_rules_text_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pRulesTable.m_pRulesString[") && field_path.ends_with(".m_strName")
}

pub(in super::super) fn is_npc_service_text_field_path(field_path: &str) -> bool {
    field_path.starts_with("m_pNpcTable.m_pNpcServiceData[")
        && field_path.ends_with(".m_strService")
}

pub(in super::super) fn path_to_string(parts: &[String]) -> String {
    let mut result = String::new();
    for part in parts {
        if part.starts_with('[') {
            result.push_str(part);
        } else if result.is_empty() {
            result.push_str(part);
        } else {
            result.push('.');
            result.push_str(part);
        }
    }
    result
}

pub(in super::super) fn collect_npc_import_manifest_paths(root: &Path, manifests: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_npc_import_manifest_paths(&path, manifests);
            continue;
        }
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.ends_with(".npc-import.json"))
        {
            manifests.push(path);
        }
    }
}

pub(in super::super) fn load_project_client_index(project_path: &Path) -> Option<ClientFileIndex> {
    let index_path = project_path.join("cache").join("bundle-index.json");
    serde_json::from_str::<ClientFileIndex>(&fs::read_to_string(index_path).ok()?).ok()
}

pub(in super::super) fn client_index_matches_project_source(project_path: &Path, index: &ClientFileIndex) -> bool {
    project_source_dir_from_patch_config(project_path)
        .map(|source| PathBuf::from(&index.source_dir) == source)
        .unwrap_or(true)
}

pub(in super::super) fn export_world_patch_manifest(
    path: String,
    mut document: serde_json::Value,
) -> EditorResult<serde_json::Value> {
    migrate_world_document_json(&mut document);
    let plan = analyze_world_bundle_plan_value(&document);
    let manifest = json!({
        "format": "fftools.world-patch.v1",
        "schemaVersion": 1,
        "generatedAtUnixMs": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default(),
        "tileId": document.get("tileId").and_then(serde_json::Value::as_str).unwrap_or("Map_00_00"),
        "sourceMapBundle": document.get("sourceMapBundle").cloned().unwrap_or(JsonValue::Null),
        "sourceResourceBundle": document.get("sourceResourceBundle").cloned().unwrap_or(JsonValue::Null),
        "document": stripped_world_authoring_document(&document),
        "bundlePlan": plan,
    });
    let data = serde_json::to_string_pretty(&manifest)
        .map_err(|err| EditorError::Json(err).to_string())?;
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| EditorError::Io(err).to_string())?;
    }
    fs::write(&path, format!("{data}\n")).map_err(|err| EditorError::Io(err).to_string())?;
    Ok(manifest)
}

pub(in super::super) fn preview_world_asset(
    bundle_path: String,
    project_dir: Option<String>,
    kind: String,
    container_path: Option<String>,
) -> EditorResult<serde_json::Value> {
    let paths = container_path.into_iter().collect::<Vec<_>>();
    if kind.eq_ignore_ascii_case("texture") {
        preview_bundle_container_texture(bundle_path, project_dir, paths)
    } else {
        preview_bundle_container_model(bundle_path, project_dir, paths)
    }
}

pub(in super::super) fn world_asset_kind(path: &str, type_counts: &BTreeMap<String, usize>) -> &'static str {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    if lower.ends_with(".kfm")
        || lower.ends_with(".nif")
        || lower.ends_with(".prefab")
        || lower.ends_with(".glb")
        || lower.ends_with(".gltf")
        || lower.ends_with(".obj")
        || (type_counts.get("Mesh").copied().unwrap_or_default() > 0
            && (lower.starts_with("mob/")
                || lower.starts_with("wear/")
                || lower.starts_with("weapon/")
                || lower.contains("/mob/")
                || lower.contains("/wear/")
                || lower.contains("/weapon/")))
    {
        "model"
    } else if lower.ends_with(".dds")
        || lower.ends_with(".png")
        || lower.ends_with(".tga")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
    {
        "texture"
    } else if type_counts.get("Material").copied().unwrap_or_default() > 0 {
        "material"
    } else {
        "other"
    }
}

pub(in super::super) fn load_world_manifest_summary(project: &Path) -> Result<WorldManifestSummary, String> {
    let manifest_path = project.join("world").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(WorldManifestSummary::default());
    }
    let manifest = serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(&manifest_path)
            .map_err(|err| format!("{}: {err}", manifest_path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    if manifest.get("format").and_then(serde_json::Value::as_str) != Some("fftools.world-patch.v1")
    {
        return Err(format!(
            "Unsupported world manifest format in {}",
            manifest_path.display()
        ));
    }
    let objects = json_array_len(&manifest, &["document", "objects"]);
    let colliders = json_array_len(&manifest, &["document", "colliders"]);
    let water = json_array_len(&manifest, &["document", "waterVolumes"]);
    let decisions = manifest
        .get("bundlePlan")
        .and_then(|plan| plan.get("decisions"))
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    Ok(WorldManifestSummary {
        staged_changes: objects + colliders + water + decisions,
    })
}

pub(in super::super) fn index_client_project(source_dir: String, project_dir: String) -> EditorResult<ClientFileIndex> {
    let source_path = PathBuf::from(&source_dir);
    if !source_path.exists() || !source_path.is_dir() {
        return Err(EditorError::MissingPath(source_dir).to_string());
    }
    if project_dir.trim().is_empty() {
        return Err("an explicit work directory is required".to_string());
    }
    let source_path = source_path.canonicalize().map_err(|err| err.to_string())?;
    let project_path = fusionforge::workspace::prepare_work_dir(&source_path, Path::new(&project_dir))?;
    fs::create_dir_all(project_path.join("cache").join("extracted-bundles"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    if let Some(index) = load_reusable_client_index(&source_path, &project_path) {
        return Ok(index);
    }
    let mut bundles = Vec::new();
    let mut unity_files = Vec::new();
    scan_client_files(&source_path, &mut bundles, &mut unity_files)
        .map_err(|err| err.to_string())?;
    bundles.sort_by(|a, b| a.path.cmp(&b.path));
    unity_files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut fonts = Vec::new();
    let mut audio_clips = Vec::new();

    for bundle in &mut bundles {
        let bundle_path = PathBuf::from(&bundle.path);
        match extract_bundle_cached(&project_path, &bundle_path) {
            Ok(cache_dir) => {
                let container = bundle_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("bundle")
                    .to_string();
                bundle.cache_dir = Some(cache_dir.to_string_lossy().to_string());
                bundle.extracted_files = extracted_files_in_dir(&cache_dir);
                bundle.assets = inspect_cached_bundle_assets(&cache_dir);
                fonts.extend(inspect_cached_bundle_fonts(
                    &cache_dir,
                    &container,
                    &bundle.path,
                ));
                audio_clips.extend(inspect_cached_bundle_audio(
                    &cache_dir,
                    &container,
                    &bundle.path,
                ));
                for file in bundle
                    .extracted_files
                    .iter()
                    .filter(|file| is_unity_or_managed_file(&file.name))
                {
                    unity_files.push(indexed_runtime_file(file, &cache_dir));
                }
                if bundle.extracted_files.is_empty() {
                    bundle.errors.push("Bundle extracted no files.".to_string());
                }
                if bundle.assets.is_empty() {
                    bundle
                        .errors
                        .push("No Unity assets parsed from extracted files.".to_string());
                }
            }
            Err(err) => bundle.errors.push(err),
        }
    }

    unity_files.sort_by(|a, b| a.path.cmp(&b.path));
    unity_files.dedup_by(|a, b| a.path == b.path);
    fonts.sort_by(|a, b| {
        (&a.family, &a.name, &a.container, &a.asset, a.path_id).cmp(&(
            &b.family,
            &b.name,
            &b.container,
            &b.asset,
            b.path_id,
        ))
    });
    audio_clips.sort_by(|a, b| {
        (&a.container, &a.asset, &a.name, a.path_id).cmp(&(
            &b.container,
            &b.asset,
            &b.name,
            b.path_id,
        ))
    });


    let index = ClientFileIndex {
        source_dir: source_path.to_string_lossy().to_string(),
        project_dir: project_path.to_string_lossy().to_string(),
        bundles,
        unity_files,
        fonts,
        audio_clips,
        cache_index_path: project_path
            .join("cache")
            .join("bundle-index.json")
            .to_string_lossy()
            .to_string(),
        translation_index_path: String::new(),
        translation_count: 0,
    };
    let data =
        serde_json::to_string_pretty(&index).map_err(|err| EditorError::Json(err).to_string())?;
    fs::write(&index.cache_index_path, format!("{data}\n"))
        .map_err(|err| EditorError::Io(err).to_string())?;
    Ok(index)
}

pub(crate) const CONTAINER_ASSET_POINTER_LIMIT: usize = 128;

#[derive(Debug, Clone)]
pub(crate) struct ContainerAssetBytesResolution {
    pub bytes: Vec<u8>,
    pub asset_index: usize,
    pub path_id: i64,
    pub object_type: String,
}

#[derive(Debug, Clone)]
pub(crate) enum ContainerAssetBytesSearch {
    Found(ContainerAssetBytesResolution),
    GraphExhausted,
    BudgetExceeded,
}

pub(crate) fn container_asset_bytes_with_source_budget(
    env: &fusionforge::UnityEnvironment,
    asset_index: usize,
    path_id: i64,
    visited: &mut BTreeSet<(usize, i64)>,
    budget: usize,
) -> ContainerAssetBytesSearch {
    if visited.contains(&(asset_index, path_id)) {
        return ContainerAssetBytesSearch::GraphExhausted;
    }
    if visited.len() >= budget {
        return ContainerAssetBytesSearch::BudgetExceeded;
    }
    visited.insert((asset_index, path_id));
    let Some(asset) = env.assets.get(asset_index) else {
        return ContainerAssetBytesSearch::GraphExhausted;
    };
    let Some(info) = asset.objects.get(&path_id) else {
        return ContainerAssetBytesSearch::GraphExhausted;
    };
    let object_type = asset.object_type_name(info);
    let Ok(body) = asset.read_object(asset_index, info) else {
        return ContainerAssetBytesSearch::GraphExhausted;
    };
    if let Some(bytes) = object_payload_bytes(&body) {
        return ContainerAssetBytesSearch::Found(ContainerAssetBytesResolution {
            bytes,
            asset_index,
            path_id,
            object_type,
        });
    }
    let mut pointers = Vec::new();
    collect_value_pointers(&body, &mut pointers);
    for pointer in pointers {
        let Ok(key) = env.resolve_pointer(&pointer) else {
            continue;
        };
        match container_asset_bytes_with_source_budget(env, key.asset, key.path_id, visited, budget)
        {
            ContainerAssetBytesSearch::Found(resolution) => {
                return ContainerAssetBytesSearch::Found(resolution);
            }
            ContainerAssetBytesSearch::BudgetExceeded => {
                return ContainerAssetBytesSearch::BudgetExceeded;
            }
            ContainerAssetBytesSearch::GraphExhausted => {}
        }
    }
    ContainerAssetBytesSearch::GraphExhausted
}
