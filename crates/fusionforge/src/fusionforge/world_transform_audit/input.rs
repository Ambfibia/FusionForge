
pub(super) fn parse_signed_decimal(value: &str) -> Option<i32> {
    let (negative, digits) = value
        .strip_prefix('-')
        .map(|digits| (true, digits))
        .unwrap_or((false, value));
    if digits.is_empty() {
        return None;
    }
    let parsed = digits.parse::<i32>().ok()?;
    Some(if negative { -parsed } else { parsed })
}
