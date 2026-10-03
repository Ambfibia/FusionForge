use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ImportInputProof {
    pub(super) path: String,
    pub(super) blake3: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ImportReportCounts {
    pub(super) discovered_russian_files: u64,
    pub(super) accepted_russian_files: u64,
    pub(super) ignored_russian_files: u64,
    pub(super) localized_keys: u64,
    pub(super) english_only_ambiguous_keys: u64,
    pub(super) quarantined_russian_files: u64,
    pub(super) english_voice_keys: u64,
    pub(super) unresolved_english_keys: u64,
    pub(super) identical_locale_payloads: u64,
}

pub(super) fn copy_and_verify(source: &VoiceSource, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let bytes = fs::copy(&source.absolute_path, target).map_err(|error| io_at(target, error))?;
    let hash = hash_file(target)?;
    if bytes != source.bytes || hash != source.blake3 {
        return invalid(format!(
            "source changed while staging {}",
            source.absolute_path.display()
        ));
    }
    Ok(())
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_at(parent, source))?;
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| io_at(path, source))?;
    file.write_all(bytes)
        .map_err(|source| io_at(path, source))?;
    file.sync_all().map_err(|source| io_at(path, source))
}
