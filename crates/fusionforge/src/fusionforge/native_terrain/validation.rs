
pub(super) fn is_transient_windows_rename_error(error: &std::io::Error) -> bool {
    #[cfg(windows)]
    {
        error.kind() == std::io::ErrorKind::PermissionDenied
            || matches!(error.raw_os_error(), Some(5 | 32 | 33))
    }
    #[cfg(not(windows))]
    {
        let _ = error;
        false
    }
}
