use super::*;

#[derive(Debug)]
pub(super) struct NormalizationPlan {
    pub(super) file_routes: BTreeMap<String, String>,
    pub(super) package_routes: Vec<ObjectPackageRoute>,
    pub(super) members: usize,
    pub(super) textures: usize,
    pub(super) renamed_members: usize,
}
