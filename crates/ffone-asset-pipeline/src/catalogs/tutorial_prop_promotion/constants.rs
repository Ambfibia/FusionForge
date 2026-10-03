use super::*;

pub const TUTORIAL_PROP_PROMOTION_SCHEMA: &str = "ffone.tutorial-prop-promotion-report.v1";

pub(super) const SOURCE_PACKAGE_ROOT: &str = "tutorial/models/mob";

pub(super) const CANDIDATES: &[PromotionCandidate] = &[
    PromotionCandidate {
        id: "etc_domeglass_04",
        glb_name: "ETC_domeglass_04.glb",
        runtime_source: "crates/ffone-client/src/main.rs",
    },
    PromotionCandidate {
        id: "npc_building",
        glb_name: "npc_building.glb",
        runtime_source: "crates/ffone-client/src/tutorial_actors.rs",
    },
];
