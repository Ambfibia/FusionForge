use super::*;

pub(super) fn write_atomic_create_new(path: &Path, report: &SemanticAssetOrganizationReport) -> Result<()> {
    if path.exists() {
        return Err(PipelineError::OutputExists(path.to_owned()));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    let mut bytes = serde_json::to_vec_pretty(report).map_err(|source| PipelineError::Json {
        path: path.to_string_lossy().into_owned(),
        source,
    })?;
    bytes.push(b'\n');
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| PipelineError::InvalidOutputPath(path.to_owned()))?;
    let mut staging = None;
    for counter in 0..1_000_u32 {
        let candidate = parent.join(format!(
            ".{filename}.{}.{}.tmp",
            std::process::id(),
            counter
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                staging = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_at(&candidate, error)),
        }
    }
    let Some((staging_path, mut file)) = staging else {
        return Err(PipelineError::StagingCollision(path.to_owned()));
    };
    let result = (|| {
        file.write_all(&bytes)
            .map_err(|error| io_at(&staging_path, error))?;
        file.sync_all()
            .map_err(|error| io_at(&staging_path, error))?;
        drop(file);
        fs::hard_link(&staging_path, path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                PipelineError::OutputExists(path.to_owned())
            } else {
                io_at(path, error)
            }
        })?;
        Ok(())
    })();
    let cleanup = fs::remove_file(&staging_path);
    if let Err(error) = result {
        return Err(error);
    }
    cleanup.map_err(|error| io_at(&staging_path, error))
}
