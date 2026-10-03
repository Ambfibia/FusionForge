use super::*;

pub(super) fn scene_asset_belongs_to_map(asset_name: &str, map: &str) -> bool {
    asset_name
        .strip_prefix("BuildPlayer-")
        .and_then(|value| value.split(['#', '.']).next())
        .is_some_and(|value| value.eq_ignore_ascii_case(map))
}

pub(super) fn readable_absolute_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}
