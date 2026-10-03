use super::*;

pub(super) fn read_file(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|error| io_at(path, error))
}
