use super::*;

pub const CHARACTER_CREATION_ROOT: &str = "data/character_creation";

pub const CHARACTER_CREATION_NAME_WHEEL_SCHEMA: &str = "ffone.character-creation.name-wheel.v1";

pub const CHARACTER_CREATION_APPEARANCE_SCHEMA: &str = "ffone.character-creation.appearance.v1";

pub const CHARACTER_CREATION_AVATAR_ITEMS_SCHEMA: &str = "ffone.character-creation.avatar-items.v1";

pub(super) const PROTOCOL_0104: u16 = 104;

pub(super) const TABLE_SET_SCHEMA: &str = "ffone.table-set.v1";

pub(super) const EXPECTED_FIRST_NAMES: usize = 601;

pub(super) const EXPECTED_MIDDLE_NAMES: usize = 601;

pub(super) const EXPECTED_LAST_NAMES: usize = 602;

pub(super) const EXPECTED_CREATION_ROWS: usize = 32;

pub(super) const AVATAR_ITEM_CATEGORIES: [AvatarItemCategory; 10] = [
    AvatarItemCategory::Back,
    AvatarItemCategory::Glasses,
    AvatarItemCategory::Hat,
    AvatarItemCategory::Head,
    AvatarItemCategory::Face,
    AvatarItemCategory::Pants,
    AvatarItemCategory::Shirt,
    AvatarItemCategory::Shoes,
    AvatarItemCategory::Vehicle,
    AvatarItemCategory::Weapon,
];
