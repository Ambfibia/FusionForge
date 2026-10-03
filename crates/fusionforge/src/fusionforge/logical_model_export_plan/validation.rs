use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PhysicalOccurrenceResolutionError {
    pub(super) code_suffix: &'static str,
    pub(super) detail: String,
}

impl PhysicalOccurrenceResolutionError {
    pub(super) fn new(code_suffix: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code_suffix,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KfmResolveError {
    pub code: String,
    pub detail: String,
    pub pointer_traversal: Option<KfmPointerTraversalEvidence>,
    pub preload_ownership: Option<KfmPreloadOwnershipEvidence>,
}

impl KfmResolveError {
    pub fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
            pointer_traversal: None,
            preload_ownership: None,
        }
    }

    pub(super) fn with_pointer_traversal(mut self, evidence: KfmPointerTraversalEvidence) -> Self {
        self.pointer_traversal = Some(evidence);
        self
    }

    pub(super) fn with_preload_ownership(mut self, evidence: KfmPreloadOwnershipEvidence) -> Self {
        self.preload_ownership = Some(evidence);
        self
    }
}

pub(super) fn validate_sparse_preload_hierarchy_membership(
    preload_game_objects: &BTreeSet<(usize, i64)>,
    hierarchy_game_objects: &BTreeSet<(usize, i64)>,
    preload_transforms: &BTreeSet<(usize, i64)>,
    hierarchy_transforms: &BTreeSet<(usize, i64)>,
) -> Result<(), String> {
    let foreign_game_objects = preload_game_objects
        .difference(hierarchy_game_objects)
        .copied()
        .collect::<Vec<_>>();
    if !foreign_game_objects.is_empty() {
        return Err(format!(
            "exact preload contains {} GameObjects outside the root hierarchy: {:?}",
            foreign_game_objects.len(),
            foreign_game_objects.into_iter().take(8).collect::<Vec<_>>()
        ));
    }

    let foreign_transforms = preload_transforms
        .difference(hierarchy_transforms)
        .copied()
        .collect::<Vec<_>>();
    if !foreign_transforms.is_empty() {
        return Err(format!(
            "exact preload contains {} Transforms outside the root hierarchy: {:?}",
            foreign_transforms.len(),
            foreign_transforms.into_iter().take(8).collect::<Vec<_>>()
        ));
    }

    Ok(())
}
