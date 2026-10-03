
pub const GAMEPLAY_UI_LAYOUT: &str = "ui/gameplay/layout/gameplay_hud.json";

pub(super) const LAYOUT_BYTES: &[u8] = include_bytes!("../../../fixtures/ui/gameplay_hud.layout.json");

// The immediately preceding immutable layout revision is accepted only as an
// installer input so an owned installation can be upgraded transactionally.
// Unknown layout bytes still fail closed.
pub(super) const PREVIOUS_LAYOUT_BYTES: &[u8] =
    include_bytes!("../../../fixtures/previous-gameplay-hud-layout-e601142a.json");

#[cfg(test)]
pub(super) const PREVIOUS_LAYOUT_BLAKE3: &str =
    "e601142a09f851b10133ab8dee9f08ed382fc704fb7c1dbe5e5eec82249b5461";

pub(super) fn is_supported_layout_revision(bytes: &[u8]) -> bool {
    bytes == LAYOUT_BYTES || bytes == PREVIOUS_LAYOUT_BYTES
}
