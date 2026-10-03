use super::*;

#[derive(Default)]
pub(super) struct SourceVisualSelection {
    pub(super) excluded_biped_helper_bindings: BTreeSet<(usize, usize)>,
    pub(super) excluded_mesh_parts: usize,
    pub(super) excluded_source_mesh_ids: BTreeSet<String>,
    pub(super) preserved_skinned_mesh_bindings: usize,
}

impl SourceVisualSelection {
    pub(super) fn includes(&self, mesh_index: usize, binding_index: usize) -> bool {
        !self
            .excluded_biped_helper_bindings
            .contains(&(mesh_index, binding_index))
    }

    pub(super) fn report(&self, source: &SourceDocument) -> Result<SourceGeometryFilterReport> {
        Ok(SourceGeometryFilterReport {
            policy: "exclude-3ds-max-biped-display-meshes-v1".to_owned(),
            reason: "Rigid MeshFilter geometry named `Biped Object` (including exporter collision suffixes `@#N`) is a 3ds Max Biped viewport helper, not game geometry. Its Transform nodes remain in the hierarchy so bones, skin palettes and animation targets are unchanged.".to_owned(),
            excluded_rigid_mesh_bindings: u64_count(
                self.excluded_biped_helper_bindings.len(),
                "excluded Biped helper binding count",
            )?,
            excluded_mesh_parts: u64_count(
                self.excluded_mesh_parts,
                "excluded Biped helper mesh-part count",
            )?,
            excluded_source_mesh_ids: self.excluded_source_mesh_ids.iter().cloned().collect(),
            preserved_transform_nodes: u64_count(
                source.model_hierarchy.nodes.len(),
                "preserved Transform node count",
            )?,
            preserved_skinned_mesh_bindings: u64_count(
                self.preserved_skinned_mesh_bindings,
                "preserved skinned mesh binding count",
            )?,
        })
    }
}
