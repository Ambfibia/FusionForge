//! Disposable navigation storage, independent of legacy patch projects.
use std::path::{Path, PathBuf};

/// Resolve even a not-yet-created destination through its nearest existing ancestor.
pub(super) fn resolve_destination(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|err| err.to_string())?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            other => normalized.push(other),
        }
    }
    let absolute = normalized;
    let mut ancestor = absolute.as_path();
    let mut tail = Vec::new();
    while !ancestor.exists() {
        tail.push(
            ancestor
                .file_name()
                .ok_or("invalid work directory")?
                .to_os_string(),
        );
        ancestor = ancestor
            .parent()
            .ok_or("work directory has no existing ancestor")?;
    }
    let mut resolved = ancestor.canonicalize().map_err(|err| err.to_string())?;
    for component in tail.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

/// New conversion cases are always editor-owned and never reuse partial output.
pub(crate) fn fresh_case_dir(path: &Path) -> Result<PathBuf, String> {
    let root = crate::repository_root().join("work/cases");
    create_fresh_case(&root, path)
}

fn create_fresh_case(root: &Path, path: &Path) -> Result<PathBuf, String> {
    reject_project_work_path(path)?;
    let root = resolve_destination(root)?;
    let path = resolve_destination(path)?;
    if path == root || !path.starts_with(&root) {
        return Err(format!("conversion case must be below {}", root.display()));
    }
    reject_project_work_path(&path)?;
    std::fs::create_dir_all(path.parent().ok_or("case has no parent")?)
        .map_err(|err| err.to_string())?;
    std::fs::create_dir(&path).map_err(|err| {
        format!(
            "conversion requires a fresh case directory {}: {err}",
            path.display()
        )
    })?;
    Ok(path)
}

fn reject_project_work_path(path: &Path) -> Result<(), String> {
    if path.components().any(|part| {
        let name = part.as_os_str().to_string_lossy().to_ascii_lowercase();
        name == ".ffclienteditor" || name.ends_with(".ffclient")
    }) {
        return Err(
            "legacy project directories are retired; use a plain Editor work directory".into(),
        );
    }
    Ok(())
}

pub(crate) fn prepare_work_dir(source: &Path, work: &Path) -> Result<PathBuf, String> {
    reject_project_work_path(work)?;
    let source = source.canonicalize().map_err(|err| err.to_string())?;
    let work = resolve_destination(work)?;
    reject_project_work_path(&work)?;
    if work.starts_with(&source) || source.starts_with(&work) {
        return Err("source and work directories must be separate, non-nested trees".into());
    }
    std::fs::create_dir_all(&work).map_err(|err| format!("{}: {err}", work.display()))?;
    Ok(work)
}

#[cfg(test)]
mod tests;
