use dctx0_core::VerificationValue;
use dctx0_target_sdk::application_protocol::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationPayload {
    raw: Vec<u8>,
    value: VerificationValue,
}

#[test]
fn application_frames_preserve_invalid_raw_bytes_and_exact_values_without_worker_packets() {
    for value in [
        VerificationValue::Integer("90071992547409931234567890".into()),
        VerificationValue::String("🦀\0e\u{301}".into()),
    ] {
        let packet = AssessmentFrame::new(
            1,
            ObservationPayload {
                raw: vec![0xff, 0, b'{', b'"'],
                value,
            },
        );
        let bytes = encode(&packet, 1024).unwrap();
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert_eq!(decode::<ObservationPayload>(&bytes, 1024).unwrap(), packet);
        assert!(decode::<ObservationPayload>(&bytes, bytes.len() - 1).is_err());
        assert!(encode(&packet, 1).is_err());
    }
}

#[test]
fn frames_reject_versions_ids_malformed_data_unknown_fields_and_unbounded_budgets() {
    for id in [0, dctx0_target_sdk::MAX_REQUEST_ID + 1] {
        assert!(encode(&AssessmentFrame::new(id, true), 1024).is_err());
    }
    let mut packet = AssessmentFrame::new(1, true);
    packet.protocol_version += 1;
    assert!(encode(&packet, 1024).is_err());
    for limit in [0, dctx0_target_sdk::MAX_MESSAGE_BYTES + 1] {
        assert!(encode(&AssessmentFrame::new(1, true), limit).is_err());
        assert!(decode::<bool>(b"{}", limit).is_err());
    }
    for bytes in [
        br#"{"protocol_version":1,"id":1,"payload":true}"#.as_slice(),
        br#"{"protocol_version":2,"id":0,"payload":true}"#,
        br#"{"protocol_version":2,"id":1,"payload":true,"status":"PASS"}"#,
        br#"{"protocol_version":2,"id":1,"payload":true} {}"#,
        br#"{"protocol_version":2,"id":1,"request":{"kind":"describe"}}"#,
        b"\xff",
    ] {
        assert!(decode::<bool>(bytes, 1024).is_err());
    }
}

#[test]
fn payload_serialization_failure_is_a_protocol_error_not_a_successful_observation() {
    struct FailingPayload;
    impl Serialize for FailingPayload {
        fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom(
                "Observation serialization failed",
            ))
        }
    }
    let error = encode(&AssessmentFrame::new(1, FailingPayload), 1024).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Observation serialization failed")
    );
}
