use dctx0_core::{VerificationCause as C, VerificationIssue, VerificationResult as R};
use dctx0_verification::diagnostics::diagnose_result;

#[test]
fn structured_causes_preserve_outcomes_and_do_not_guess_from_messages() {
    for (code, cause) in [
        ("OPERATION_CLAUSE_VIOLATED", C::ContractViolation),
        ("NATIVE_RETURN_CONTRACT", C::ContractViolation),
        ("STATE_OBSERVER_REQUIRED", C::Adapter),
        ("NATIVE_PROTOCOL_ERROR", C::Adapter),
        ("NATIVE_ENVIRONMENT_UNAVAILABLE", C::Environment),
        ("VALUE_DEPTH", C::EvaluatorLimit),
        ("EVALUATION_BUDGET", C::EvaluatorLimit),
        ("UNSUPPORTED_OPERATOR", C::EvaluatorLimit),
        ("UNSUPPORTED_BUILTIN", C::EvaluatorLimit),
        ("MISSING_CASE", C::MissingEvidence),
        ("MANUAL_REQUIREMENT", C::InspectionRequired),
        ("APPLICATION_EXCEPTION", C::Unclassified),
        ("FUTURE_CODE", C::Unclassified),
    ] {
        // Includes a historical FAIL exception. A projection cannot rewrite it.
        let result = R::Fail {
            issue: VerificationIssue {
                code: code.into(),
                message: "database adapter timeout".into(),
                span: None,
            },
        };
        let before = result.clone();
        let diagnosis = diagnose_result("W.Access.create", &result).unwrap();
        assert_eq!(diagnosis.cause, cause);
        assert_eq!(diagnosis.subject, "W.Access.create");
        assert!(!diagnosis.next_step.is_empty());
        assert_eq!(result, before);
    }
    assert!(diagnose_result("W.Access.create", &R::Pass).is_none());
}
