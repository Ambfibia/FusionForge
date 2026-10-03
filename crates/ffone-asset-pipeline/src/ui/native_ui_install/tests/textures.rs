use super::*;

#[test]
fn overheat_routes_match_clean_serialized_textures_and_png_bytes() {
    struct Expected {
        legacy_name: &'static str,
        source: &'static str,
        destination: &'static str,
        texture_path_id: i64,
        dimensions: (u32, u32),
        blake3: &'static str,
        sha256: &'static str,
        bytes: u64,
    }

    const EXPECTED: [Expected; 3] = [
        Expected {
            legacy_name: "OverheatBG",
            source: "ui/gameplay/overheat/background.png",
            destination: "overheat/background.png",
            texture_path_id: 162,
            dimensions: (17, 86),
            blake3: "753fd3e3387b9af44df1290ab18018d254a9e1b84f79a907aa39e5703ed3f33d",
            sha256: "fc5e4960463a6243a4b81d23582e77e12da474132bb6c41a2cd3c74031700332",
            bytes: 810,
        },
        Expected {
            legacy_name: "OverheatMax",
            source: "ui/gameplay/overheat/maximum.png",
            destination: "overheat/maximum.png",
            texture_path_id: 206,
            dimensions: (22, 94),
            blake3: "c87370a094df0c16fcbe95b37af88e5b8331851002f0eafcf3f1bc9974a3f942",
            sha256: "f50b7df20a113e9b31f0b2393ef6c778aff385f414a9279ef6da15ce31cc234d",
            bytes: 940,
        },
        Expected {
            legacy_name: "OverheatNormal",
            source: "ui/gameplay/overheat/fill.png",
            destination: "overheat/fill.png",
            texture_path_id: 669,
            dimensions: (6, 80),
            blake3: "1a8c7a400ca25ab567bb91099cdfab9266f9eaa43193de11a87630ceee6bc881",
            sha256: "bf921b23e18e4e30db9defbd44214eee597d422189443894b0950fcd68abc7a2",
            bytes: 495,
        },
    ];

    let asset_root = workspace_root().join("assets").join("game");
    for expected in EXPECTED {
        let route = ROUTES
            .iter()
            .find(|route| route.destination == expected.destination)
            .unwrap_or_else(|| {
                panic!(
                    "missing clean Texture2D path ID {} route",
                    expected.texture_path_id
                )
            });
        assert_eq!(route.legacy_name, expected.legacy_name);
        assert_eq!(route.source, expected.source);
        assert_eq!(route.kind, ProjectAssetKind::Texture);

        let proof =
            strict_route_proof(route.source).expect("missing strict overheat texture proof");
        assert_eq!(proof.blake3, expected.blake3);
        assert_eq!(proof.bytes, expected.bytes);

        let bytes = fs::read(asset_root.join(expected.source)).unwrap();
        assert_eq!(bytes.len() as u64, expected.bytes);
        assert_eq!(sha256_hex(&bytes), expected.sha256);
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().dimensions(),
            expected.dimensions,
            "clean Texture2D path ID {}",
            expected.texture_path_id
        );
    }
}

