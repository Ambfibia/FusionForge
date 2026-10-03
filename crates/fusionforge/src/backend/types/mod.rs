use super::super::*;

pub(in super::super) type EditorResult<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Vec3 {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct WaterVolume {
    pub(in super::super) id: String,
    pub(in super::super) name: String,
    pub(in super::super) mode: String,
    pub(in super::super) script_name: Option<String>,
    pub(in super::super) position: Vec3,
    pub(in super::super) rotation: Vec3,
    pub(in super::super) scale: Vec3,
    pub(in super::super) surface_material: Option<String>,
    pub(in super::super) volume_collider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TileDocument {
    pub(in super::super) schema_version: u32,
    pub(in super::super) tile_id: String,
    pub(in super::super) source_map_bundle: Option<String>,
    pub(in super::super) source_resource_bundle: Option<String>,
    pub(in super::super) terrain: TerrainDocument,
    pub(in super::super) objects: Vec<SceneObject>,
    pub(in super::super) colliders: Vec<ColliderObject>,
    pub(in super::super) water_volumes: Vec<WaterVolume>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ClientExtractedFile {
    pub(in super::super) name: String,
    pub(in super::super) path: String,
    pub(in super::super) size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ClientFontRecord {
    pub(in super::super) asset: String,
    pub(in super::super) container: String,
    pub(in super::super) bundle_path: String,
    pub(in super::super) cache_dir: String,
    pub(in super::super) path_id: i64,
    pub(in super::super) name: String,
    pub(in super::super) family: String,
    pub(in super::super) line_spacing: Option<f64>,
    pub(in super::super) character_count: usize,
    pub(in super::super) has_russian: bool,
    pub(in super::super) texture_path_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ScriptAssemblyRecord {
    pub(in super::super) path: String,
    pub(in super::super) name: String,
    pub(in super::super) size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ScriptDecompileResult {
    pub(in super::super) assembly: String,
    pub(in super::super) output_dir: String,
    pub(in super::super) backend_project: String,
    pub(in super::super) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TableDataInspection {
    pub(in super::super) bundle_path: String,
    pub(in super::super) cache_dir: String,
    pub(in super::super) assets: Vec<TableDataAssetSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TableDataSectionSummary {
    pub(in super::super) path: String,
    pub(in super::super) row_count: usize,
    pub(in super::super) field_count: usize,
    pub(in super::super) fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcIconRecord {
    pub(in super::super) index: usize,
    pub(in super::super) icon_type: i64,
    pub(in super::super) icon_number: i64,
    pub(in super::super) asset_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcBarkerRecord {
    pub(in super::super) id: usize,
    pub(in super::super) name: Option<String>,
    pub(in super::super) comment: Option<String>,
    pub(in super::super) comment1: Option<String>,
    pub(in super::super) comment2: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcBlueprint {
    pub(in super::super) npc_id: i64,
    pub(in super::super) template_npc_id: Option<i64>,
    pub(in super::super) name: String,
    pub(in super::super) internal_name: String,
    pub(in super::super) comment: Option<String>,
    pub(in super::super) comment1: Option<String>,
    pub(in super::super) greeting_name: Option<String>,
    pub(in super::super) greeting_comment: Option<String>,
    pub(in super::super) greeting_comment1: Option<String>,
    pub(in super::super) greeting_key: Option<String>,
    #[serde(default)]
    pub(in super::super) authoring_model_path: Option<String>,
    pub(in super::super) model_bundle: Option<String>,
    pub(in super::super) model_asset: Option<String>,
    pub(in super::super) texture_bundle: Option<String>,
    pub(in super::super) texture_asset: Option<String>,
    pub(in super::super) icon_bundle: Option<String>,
    pub(in super::super) icon_asset: Option<String>,
    pub(in super::super) audio_source: Option<String>,
    pub(in super::super) audio_prefix: Option<String>,
    pub(in super::super) animation_set: Option<String>,
    #[serde(default)]
    pub(in super::super) generated_icon: Option<NpcGeneratedIcon>,
    #[serde(default)]
    pub(in super::super) profile: BTreeMap<String, JsonValue>,
    pub(in super::super) spawn_map: Option<String>,
    pub(in super::super) spawn_position: Option<Vec3>,
    pub(in super::super) spawn_angle: Option<i64>,
    pub(in super::super) spawn_json_id: Option<i64>,
    pub(in super::super) notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NpcGeneratedIconCamera {
    pub(crate) position: Vec3,
    pub(crate) target: Vec3,
    pub(crate) fov: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NpcGeneratedIcon {
    pub(crate) file: String,
    pub(crate) asset_path: String,
    pub(crate) template_asset_path: String,
    pub(crate) size: u32,
    pub(crate) camera: Option<NpcGeneratedIconCamera>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NpcGeneratedIconPlan {
    pub(crate) npc_id: usize,
    pub(crate) icon_type: i64,
    pub(crate) icon_number: i64,
    pub(crate) icon_row_index: usize,
    pub(crate) asset_path: String,
    pub(crate) relative_file: String,
    pub(crate) template_asset_path: String,
    pub(crate) icon_bundle: String,
    pub(crate) size: u32,
    pub(crate) cloned_shared_row: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NpcGeneratedIconAttachResult {
    pub(crate) manifest_path: String,
    pub(crate) table_data_patch_path: String,
    pub(crate) generated_icon: NpcGeneratedIcon,
    pub(crate) icon_type: i64,
    pub(crate) icon_number: i64,
    pub(crate) icon_row_index: usize,
    pub(crate) cloned_shared_row: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcIconPreview {
    pub(in super::super) icon_path: String,
    pub(in super::super) bundle_path: String,
    pub(in super::super) container_path: Option<String>,
    pub(in super::super) asset: Option<String>,
    pub(in super::super) path_id: Option<i64>,
    pub(in super::super) name: Option<String>,
    pub(in super::super) width: Option<i64>,
    pub(in super::super) height: Option<i64>,
    pub(in super::super) data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct NpcSourceBuildInspection {
    pub(in super::super) index: ClientFileIndex,
    pub(in super::super) table_data_bundle: String,
    pub(in super::super) catalog: NpcCatalogInspection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct CacheMetadata {
    pub(in super::super) source_path: String,
    pub(in super::super) source_size: u64,
    pub(in super::super) source_modified_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct BuildSourceFileSnapshot {
    pub(in super::super) size: u64,
    pub(in super::super) modified_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct BuildSourceSnapshot {
    pub(in super::super) format: String,
    pub(in super::super) source_dir: String,
    pub(in super::super) files: BTreeMap<String, BuildSourceFileSnapshot>,
    #[serde(default)]
    pub(in super::super) patched_files: BTreeSet<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(in super::super) struct PreviewSemanticDedup {
    pub(in super::super) meshes: usize,
    pub(in super::super) animations: usize,
    pub(in super::super) joints: usize,
}

#[derive(Clone)]
pub(in super::super) struct ExactBatchEnvironment {
    pub(in super::super) bundle_key: String,
    pub(in super::super) project_key: String,
    pub(in super::super) extract_dir: PathBuf,
    pub(in super::super) environment: Rc<fusionforge::UnityEnvironment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) struct NpcGeneratedIconBinding {
    pub(in super::super) row_index: usize,
    pub(in super::super) icon_number: i64,
    pub(in super::super) cloned_shared_row: bool,
}

#[derive(Debug, Clone)]
pub(in super::super) struct NpcTargetExternalRef {
    pub(in super::super) asset_name: String,
    pub(in super::super) path_id: i64,
    pub(in super::super) object_type: String,
    pub(in super::super) object_name: String,
}

pub(in super::super) struct NativeBuildTempDir {
    pub(in super::super) root: PathBuf,
    pub(in super::super) extracted: PathBuf,
}

impl NativeBuildTempDir {
    pub(in super::super) fn path(&self) -> &Path {
        &self.extracted
    }
}

impl Drop for NativeBuildTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug)]
pub(in super::super) struct LegacyLayoutInputFile {
    pub(in super::super) path: PathBuf,
    pub(in super::super) labels: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub(in super::super) struct LegacyLayoutInputFiles {
    pub(in super::super) by_canonical_path: BTreeMap<String, LegacyLayoutInputFile>,
}

impl LegacyLayoutInputFiles {
    pub(in super::super) fn add_file(&mut self, label: impl Into<String>, path: &Path) -> Result<(), String> {
        if !path.is_file() {
            return Err(format!(
                "legacy layout cache input is not a file: {}",
                path.display()
            ));
        }
        let canonical = path
            .canonicalize()
            .map_err(|err| format!("{}: {err}", path.display()))?;
        let key = canonical
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        let entry = self
            .by_canonical_path
            .entry(key)
            .or_insert_with(|| LegacyLayoutInputFile {
                path: canonical,
                labels: BTreeSet::new(),
            });
        entry.labels.insert(label.into());
        Ok(())
    }

    pub(in super::super) fn add_tree(&mut self, label: &str, root: &Path) -> Result<(), String> {
        if !root.exists() {
            return Ok(());
        }
        let mut files = Vec::new();
        collect_existing_files_recursive(root, &mut files)?;
        files.sort();
        for file in files {
            let relative = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            self.add_file(format!("{label}/{relative}"), &file)?;
        }
        Ok(())
    }

    pub(in super::super) fn add_manifest_entries(
        &mut self,
        label: &str,
        base_dir: &Path,
        manifest_path: &Path,
        entries: &[JsonValue],
    ) -> Result<(), String> {
        if manifest_path.is_file() {
            self.add_file(format!("{label}/manifest.json"), manifest_path)?;
        }
        for (index, entry) in entries.iter().enumerate() {
            let Some(file) = entry
                .get("file")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let path = PathBuf::from(file);
            let path = if path.is_absolute() {
                path
            } else {
                base_dir.join(path)
            };
            self.add_file(
                format!("{label}/entry-{index}/{}", file.replace('\\', "/")),
                &path,
            )?;
        }
        Ok(())
    }

    pub(in super::super) fn digest(self, patch_config: &JsonValue) -> Result<String, String> {
        let mut entries = self.by_canonical_path.into_values().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.labels.cmp(&right.labels));
        let mut hasher = Sha256::new();
        update_layout_input_field(&mut hasher, LEGACY_LAYOUT_INPUT_FORMAT.as_bytes());
        update_layout_input_field(
            &mut hasher,
            &serde_json::to_vec(patch_config).map_err(|err| err.to_string())?,
        );
        let mut previous_labels = None::<BTreeSet<String>>;
        for entry in entries {
            if previous_labels.as_ref() == Some(&entry.labels) {
                return Err(format!(
                    "legacy layout cache inputs have duplicate logical labels: {}",
                    entry.labels.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            previous_labels = Some(entry.labels.clone());
            let labels = serde_json::to_vec(&entry.labels).map_err(|err| err.to_string())?;
            let (bytes, digest) = sha256_file_content(&entry.path)?;
            update_layout_input_field(&mut hasher, &labels);
            hasher.update(bytes.to_le_bytes());
            update_layout_input_field(&mut hasher, digest.as_bytes());
        }
        Ok(format!("{:x}", hasher.finalize()))
    }
}
