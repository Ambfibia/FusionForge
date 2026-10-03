use super::*;

pub(super) fn collect_runtime_files(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    for entry in fs::read_dir(current).map_err(|error| io_at(current, error))? {
        let entry = entry.map_err(|error| io_at(current, error))?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|error| io_at(&path, error))?;
        if kind.is_dir() {
            collect_runtime_files(root, &path, files)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid_error("runtime gameplay UI path escaped its root"))?;
            let relative = relative
                .to_str()
                .ok_or_else(|| invalid_error("runtime gameplay UI route is not UTF-8"))?
                .replace('\\', "/");
            let route = format!("{GAMEPLAY_UI_ROOT}/{relative}");
            let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
            if files.insert(route.clone(), bytes).is_some() {
                return invalid(format!("duplicate runtime gameplay UI route {route:?}"));
            }
        } else {
            return invalid(format!(
                "unsupported runtime gameplay UI entry {}",
                path.display()
            ));
        }
    }
    Ok(())
}
