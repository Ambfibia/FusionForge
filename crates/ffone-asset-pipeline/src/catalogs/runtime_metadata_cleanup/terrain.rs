
pub(super) fn proven_conversion_terrain_root(relative: &str) -> Option<String> {
    if let Some(tail) = relative.strip_prefix("world/maps/") {
        let (map_id, tail) = tail.split_once('/')?;
        tail.starts_with("terrain/")
            .then(|| format!("world/maps/{map_id}/terrain"))
    } else if let Some(tail) = relative.strip_prefix("world/tutorial/terrain/tiles/") {
        let (tile_id, _) = tail.split_once('/')?;
        Some(format!("world/tutorial/terrain/tiles/{tile_id}"))
    } else {
        None
    }
}
