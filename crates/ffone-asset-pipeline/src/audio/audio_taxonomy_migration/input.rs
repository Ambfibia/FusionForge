use super::*;

pub(super) fn discover_recovery_oggs(root: &Path) -> Result<Vec<RecoveryOgg>> {
    fn visit(root: &Path, current: &Path, output: &mut Vec<RecoveryOgg>) -> Result<()> {
        let mut entries = fs::read_dir(current)
            .map_err(|source| io_at(current, source))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|source| io_at(current, source))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| io_at(&path, source))?;
            if file_type.is_dir() {
                visit(root, &path, output)?;
                continue;
            }
            if !file_type.is_file()
                || !path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("ogg"))
            {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    invalid_error(format!("non-UTF-8 recovery OGG: {}", path.display()))
                })?;
            let (_, true_name) = parse_named_ogg_stem(stem).ok_or_else(|| {
                invalid_error(format!(
                    "recovery OGG must be named <pathId>__<TrueName>.ogg: {}",
                    path.display()
                ))
            })?;
            let (bytes, blake3) = inspect_ogg(&path)?;
            let packet_blake3 = ogg_packet_hash(&path)?;
            output.push(RecoveryOgg {
                absolute_path: path.clone(),
                relative_path: portable_relative(root, &path)?,
                true_name,
                bytes,
                blake3,
                packet_blake3,
            });
        }
        Ok(())
    }
    let mut output = Vec::new();
    visit(root, root, &mut output)?;
    output.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(output)
}

pub(super) fn parse_named_ogg_stem(stem: &str) -> Option<(u64, String)> {
    let (path_id, true_name) = stem.split_once("__")?;
    let path_id = path_id.parse().ok()?;
    Some((path_id, true_name.to_owned()))
}

pub(super) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).map_err(|source| io_at(path, source))?;
    serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })
}
