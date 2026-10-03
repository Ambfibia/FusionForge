use super::*;

pub const GAMEPLAY_UI_CATALOG: &str = "ui/gameplay/catalog.json";

pub const GAMEPLAY_UI_CATALOG_SCHEMA: &str = "ffone.semantic-gameplay-ui-catalog.v1";

pub(super) const ARCHIVED_GAMEPLAY_UI_CATALOG_BYTES: u64 = 31_511;

pub(super) const ARCHIVED_GAMEPLAY_UI_CATALOG_BLAKE3: &str =
    "5007904c8b0437f3e22dec846e101c35285b138cf2c2ce4295053150da694d55";

pub(super) const ARCHIVED_GAMEPLAY_UI_INDEX_SCHEMA: &str = "ffone.conversion-metadata-revision-index.v1";

pub(super) const GUI_SKIN_PATH: &str = "ui/gameplay/skins/retrobution-20260613.json";

#[derive(Clone, Copy)]
pub(super) struct Route {
    #[cfg_attr(not(test), allow(dead_code, reason = "read by tests"))]
    pub(super) legacy_name: &'static str,
    pub(super) source: &'static str,
    pub(super) destination: &'static str,
    pub(super) kind: ProjectAssetKind,
}

#[derive(Clone, Copy)]
pub(super) struct StrictRouteProof {
    pub(super) source: &'static str,
    pub(super) blake3: &'static str,
    pub(super) bytes: u64,
}

