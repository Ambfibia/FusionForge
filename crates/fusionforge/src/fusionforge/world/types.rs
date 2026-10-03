use super::*;

#[derive(Debug, Clone)]
pub struct WorldInspectOptions {
    pub repo_root: PathBuf,
    pub map_bundle: Option<PathBuf>,
    pub resource_bundle: Option<PathBuf>,
    pub build_root: Option<PathBuf>,
    pub neighbor_radius: u8,
}
