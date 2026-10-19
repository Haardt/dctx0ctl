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
                resources: None,
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
        value: VerificationValue::String("a".repeat(MAX_MESSAGE_BYTES).into()),
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
                    resources: None,
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

#[test]
fn resource_observations_are_optional_and_strictly_typed() {
    let original = br#"{"protocol_version":1,"message":{"kind":"reply","id":1,"reply":{"kind":"invoked","observation":{"kind":"returned","value":{"kind":"unit"}},"events":[]}}}"#;
    let mut value: serde_json::Value = serde_json::from_slice(original).unwrap();
    let legacy = decode_message(original, MAX_MESSAGE_BYTES).unwrap();
    assert!(matches!(
        legacy.message,
        WorkerMessage::Reply {
            reply: WorkerReply::Invoked {
                resources: None,
                ..
            },
            ..
        }
    ));
    value["message"]["reply"]["resources"] = serde_json::json!([]);
    let present = decode_message(&serde_json::to_vec(&value).unwrap(), MAX_MESSAGE_BYTES).unwrap();
    assert!(
        matches!(present.message, WorkerMessage::Reply { reply: WorkerReply::Invoked { resources: Some(resources), .. }, .. } if resources.is_empty())
    );
    value["message"]["reply"]["resources"] =
        serde_json::json!([{ "resource":"W.Item", "access":"write", "operation":"W.Store.save" }]);
    assert!(decode_message(&serde_json::to_vec(&value).unwrap(), MAX_MESSAGE_BYTES).is_ok());
    value["message"]["reply"]["resources"][0]["access"] = serde_json::json!("guessed");
    assert!(decode_message(&serde_json::to_vec(&value).unwrap(), MAX_MESSAGE_BYTES).is_err());
}

#[test]
fn dependency_resolution_requests_preserve_the_resolution_mode() {
    for mode in [DependencyResolution::Import, DependencyResolution::Require] {
        let request = RequestEnvelope {
            protocol_version: 1,
            id: 1,
            request: WorkerRequest::ResolveDependency {
                mode,
                specifier: "package".into(),
                importer: "/captured/app.ts".into(),
            },
        };
        let encoded = encode_request(&request, MAX_MESSAGE_BYTES).unwrap();
        let decoded: RequestEnvelope =
            serde_json::from_slice(encoded.strip_suffix(b"\n").unwrap_or(&encoded)).unwrap();
        assert_eq!(decoded, request);
    }
}

#[test]
fn explicitly_budgeted_large_frames_preserve_lower_caller_limits() {
    let request = RequestEnvelope {
        protocol_version: SDK_VERSION,
        id: 1,
        request: WorkerRequest::Validate {
            subject: "W.Body".into(),
            value: VerificationValue::String("x".repeat(2 * DEFAULT_MESSAGE_BYTES).into()),
        },
    };
    assert!(encode_request(&request, DEFAULT_MESSAGE_BYTES).is_err());
    let encoded = encode_request(&request, 4 * DEFAULT_MESSAGE_BYTES).unwrap();
    assert!(encoded.len() > 2 * DEFAULT_MESSAGE_BYTES);
    assert!(encoded.len() < 4 * DEFAULT_MESSAGE_BYTES);
    let reply = MessageEnvelope {
        protocol_version: SDK_VERSION,
        message: WorkerMessage::Reply {
            id: 1,
            reply: WorkerReply::ValueChecked {
                outcome: ValueCheck::Accepted {
                    value: VerificationValue::String("x".repeat(2 * DEFAULT_MESSAGE_BYTES).into()),
                },
            },
        },
    };
    let encoded = serde_json::to_vec(&reply).unwrap();
    assert!(decode_message(&encoded, DEFAULT_MESSAGE_BYTES).is_err());
    assert!(decode_message(&encoded, 4 * DEFAULT_MESSAGE_BYTES).is_ok());
}
