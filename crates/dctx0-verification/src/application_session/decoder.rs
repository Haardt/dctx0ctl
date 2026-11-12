use super::{DecoderOutcome, failed, issue, unknown};
use crate::ProgramPlan;
use dctx0_core::*;
use dctx0_interpreter::{ValueContract, check_value};

pub(super) struct DecoderCase {
    pub subject: String,
    pub ty: TypeRef,
    pub input: VerificationValue,
    pub rejection: bool,
}

pub(super) fn case(plan: &ProgramPlan, id: &str) -> Option<DecoderCase> {
    for contract in &plan.contracts {
        for (kind, corpus, rejection) in [
            ("value", &contract.corpus, false),
            ("rejection", &contract.rejection_corpus, true),
        ] {
            let prefix = format!("{}:{kind}:", contract.subject);
            if let Some(index) = id
                .strip_prefix(&prefix)
                .and_then(|s| s.parse::<usize>().ok())
                && let Some(input) = corpus.values.get(index)
            {
                return Some(DecoderCase {
                    subject: contract.subject.clone(),
                    ty: contract.ty.clone(),
                    input: input.clone(),
                    rejection,
                });
            }
        }
    }
    None
}

pub(super) fn assess(
    ir: &WorkspaceIr,
    plan: &ProgramPlan,
    case: &DecoderCase,
    raw: &[u8],
    outcome: DecoderOutcome,
    record: &mut VerificationCaseRecord,
) -> VerificationResult {
    if raw_json(&case.input).as_deref() != Ok(raw) {
        return failed(
            "APPLICATION_INPUT_CHANGED",
            "Production decoder input differs from its centrally selected case",
        );
    }
    assess_outcome(ir, plan, case, outcome, record)
}

pub(super) fn assess_outcome(
    ir: &WorkspaceIr,
    plan: &ProgramPlan,
    case: &DecoderCase,
    outcome: DecoderOutcome,
    record: &mut VerificationCaseRecord,
) -> VerificationResult {
    match outcome {
        DecoderOutcome::BindingError { message } => unknown("APPLICATION_BINDING_ERROR", message),
        DecoderOutcome::Exception { message } => unknown("APPLICATION_EXCEPTION", message),
        DecoderOutcome::Rejected { .. } if case.rejection => VerificationResult::Pass,
        DecoderOutcome::Rejected { reason } => failed("NATIVE_VALUE_REJECTED", reason),
        DecoderOutcome::Accepted { .. } if case.rejection => failed(
            "NATIVE_INVALID_VALUE_ACCEPTED",
            "Production accepted a centrally rejected input",
        ),
        DecoderOutcome::Accepted { value } => {
            record.bindings.insert("native_value".into(), value.clone());
            match check_value(ir, &case.ty, &value, &plan.budgets) {
                ValueContract::Accepted => crate::constructor_value_equality(
                    ir,
                    &case.subject,
                    &case.ty,
                    &case.input,
                    &value,
                    &plan.budgets,
                ),
                ValueContract::Rejected { reason } => failed("NATIVE_VALUE_CONTRACT", reason),
                ValueContract::Unknown { issue } => VerificationResult::Unknown { issue },
            }
        }
    }
}

/// Default JSON testing representation. Integer/Float text remains exact; JSON
/// serialization is separate from the production decoder and cannot reject for it.
pub(super) fn raw_json(value: &VerificationValue) -> Result<Vec<u8>, VerificationIssue> {
    fn text(value: &VerificationValue) -> Result<String, VerificationIssue> {
        use VerificationValue as V;
        let encoded = match value {
            V::Unit | V::Option(None) => "null".into(),
            V::Boolean(v) => v.to_string(),
            V::Integer(v) | V::Decimal(v) | V::Float(v) => {
                // JSON syntax validation must not convert through f64 and lose digits.
                if !number(v) {
                    return Err(issue(
                        "APPLICATION_JSON_NUMBER",
                        "Unrepresentable JSON number",
                    ));
                }
                if matches!(value, V::Float(_)) && !v.contains(['.', 'e', 'E']) {
                    format!("{v}.0")
                } else {
                    v.clone()
                }
            }
            V::String(v) => serde_json::to_string(v)
                .map_err(|e| issue("APPLICATION_JSON_CODEC", e.to_string()))?,
            V::List(v) => format!(
                "[{}]",
                v.iter().map(text).collect::<Result<Vec<_>, _>>()?.join(",")
            ),
            V::Record(fields) => {
                let pairs = fields
                    .iter()
                    .map(|(key, value)| {
                        let key = serde_json::to_string(key)
                            .map_err(|e| issue("APPLICATION_JSON_CODEC", e.to_string()))?;
                        Ok(format!("{key}:{}", text(value)?))
                    })
                    .collect::<Result<Vec<_>, VerificationIssue>>()?;
                format!("{{{}}}", pairs.join(","))
            }
            V::Option(Some(v)) => text(v)?,
            V::Variant {
                name,
                payload: None,
                ..
            } => serde_json::to_string(name)
                .map_err(|e| issue("APPLICATION_JSON_CODEC", e.to_string()))?,
            V::Variant { .. } | V::Failure { .. } => {
                return Err(issue(
                    "APPLICATION_JSON_CODEC",
                    "Payload variants/failure inputs need an explicit JSON binding",
                ));
            }
        };
        Ok(encoded)
    }
    text(value).map(String::into_bytes)
}

