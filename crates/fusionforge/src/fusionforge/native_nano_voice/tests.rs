use super::*;
#[test]
fn sound_repair_preserves_other_events_and_is_idempotent() {
    let mut a = json!({"extras":{"duration":1.0,"nonTrs":{"events":[{"time":0.1,"functionName":"end","intParameter":7},{"time":0.5,"functionName":"sound","stringParameter":"wrong"}]}}});
    let source = json!({"m_Events":[{"time":0.25,"functionName":"sound","data":"accepted","messageOptions":0}]});
    sounds(&mut a, &source).unwrap();
    assert_eq!(a["extras"]["nonTrs"]["events"][0]["intParameter"], 7);
    let before = a.clone();
    sounds(&mut a, &source).unwrap();
    assert_eq!(before, a);
}
#[test]
fn event_past_duration_is_rejected() {
    let mut a = json!({"extras":{"duration":1.0,"nonTrs":{"events":[]}}});
    assert!(
        sounds(
            &mut a,
            &json!({"m_Events":[{"time":2.0,"functionName":"sound"}]})
        )
        .is_err()
    );
}
