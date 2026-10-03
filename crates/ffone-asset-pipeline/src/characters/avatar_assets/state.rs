use super::*;

/// Renderer-relevant subset shared by login and shard style payloads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarStyleSelection {
    pub gender: AvatarGender,
    pub face_style: i8,
    pub hair_style: i8,
    pub hair_color: i8,
    pub skin_color: i8,
    pub eye_color: i8,
    pub height: i8,
    pub body: i8,
}

impl TryFrom<&CharacterStyle0104> for AvatarStyleSelection {
    type Error = String;

    fn try_from(style: &CharacterStyle0104) -> Result<Self, Self::Error> {
        Ok(Self {
            gender: AvatarGender::from_protocol(style.gender)?,
            face_style: style.face_style,
            hair_style: style.hair_style,
            hair_color: style.hair_color,
            skin_color: style.skin_color,
            eye_color: style.eye_color,
            height: style.height,
            body: style.body,
        })
    }
}

impl TryFrom<&PcStyle0104> for AvatarStyleSelection {
    type Error = String;

    fn try_from(style: &PcStyle0104) -> Result<Self, Self::Error> {
        Ok(Self {
            gender: AvatarGender::from_protocol(style.gender)?,
            face_style: style.face_style,
            hair_style: style.hair_style,
            hair_color: style.hair_color,
            skin_color: style.skin_color,
            eye_color: style.eye_color,
            height: style.height,
            body: style.body,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarItemSelection {
    pub item_type: i16,
    pub item_id: i16,
    pub option: i32,
    pub time_limit: i32,
}

impl From<EquippedItem0104> for AvatarItemSelection {
    fn from(item: EquippedItem0104) -> Self {
        Self {
            item_type: item.item_type,
            item_id: item.item_id,
            option: item.option,
            time_limit: item.time_limit,
        }
    }
}

impl From<ItemBase0104> for AvatarItemSelection {
    fn from(item: ItemBase0104) -> Self {
        Self {
            item_type: item.item_type,
            item_id: item.item_id,
            option: item.option,
            time_limit: item.time_limit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeResolutionStatus {
    Verified,
    HashVerifiedCandidateUnproven,
    BlockedProvenance,
    Ambiguous,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarTableSelection {
    pub table: String,
    pub row_index: usize,
    pub item_number: i64,
    pub mesh_index: usize,
    pub equip_type: i64,
}
