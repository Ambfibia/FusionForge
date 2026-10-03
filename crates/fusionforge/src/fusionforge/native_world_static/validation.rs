use super::*;

pub(super) fn reject_links_recursive(root: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|err| format!("could not inspect output root {}: {err}", root.display()))?;
    if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
        return Err(format!(
            "output root is a symlink/reparse point: {}",
            root.display()
        ));
    }
    let _ = collect_regular_files(root)?;
    Ok(())
}

pub(super) fn validate_fresh_output(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!(
            "fresh output root must be absolute: {}",
            path.display()
        ));
    }
    if path.file_name().is_none() {
        return Err(format!(
            "fresh output root must name a directory below a parent: {}",
            path.display()
        ));
    }
    if fs::symlink_metadata(path).is_ok() {
        return Err(format!(
            "fresh output root already exists: {}",
            path.display()
        ));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                current.push(component.as_os_str());
            }
            Component::CurDir | Component::ParentDir => {
                return Err(format!(
                    "fresh output root contains a relative traversal component: {}",
                    path.display()
                ));
            }
        }
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            continue;
        };
        if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
            return Err(format!(
                "fresh output path traverses a symlink/reparse point: {}",
                current.display()
            ));
        }
        if current != path && !metadata.is_dir() {
            return Err(format!(
                "fresh output path traverses a non-directory: {}",
                current.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_relative_path(path: &str, expected_extension: Option<&str>) -> Result<(), String> {
    if path.is_empty()
        || path.contains('\\')
        || path.contains('\0')
        || path.starts_with('/')
        || path.ends_with('/')
    {
        return Err(format!("invalid canonical relative path {path:?}"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "relative path contains a root/traversal component: {path:?}"
        ));
    }
    for component in path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains(':')
            || component.ends_with(' ')
            || component.ends_with('.')
            || component.chars().any(char::is_control)
        {
            return Err(format!("unsafe relative path component in {path:?}"));
        }
        let stem = component
            .split('.')
            .next()
            .unwrap_or(component)
            .to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        ) {
            return Err(format!(
                "Windows-reserved relative path component in {path:?}"
            ));
        }
    }
    if let Some(expected) = expected_extension {
        if parsed.extension().and_then(|value| value.to_str()) != Some(expected) {
            return Err(format!(
                "relative path {path:?} does not have exact .{expected} extension"
            ));
        }
    }
    Ok(())
}
