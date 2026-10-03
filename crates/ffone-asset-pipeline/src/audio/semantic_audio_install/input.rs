use super::*;

pub(super) fn collect_files(root: &Path) -> Result<Vec<String>> {
    fn visit(root: &Path, current: &Path, output: &mut Vec<String>) -> Result<()> {
        let mut entries = fs::read_dir(current)
            .map_err(|error| io_at(current, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(current, error))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| io_at(&path, error))?;
            if file_type.is_dir() {
                visit(root, &path, output)?;
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| invalid_error("staged file escaped staging root"))?
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                output.push(relative);
            } else {
                return invalid(format!(
                    "staging tree contains a non-file, non-directory entry at {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    let mut output = Vec::new();
    visit(root, root, &mut output)?;
    output.sort();
    Ok(output)
}