pub(super) const STRICT_ROUTE_PROOFS: &[StrictRouteProof] = &[
    StrictRouteProof {
        source: "ui/gameplay/mission/journal/acceptbut.png",
        blake3: "246ee69da2164dbe3799364e9a3ca8743d53343e660ae80fd1cb43c896f64c18",
        bytes: 9_793,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/journal/activedlg.png",
        blake3: "16ecfee101981d224dfbf380eca0c69b2130e3fb73ff2a169e9fc2f8116365f4",
        bytes: 9_736,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/current_mission.png",
        blake3: "cb2366822c544ca39baac398e09cd30f46cd08a09e113d7dfb211ecadb79f1ce",
        bytes: 1_846,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/journal/enddlg.png",
        blake3: "afd24286829f08552aaf2771e6d9a2df1d321a6ca4684554b2000a2915070a27",
        bytes: 10_889,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/nanocom/menu_box.png",
        blake3: "41d91d337c9c6ccb60b7c2c27cf532f24eb86af2203b02d61a59733209dc696c",
        bytes: 5_159,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/mission_back.png",
        blake3: "1e28d3a91d485c6a761a5261b146e79cb8f98516407be2cca70286ad2352e2fd",
        bytes: 804,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/mission_body.png",
        blake3: "899a2b5463a168dcfd14f90e4ee8cd2c6c7b1b1d3c809eaf4477b338690cf9bd",
        bytes: 156,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/mission_bottom_func.png",
        blake3: "66443c65e845ad335aa9aba6d3145af37bee703fda28e8a1d94c6913565430f3",
        bytes: 1_512,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/mission_bottom_func2.png",
        blake3: "150d4fb586c2ec3a43faa6ee0048e9d3ddd1c9bffeca7f6b09ee74488cb37a67",
        bytes: 2_014,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/mission_button.png",
        blake3: "91922985e81a42f92bd2c6b4c9f8d0b9783368ec5d178442fb44231c41c8c65c",
        bytes: 252,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/mission_button_over.png",
        blake3: "6f106b82633720618467eea532b2bdbb49dfd9cd0c278164b8fe413a54b83975",
        bytes: 357,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/mission_top.png",
        blake3: "ce72ca3d48bd3539b111bbb350918118931d2470e4720c875bc8cdf7fc0c723f",
        bytes: 1_555,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/npc_multi_window.png",
        blake3: "242cceb8d78c91f57284ee5225789e64862119ec9aec7cfa56f889cf73bb30a3",
        bytes: 3_227,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/npc_window.png",
        blake3: "5bf7085e7f1adfbe703596d204b651948e64bdc3e9cc3dd1f5912f9d0018922f",
        bytes: 2_917,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/npcicon_exit.png",
        blake3: "8ccf4bc80b9d3dc15e933d89b227542675eebfa0ab7a5ccde9c94d676e0e45b1",
        bytes: 1_846,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/npcicon_mission.png",
        blake3: "162c58baaf741655d4e1d137ec6d3c4fe6b677bafccd8ca433ffbdd68ec1e53f",
        bytes: 2_020,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/npcicon_warp.png",
        blake3: "88c05a49f9d9ddb2a8e277553b9ead280c7c3835ba325fdb0bbe27b9e22479c3",
        bytes: 1_647,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/journal/offdlg.png",
        blake3: "d540abff80c6a9fec41fd5bce768f7ef76caced8578ed0c31bb1da1713f9f09e",
        bytes: 10_739,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/npc/sel_mission_back.png",
        blake3: "76e92831398cf7347bd570bec2ed17d6e8b8a7ba855a82c86d96c59d56576b07",
        bytes: 413,
    },
    StrictRouteProof {
        source: "ui/rule/close.png",
        blake3: "da2230505b0674966516d96575ff6c2549ce02d379a00de426a9133c26bba56e",
        bytes: 1_638,
    },
    StrictRouteProof {
        source: "ui/world-map/controls/closeover.png",
        blake3: "b2de719226535cfbab5da2ee96ca36970d732ac21934123645920cc41de4c4bb",
        bytes: 1_518,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/journal/reward_box.png",
        blake3: "e0144233f92a408df17fad3add6d19bb9a13ec1d7baf3bea775299a6939f8f5d",
        bytes: 1_744,
    },
    StrictRouteProof {
        source: "ui/gameplay/quick-slot/slotbox.png",
        blake3: "14f19a20190c64e160b4ea01de8fd744eee822806546b70118bb3f78f133ff5b",
        bytes: 2_536,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/journal/fmicon.png",
        blake3: "eb8545d71d1f40dc9d3b18e1ce76b4645779b930ce28c45e9002565e7c4efdef",
        bytes: 3_492,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/journal/window.png",
        blake3: "a66dfc17ea7a386db6cf0f22364ca402f2cc5dac876ce73721b3b5ef5d43d6c1",
        bytes: 7_748,
    },
    StrictRouteProof {
        source: "ui/gameplay/guide/ncp_icon_back.png",
        blake3: "01a09c4459e99127adf34129588ab8879cdc93399d797bafd81032a476ee3103",
        bytes: 247,
    },
    StrictRouteProof {
        source: "ui/gameplay/mission/journal/allow_right.png",
        blake3: "3e81cb99991d7acf5b3d49b69fc3cee3b506bb4152f41d139e4e59438166c16d",
        bytes: 1_754,
    },
    StrictRouteProof {
        source: "ui/gameplay/nanocom/nanocom_message_npc.png",
        blake3: "53a2832bc101e3871092c023162f1808aacf08d2ea3dfcf6c6c63cc6cad5b02f",
        bytes: 5_727,
    },
    StrictRouteProof {
        source: "ui/gameplay/minimap/map_icon_02.png",
        blake3: "bd17bd976a44c1d8d85a8d68f428c5eb79b2c12707cf3f593f124df938fa473e",
        bytes: 626,
    },
    StrictRouteProof {
        source: "ui/gameplay/minimap/map_icon_03.png",
        blake3: "64934cd2e3584d3d164749a63ca2ea025caeb6733fd738c1da896443de49cc2b",
        bytes: 551,
    },
    StrictRouteProof {
        source: "ui/gameplay/minimap/plus_but_1.png",
        blake3: "e3a5b6237a67ff54e3987d58d21e3561b7139dcae73b3e987c290e839e0d70d0",
        bytes: 723,
    },
    StrictRouteProof {
        source: "ui/gameplay/minimap/minus_but_1.png",
        blake3: "f4a5956b44acd5e29fd7bb12a8270a409c3846ab49cbe47d90f4643f5ce27399",
        bytes: 640,
    },
    StrictRouteProof {
        source: "ui/gameplay/system/systemDialogBox.png",
        blake3: "02dcd4cacc11aee21f86db91d634a54cabc67cd0306f3870a79542f4ea16d190",
        bytes: 3_691,
    },
    StrictRouteProof {
        source: "ui/gameplay/group/group_freechat_icon.png",
        blake3: "80034a44671b6a56115502c88a75bf2963fe9b353251c10e1baa4be88808c5a6",
        bytes: 550,
    },
    StrictRouteProof {
        source: "ui/gameplay/group/group_info.png",
        blake3: "b52f4199a90d9ae6eaae7338d7ce3469e63d89163621504e123c9e97cc0e3845",
        bytes: 1_441,
    },
    StrictRouteProof {
        source: "ui/gameplay/group/group_nano_hpframe.png",
        blake3: "4265f5f5ec78fc5f34903a63081f78cd12d3279215fc5331c04c0c0efab8640c",
        bytes: 256,
    },
    StrictRouteProof {
        source: "ui/gameplay/group/hp_bar.png",
        blake3: "9b5912edaf9a81641b1c8f0ce53ff73fdac88f8330fc566fbf335981d1e81adf",
        bytes: 510,
    },
    StrictRouteProof {
        source: "ui/gameplay/group/npc_co_op.png",
        blake3: "4b058d4bc650920b7f743b752996a2a44b5b5fa938b6991498395ea007f51dc7",
        bytes: 633,
    },
    StrictRouteProof {
        source: "ui/gameplay/chat/alert.png",
        blake3: "df2f08815d97ce578c4624d011fd4bd0e110f63401bc9e58d50eaf9a97201822",
        bytes: 553,
    },
    StrictRouteProof {
        source: "ui/gameplay/overheat/background.png",
        blake3: "753fd3e3387b9af44df1290ab18018d254a9e1b84f79a907aa39e5703ed3f33d",
        bytes: 810,
    },
    StrictRouteProof {
        source: "ui/gameplay/overheat/maximum.png",
        blake3: "c87370a094df0c16fcbe95b37af88e5b8331851002f0eafcf3f1bc9974a3f942",
        bytes: 940,
    },
    StrictRouteProof {
        source: "ui/gameplay/overheat/fill.png",
        blake3: "1a8c7a400ca25ab567bb91099cdfab9266f9eaa43193de11a87630ceee6bc881",
        bytes: 495,
    },
    StrictRouteProof {
        source: "ui/gameplay/quick-slot/quickslot_main.png",
        blake3: "35d2a07b3d510eaf58599e87bf45b8b944deff8d38de82d94225a8a385a84e52",
        bytes: 1_198,
    },
    StrictRouteProof {
        source: "ui/gameplay/quick-slot/slotboxempty.png",
        blake3: "bd4a491d41cc10005e906750fb8db5f89cb1ff55cc4aa52639a6b7bcf011f326",
        bytes: 868,
    },
    StrictRouteProof {
        source: "ui/gameplay/quick-slot/boxalpha.png",
        blake3: "3bb1adcd32b2ba276c47c9c3c0ca439f1044379b70959c57d03ffa6a471d6f18",
        bytes: 326,
    },
    StrictRouteProof {
        source: "ui/gameplay/buddy/window.png",
        blake3: "edfec9df9ab501a43f7e806deb404d5b5d44ede75379b123e91ec73d735ac235",
        bytes: 576,
    },
    StrictRouteProof {
        source: "ui/gameplay/buddy/selection.png",
        blake3: "3bc80f9095dd3b10fd80d5f098ac10d3313c9e52c8ea093c49cc675aca2d903c",
        bytes: 419,
    },
    StrictRouteProof {
        source: "ui/gameplay/player/freechat_icon.png",
        blake3: "fab450c90789a5ac392e99d729e7bf1397fb3800f02a6f8620d0c752eb20d80e",
        bytes: 555,
    },
    StrictRouteProof {
        source: "ui/launcher/login/ff-textfield-normal.png",
        blake3: "e7437d400ef6de665a0dba0d07da33a5f9286fa59ba423a49e72502215bb2107",
        bytes: 217,
    },
];

