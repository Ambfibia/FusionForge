use super::*;

fn object(fields: impl IntoIterator<Item = (&'static str, UnityValue)>) -> UnityValue {
    UnityValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn array(values: Vec<UnityValue>) -> UnityValue {
    UnityValue::Array(values)
}

fn string(value: &str) -> UnityValue {
    UnityValue::String(value.to_string())
}

fn int(value: i64) -> UnityValue {
    UnityValue::Int(value)
}

fn null_mesh() -> UnityValue {
    object([
        ("m_pstrFMeshModelString", string("null")),
        ("m_pstrFTextureString", string("null")),
        ("m_pstrFTextureString2", string("null")),
        ("m_pstrMMeshModelString", string("null")),
        ("m_pstrMTextureString", string("null")),
        ("m_pstrMTextureString2", string("null")),
    ])
}

fn empty_hnpc() -> UnityValue {
    object([("TableElement", array(Vec::new()))])
}

#[test]
fn npc_and_nano_use_only_live_mesh_rows_and_keep_cross_family_hints() {
    let shared_texture = "Shared\\Body";
    let npc_mesh = object([
        ("m_pstrMMeshModelString", string("npc_Dexter")),
        ("m_pstrMTextureString", string(shared_texture)),
        ("m_pstrMTextureString2", string("null")),
        ("m_pstrFMeshModelString", string("Dexter_Hurt_1")),
        ("m_pstrFTextureString", string("Dexter_Hurt_2.wav")),
        ("m_pstrFTextureString2", string("null")),
    ]);
    let nano_mesh = object([
        ("m_pstrMMeshModelString", string("nano_Dexter")),
        ("m_pstrMTextureString", string(shared_texture)),
        ("m_pstrMTextureString2", string("null")),
    ]);
    let xdtdatas = object([
        (
            "m_pNpcTable",
            object([
                (
                    "m_pNpcData",
                    array(vec![
                        object([("m_iHNpc", int(0)), ("m_iMesh", int(1))]),
                        object([("m_iHNpc", int(7)), ("m_iMesh", int(2))]),
                    ]),
                ),
                (
                    "m_pNpcMeshData",
                    array(vec![null_mesh(), npc_mesh, null_mesh()]),
                ),
            ]),
        ),
        (
            "m_pNanoTable",
            object([
                ("m_pNanoData", array(vec![object([("m_iMesh", int(1))])])),
                ("m_pNanoMeshData", array(vec![null_mesh(), nano_mesh])),
            ]),
        ),
    ]);

    let index = SemanticIndex::from_table_values(&xdtdatas, &empty_hnpc()).unwrap();
    assert_eq!(
        index
            .hint("MOB/NPC_DEXTER.KFM")
            .unwrap()
            .owner(SemanticFamily::Npc),
        OwnerHint::One("npcmesh:npc_dexter".to_string())
    );
    assert!(index.hint("mob/null.kfm").is_none());
    assert!(index.hint("sound/dexter_hurt_1.wav").is_some());
    let shared = index.hint("texture/shared/body.dds").unwrap();
    assert_eq!(
        shared.families,
        BTreeSet::from([SemanticFamily::Npc, SemanticFamily::Nano])
    );
    assert_eq!(
        shared.owner(SemanticFamily::Npc),
        OwnerHint::One("npcmesh:npc_dexter".to_string())
    );
    assert_eq!(
        shared.owner(SemanticFamily::Nano),
        OwnerHint::One("nano:nano_dexter".to_string())
    );
}

#[test]
fn item_tables_add_face_head_variants_sounds_and_shared_owners() {
    let mesh_a = object([
        ("m_pstrFMeshModelString", string("f_face_001")),
        ("m_pstrFTextureString", string("f_face_001")),
        ("m_pstrFTextureString2", string("null")),
        ("m_pstrMMeshModelString", string("m_face_001")),
        ("m_pstrMTextureString", string("shared_face")),
        ("m_pstrMTextureString2", string("null")),
    ]);
    let mesh_b = object([
        ("m_pstrFMeshModelString", string("f_face_002")),
        ("m_pstrFTextureString", string("f_face_002")),
        ("m_pstrFTextureString2", string("null")),
        ("m_pstrMMeshModelString", string("m_face_002")),
        ("m_pstrMTextureString", string("shared_face")),
        ("m_pstrMTextureString2", string("null")),
    ]);
    let xdtdatas = object([(
        "m_pFaceItemTable",
        object([
            (
                "m_pItemData",
                array(vec![
                    object([
                        ("m_iMesh", int(1)),
                        ("m_iSound1", int(1)),
                        ("m_iSound2", int(0)),
                    ]),
                    object([
                        ("m_iMesh", int(2)),
                        ("m_iSound1", int(0)),
                        ("m_iSound2", int(0)),
                    ]),
                ]),
            ),
            ("m_pItemMeshData", array(vec![null_mesh(), mesh_a, mesh_b])),
            (
                "m_pItemSoundData",
                array(vec![
                    object([]),
                    object([
                        ("m_pstrSoundString1", string("Face.wav")),
                        ("m_pstrSoundString2", string(" ")),
                        ("m_pstrSoundString3", string("null")),
                    ]),
                ]),
            ),
        ]),
    )]);

    let index = SemanticIndex::from_table_values(&xdtdatas, &empty_hnpc()).unwrap();
    assert!(index.hint("wear/f_face_001_type05.nif").is_some());
    assert!(index.hint("texture/f_face_001_e.dds").is_some());
    assert!(index.hint("sound/face.wav").is_some());
    assert_eq!(
        index
            .hint("texture/shared_face.dds")
            .unwrap()
            .owner(SemanticFamily::Items),
        OwnerHint::Shared
    );
}

#[test]
fn hnpc_only_assets_are_separate_from_item_shared_and_player_base() {
    let item_mesh = object([
        ("m_pstrFMeshModelString", string("hat_shared")),
        ("m_pstrFTextureString", string("hat_shared")),
        ("m_pstrFTextureString2", string("null")),
        ("m_pstrMMeshModelString", string("hat_shared")),
        ("m_pstrMTextureString", string("hat_shared")),
        ("m_pstrMTextureString2", string("null")),
    ]);
    let xdtdatas = object([(
        "m_pHatItemTable",
        object([
            ("m_pItemData", array(vec![object([("m_iMesh", int(1))])])),
            ("m_pItemMeshData", array(vec![null_mesh(), item_mesh])),
        ]),
    )]);
    let all_hnpc = object([(
        "TableElement",
        array(vec![object([
            ("strHairMesh", string("f_hnpc_hair.nif")),
            ("strHairTexture", string("f_hnpc_hair.dds")),
            ("strHatMesh", string("hat_shared.nif")),
            ("strHatTextureM", string("hat_shared.dds")),
            ("strHatTextureS", string("unique_sub.dds")),
        ])]),
    )]);

    let index = SemanticIndex::from_table_values(&xdtdatas, &all_hnpc).unwrap();
    assert_eq!(
        index
            .hint("wear/hat_shared.nif")
            .unwrap()
            .owner(SemanticFamily::Items),
        OwnerHint::One("item:m_phatitemtable:1".to_string())
    );
    assert_eq!(
        index.hint("wear/hat_shared.nif").unwrap().families,
        BTreeSet::from([SemanticFamily::Items])
    );
    assert_eq!(
        index
            .hint("texture/unique_sub.dds")
            .unwrap()
            .owner(SemanticFamily::Hnpc),
        OwnerHint::One("hnpcwear:hat_shared".to_string())
    );
    assert_eq!(
        index.hint("wear/f_hnpc_hair.nif").unwrap().families,
        BTreeSet::from([SemanticFamily::Hnpc])
    );
    assert_eq!(
        index
            .hint("texture/f_hnpc_hair.dds")
            .unwrap()
            .owner(SemanticFamily::Hnpc),
        OwnerHint::One("hnpcwear:f_hnpc_hair".to_string())
    );
    assert_eq!(
        index
            .hint("actor/m.kfm")
            .unwrap()
            .owner(SemanticFamily::Player),
        OwnerHint::One("player:actor".to_string())
    );
    assert_eq!(
        index.hint("texture/f_skin.dds").unwrap().families,
        BTreeSet::from([SemanticFamily::Player])
    );
}

#[test]
fn unclassified_normalizes_and_deduplicates_routes() {
    let index = SemanticIndex::from_table_values(&object([]), &empty_hnpc()).unwrap();
    assert_eq!(
        index.unclassified(["Unknown\\Thing.dds", "unknown/thing.dds", "Actor/M.KFM",]),
        vec!["unknown/thing.dds".to_string()]
    );
}
