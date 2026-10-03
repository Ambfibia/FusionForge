use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSerSelection {
    pub gender: i8,
    pub face_style: i8,
    pub hair_style: i8,
    pub hair_color: i8,
    pub skin_color: i8,
    pub eye_color: i8,
    pub height: i8,
    pub body: i8,
    pub equipment: Vec<TestSerEquipment>,
}

pub(super) fn test_ser_selection() -> TestSerSelection {
    TestSerSelection {
        gender: 1,
        face_style: 5,
        hair_style: 23,
        hair_color: 1,
        skin_color: 1,
        eye_color: 1,
        height: 0,
        body: 0,
        equipment: vec![
            TestSerEquipment {
                slot: "hand".to_owned(),
                item_type: 0,
                item_id: 1,
                exact_route: "wear/theown_discobomb.nif".to_owned(),
            },
            TestSerEquipment {
                slot: "upper_body".to_owned(),
                item_type: 1,
                item_id: 30,
                exact_route: "wear/m_shirt_baseballset.nif".to_owned(),
            },
            TestSerEquipment {
                slot: "lower_body".to_owned(),
                item_type: 2,
                item_id: 30,
                exact_route: "wear/m_pants_beltarmorset.nif".to_owned(),
            },
            TestSerEquipment {
                slot: "foot".to_owned(),
                item_type: 3,
                item_id: 30,
                exact_route: "wear/m_shoes_blooarmorset.nif".to_owned(),
            },
        ],
    }
}
