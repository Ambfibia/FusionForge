use super::*;
#[test]
fn server_sync_rejects_unrelated_changes_and_preserves_server_extensions() {
    let value = json!({"m_pNanoTable":{"m_pNanoData":[{"m_iNanoNumber":1,"m_iTune":[1,1,1]}],
        "m_pNanoTuneData":[{"m_iTuneNumber":0,"m_iSkillID":0},{"m_iTuneNumber":1,"m_iSkillID":1}]},
        "m_pSkillTable":{"m_pSkillData":[{"m_iSkillNumber":0},{"m_iSkillNumber":1}]}});
    let mut native = json!({"tables":[{"value":value}]});
    let mut server = value.clone();
    server["m_pNanoTable"]["m_pNanoTuneData"][1]["m_iTuneNumber"] = json!(0);
    server["unrelated"] = json!({"keep":[1,2,3]});
    for (table, rows, field) in [
        ("m_pNanoTable", "m_pNanoData", "m_iNanoNumber"),
        ("m_pNanoTable", "m_pNanoTuneData", "m_iSkillID"),
        ("m_pSkillTable", "m_pSkillData", "m_iSkillNumber"),
    ] {
        let mut invalid = server.clone();
        invalid[table][rows][0][field] = json!(999);
        assert!(sync_nano_tuning(&mut native, &mut invalid).is_err());
    }
    sync_nano_tuning(&mut native, &mut server).unwrap();
    assert_eq!(server["m_pNanoTable"], value["m_pNanoTable"]);
    assert_eq!(server["unrelated"], json!({"keep":[1,2,3]}));
    let after = server.clone();
    sync_nano_tuning(&mut native, &mut server).unwrap();
    assert_eq!(server, after);
}
#[test]
fn alias_repair_preserves_every_other_row_and_refuses_other_owners() {
    let mut rows = vec![json!({"m_iNanoNumber":0}); 71];
    rows[41] = json!({"m_iNanoNumber":41,"m_iMesh":40});
    rows[66] = json!({"m_iNanoNumber":66,"m_iMesh":52});
    rows[52] = json!({"m_iNanoNumber":52,"m_iMesh":52,"m_iNanoName":51});
    let mut table = json!({"m_pNanoData":rows,"keep":{"x":1}});
    let original = table.clone();
    retire_nano_alias(&mut table).unwrap();
    let result = table.clone();
    retire_nano_alias(&mut table).unwrap();
    assert_eq!(table, result);
    table["m_pNanoData"][52] = original["m_pNanoData"][52].clone();
    assert_eq!(table, original);
    table["m_pNanoData"][66]["m_iMesh"] = json!(99);
    assert!(retire_nano_alias(&mut table).is_err());
}
#[test]
fn fred_rejects_truncation_and_wrong_version() {
    assert!(fred_skin(b"glTF").is_err());
    assert!(fred_skin(&[0; 32]).is_err());
}
