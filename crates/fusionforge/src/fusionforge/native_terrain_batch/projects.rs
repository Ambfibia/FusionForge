use super::*;

#[derive(Debug)]
pub(super) struct SessionDirectory {
    pub(super) path: PathBuf,
    pub(super) keep: bool,
}

impl SessionDirectory {
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

impl Drop for SessionDirectory {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
