use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourcePreflightDocument {
    pub(super) schema: String,
    pub(super) logical_name: String,
    pub(super) model_hierarchy: PreflightHierarchy,
    #[serde(default)]
    pub(super) textures: BTreeMap<String, PreflightTexture>,
    #[serde(default)]
    pub(super) materials: BTreeMap<String, Value>,
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

#[derive(Debug, Default)]
pub(super) struct ArrayLength(pub(super) usize);

impl<'de> Deserialize<'de> for ArrayLength {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LengthVisitor;

        impl<'de> Visitor<'de> for LengthVisitor {
            type Value = ArrayLength;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an array")
            }

            fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut length = 0usize;
                while sequence.next_element::<IgnoredAny>()?.is_some() {
                    length = length
                        .checked_add(1)
                        .ok_or_else(|| serde::de::Error::custom("array length overflow"))?;
                }
                Ok(ArrayLength(length))
            }
        }

        deserializer.deserialize_seq(LengthVisitor)
    }
}

pub(super) struct BatchPlan {
    pub(super) source: PathBuf,
    pub(super) source_relative: String,
    pub(super) source_sha256: String,
    pub(super) family: String,
    pub(super) semantic_directories: Vec<String>,
    pub(super) logical_name: String,
    pub(super) output_glb: PathBuf,
    pub(super) output_files: Vec<PathBuf>,
    pub(super) blocker: Option<LogicalModelBatchBlocker>,
}

pub(super) struct BatchCoordinateEvidence {
    pub(super) coordinate_status: String,
    pub(super) runtime_spawn_policy: String,
    pub(super) skinning_basis_parity_status: String,
    pub(super) skinning_basis_parity_max_error: Option<f64>,
    pub(super) current_pose_bind_identity_deviation_max: Option<f64>,
}

pub(super) struct BatchStagingDirectory {
    pub(super) path: PathBuf,
    pub(super) published: bool,
}

impl BatchStagingDirectory {
    pub(super) fn create(output: &Path) -> Result<Self> {
        let parent = output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
        let name = output
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty() && !matches!(*value, "." | ".."))
            .ok_or_else(|| PipelineError::InvalidOutputPath(output.to_path_buf()))?;
        for _ in 0..64 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{name}.ffone-logical-model-batch.{}.{}",
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
        if fs::symlink_metadata(output).is_ok() {
            return Err(PipelineError::OutputExists(output.to_path_buf()));
        }
        fs::rename(&self.path, output).map_err(|error| io_at(output, error))?;
        self.published = true;
        Ok(())
    }
}

impl Drop for BatchStagingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
