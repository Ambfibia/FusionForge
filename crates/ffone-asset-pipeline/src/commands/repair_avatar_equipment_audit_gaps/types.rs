use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct WhiteSlotRepair {
    pub(super) category: AvatarItemCategory,
    pub(super) item_number: u32,
    pub(super) donor_texture_true_name: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RepairGender {
    Male,
    Female,
}

impl RepairGender {
    pub(super) const ALL: [Self; 2] = [Self::Male, Self::Female];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Male => "male",
            Self::Female => "female",
        }
    }
}
