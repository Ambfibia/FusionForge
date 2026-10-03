
pub const TUTORIAL_EFFECT_ROOT: &str = "map/shared/effects";

pub const TUTORIAL_PROJECTILE_ROOT: &str = "map/shared/projectiles";

pub const TUTORIAL_EFFECT_CLOSURE_SCHEMA: &str = "ffone.tutorial-effect-closure.v1";

pub const TUTORIAL_BULLET_ROW_SCHEMA: &str = "ffone.tutorial-bullet-row.v1";

pub const RETROBUTION_TUTORIAL_BUILD_ID: &str =
    ffone_runtime_contracts::RETROBUTION_TUTORIAL_BUILD_ID;

/// IDs referenced by `cntutorialscript.cs`, exact tutorial character clips,
/// or the gameplay Nano call path. This is deliberately sorted and complete;
/// 60 is Buttercup's tutorial skill-1 `tag_p` event, 527..=529 are all three
/// Nano table styles plus `NanoMoveController`'s fixed 527 offset, while 736
/// is Bubbles' `melee1` eye-beam event (`736:tag01`).
pub const RETROBUTION_TUTORIAL_EFFECT_IDS: [i32; 33] = [
    4, 10, 38, 60, 366, 372, 527, 528, 529, 653, 668, 705, 723, 734, 736, 739, 740, 741, 742, 750,
    751, 757, 767, 771, 772, 774, 778, 809, 812, 813, 817, 865, 866,
];

/// IDs recovered from the exact `particle`/`tag_p` AnimationEvents in the
/// clean-primary Fusion character clips published by FFOne.
pub const RETROBUTION_FUSION_ACTOR_EFFECT_IDS: [i32; 95] = [
    530, 537, 538, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554, 555, 556, 557,
    558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568, 569, 570, 571, 572, 573, 574, 575, 576,
    577, 578, 579, 580, 581, 582, 583, 584, 592, 593, 600, 601, 602, 603, 604, 605, 606, 607, 608,
    609, 610, 611, 612, 613, 614, 615, 616, 617, 618, 619, 620, 621, 622, 623, 624, 625, 626, 627,
    628, 629, 630, 631, 633, 634, 635, 636, 637, 638, 639, 640, 641, 642, 643, 644, 645, 647, 650,
];

/// Disjoint exact EffectPackage IDs referenced by clean-primary non-Fusion
/// character AnimationEvents, including the local player's inventory computer
/// variants (ES425/833/834).
pub const RETROBUTION_CHARACTER_ACTOR_EFFECT_IDS: [i32; 26] = [
    3, 21, 50, 425, 503, 688, 689, 690, 691, 692, 693, 694, 695, 697, 698, 699, 700, 726, 737, 763,
    764, 765, 768, 769, 833, 834,
];

/// Exact player-position effect loaded by clean-primary
/// `NpcIconMode.LoadWarpEffect` after WarpOK succeeds.
pub const RETROBUTION_NPC_WARP_EFFECT_IDS: [i32; 1] =
    ffone_runtime_contracts::RETROBUTION_NPC_WARP_EFFECT_IDS;

/// Exact EffectPackage indices referenced by world-map `EPElementController`
/// records. These use the same serialized Effects.resourceFile dependency
/// graph and native particle implementation as the tutorial effect catalog.
pub const RETROBUTION_WORLD_EP_EFFECT_IDS: [i32; 7] = [461, 464, 465, 466, 533, 534, 594];

/// Every EffectPackage reachable through clean-primary
/// `NpcMoveController.MakeGameIcon`: ordinary TableData icons, the two
/// ring-race state branches, and the registered Recall Point replacement.
pub const RETROBUTION_NPC_GAME_ICON_EFFECT_IDS: [i32; 20] = [
    66, 395, 446, 672, 673, 674, 675, 676, 677, 678, 679, 680, 681, 682, 683, 685, 811, 824, 825,
    826,
];

pub const RETROBUTION_WEAPON_EFFECT_IDS: [i32; 23] =
    ffone_runtime_contracts::RETROBUTION_WEAPON_EFFECT_IDS;

pub const RETROBUTION_TUTORIAL_BULLET_TYPES: [i32; 39] =
    ffone_runtime_contracts::RETROBUTION_TUTORIAL_BULLET_TYPES;

pub const RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS: [i32; 28] =
    ffone_runtime_contracts::RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS;

pub(super) const EFFECTS_DEPENDENCY_B4: &str = "CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a";

pub(super) const EFFECTS_DEPENDENCY_BD5: &str = "CustomAssetBundle-bd5f53480423447d7bcaed95cb2a96c8";
