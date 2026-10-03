use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileEvidence {
    pub path: String,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentSourceCounts {
    pub(super) equipment_entities: u64,
    pub(super) entities_with_model_routes: u64,
    pub(super) unique_routes: u64,
    pub(super) resolved_routes: u64,
    pub(super) selected_resolved_routes: u64,
    pub(super) exported_physical_models: u64,
    pub(super) total_blockers: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EquipmentSourceBlocker {
    pub(super) code: String,
    pub(super) category: String,
    pub(super) exact_route: String,
    pub(super) normalized_route: String,
    #[serde(default)]
    pub(super) table_rows: Vec<Value>,
    #[serde(default)]
    pub(super) owners: Vec<Value>,
    pub(super) detail: String,
    pub(super) evidence: Value,
    #[serde(default)]
    pub(super) required_evidence: Vec<String>,
    pub(super) disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourcePreflightDocument {
    pub(super) schema: String,
    pub(super) logical_name: String,
    pub(super) model_hierarchy: PreflightHierarchy,
    #[serde(default)]
    pub(super) textures: BTreeMap<String, PreflightTexture>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PreflightHierarchy {
    pub(super) roots: Vec<PreflightRoot>,
    pub(super) nodes: Vec<PreflightNode>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PreflightRoot {
    pub(super) name: String,
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct PreflightNode {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) parent: Option<String>,
}

pub(super) struct EquipmentPlan {
    pub(super) export: EquipmentSourceExport,
    pub(super) source: PathBuf,
    pub(super) output_glb: PathBuf,
    pub(super) output_files: Vec<PathBuf>,
}

pub(super) struct PreflightResult {
    pub(super) source_manifest_path: PathBuf,
    pub(super) source_manifest_bytes: Vec<u8>,
    pub(super) source_manifest: EquipmentSourceManifest,
    pub(super) plans: Vec<EquipmentPlan>,
    pub(super) collision_blockers: BTreeMap<usize, EquipmentLogicalModelBlocker>,
    pub(super) upstream_blockers: Vec<EquipmentLogicalModelBlocker>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct OutputClaim {
    pub(super) owner: Option<usize>,
    pub(super) exact: String,
    pub(super) kind: &'static str,
}

pub(super) struct EquipmentBatchStagingDirectory {
    pub(super) path: PathBuf,
    pub(super) published: bool,
}

impl EquipmentBatchStagingDirectory {
    pub(super) fn create(output: &Path) -> Result<Self> {
        let parent = output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = output
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty() && !matches!(*value, "." | ".."))
            .ok_or_else(|| PipelineError::InvalidOutputPath(output.to_path_buf()))?;
        for _ in 0..64 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{name}.ffone-equipment-logical-model-batch.{}.{}",
                std::process::id(),
                sequence
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_at(path, error)),
            }
        }
        Err(PipelineError::StagingCollision(output.to_path_buf()))
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn publish(&mut self, output: &Path) -> Result<()> {
        reject_existing_output(output)?;
        const DELAYS: [u64; 7] = [25, 50, 100, 200, 400, 800, 1_600];
        for delay in DELAYS {
            match fs::rename(&self.path, output) {
                Ok(()) => {
                    self.published = true;
                    return Ok(());
                }
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    thread::sleep(Duration::from_millis(delay));
                }
                Err(error) => return Err(io_at(output, error)),
            }
        }
        fs::rename(&self.path, output).map_err(|error| io_at(output, error))?;
        self.published = true;
        Ok(())
    }
}

impl Drop for EquipmentBatchStagingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
