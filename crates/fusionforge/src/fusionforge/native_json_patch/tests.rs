use super::*;
use serde_json::json;
#[test]
fn null_preimage_is_distinct_from_missing_property() {
    let edits: Vec<Edit> = serde_json::from_value(json!([
        {"path":"/null","before":null,"after":1},
        {"path":"/new","after":2}
    ]))
    .unwrap();
    assert!(apply(br#"{}"#, &edits).is_err());
    assert!(apply(br#"{"null":null,"new":null}"#, &edits).is_err());
    let output = apply(br#"{"null":null}"#, &edits).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        json!({"null":1,"new":2})
    );
}
#[test]
fn edits_are_guarded_idempotent_and_preserve_float_spelling() {
    let bytes = b"{\r\n  \"other\": 1.2345000,\r\n  \"m_pNanoTable\": {\"id\": 52}\r\n}\r\n";
    let edits = vec![Edit {
        path: "/@tables/m_pNanoTable/id".into(),
        before: Some(json!(52)),
        after: json!(0),
    }];
    let after = apply(bytes, &edits).unwrap();
    assert_eq!(
        after,
        String::from_utf8_lossy(bytes).replace("52", "0").as_bytes()
    );
    assert_eq!(apply(&after, &edits).unwrap(), after);
    assert!(apply(b"{\"m_pNanoTable\":{\"id\":66}}", &edits).is_err());
}
#[test]
fn semantic_table_selection_and_append_do_not_depend_on_table_order() {
    let bytes=br#"{"tables":[{"name":"other","value":{}},{"name":"xdt","value":{"m_pNanoTable":{"rows":[0]}}}]}"#;
    let edits = vec![Edit {
        path: "/@tables/m_pNanoTable/rows/1".into(),
        before: None,
        after: json!({"id":67}),
    }];
    let after = apply(bytes, &edits).unwrap();
    let value: Value = serde_json::from_slice(&after).unwrap();
    assert_eq!(
        value["tables"][1]["value"]["m_pNanoTable"]["rows"],
        json!([0,{"id":67}])
    );
    assert_eq!(apply(&after, &edits).unwrap(), after);
}
