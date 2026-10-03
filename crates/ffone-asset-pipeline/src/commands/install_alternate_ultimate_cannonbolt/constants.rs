use super::*;

pub(super) const SOURCE_ALIAS: &str = "alternate";

pub(super) const SOURCE_BUILD: &str = "6543a2bb-d154-4087-b9ee-3c8aa778580a";

pub(super) const PANTS_MODELS: &[ModelSpec] = &[
    ModelSpec {
        gender: "female",
        true_name: "f_pants_ultimatecannonboltpants",
        exact_route: "wear/f_pants_ultimatecannonboltpants.nif",
    },
    ModelSpec {
        gender: "male",
        true_name: "m_pants_ultimatecannonboltpants",
        exact_route: "wear/m_pants_ultimatecannonboltpants.nif",
    },
];

pub(super) const SHIRT_MODELS: &[ModelSpec] = &[
    ModelSpec {
        gender: "female",
        true_name: "f_shirt_ultimatecannonboltshirt",
        exact_route: "wear/f_shirt_ultimatecannonboltshirt.nif",
    },
    ModelSpec {
        gender: "male",
        true_name: "m_shirt_ultimatecannonboltshirt",
        exact_route: "wear/m_shirt_ultimatecannonboltshirt.nif",
    },
];

pub(super) const SHOES_MODELS: &[ModelSpec] = &[
    ModelSpec {
        gender: "female",
        true_name: "f_shoes_ultimatecannonboltshoes",
        exact_route: "wear/f_shoes_ultimatecannonboltshoes.nif",
    },
    ModelSpec {
        gender: "male",
        true_name: "m_shoes_ultimatecannonboltshoes",
        exact_route: "wear/m_shoes_ultimatecannonboltshoes.nif",
    },
];

pub(super) const SETS: &[SetSpec] = &[
    SetSpec {
        category: "pants",
        set_name: "pants_ultimatecannonboltpants",
        texture_true_name: "pants_ultimatecannonboltpants",
        texture_resource: "Character_Texture_pants.resourceFile",
        texture_path_id: 2_022_728_207,
        item_number: 554,
        aliases: &["f_pants_ultimatecannonbolt", "m_pants_ultimatecannonbolt"],
        models: PANTS_MODELS,
    },
    SetSpec {
        category: "shirt",
        set_name: "shirt_ultimatecannonboltshirt",
        texture_true_name: "shirt_ultimatecannonboltshirt",
        texture_resource: "Character_Texture_shirts.resourceFile",
        texture_path_id: 2_288_177_013,
        item_number: 643,
        aliases: &["f_shirt_ultimatecannonbolt", "m_shirt_ultimatecannonbolt"],
        models: SHIRT_MODELS,
    },
    SetSpec {
        category: "shoes",
        set_name: "shoes_ultimatecannonboltshoes",
        texture_true_name: "shoes_ultimatecannonboltshoes",
        texture_resource: "Character_Texture_shoes.resourceFile",
        texture_path_id: 3_590_273_566,
        item_number: 556,
        aliases: &["shoes_ultimatecannonbolt"],
        models: SHOES_MODELS,
    },
];
