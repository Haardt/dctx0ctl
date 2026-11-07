#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variant_payloads_and_options() {
        for value in [
            json!({"kind":"Empty","payload":null}),
            json!({"kind":"Count","payload":12}),
            json!({"kind":"Maybe","payload":null}),
            json!({"kind":"Maybe","payload":"present"}),
        ] {
            let wire = encode("W.Choice", &value).unwrap();
            assert_eq!(decode("W.Choice", &wire).unwrap(), value);
        }
        let invocation = json!({"subject":"actual","arguments":[]});
        let calls = vec![json!({"invocation":invocation})];
        assert!(recorded_calls(Some(&calls), &invocation, 1).is_ok());
        assert!(recorded_calls(None, &invocation, 1).is_err());
        assert!(recorded_calls(Some(&calls), &invocation, 0).is_err());
        let error = crate::contract::TechnicalError::environment("private");
        assert!(technical_failure(&error, "environment").is_ok());
        assert!(technical_failure(&error, "adapter").is_err());
        assert!(
            unavailable_observation(Ok(
                &json!({"kind":"unavailable","reason":"missing actual state"})
            ))
            .is_ok()
        );
    }
    #[test]
    fn exact_numbers_nested_records_and_invalid_input() {
        let exact = "900719925474099312345678901234567890";
        let wire = json!({"kind":"integer","value":exact});
        let n = decode("W.Id", &wire).unwrap();
        assert_eq!(n.to_string(), exact);
        assert_eq!(encode("W.Id", &n).unwrap(), wire);
        let native = json!({"id":12,"child":{"code":"ok"},"items":["a"]});
        let wire = encode("W.Record", &native).unwrap();
        assert_eq!(decode("W.Record", &wire).unwrap(), native);
        let invalid = json!({"kind":"record","value":{"extra":{"kind":"boolean","value":true}}});
        assert!(decode("W.Record", &invalid).is_err());
        assert_eq!(
            decode_input("W.Record", &invalid).unwrap(),
            json!({"extra":true})
        );
        assert!(decode("W.Unsupported", &json!({"kind":"string","value":"x"})).is_err());
        assert!(
            decode(
                "W.NestedOption",
                &json!({"kind":"option","value":{"kind":"option","value":null}})
            )
            .is_err()
        );
    }
    #[test]
    fn independent_failure_observations() {
        let record = json!({"kind":"record","value":{"code":{"kind":"string","value":"DENIED"}}});
        let o = json!({"kind":"returned","value":{"kind":"failure","value":{"declaration":"W.Denied","payload":record}}});
        let f = declared_failure(Ok(&o), "W.Denied").unwrap();
        assert!(string_field(&f["payload"], "code", "DENIED").is_ok());
        assert!(string_field(&f["payload"], "code", "WRONG").is_err());
        assert!(returned_record(Ok(&o)).is_err());
        assert!(declared_failure(Ok(&o), "W.Record").is_err());
        let technical = crate::contract::TechnicalError::environment("secret");
        assert!(declared_failure(Err(&technical), "W.Denied").is_err());
        for kind in ["threw", "unavailable"] {
            assert!(declared_failure(Ok(&json!({"kind":kind})), "W.Denied").is_err());
        }
    }
    #[test]
    fn child_environment_and_cleanup_on_failure() {
        let mut env = std::collections::BTreeMap::new();
        env.insert("PATH".into(), None);
        let cmd =
            crate::support::plan_command("unused", std::path::Path::new("."), &[], &env).unwrap();
        assert!(cmd.get_envs().any(|(k, v)| k == "PATH" && v.is_none()));
        assert!(std::env::var_os("PATH").is_some());
        let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let check = closed.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _fixture = crate::support::Fixture {
                environment: env,
                cleanup: Some(Box::new(move || {
                    closed.store(true, std::sync::atomic::Ordering::SeqCst)
                })),
            };
            panic!("deliberate assertion failure")
        }));
        assert!(result.is_err());
        assert!(check.load(std::sync::atomic::Ordering::SeqCst));
    }
}