#[test]
fn quick_slot_routes_match_clean_serialized_textures_and_png_bytes() {
    struct Expected {
        legacy_name: &'static str,
        source: &'static str,
        destination: &'static str,
        texture_path_id: i64,
        serialized_reference: &'static str,
        dimensions: (u32, u32),
        blake3: &'static str,
        sha256: &'static str,
        bytes: u64,
    }

    // Clean `main.unity3d/sharedassets0.assets`:
    // - cnQuickSlot MonoBehaviour 1568: QuickSlotBG -> Texture2D 482;
    // - FusionFallInvenSkin MonoBehaviour 1366: slotbox/slotboxempty
    //   GUIStyle backgrounds -> Texture2D 50/239;
    // - MenuChatScript MonoBehaviour 1562: BoxAlpha -> Texture2D 546.
    const EXPECTED: [Expected; 4] = [
        Expected {
            legacy_name: "quickslot_main",
            source: "ui/gameplay/quick-slot/quickslot_main.png",
            destination: "quick-slot/quickslot_main.png",
            texture_path_id: 482,
            serialized_reference: "cnQuickSlot[1568].QuickSlotBG",
            dimensions: (290, 39),
            blake3: "35d2a07b3d510eaf58599e87bf45b8b944deff8d38de82d94225a8a385a84e52",
            sha256: "edf438d4626e1909601ce596efc50ecca1ca1b843f1e69e9d9ceb7745cd6e4e7",
            bytes: 1_198,
        },
        Expected {
            legacy_name: "slotbox",
            source: "ui/gameplay/quick-slot/slotbox.png",
            destination: "quick-slot/slotbox.png",
            texture_path_id: 50,
            serialized_reference: "FusionFallInvenSkin[1366].slotbox",
            dimensions: (62, 62),
            blake3: "14f19a20190c64e160b4ea01de8fd744eee822806546b70118bb3f78f133ff5b",
            sha256: "2777645986c2b0ea46cc527faa9ed7c8471af4a5667b272df123b60990abf3ea",
            bytes: 2_536,
        },
        Expected {
            legacy_name: "slotboxempty",
            source: "ui/gameplay/quick-slot/slotboxempty.png",
            destination: "quick-slot/slotboxempty.png",
            texture_path_id: 239,
            serialized_reference: "FusionFallInvenSkin[1366].slotboxempty",
            dimensions: (62, 62),
            blake3: "bd4a491d41cc10005e906750fb8db5f89cb1ff55cc4aa52639a6b7bcf011f326",
            sha256: "4b0eacb923af7210ae924c68edb4f0c46d0d3f30d113239af78a1dd5d3b826da",
            bytes: 868,
        },
        Expected {
            legacy_name: "boxalpha",
            source: "ui/gameplay/quick-slot/boxalpha.png",
            destination: "quick-slot/boxalpha.png",
            texture_path_id: 546,
            serialized_reference: "MenuChatScript[1562].BoxAlpha",
            dimensions: (64, 64),
            blake3: "3bb1adcd32b2ba276c47c9c3c0ca439f1044379b70959c57d03ffa6a471d6f18",
            sha256: "02790a13362c4ab24ff7629f5ed49312c81f665ac6ff522a0b17da7b73031973",
            bytes: 326,
        },
    ];

    let quick_slot_routes = ROUTES
        .iter()
        .filter(|route| route.destination.starts_with("quick-slot/"))
        .collect::<Vec<_>>();
    assert_eq!(quick_slot_routes.len(), EXPECTED.len());
    assert_eq!(
        quick_slot_routes
            .iter()
            .map(|route| route.destination)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED.len()
    );

    let asset_root = workspace_root().join("assets").join("game");
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap())
            .unwrap();
    for expected in EXPECTED {
        let route = quick_slot_routes
            .iter()
            .copied()
            .find(|route| route.destination == expected.destination)
            .unwrap_or_else(|| {
                panic!(
                    "missing clean Texture2D path ID {} route from {}",
                    expected.texture_path_id, expected.serialized_reference
                )
            });
        assert_eq!(route.legacy_name, expected.legacy_name);
        assert_eq!(route.source, expected.source);
        assert_eq!(route.kind, ProjectAssetKind::Texture);

        let proof =
            strict_route_proof(route.source).expect("missing strict quick-slot texture proof");
        assert_eq!(proof.blake3, expected.blake3);
        assert_eq!(proof.bytes, expected.bytes);

        let bytes = fs::read(asset_root.join(expected.source)).unwrap();
        assert_eq!(bytes.len() as u64, expected.bytes);
        assert_eq!(sha256_hex(&bytes), expected.sha256);
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().dimensions(),
            expected.dimensions,
            "clean Texture2D path ID {} from {}",
            expected.texture_path_id,
            expected.serialized_reference
        );
        verify_route_source(
            &manifest,
            expected.source,
            ProjectAssetKind::Texture,
            &bytes,
        )
        .unwrap();
    }
}

