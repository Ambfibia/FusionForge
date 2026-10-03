use super::*;

pub const PLAYER_AVATAR_COOK_REPORT_SCHEMA: &str = "ffone.player-avatar-cook-report.v1";

pub(super) const TEST_SER_PROFILE: &str = "test_ser_male_creation_row_1";

pub(super) const REPORT_OUTPUT: &str = "data/catalog/avatar/test_ser_male_cook_report.json";

pub(super) const PARTS: &[PartSpec] = &[
    PartSpec {
        part: "face",
        participation: "combined_skinned_mesh",
        route: "wear/m_face_001_type01.nif",
        semantic_glb: "characters/player/male/test_ser/face/m_face_001_type01.glb",
    },
    PartSpec {
        part: "hair",
        participation: "combined_skinned_mesh",
        route: "wear/m_head_023_type01.nif",
        semantic_glb: "characters/player/male/test_ser/hair/m_head_023_type01.glb",
    },
    PartSpec {
        part: "upper_body",
        participation: "combined_skinned_mesh",
        route: "wear/m_shirt_baseballset.nif",
        semantic_glb: "characters/player/male/test_ser/upper_body/m_shirt_baseballset.glb",
    },
    PartSpec {
        part: "lower_body",
        participation: "combined_skinned_mesh",
        route: "wear/m_pants_beltarmorset.nif",
        semantic_glb: "characters/player/male/test_ser/lower_body/m_pants_beltarmorset.glb",
    },
    PartSpec {
        part: "feet",
        participation: "combined_skinned_mesh",
        route: "wear/m_shoes_blooarmorset.nif",
        semantic_glb: "characters/player/male/test_ser/feet/m_shoes_blooarmorset.glb",
    },
    PartSpec {
        part: "weapon",
        participation: "combat_secondary",
        route: "wear/theown_discobomb.nif",
        semantic_glb: "characters/player/male/test_ser/weapon/theown_discobomb.glb",
    },
];
