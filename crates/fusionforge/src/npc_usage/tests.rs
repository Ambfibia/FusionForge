use super::*;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

struct FixtureDir(PathBuf);

impl FixtureDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ffclienteditor-npc-usage-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create fixture directory");
        Self(path)
    }

    fn write(&self, name: &str, value: JsonValue) {
        fs::write(
            self.0.join(name),
            serde_json::to_vec_pretty(&value).expect("serialize fixture"),
        )
        .expect("write fixture");
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> FixtureDir {
    let dir = FixtureDir::new();
    dir.write(
        "xdt.json",
        json!({
            "m_pNpcTable": {
                "m_pNpcData": [
                    {"m_iNpcNumber": 0},
                    {"m_iNpcNumber": 1},
                    {"m_iNpcNumber": 42},
                    {"m_iNpcNumber": 3},
                    {"m_iNpcNumber": 4},
                    {"m_iNpcNumber": 5},
                    {"m_iNpcNumber": 6}
                ],
                "m_pNpcGroupData": [
                    {"iMember1": 0, "iMember2": 0, "iMember3": 0, "iMember4": 0, "iMember5": 0},
                    {"iMember1": 1, "iMember2": 0, "iMember3": 0, "iMember4": 0, "iMember5": 0}
                ]
            },
            "m_pMissionTable": {"m_pMissionData": [{
                "m_iHTaskID": 700,
                "m_iHMissionID": 70,
                "m_iHNPCID": 3,
                "m_iHJournalNPCID": 0,
                "m_iHTerminatorNPCID": 0,
                "m_iCSUDEFNPCID": 0,
                "m_iSTGrantWayPoint": 5,
                "m_iSTSpawnMonsterID": 0,
                "m_iSTDialogBubbleNPCID": 0,
                "m_iSUDialogBubbleNPCID": 0,
                "m_iFDialogBubbleNPCID": 0,
                "m_iSTMessageSendNPC": 0,
                "m_iSUMessageSendNPC": 0,
                "m_iFMessageSendNPC": 0,
                "m_iCSUEnemyID": [5, 0, 0]
            }]},
            "m_pVendorTable": {"m_pItemData": [
                {"m_iNpcNumber": 4, "m_iSortNumber": 9, "m_iitemID": 123}
            ]},
            "m_pInstanceTable": {"m_pWarpData": [
                {"m_iNpcNumber": 1, "m_iWarpNumber": 8}
            ]},
            "m_pTransportationTable": {
                "m_pBroomstickLocation": [],
                "m_pTransportationWarpLocation": [],
                "m_pTransportationData": []
            }
        }),
    );
    dir.write(
        "NPCs.json",
        json!({"NPCs": {"0": {"iNPCType": 1, "iX": 10, "iY": 20, "iZ": 30}}}),
    );
    dir.write(
        "mobs.json",
        json!({
            "mobs": {"0": {"iNPCType": 2, "iX": 1, "iY": 2, "iZ": 3}},
            "groups": {"0": {
                "iNPCType": 3,
                "iX": 4,
                "iY": 5,
                "iZ": 6,
                "aFollowers": [{"iNPCType": 4}]
            }}
        }),
    );
    dir.write(
        "gruntwork.json",
        json!({"mobs": [{"iNPCType": 5, "iX": 7, "iY": 8, "iZ": 9}], "groups": []}),
    );
    dir.write(
        "paths.json",
        json!({"npc": {"0": {"aNPCTypes": [2], "aNPCIDs": [1], "aPoints": []}}}),
    );
    dir.write(
        "drops.json",
        json!({"Mobs": {"0": {"MobID": 5, "MobDropID": 44}}}),
    );
    dir
}

#[test]
fn classifies_map_quest_other_and_unused_references() {
    let dir = fixture();
    let inspection = inspect_npc_usage(dir.0.to_string_lossy().into_owned()).expect("inspect");

    assert_eq!(inspection.server_catalog_count, 6);
    assert_eq!(inspection.catalog_count, 6);
    assert_eq!(inspection.on_map_count, 5);
    assert_eq!(inspection.used_count, 5);
    assert_eq!(inspection.unused_count, 1);
    assert!(inspection
        .warnings
        .iter()
        .any(|warning| warning.contains("row 2 stores 42")));

    let npc_3 = inspection
        .records
        .iter()
        .find(|record| record.npc_id == 3)
        .unwrap();
    assert!(npc_3.on_map);
    assert!(npc_3.quest_referenced);
    assert!(npc_3
        .quest_samples
        .iter()
        .any(|sample| sample.field == "m_iHNPCID" && sample.row == "task 700"));

    let npc_5 = inspection
        .records
        .iter()
        .find(|record| record.npc_id == 5)
        .unwrap();
    assert_eq!(npc_5.quest_reference_count, 2);
    assert!(npc_5.other_referenced);
    assert!(npc_5
        .other_samples
        .iter()
        .any(|sample| sample.source_file == "drops.json"));

    let npc_6 = inspection
        .records
        .iter()
        .find(|record| record.npc_id == 6)
        .unwrap();
    assert!(npc_6.unused);
    assert!(!npc_6.used);
    assert!(npc_6.sample_descriptions.is_empty());
}

#[test]
fn accepts_xdt_file_and_uses_client_catalog_domain() {
    let dir = fixture();
    let inspection = inspect_npc_usage_for_catalog(
        dir.0.join("xdt.json").to_string_lossy().into_owned(),
        vec![1, 6, 9],
    )
    .expect("inspect selected client IDs");

    assert_eq!(inspection.catalog_count, 3);
    assert_eq!(
        inspection
            .records
            .iter()
            .map(|record| record.npc_id)
            .collect::<Vec<_>>(),
        vec![1, 6, 9]
    );
    let staged = inspection
        .records
        .iter()
        .find(|record| record.npc_id == 9)
        .unwrap();
    assert!(!staged.server_catalog_present);
    assert!(staged.unused);
    assert!(inspection.xdt_path.ends_with("/xdt.json"));
}
