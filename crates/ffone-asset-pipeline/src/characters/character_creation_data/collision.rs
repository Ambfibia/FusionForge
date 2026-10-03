
pub(super) fn strip_collision_suffix(stem: &str) -> &str {
    let Some((prefix, suffix)) = stem.rsplit_once("--") else {
        return stem;
    };
    if suffix.len() == 16 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        prefix
    } else {
        stem
    }
}
