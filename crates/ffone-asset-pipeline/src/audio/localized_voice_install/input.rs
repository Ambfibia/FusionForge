use super::*;

pub(super) fn collect_russian_sources(root: &Path) -> Result<Vec<RussianSourceFile>> {
    fn visit(root: &Path, directory: &Path, output: &mut Vec<RussianSourceFile>) -> Result<()> {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| io_at(directory, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(directory, error))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| io_at(&path, error))?;
            if file_type.is_symlink() {
                return invalid(format!(
                    "Russian voice source contains a symlink, which is not reproducible: {}",
                    path.display()
                ));
            }
            if file_type.is_dir() {
                visit(root, &path, output)?;
                continue;
            }
            if !file_type.is_file()
                || !path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("ogg"))
            {
                continue;
            }
            let relative_path = portable_relative(root, &path)?;
            let parsed = parse_russian_source_path(&path);
            output.push(RussianSourceFile {
                absolute_path: path,
                relative_path,
                container: parsed.as_ref().map(|parsed| parsed.0.clone()),
                path_id: parsed.as_ref().map(|parsed| parsed.1),
                true_name: parsed.map(|parsed| parsed.2),
            });
        }
        Ok(())
    }

    let mut output = Vec::new();
    visit(root, root, &mut output)?;
    output.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(output)
}

pub(super) fn parse_ogg_filename(filename: &str) -> Option<(u64, String)> {
    let stem = filename
        .get(..filename.len().checked_sub(4)?)?
        .strip_suffix("")
        .filter(|_| filename[filename.len() - 4..].eq_ignore_ascii_case(".ogg"))?;
    let (path_id, true_name) = stem.split_once("__")?;
    if true_name.is_empty() {
        return None;
    }
    Some((path_id.parse().ok()?, true_name.to_owned()))
}

pub(super) fn parse_provenance_file(file: &str) -> Option<(String, u64, String)> {
    let components = file
        .replace('\\', "/")
        .split('/')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for window in components.windows(2) {
        if folded(&window[0]).starts_with("customassetbundle-") {
            let (path_id, true_name) = parse_ogg_filename(&window[1])?;
            return Some((window[0].clone(), path_id, true_name));
        }
    }
    None
}

pub(super) fn collect_files(root: &Path) -> Result<Vec<String>> {
    fn visit(root: &Path, current: &Path, output: &mut Vec<String>) -> Result<()> {
        let mut entries = fs::read_dir(current)
            .map_err(|error| io_at(current, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(current, error))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| io_at(&path, error))?;
            if kind.is_dir() {
                visit(root, &path, output)?;
            } else if kind.is_file() {
                output.push(portable_relative(root, &path)?);
            } else {
                return invalid(format!(
                    "staging tree contains a non-file entry at {}",
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
