use super::*;

pub const AUDIO_TAXONOMY_MIGRATION_SCHEMA: &str = "ffone.audio-taxonomy-migration.v1";

/// Exact voice clips that the legacy importer could only place in its
/// uncertain shared-SFX fallback. Their serialized names identify Computress,
/// and the byte identities keep this correction closed to the audited source.
pub(super) const PROVEN_COMPUTRESS_DIALOGUE_SFX: &[(&str, &str, &str)] = &[
    (
        "sfx/shared/computress_tagline01",
        "Computress_Tagline01",
        "891306875380a179744ee60dea6883c9ef2bd3583f7c93757082b57c48ee105d",
    ),
    (
        "sfx/shared/computress_upsell02",
        "Computress_Upsell02",
        "083a0c5cbe5a15d4d30d7741083a05dfc3184e0b16b4b7e184c213a57f9385da",
    ),
];

#[derive(Clone, Debug)]
pub struct AudioTaxonomyMigrationOptions {
    pub asset_root: PathBuf,
    pub recovery_nano_root: PathBuf,
    pub apply: bool,
}

impl AudioTaxonomyMigrationOptions {
    #[must_use]
    pub fn new(asset_root: impl Into<PathBuf>, recovery_nano_root: impl Into<PathBuf>) -> Self {
        Self {
            asset_root: asset_root.into(),
            recovery_nano_root: recovery_nano_root.into(),
            apply: false,
        }
    }

    #[must_use]
    pub fn with_apply(mut self, apply: bool) -> Self {
        self.apply = apply;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTaxonomyMigrationReport {
    pub schema: String,
    pub applied: bool,
    pub source_catalog_schema: String,
    pub output_catalog_schema: String,
    pub catalog_assets: u64,
    pub catalog_files: u64,
    pub catalog_file_bytes: u64,
    pub nano_voice_assets: u64,
    pub nano_skill_assets: u64,
    pub computress_voice_assets: u64,
    pub recovery_files_scanned: u64,
    pub recovery_exact_matches: u64,
    pub recovery_voice_added: u64,
    pub recovery_skills_added: u64,
    pub moved_files: u64,
    pub preserved_files: u64,
    pub aliases_added: u64,
    pub normalized_scopes: u64,
    pub normalized_source_paths: u64,
    pub manifest_files: u64,
    pub catalog_path: String,
}

pub fn migrate_audio_taxonomy(
    options: &AudioTaxonomyMigrationOptions,
) -> Result<AudioTaxonomyMigrationReport> {
    let asset_root = canonical_directory(&options.asset_root, "asset root")?;
    let recovery_root = canonical_directory(&options.recovery_nano_root, "Nano recovery root")?;
    let plan = build_plan(&asset_root, &recovery_root)?;
    if !options.apply {
        return Ok(plan.report);
    }
    apply_plan(&asset_root, plan)
}

pub(super) fn strict_voice_prefix(asset: &StrictAudioAsset, locale: &str) -> String {
    format!("audio/voice/{locale}/{}/", asset.owner)
}

pub(super) struct AudioMoveTarget {
    pub(super) source: Option<PathBuf>,
    pub(super) target: PathBuf,
    pub(super) staged: PathBuf,
    pub(super) backup: Option<PathBuf>,
}
