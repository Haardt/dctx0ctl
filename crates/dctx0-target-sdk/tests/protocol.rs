use dctx0_core::*;
use dctx0_target_sdk::*;
#[test]
fn protocol_rejects_versions_success_flags_corruption_and_resource_exhaustion() {
    let valid =
        br#"{"protocol_version":1,"message":{"kind":"reply","id":1,"reply":{"kind":"ended"}}}"#;
    assert!(matches!(
        decode_message(valid, MAX_MESSAGE_BYTES).unwrap().message,
        WorkerMessage::Reply {
            reply: WorkerReply::Ended,
            ..
        }
    ));
    for bytes in [b"{".as_slice(),br#"{"protocol_version":2,"message":{"kind":"reply","id":1,"reply":{"kind":"ended"}}}"#,br#"{"protocol_version":1,"message":{"kind":"reply","id":1,"reply":{"kind":"ended","PASS":true}}}"#,br#"{"protocol_version":1,"message":{"kind":"reply","id":0,"reply":{"kind":"ended"}}}"#,br#"{"protocol_version":1,"message":{"kind":"mock-call","request_id":9007199254740992,"invocation":{"subject":"X","arguments":[]}}}"#] {
  assert!(decode_message(bytes,MAX_MESSAGE_BYTES).unwrap_err().to_string().starts_with("Protocol:"));
 }
    assert!(decode_message(valid, 1).is_err());
    assert!(decode_message(&vec![b' '; MAX_MESSAGE_BYTES + 1], usize::MAX).is_err());
    let technical = MessageEnvelope {
        protocol_version: 1,
        message: WorkerMessage::TechnicalError {
            id: None,
            category: "cleanup".into(),
            message: "Unavailable".into(),
        },
    };
    assert_eq!(
        decode_message(&serde_json::to_vec(&technical).unwrap(), MAX_MESSAGE_BYTES).unwrap(),
        technical
    );
    let nested = MessageEnvelope {
        protocol_version: 1,
        message: WorkerMessage::Reply {
            id: 1,
            reply: WorkerReply::Invoked {
                calls: None,
                observation: Observation::Returned {
                    value: VerificationValue::List(vec![
                        VerificationValue::Unit,
                        VerificationValue::Integer("1".into()),
                    ]),
                },
                events: vec![],
            },
        },
    };
    assert_eq!(
        decode_message(&serde_json::to_vec(&nested).unwrap(), MAX_MESSAGE_BYTES).unwrap(),
        nested
    );
    for value in [
        serde_json::json!({"kind":"unit","PASS":true}),
        serde_json::json!({"kind":"integer","data":"1"}),
    ] {
        let message = serde_json::json!({"protocol_version":1,"message":{"kind":"reply","id":1,"reply":{"kind":"invoked","observation":{"kind":"returned","value":value},"events":[]}}});
        assert!(decode_message(&serde_json::to_vec(&message).unwrap(), MAX_MESSAGE_BYTES).is_err());
    }
}
#[test]
fn request_frames_preserve_exact_values_and_use_safe_positive_identifiers() {
    let mut request = RequestEnvelope {
        protocol_version: 1,
        id: MAX_REQUEST_ID,
        request: WorkerRequest::Invoke {
            invocation: Invocation {
                subject: "X".into(),
                arguments: vec![
                    VerificationValue::Integer("9007199254740993".into()),
                    VerificationValue::Decimal("0.1000000000000000001".into()),
                ],
            },
        },
    };
    let bytes = encode_request(&request, MAX_MESSAGE_BYTES).unwrap();
    assert_eq!(*bytes.last().unwrap(), b'\n');
    assert_eq!(
        serde_json::from_slice::<RequestEnvelope>(&bytes).unwrap(),
        request
    );
    assert!(encode_request(&request, 1).is_err());
    for id in [0, MAX_REQUEST_ID + 1] {
        request.id = id;
        assert!(encode_request(&request, MAX_MESSAGE_BYTES).is_err());
    }
    request.id = 1;
    request.protocol_version = 2;
    assert!(encode_request(&request, MAX_MESSAGE_BYTES).is_err());
    request.protocol_version = 1;
    request.request = WorkerRequest::Validate {
        subject: "X".into(),
        value: VerificationValue::String("a".repeat(MAX_MESSAGE_BYTES)),
    };
    assert!(encode_request(&request, usize::MAX).is_err());
}

#[test]
fn absent_call_observers_differ_from_observed_zero_and_preserve_actual_calls() {
    for calls in [
        None,
        Some(vec![]),
        Some(vec![InvocationObservation {
            invocation: Invocation {
                subject: "Store.load".into(),
                arguments: vec![VerificationValue::Integer("9007199254740993".into())],
            },
            observation: Observation::Returned {
                value: VerificationValue::Unit,
            },
        }]),
    ] {
        let frame = MessageEnvelope {
            protocol_version: 1,
            message: WorkerMessage::Reply {
                id: 1,
                reply: WorkerReply::Invoked {
                    observation: Observation::Returned {
                        value: VerificationValue::Unit,
                    },
                    events: vec![],
                    calls: calls.clone(),
                },
            },
        };
        let bytes = serde_json::to_vec(&frame).unwrap();
        assert_eq!(decode_message(&bytes, MAX_MESSAGE_BYTES).unwrap(), frame);
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            json["message"]["reply"].get("calls").is_some(),
            calls.is_some()
        );
    }
    let malformed=br#"{"protocol_version":1,"message":{"kind":"reply","id":1,"reply":{"kind":"invoked","observation":{"kind":"returned","value":{"kind":"unit"}},"events":[],"calls":null}}}"#;
    assert!(decode_message(malformed, MAX_MESSAGE_BYTES).is_err());
}