fn number(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = usize::from(bytes.first() == Some(&b'-'));
    if i >= bytes.len() {
        return false;
    }
    if bytes[i] == b'0' {
        i += 1;
    } else if matches!(bytes[i], b'1'..=b'9') {
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    } else {
        return false;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let first = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == first {
            return false;
        }
    }
    if bytes.get(i).is_some_and(|b| matches!(b, b'e' | b'E')) {
        i += 1;
        if bytes.get(i).is_some_and(|b| matches!(b, b'+' | b'-')) {
            i += 1;
        }
        let first = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == first {
            return false;
        }
    }
    i == bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VerificationScope;

    #[test]
    fn json_binding_preserves_digits_and_rejects_unrepresentable_values_before_production() {
        for text in [
            "", "-", "+1", "00", "01", "--1", ".1", "1.", "1e", "1e+", "NaN", "Infinity", "1 2",
        ] {
            assert_eq!(
                raw_json(&VerificationValue::Integer(text.into()))
                    .unwrap_err()
                    .code,
                "APPLICATION_JSON_NUMBER",
                "{text}"
            );
        }
        for text in [
            "0",
            "-0",
            "1e+1234",
            "-1.25E-09",
            "900719925474099312345678901234567890",
        ] {
            assert_eq!(
                raw_json(&VerificationValue::Decimal(text.into())).unwrap(),
                text.as_bytes()
            );
        }
        assert_eq!(
            raw_json(&VerificationValue::Float("1".into())).unwrap(),
            b"1.0"
        );
        for value in [
            VerificationValue::Variant {
                declaration: "V".into(),
                name: "Some".into(),
                payload: Some(Box::new(VerificationValue::Unit)),
            },
            VerificationValue::Failure {
                declaration: "Failure".into(),
                payload: None,
            },
        ] {
            assert_eq!(raw_json(&value).unwrap_err().code, "APPLICATION_JSON_CODEC");
        }
    }

    #[test]
    fn decoder_rejection_binding_exception_and_unknown_scope_are_not_successful_constructors() {
        let source = Source {
            file: "year.dctx0".into(),
            text: "workspace W { value Year = Int { range 3..4 } }".into(),
        };
        let mut ir = dctx0_language::compile(&[source]).ir.unwrap();
        let mut plan = crate::project_program(
            &ir,
            "snapshot",
            1,
            VerificationScope {
                subjects: ["W.Year".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let case = case(&plan, "W.Year:value:0").unwrap();
        let mut record = VerificationCaseRecord {
            id: "W.Year:value:0".into(),
            subject: "W.Year".into(),
            kind: VerificationCaseKind::Contract,
            bindings: Default::default(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        };
        for (outcome, expected) in [
            (
                DecoderOutcome::Rejected {
                    reason: "unexpected rejection".into(),
                },
                "NATIVE_VALUE_REJECTED",
            ),
            (
                DecoderOutcome::BindingError {
                    message: "codec failed".into(),
                },
                "APPLICATION_BINDING_ERROR",
            ),
            (
                DecoderOutcome::Exception {
                    message: "I/O failure".into(),
                },
                "APPLICATION_EXCEPTION",
            ),
            (
                DecoderOutcome::Accepted {
                    value: VerificationValue::Integer("9".into()),
                },
                "NATIVE_VALUE_CONTRACT",
            ),
        ] {
            let result = assess_outcome(&ir, &plan, &case, outcome, &mut record);
            let code = match result {
                VerificationResult::Fail { issue } | VerificationResult::Unknown { issue } => {
                    issue.code
                }
                _ => panic!("Unexpected constructor PASS"),
            };
            assert_eq!(code, expected);
        }
        plan.budgets.evaluation_steps = 0;
        assert!(matches!(
            crate::constructor_value_equality(
                &ir,
                &case.subject,
                &case.ty,
                &case.input,
                &case.input,
                &plan.budgets
            ),
            VerificationResult::Unknown { .. }
        ));
        ir.symbols.remove("W.Year");
        let VerificationResult::Unknown { issue } = crate::constructor_value_equality(
            &ir,
            &case.subject,
            &case.ty,
            &case.input,
            &case.input,
            &plan.budgets,
        ) else {
            panic!("Unknown source must not be proved")
        };
        assert_eq!(issue.code, "APPLICATION_IR_SCOPE");
    }
    #[test]
    fn native_decoder_uses_the_shared_constructor_comparison_and_retains_actual_text() {
        let ir = dctx0_language::compile(&[Source {
            file: "float.dctx0".into(),
            text: "workspace W { value X = Float }".into(),
        }])
        .ir
        .unwrap();
        let plan = crate::project_program(
            &ir,
            "snapshot",
            1,
            crate::VerificationScope {
                subjects: ["W.X".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let mut c = case(&plan, "W.X:value:0").unwrap();
        c.input = VerificationValue::Float("1e308".into());
        let mut record = VerificationCaseRecord {
            id: "W.X:value:0".into(),
            subject: "W.X".into(),
            kind: VerificationCaseKind::Contract,
            bindings: Default::default(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        };
        for (text, pass) in [("1e+308", true), ("1.0000000000000002e308", false)] {
            let actual = VerificationValue::Float(text.into());
            let native = assess_outcome(
                &ir,
                &plan,
                &c,
                DecoderOutcome::Accepted {
                    value: actual.clone(),
                },
                &mut record,
            );
            let shared = crate::constructor_value_equality(
                &ir,
                &c.subject,
                &c.ty,
                &c.input,
                &actual,
                &plan.budgets,
            );
            assert_eq!(native, shared);
            assert_eq!(matches!(native, VerificationResult::Pass), pass);
            assert_eq!(record.bindings["native_value"], actual);
            assert_eq!(c.input, VerificationValue::Float("1e308".into()));
        }
    }
}
