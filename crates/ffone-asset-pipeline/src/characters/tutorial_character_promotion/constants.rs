use super::*;

pub const TUTORIAL_CHARACTER_PROMOTION_SCHEMA: &str =
    "ffone.tutorial-character-promotion-report.v1";

pub(super) const TUTORIAL_CHARACTER_ROOT: &str = "tutorial/models/mob";

pub(super) const CANDIDATES: &[PromotionCandidate] = &[
    candidate(
        "fusion_buttercup",
        "fusion_buttercup.glb",
        RuntimeCharacterCategory::Fusion,
    ),
    candidate("mob_bat", "mob_bat.glb", RuntimeCharacterCategory::Mob),
    candidate(
        "mob_cerberus",
        "mob_cerberus.glb",
        RuntimeCharacterCategory::Mob,
    ),
    candidate("mob_spawn", "mob_spawn.glb", RuntimeCharacterCategory::Mob),
    candidate("npc_ben", "npc_ben.glb", RuntimeCharacterCategory::Npc),
    candidate(
        "npc_fusiongate",
        "npc_fusiongate.glb",
        RuntimeCharacterCategory::Npc,
    ),
    candidate(
        "npc_scamper",
        "npc_scamper.glb",
        RuntimeCharacterCategory::Npc,
    ),
    candidate(
        "t_numbuhfive",
        "t_numbuhfive.glb",
        RuntimeCharacterCategory::Npc,
    ),
];
