use dctx0_core::*;

#[test]
fn historical_case_records_do_not_invent_resource_observations() {
    let record: VerificationCaseRecord = serde_json::from_value(serde_json::json!({
        "id":"case", "subject":"W.Operation", "kind":"operation", "bindings":{},
        "setup":[], "arguments":[], "observations":[], "events":null, "states":[],
        "result":{"status":"PASS"}, "elapsed_us":1
    }))
    .unwrap();
    assert_eq!(record.resources, None);
    assert!(
        serde_json::to_value(&record)
            .unwrap()
            .get("resources")
            .is_none()
    );
    let observed = VerificationCaseRecord {
        resources: Some(vec![]),
        ..record
    };
    assert_eq!(
        serde_json::to_value(&observed).unwrap()["resources"],
        serde_json::json!([])
    );
    let read: ResourceObservation =
        serde_json::from_value(serde_json::json!({"resource":"W.Item", "access":"read"})).unwrap();
    assert_eq!(read.operation, None);
    assert_eq!(read.access, ResourceAccess::Read);
    assert!(
        serde_json::from_value::<ResourceObservation>(
            serde_json::json!({"resource":"W.Item", "access":"read", "count":1})
        )
        .is_err()
    );
}
