use super::*;

pub(super) fn parse_string_matrix(matrix: &[[String; 4]; 4]) -> Result<[[f64; 4]; 4]> {
    let mut parsed = [[0.0; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            parsed[row][column] = matrix[row][column]
                .parse::<f64>()
                .map_err(|_| invalid_error("catalog matrix contains invalid f64"))?;
        }
    }
    Ok(parsed)
}

pub(super) fn parse_tile_grid(tile_id: &str) -> Result<[i32; 2]> {
    let coordinates = tile_id
        .strip_prefix("map_")
        .ok_or_else(|| invalid_error(format!("map tile has no map_ prefix: {tile_id:?}")))?;
    let (column, row) = coordinates
        .split_once('_')
        .ok_or_else(|| invalid_error(format!("map tile has no coordinate pair: {tile_id:?}")))?;
    let column = column
        .parse::<i32>()
        .map_err(|_| invalid_error(format!("invalid map tile column: {tile_id:?}")))?;
    let row = row
        .parse::<i32>()
        .map_err(|_| invalid_error(format!("invalid map tile row: {tile_id:?}")))?;
    Ok([column, row])
}

pub(super) fn read_vec3_accessor(
    binary: &[u8],
    document: &JsonValue,
    accessor_index: usize,
    path: &str,
) -> Result<Vec<[f32; 3]>> {
    let layout = accessor_layout(document, accessor_index, path)?;
    let mut values = Vec::with_capacity(layout.count);
    for index in 0..layout.count {
        let offset = layout.offset + index * layout.stride;
        values.push([
            read_f32(binary, offset, path)?,
            read_f32(binary, offset + 4, path)?,
            read_f32(binary, offset + 8, path)?,
        ]);
    }
    Ok(values)
}

pub(super) fn discover_tutorial_metadata_root(project_root: &Path, asset_root: &Path) -> Result<PathBuf> {
    let active = asset_root.join("world/tutorial/static/tiles");
    if active.is_dir() {
        return canonical_directory(&active, "active tutorial metadata root");
    }
    let revisions = project_root.join(format!(
        "../FusionForge/work/ffone/migration-archive/{SOURCE_BUILD}/conversion-metadata/revisions"
    ));
    let mut candidates = Vec::<(usize, PathBuf)>::new();
    for entry in fs::read_dir(&revisions).map_err(|error| io_at(&revisions, error))? {
        let entry = entry.map_err(|error| io_at(&revisions, error))?;
        let candidate = entry.path().join("files/world/tutorial/static/tiles");
        if !candidate.is_dir() {
            continue;
        }
        let count = fs::read_dir(&candidate)
            .map_err(|error| io_at(&candidate, error))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().join("hierarchy.json").is_file()
                    && entry.path().join("materials.json").is_file()
            })
            .count();
        candidates.push((count, candidate));
    }
    candidates.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let Some((count, candidate)) = candidates.into_iter().next() else {
        return invalid("no archived tutorial static metadata root is available");
    };
    if count == 0 {
        return invalid("archived tutorial static metadata has no complete tiles");
    }
    canonical_directory(&candidate, "archived tutorial metadata root")
}

pub(super) fn collect_files(root: &Path) -> Result<Vec<(String, u64, String)>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                let bytes = read_regular_file(&path, "organized map artifact")?;
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| invalid_error("organized artifact escaped root"))?;
                files.push((slash_path(relative), bytes.len() as u64, hash_bytes(&bytes)));
            }
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

pub(super) fn read_regular_file(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} is not a regular file: {}", path.display()));
    }
    fs::read(path).map_err(|error| io_at(path, error))
}

pub(super) fn read_f32(bytes: &[u8], offset: usize, path: &str) -> Result<f32> {
    bytes
        .get(offset..offset + 4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("fixed slice")))
        .ok_or_else(|| invalid_error(format!("{path:?} truncated f32 at {offset}")))
}
