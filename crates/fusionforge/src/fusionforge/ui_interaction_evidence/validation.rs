use super::*;

pub(super) fn require_bytes_and_hash(
    label: &str,
    bytes: &[u8],
    expected_bytes: u64,
    expected_sha256: &str,
) -> Result<(), String> {
    if bytes.len() as u64 != expected_bytes {
        return Err(format!(
            "{label} byte count mismatch: expected {expected_bytes}, found {}",
            bytes.len()
        ));
    }
    let expected = normalize_sha256(expected_sha256)?;
    let actual = sha256_hex(bytes);
    if actual != expected {
        return Err(format!(
            "{label} SHA-256 mismatch: expected {expected}, found {actual}"
        ));
    }
    Ok(())
}

pub(super) fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    normalize_sha256(value)
        .map(|_| ())
        .map_err(|error| format!("invalid {label} SHA-256: {error}"))
}