pub(super) fn strict_route_proof(source: &str) -> Option<&'static StrictRouteProof> {
    STRICT_ROUTE_PROOFS
        .iter()
        .find(|proof| proof.source == source)
}

pub(super) const fn route(
    legacy_name: &'static str,
    source: &'static str,
    destination: &'static str,
    kind: ProjectAssetKind,
) -> Route {
    Route {
        legacy_name,
        source,
        destination,
        kind,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArchivedRevisionIndex {
    pub(super) schema: String,
    pub(super) files: Vec<ArchivedRevisionFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArchivedCatalog {
    pub(super) schema: String,
    pub(super) assets: Vec<ArchivedCatalogAsset>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArchivedCatalogAsset {
    pub(super) path: String,
    pub(super) source_path: String,
}

pub(super) fn load_archived_catalog(asset_root: &Path) -> Result<ArchivedCatalog> {
    let project_root = asset_root
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| invalid_error("asset root is not nested under <project>/assets/game"))?;
    let revision_root = project_root
        .join("content")
        .join("imported")
        .join("retrobution-20260613")
        .join("conversion-metadata")
        .join("revisions")
        .join(ARCHIVED_GAMEPLAY_UI_REVISION);
    let index_path = revision_root.join("index.json");
    let index_bytes = fs::read(&index_path).map_err(|error| io_at(&index_path, error))?;
    let index: ArchivedRevisionIndex =
        serde_json::from_slice(&index_bytes).map_err(|source| PipelineError::Json {
            path: index_path.display().to_string(),
            source,
        })?;
    if index.schema != ARCHIVED_GAMEPLAY_UI_INDEX_SCHEMA {
        return invalid(format!(
            "unexpected conversion archive schema {:?}",
            index.schema
        ));
    }
    let entry = index
        .files
        .iter()
        .find(|entry| entry.source_path == GAMEPLAY_UI_CATALOG)
        .ok_or_else(|| invalid_error("conversion archive does not index gameplay UI catalog"))?;
    if entry.archive_path != "files/ui/gameplay/catalog.json"
        || entry.bytes != ARCHIVED_GAMEPLAY_UI_CATALOG_BYTES
        || entry.blake3 != ARCHIVED_GAMEPLAY_UI_CATALOG_BLAKE3
    {
        return invalid("conversion archive index identity mismatch for gameplay UI catalog");
    }
    let catalog_path = revision_root.join(
        entry
            .archive_path
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    );
    let bytes = fs::read(&catalog_path).map_err(|error| io_at(&catalog_path, error))?;
    let actual_blake3 = blake3::hash(&bytes).to_hex().to_string();
    if bytes.len() as u64 != ARCHIVED_GAMEPLAY_UI_CATALOG_BYTES
        || actual_blake3 != ARCHIVED_GAMEPLAY_UI_CATALOG_BLAKE3
    {
        return invalid(format!(
            "archived gameplay UI catalog identity mismatch: blake3={actual_blake3}, bytes={}",
            bytes.len()
        ));
    }
    let catalog: ArchivedCatalog =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    if catalog.schema != GAMEPLAY_UI_CATALOG_SCHEMA {
        return invalid(format!(
            "unexpected archived gameplay UI schema {:?}",
            catalog.schema
        ));
    }
    Ok(catalog)
}

pub(super) fn desired_route_sources() -> BTreeMap<String, (&'static str, ProjectAssetKind)> {
    let mut desired = BTreeMap::new();
    for route in ROUTES {
        desired.insert(
            format!("{GAMEPLAY_UI_ROOT}/{}", route.destination),
            (route.source, route.kind),
        );
    }
    for route in VERIFIED_ICON_ROUTES {
        desired.insert(
            format!("{GAMEPLAY_UI_ROOT}/{}", route.destination),
            (route.source, ProjectAssetKind::Texture),
        );
    }
    desired.insert(
        GUI_SKIN_PATH.to_owned(),
        (GUI_SKIN_PATH, ProjectAssetKind::Data),
    );
    desired
}

pub(super) fn verify_route_source(
    manifest: &ProjectAssetManifest,
    source: &str,
    kind: ProjectAssetKind,
    bytes: &[u8],
) -> Result<()> {
    let source_entry = manifest
        .files
        .iter()
        .find(|entry| entry.path == source)
        .ok_or_else(|| invalid_error(format!("source manifest does not contain {source:?}")))?;
    let actual_blake3 = blake3::hash(bytes).to_hex().to_string();
    if source_entry.source_path != source
        || source_entry.kind != kind
        || source_entry.bytes != bytes.len() as u64
        || source_entry.blake3 != actual_blake3
    {
        return invalid(format!("source manifest identity mismatch for {source:?}"));
    }
    if let Some(proof) = strict_route_proof(source)
        && (proof.bytes != bytes.len() as u64 || proof.blake3 != actual_blake3)
    {
        return invalid(format!(
            "strict native UI identity mismatch for {source:?}: expected blake3={} bytes={}, actual blake3={actual_blake3} bytes={}",
            proof.blake3,
            proof.bytes,
            bytes.len()
        ));
    }
    Ok(())
}

pub(super) fn replace_manifest(path: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("asset manifest has no parent"))?;
    let next = parent.join(".asset-manifest.gameplay-ui.next");
    let backup = parent.join(".asset-manifest.gameplay-ui.backup");
    if next.exists() || backup.exists() {
        return invalid(format!(
            "stale manifest transaction file at {} or {}",
            next.display(),
            backup.display()
        ));
    }
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: path.display().to_string(),
        source,
    })?;
    bytes.push(b'\n');
    write_new(&next, &bytes)?;
    fs::rename(path, &backup).map_err(|error| io_at(path, error))?;
    if let Err(error) = fs::rename(&next, path) {
        let _ = fs::rename(&backup, path);
        let _ = fs::remove_file(&next);
        return Err(io_at(path, error));
    }
    fs::remove_file(&backup).map_err(|error| io_at(&backup, error))
}
