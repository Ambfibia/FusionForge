use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationProvenance {
    pub source_build: String,
    pub table_set: CharacterCreationAssetReference,
    pub player_equipment_catalog: CharacterCreationAssetReference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationNameWheel {
    pub schema: String,
    pub protocol: u16,
    pub provenance: CharacterCreationProvenance,
    pub first_names: Vec<NameWheelEntry>,
    pub middle_names: Vec<NameWheelEntry>,
    pub last_names: Vec<NameWheelEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NameWheelEntry {
    pub code: u16,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterGender {
    Male,
    Female,
}

impl CharacterGender {
    pub const fn protocol_code(self) -> u8 {
        match self {
            Self::Male => 1,
            Self::Female => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterAppearanceCategory {
    Face,
    Hair,
    Shirt,
    Pants,
    Shoes,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationAppearance {
    pub schema: String,
    pub protocol: u16,
    pub provenance: CharacterCreationProvenance,
    pub constraints: CharacterAppearanceConstraints,
    pub color_contract: CharacterColorContract,
    pub texture_rules: CharacterTextureRules,
    pub maxima: CharacterCreationMaxima,
    pub creation_rows: Vec<CharacterCreationRow>,
    pub choices: Vec<CharacterCreationChoice>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterColorContract {
    pub source_asset: String,
    pub source_path_id: i64,
    pub serialized_color_space: String,
    pub runtime_uniform_policy: String,
    pub actor_skin_tint_multiplier: f32,
    pub skin: Vec<CharacterPaletteColor>,
    pub hair: Vec<CharacterPaletteColor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterPaletteColor {
    pub code: u8,
    pub rgba: [f32; 4],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterAppearanceConstraints {
    pub gender_codes: Vec<u8>,
    pub body_codes: Vec<u8>,
    pub height_codes: Vec<u8>,
    pub skin_color_codes: Vec<u8>,
    pub hair_color_codes: Vec<u8>,
    pub eye_color_codes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationMaxima {
    pub male_face: u16,
    pub female_face: u16,
    pub male_hair: u16,
    pub female_hair: u16,
    pub male_shirts: u16,
    pub female_shirts: u16,
    pub male_pants: u16,
    pub female_pants: u16,
    pub male_shoes: u16,
    pub female_shoes: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationRow {
    pub creation_index: u16,
    pub male_face: u32,
    pub female_face: u32,
    pub male_hair: u32,
    pub female_hair: u32,
    pub male_shirt: u32,
    pub female_shirt: u32,
    pub male_pants: u32,
    pub female_pants: u32,
    pub male_shoes: u32,
    pub female_shoes: u32,
    pub weapon: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationChoice {
    pub category: CharacterAppearanceCategory,
    pub gender: CharacterGender,
    pub creation_index: u16,
    pub value: u32,
    pub label: String,
    pub icon: Option<AvatarIconReference>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarItemCategory {
    Back,
    Glasses,
    Hat,
    Head,
    Face,
    Pants,
    Shirt,
    Shoes,
    Vehicle,
    Weapon,
}

impl AvatarItemCategory {
    pub const fn table_name(self) -> &'static str {
        match self {
            Self::Back => "m_pBackItemTable",
            Self::Glasses => "m_pGlassItemTable",
            Self::Hat => "m_pHatItemTable",
            Self::Head => "m_pHeadItemTable",
            Self::Face => "m_pFaceItemTable",
            Self::Pants => "m_pPantsItemTable",
            Self::Shirt => "m_pShirtsItemTable",
            Self::Shoes => "m_pShoesItemTable",
            Self::Vehicle => "m_pVehicleItemTable",
            Self::Weapon => "m_pWeaponItemTable",
        }
    }

    pub const fn equipment_category(self) -> &'static str {
        match self {
            Self::Face => "mask",
            Self::Back => "back",
            Self::Glasses => "glasses",
            Self::Hat => "hat",
            Self::Head => "head",
            Self::Pants => "pants",
            Self::Shirt => "shirt",
            Self::Shoes => "shoes",
            Self::Vehicle => "vehicle",
            Self::Weapon => "weapon",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationAvatarItems {
    pub schema: String,
    pub protocol: u16,
    pub provenance: CharacterCreationProvenance,
    pub lookup_complete: bool,
    pub counts: CharacterCreationAvatarItemCounts,
    pub items: Vec<AvatarItemLookup>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterCreationAvatarItemCounts {
    pub categories: u64,
    pub items: u64,
    pub model_references: u64,
    pub resolved_models: u64,
    pub texture_references: u64,
    pub resolved_textures: u64,
    pub icon_references: u64,
    pub resolved_icons: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvatarItemVisual {
    pub source_model_true_name: Option<String>,
    pub model_status: NativeLookupStatus,
    pub models: Vec<AvatarModelReference>,
    pub primary_texture: Option<AvatarTextureReference>,
    pub secondary_texture: Option<AvatarTextureReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvatarIconReference {
    pub icon_type: u8,
    pub icon_number: u32,
    pub true_name: String,
    pub status: NativeLookupStatus,
    pub candidates: Vec<CharacterCreationAssetReference>,
}

#[derive(Clone, Debug)]
pub(super) struct ItemTable<'a> {
    pub(super) item_data: &'a [Value],
    pub(super) icon_data: &'a [Value],
    pub(super) mesh_data: &'a [Value],
    pub(super) string_data: &'a [Value],
}
