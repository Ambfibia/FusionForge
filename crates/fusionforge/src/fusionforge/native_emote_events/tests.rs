use super::*;
#[test]
fn events_preserve_sound_order_and_require_one_end() {
    let rows = json!([{"functionName":"sound","time":0.5,"data":"x"},{"functionName":"sound","time":0.2,"data":"y"},{"functionName":"end","time":1.0}]);
    assert_eq!(
        events(&rows).unwrap(),
        (1.0, vec![(0.5, "x".into()), (0.2, "y".into())])
    );
    assert!(events(&json!([])).is_err());
    assert!(
        events(&json!([{"functionName":"end","time":1.0},{"functionName":"end","time":2.0}]))
            .is_err()
    );
    assert!(events(&json!([{"functionName":"end","time":-1.0}])).is_err());
}
