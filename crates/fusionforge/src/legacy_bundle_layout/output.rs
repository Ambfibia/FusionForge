use super::*;

pub(super) fn transactional_publish(out_dir: &Path, staged: &[(String, PathBuf)]) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    for (name, path) in staged {
        if !path.is_file() {
            return Err(format!("staged bundle is absent: {}", path.display()));
        }
        if !unique.insert(name.to_ascii_lowercase()) {
            return Err(format!("duplicate staged output name: {name}"));
        }
    }
    let nonce = sha1_hex(
        staged
            .iter()
            .flat_map(|(name, _)| name.as_bytes().iter().copied())
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let mut entries = Vec::<(PathBuf, PathBuf, PathBuf, bool)>::new();
    for (name, source) in staged {
        let target = out_dir.join(name);
        let pending = out_dir.join(format!(".{name}.fflayout-new-{}", &nonce[..12]));
        let backup = out_dir.join(format!(".{name}.fflayout-old-{}", &nonce[..12]));
        if backup.is_file() {
            if target.is_file() {
                return Err(format!(
                    "stale transaction backup and target both exist: {} and {}; refusing to discard either",
                    backup.display(),
                    target.display()
                ));
            }
            fs::rename(&backup, &target).map_err(|err| {
                format!(
                    "could not recover stale transaction backup {} -> {}: {err}",
                    backup.display(),
                    target.display()
                )
            })?;
        }
        if pending.is_file() {
            fs::remove_file(&pending).map_err(|err| format!("{}: {err}", pending.display()))?;
        }
        fs::copy(source, &pending)
            .map_err(|err| format!("{} -> {}: {err}", source.display(), pending.display()))?;
        entries.push((target, pending, backup, false));
    }

    for index in 0..entries.len() {
        let (target, pending, backup, committed) = &mut entries[index];
        let result = (|| {
            if target.is_file() {
                fs::rename(&*target, &*backup).map_err(|err| {
                    format!("{} -> {}: {err}", target.display(), backup.display())
                })?;
            }
            fs::rename(&*pending, &*target)
                .map_err(|err| format!("{} -> {}: {err}", pending.display(), target.display()))?;
            *committed = true;
            Ok::<(), String>(())
        })();
        if let Err(err) = result {
            for (target, pending, backup, committed) in entries.iter().rev() {
                if *committed {
                    let _ = fs::remove_file(target);
                }
                if backup.is_file() {
                    let _ = fs::rename(backup, target);
                }
                let _ = fs::remove_file(pending);
            }
            return Err(format!("legacy layout transaction rolled back: {err}"));
        }
    }
    for (_, _, backup, _) in &entries {
        if backup.is_file() {
            fs::remove_file(backup).map_err(|err| format!("{}: {err}", backup.display()))?;
        }
    }
    Ok(())
}

pub(super) struct LayoutCacheWriteGuard {
    pub(super) parent: PathBuf,
    pub(super) pending: PathBuf,
    pub(super) backup: PathBuf,
}

impl Drop for LayoutCacheWriteGuard {
    fn drop(&mut self) {
        let _ = remove_layout_cache_dir(&self.pending, &self.parent);
        let _ = remove_layout_cache_dir(&self.backup, &self.parent);
    }
}

pub(super) fn publish_layout_report(
    project: &Path,
    patch_config: &JsonValue,
    report_data: &str,
) -> Result<(), String> {
    let report_path = layout_report_path(project, patch_config);
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let report_pending = report_path.with_extension("json.fflayout-new");
    fs::write(&report_pending, format!("{report_data}\n"))
        .map_err(|err| format!("{}: {err}", report_pending.display()))?;
    if report_path.is_file() {
        fs::remove_file(&report_path).map_err(|err| format!("{}: {err}", report_path.display()))?;
    }
    fs::rename(&report_pending, &report_path).map_err(|err| {
        format!(
            "{} -> {}: {err}",
            report_pending.display(),
            report_path.display()
        )
    })
}
