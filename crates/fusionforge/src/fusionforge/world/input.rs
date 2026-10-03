use super::*;

pub(super) fn parse_signed_decimal_component(value: &str) -> Option<i32> {
    let negative = value.starts_with('-');
    let digits = if negative { &value[1..] } else { value };
    if digits.is_empty() {
        return None;
    }
    let parsed = digits.parse::<i32>().ok()?;
    Some(if negative { -parsed } else { parsed })
}

pub(super) fn find_file_recursive(root: &Path, name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, name) {
                return Some(found);
            }
        } else if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(name))
        {
            return Some(path);
        }
    }
    None
}

pub(super) fn collect_neighbor_map_bundles(
    map_path: Option<&Path>,
    build_root: Option<&Path>,
    radius: u8,
) -> Vec<PathBuf> {
    let Some(map_path) = map_path else {
        return Vec::new();
    };
    let Some((center_x, center_y, x_width, y_width)) = parse_map_bundle_name(map_path) else {
        return Vec::new();
    };
    let mut neighbors = Vec::new();
    let mut seen = HashSet::from([normalize_path(map_path)]);
    let radius = radius as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx == 0 && dy == 0 {
                continue;
            }
            let x = center_x + dx;
            let y = center_y + dy;
            let tile_id = map_tile_id(x, y, x_width, y_width);
            let Some(candidate) = find_map_bundle(&tile_id, map_path, build_root) else {
                continue;
            };
            if seen.insert(normalize_path(&candidate)) {
                neighbors.push(candidate);
            }
        }
    }
    neighbors
}
