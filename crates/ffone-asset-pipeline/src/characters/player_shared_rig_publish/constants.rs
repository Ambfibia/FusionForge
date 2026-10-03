use super::*;

pub(super) const MALE: GenderSpec = GenderSpec {
    gender: PlayerRigGender::Male,
    route: "actor/m.kfm",
    root_name: "m",
    expected_root_path_id: 147_398,
    expected_actor_bones: 133,
    expected_clip_count: 230,
    expected_creator_choices: 118,
    expected_unique_creator_parts: 54,
    transform_indices_field: "transformIndicesM",
    skeleton_glb: MALE_SHARED_SKELETON_GLB_PATH,
    clips: [
        ("stand1", 237_023),
        ("height_Add", 34_244),
        ("height", 34_245),
        ("shape_Add", 34_248),
        ("shape", 34_253),
    ],
    default_parts: [
        "wear/m_face_001_type01.nif",
        "wear/m_head_001_type01.nif",
        "wear/m_shirt_coolshirt.nif",
        "wear/m_pants_camo.nif",
        "wear/m_shoes_bluewalker.nif",
    ],
};

pub(super) const FEMALE: GenderSpec = GenderSpec {
    gender: PlayerRigGender::Female,
    route: "actor/w.kfm",
    root_name: "w",
    expected_root_path_id: 147_329,
    expected_actor_bones: 159,
    expected_clip_count: 228,
    expected_creator_choices: 113,
    expected_unique_creator_parts: 51,
    transform_indices_field: "transformIndicesF",
    skeleton_glb: FEMALE_SHARED_SKELETON_GLB_PATH,
    clips: [
        ("stand1", 237_027),
        ("height_Add", 34_516),
        ("height", 34_475),
        ("shape_Add", 34_227),
        ("shape", 34_372),
    ],
    default_parts: [
        "wear/f_face_001_type01.nif",
        "wear/f_head_001_type01.nif",
        "wear/f_shirt_coolshirt.nif",
        "wear/f_pants_camo.nif",
        "wear/f_shoes_bluewalker.nif",
    ],
};
