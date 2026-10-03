pub(super) const DEFAULT_LOCALE: &str = "en";

pub(super) const RUSSIAN_LOCALE: &str = "ru";

pub(super) fn valid_locale_id(locale: &str) -> bool {
    !locale.is_empty()
        && !locale.starts_with('-')
        && !locale.ends_with('-')
        && !locale.contains("--")
        && locale
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
