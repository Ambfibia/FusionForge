use super::*;

pub const TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA: &str = "ffone.tutorial-static-world-install.v2";

pub const TUTORIAL_STATIC_WORLD_SOURCE_BUILD: &str = "retrobution-20260613";

pub const WORLD_MAP_STATIC_WORLD_OWNERSHIP_SCHEMA: &str = "ffone.world-map-static-world-install.v1";

pub const WORLD_MAP_STATIC_WORLD_CONTRACT_SCHEMA: &str = "ffone.world-map-static-world-contract.v1";

pub(super) const WORLD_MAP_INSTALLER_ID: &str = "ffone-asset-pipeline/install-world-map-static-world";

pub(super) const INSTALLER_ID: &str = "ffone-asset-pipeline/install-tutorial-static-world";

pub(super) const LEGACY_TUTORIAL_STATIC_WORLD_OWNERSHIP_SCHEMA: &str =
    "ffone.tutorial-static-world-install.v1";

pub(super) const WORLD_SCENE_SCHEMA: &str = "ffone.native-world-scene.v2";

pub(super) const STATIC_HIERARCHY_SCHEMA: &str = "ffone.native-static-world-hierarchy.v1";

pub(super) const STATIC_MATERIALS_SCHEMA: &str = "ffone.native-static-world-materials.v1";

pub(super) const BASE_COVERAGE: &str = "native-heightmap";

pub(super) const STATIC_COVERAGE: &str = "native-heightmap+exact-static-scene";

pub(super) const CLEANUP_STATIC_REASON: &str =
    "tutorial static conversion metadata is superseded by the pinned merged scene";

pub(super) const EXACT_TILES: [TileContract; 9] = [
    TileContract {
        id: Cow::Borrowed("tile_00_00"),
        source_archive_blake3: Cow::Borrowed(
            "81866b65bd3e5ffa3fefaccc4af9829f75ec647109b5beee85f0d11ffc846481",
        ),
        scene_nodes: 10,
        exported_visuals: 2,
        runtime_visuals: 0,
        exported_colliders: 2,
        runtime_colliders: 0,
        exported_models: 0,
        vertices: 0,
        indices: 0,
    },
    TileContract {
        id: Cow::Borrowed("tile_00_01"),
        source_archive_blake3: Cow::Borrowed(
            "84a2f229ec43f81be850d837c516bc299ca293b413c51eec888230dc835a0990",
        ),
        scene_nodes: 40,
        exported_visuals: 14,
        runtime_visuals: 13,
        exported_colliders: 8,
        runtime_colliders: 7,
        exported_models: 20,
        vertices: 17_748,
        indices: 75_186,
    },
    TileContract {
        id: Cow::Borrowed("tile_00_02"),
        source_archive_blake3: Cow::Borrowed(
            "d47e2828813efc901611e66721d9b467e71b4f6268acdaba4138ea9f12ab913b",
        ),
        scene_nodes: 10,
        exported_visuals: 2,
        runtime_visuals: 1,
        exported_colliders: 2,
        runtime_colliders: 1,
        exported_models: 2,
        vertices: 10_106,
        indices: 52_920,
    },
    TileContract {
        id: Cow::Borrowed("tile_01_00"),
        source_archive_blake3: Cow::Borrowed(
            "118be2d80db061b396a8e3403ac212e0c410227ce677b8db72fb94ddf9bab1eb",
        ),
        scene_nodes: 28,
        exported_visuals: 11,
        runtime_visuals: 10,
        exported_colliders: 5,
        runtime_colliders: 4,
        exported_models: 14,
        vertices: 34_325,
        indices: 97_248,
    },
    TileContract {
        id: Cow::Borrowed("tile_01_01"),
        source_archive_blake3: Cow::Borrowed(
            "8ece3b2ef273509ecb0687c38a6f137ed9872e8c9d477cf22e111b9afbdb555e",
        ),
        scene_nodes: 2_089,
        exported_visuals: 1_044,
        runtime_visuals: 1_043,
        exported_colliders: 442,
        runtime_colliders: 441,
        exported_models: 1_484,
        vertices: 308_501,
        indices: 653_043,
    },
    TileContract {
        id: Cow::Borrowed("tile_01_02"),
        source_archive_blake3: Cow::Borrowed(
            "6f6adc681be1a5dcf74cb29c12face635caf0a2f43233aab732c5395843198b3",
        ),
        scene_nodes: 10,
        exported_visuals: 2,
        runtime_visuals: 1,
        exported_colliders: 2,
        runtime_colliders: 1,
        exported_models: 2,
        vertices: 10_106,
        indices: 52_920,
    },
    TileContract {
        id: Cow::Borrowed("tile_02_00"),
        source_archive_blake3: Cow::Borrowed(
            "f386e8695e2ded1f52e503303c6d3afd2b89135699f15d9eb44a0334a8abc12c",
        ),
        scene_nodes: 10,
        exported_visuals: 2,
        runtime_visuals: 1,
        exported_colliders: 2,
        runtime_colliders: 1,
        exported_models: 2,
        vertices: 10_106,
        indices: 52_920,
    },
    TileContract {
        id: Cow::Borrowed("tile_02_01"),
        source_archive_blake3: Cow::Borrowed(
            "4a5cad92d5de5e5b9e564f9a08dadaec9e443a881c31d01883af6bc01b6097f6",
        ),
        scene_nodes: 48,
        exported_visuals: 20,
        runtime_visuals: 19,
        exported_colliders: 8,
        runtime_colliders: 7,
        exported_models: 26,
        vertices: 48_194,
        indices: 122_016,
    },
    TileContract {
        id: Cow::Borrowed("tile_02_02"),
        source_archive_blake3: Cow::Borrowed(
            "cc87df53b312932cdd9e5be96011b3340a8ba012689bd347858c1ce33a4d3cc5",
        ),
        scene_nodes: 10,
        exported_visuals: 2,
        runtime_visuals: 1,
        exported_colliders: 2,
        runtime_colliders: 1,
        exported_models: 2,
        vertices: 10_106,
        indices: 52_920,
    },
];
