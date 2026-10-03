use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WeightChannel {
    pub(super) map_index: usize,
    pub(super) channel_index: usize,
    pub(super) channel: &'static str,
}

pub(super) struct StagingDirectory {
    pub(super) path: PathBuf,
    pub(super) keep: bool,
}

impl StagingDirectory {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