#[test]
fn buddy_routes_match_clean_serialized_textures_and_png_bytes() {
    struct Expected {
        legacy_name: &'static str,
        source: &'static str,
        destination: &'static str,
        texture_path_id: i64,
        serialized_reference: &'static str,
        dimensions: (u32, u32),
        blake3: &'static str,
        sha256: &'static str,
        bytes: u64,
    }

    // Clean `main.unity3d/sharedassets0.assets`:
    // - FusionFallChatSkin MonoBehaviour 1368:
    //   Buddy_Window -> Texture2D 115, BuddySelect -> Texture2D 391,
    //   chatTexts -> Texture2D 337;
    // - CnGuiChat MonoBehaviour 1564: texFreeChat -> Texture2D 76.
    const EXPECTED: [Expected; 4] = [
        Expected {
            legacy_name: "buddy_box",
            source: "ui/gameplay/buddy/window.png",
            destination: "buddy/window.png",
            texture_path_id: 115,
            serialized_reference: "FusionFallChatSkin[1368].Buddy_Window.normal.background",
            dimensions: (40, 77),
            blake3: "edfec9df9ab501a43f7e806deb404d5b5d44ede75379b123e91ec73d735ac235",
            sha256: "e01f66ce65ddebb8be09a91ebf04fc67814506a329bf6530f36479a533de8f61",
            bytes: 576,
        },
        Expected {
            legacy_name: "buddy_select",
            source: "ui/gameplay/buddy/selection.png",
            destination: "buddy/selection.png",
            texture_path_id: 391,
            serialized_reference: "FusionFallChatSkin[1368].BuddySelect.normal.background",
            dimensions: (38, 17),
            blake3: "3bc80f9095dd3b10fd80d5f098ac10d3313c9e52c8ea093c49cc675aca2d903c",
            sha256: "68de12850d59b84b358f17d40c78b447a8ea02769b03fd9563a89302c4b17d0e",
            bytes: 419,
        },
        Expected {
            legacy_name: "freechat_icon",
            source: "ui/gameplay/player/freechat_icon.png",
            destination: "buddy/freechat.png",
            texture_path_id: 76,
            serialized_reference: "CnGuiChat[1564].texFreeChat",
            dimensions: (18, 14),
            blake3: "fab450c90789a5ac392e99d729e7bf1397fb3800f02a6f8620d0c752eb20d80e",
            sha256: "d433073c5550f7b4d959158d4b6c3f7de7ab75b00822097a1082c451bdbad499",
            bytes: 555,
        },
        Expected {
            legacy_name: "ff-textfield-normal",
            source: "ui/launcher/login/ff-textfield-normal.png",
            destination: "buddy/list-background.png",
            texture_path_id: 337,
            serialized_reference: "FusionFallChatSkin[1368].chatTexts.normal.background",
            dimensions: (5, 25),
            blake3: "e7437d400ef6de665a0dba0d07da33a5f9286fa59ba423a49e72502215bb2107",
            sha256: "f5a79b4fda32a41147172a2ef4cfccbf62beed87b01f7a31dc4acf9e82cd1556",
            bytes: 217,
        },
    ];

    let buddy_routes = ROUTES
        .iter()
        .filter(|route| route.destination.starts_with("buddy/"))
        .collect::<Vec<_>>();
    assert_eq!(buddy_routes.len(), EXPECTED.len());
    assert_eq!(
        buddy_routes
            .iter()
            .map(|route| route.destination)
            .collect::<BTreeSet<_>>()
            .len(),
        EXPECTED.len()
    );

    let asset_root = workspace_root().join("assets").join("game");
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(asset_root.join(ASSET_MANIFEST_FILE)).unwrap())
            .unwrap();
    for expected in EXPECTED {
        let route = buddy_routes
            .iter()
            .copied()
            .find(|route| route.destination == expected.destination)
            .unwrap_or_else(|| {
                panic!(
                    "missing clean Texture2D path ID {} route from {}",
                    expected.texture_path_id, expected.serialized_reference
                )
            });
        assert_eq!(route.legacy_name, expected.legacy_name);
        assert_eq!(route.source, expected.source);
        assert_eq!(route.kind, ProjectAssetKind::Texture);

        let proof =
            strict_route_proof(route.source).expect("missing strict Buddy texture proof");
        assert_eq!(proof.blake3, expected.blake3);
        assert_eq!(proof.bytes, expected.bytes);

        let bytes = fs::read(asset_root.join(expected.source)).unwrap();
        assert_eq!(bytes.len() as u64, expected.bytes);
        assert_eq!(sha256_hex(&bytes), expected.sha256);
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().dimensions(),
            expected.dimensions,
            "clean Texture2D path ID {} from {}",
            expected.texture_path_id,
            expected.serialized_reference
        );
        verify_route_source(
            &manifest,
            expected.source,
            ProjectAssetKind::Texture,
            &bytes,
        )
        .unwrap();
    }
}
