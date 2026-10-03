use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialPropPromotionMode {
    DryRun,
    Apply,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialPropPromotionStatus {
    Promote,
    AlreadyPromoted,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorialPropRuntimeReferenceProof {
    pub source_file: String,
    pub legacy_asset_path: String,
    pub promoted_asset_path: String,
    pub before_sha256: String,
    pub after_sha256: String,
}

pub(super) enum PreparedState {
    Promote(PreparedPromotion, TutorialPropPromotionProof),
    AlreadyPromoted(TutorialPropPromotionProof),
}

pub(super) fn prepare_runtime_reference(
    project_root: &Path,
    candidate: &PromotionCandidate,
    already_promoted: bool,
) -> Result<(PathBuf, Vec<u8>, Vec<u8>, TutorialPropRuntimeReferenceProof)> {
    let path = project_root.join(native_path(candidate.runtime_source));
    let before = read_regular(&path)?;
    let text = std::str::from_utf8(&before).map_err(|_| {
        invalid_error(format!(
            "runtime source is not UTF-8: {:?}",
            candidate.runtime_source
        ))
    })?;
    let legacy = candidate.source_glb();
    let promoted = candidate.target_glb();
    let legacy_count = text.matches(&legacy).count();
    let promoted_count = text.matches(&promoted).count();
    let after = if already_promoted {
        if legacy_count != 0 || promoted_count != 1 {
            return invalid(format!(
                "promoted runtime reference must contain target exactly once and legacy zero times in {:?}",
                candidate.runtime_source
            ));
        }
        before.clone()
    } else {
        if legacy_count != 1 || promoted_count != 0 {
            return invalid(format!(
                "legacy runtime reference must contain source exactly once and target zero times in {:?}",
                candidate.runtime_source
            ));
        }
        text.replacen(&legacy, &promoted, 1).into_bytes()
    };
    let proof = TutorialPropRuntimeReferenceProof {
        source_file: candidate.runtime_source.to_owned(),
        legacy_asset_path: legacy,
        promoted_asset_path: promoted,
        before_sha256: sha256(&before),
        after_sha256: sha256(&after),
    };
    Ok((path, before, after, proof))
}

pub(super) struct RuntimeCommitState {
    pub(super) original: PathBuf,
    pub(super) backup: PathBuf,
    pub(super) staged: PathBuf,
    pub(super) backed_up: bool,
    pub(super) installed: bool,
}
