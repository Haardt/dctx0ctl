use dctx0_core::*;
use dctx0_verification::*;
use std::collections::{BTreeMap, BTreeSet};

fn ir() -> WorkspaceIr {
    dctx0_language::compile(&[Source {
        file: "spec.dctx0".into(),
        text: "workspace W { surface M { operation identity(x: Int) -> Int { pure } } }".into(),
    }])
    .ir
    .unwrap()
}
fn expr(value: ExpressionKind) -> Expression {
    Expression {
        ty: TypeRef::builtin("Boolean"),
        span: Span {
            file: "spec.dctx0".into(),
            start: Position {
                offset: 0,
                line: 1,
                column: 1,
            },
            end: Position {
                offset: 1,
                line: 1,
                column: 2,
            },
        },
        value,
    }
}
fn literal(kind: &str, text: &str) -> Expression {
    expr(ExpressionKind::Literal {
        text: text.into(),
        literal_kind: kind.into(),
    })
}
fn issue() -> VerificationIssue {
    VerificationIssue {
        code: "MISSING_OBSERVER".into(),
        message: "Cannot observe effects".into(),
        span: None,
    }
}
fn case(assertion: Expression) -> VerificationCase {
    VerificationCase {
        id: "case-1".into(),
        variables: BTreeMap::new(),
        assertion,
    }
}
fn plan(assertion: Expression) -> VerificationPlan {
    VerificationPlan {
        schema_version: 1,
        snapshot: "pinned-snapshot".into(),
        check_occurrence: 1,
        scope: VerificationScope {
            subjects: BTreeSet::from(["W.M.identity".into()]),
            tasks: BTreeSet::new(),
        },
        budgets: VerificationBudgets::default(),
        obligations: vec![Obligation {
            id: "obligation-1".into(),
            subject: "W.M.identity".into(),
            cases: vec![case(assertion)],
            unsupported: None,
            bounded_corpus: false,
        }],
    }
}
fn evidence() -> Vec<CaseEvidence> {
    vec![CaseEvidence {
        case_id: "case-1".into(),
        observations: vec![],
    }]
}
fn unknown(report: &PlanResult, code: &str) {
    assert!(
        matches!(&report.result, VerificationResult::Unknown { issue } if issue.code == code),
        "{report:?}"
    );
}
#[test]
fn mandatory_cases_empty_scope_unsupported_requirements_and_surplus_evidence_block_pass() {
    let ir = ir();
    let plan = plan(literal("Boolean", "true"));
    assert_eq!(
        assess(&ir, &plan, &evidence()).result,
        VerificationResult::Pass
    );
    assert_eq!(
        serde_json::from_str::<VerificationPlan>(&serde_json::to_string(&plan).unwrap()).unwrap(),
        plan
    );
    let missing = assess(&ir, &plan, &[]);
    unknown(&missing, "MISSING_CASE");
    assert_eq!(missing.incomplete_obligations, vec!["obligation-1"]);
    for variant in 0..11 {
        let mut invalid = plan.clone();
        match variant {
            0 => invalid.schema_version = 2,
            1 => invalid.snapshot.clear(),
            2 => invalid.check_occurrence = 0,
            3 => invalid.scope.subjects.clear(),
            4 => invalid.obligations.clear(),
            5 => invalid.obligations.push(invalid.obligations[0].clone()),
            6 => invalid.obligations[0].id.clear(),
            7 => invalid.obligations[0].subject = "other".into(),
            8 => {
                let duplicate = invalid.obligations[0].cases[0].clone();
                invalid.obligations[0].cases.push(duplicate);
            }
            9 => invalid.obligations[0].cases[0].id.clear(),
            _ => invalid
                .scope
                .subjects
                .insert("uncovered".into())
                .then_some(())
                .unwrap(),
        }
        unknown(
            &assess(&ir, &invalid, &evidence()),
            if variant == 10 {
                "MISSING_OBLIGATION"
            } else {
                "INVALID_PLAN"
            },
        );
    }
    let mut surplus = evidence();
    surplus[0].case_id = "other".into();
    unknown(&assess(&ir, &plan, &surplus), "INVALID_EVIDENCE");
    let mut duplicate = evidence();
    duplicate.extend(evidence());
    unknown(&assess(&ir, &plan, &duplicate), "INVALID_EVIDENCE");
    let mut unsupported = plan.clone();
    unsupported.obligations[0].unsupported = Some(issue());
    unknown(&assess(&ir, &unsupported, &evidence()), "MISSING_OBSERVER");
    let mut empty = plan.clone();
    empty.obligations[0].cases.clear();
    unknown(&assess(&ir, &empty, &[]), "EMPTY_CORPUS");
    let mut bounded = plan.clone();
    bounded.obligations[0].bounded_corpus = true;
    let report = assess(&ir, &bounded, &evidence());
    assert_eq!(report.result, VerificationResult::Pass);
    assert_eq!(report.bounded_obligations, vec!["obligation-1"]);
}

#[test]
fn rust_assesses_assertions_and_failures_while_preserving_incompleteness() {
    let ir = ir();
    let false_plan = plan(literal("Boolean", "false"));
    let report = assess(&ir, &false_plan, &evidence());
    assert!(
        matches!(report.result, VerificationResult::Fail { issue } if issue.code == "ASSERTION_FAILED")
    );
    unknown(
        &assess(&ir, &plan(literal("Int", "1")), &evidence()),
        "NON_BOOLEAN_ASSERTION",
    );
    unknown(
        &assess(
            &ir,
            &plan(expr(ExpressionKind::Variable {
                name: "absent".into(),
            })),
            &evidence(),
        ),
        "UNBOUND_VALUE",
    );
    let mut mixed = false_plan.clone();
    mixed.obligations[0].unsupported = Some(issue());
    let mut missing = case(literal("Boolean", "true"));
    missing.id = "case-2".into();
    mixed.obligations[0].cases.push(missing);
    let report = assess(&ir, &mixed, &evidence());
    assert!(matches!(report.result, VerificationResult::Fail { .. }));
    assert_eq!(report.incomplete_obligations, vec!["obligation-1"]);
    assert!(matches!(
        report.cases[1].result,
        VerificationResult::Unknown { .. }
    ));
    let mut leading_unknown = mixed.clone();
    leading_unknown.obligations[0].cases.reverse();
    assert!(matches!(
        assess(&ir, &leading_unknown, &evidence()).result,
        VerificationResult::Fail { .. }
    ));
}

#[test]
fn continuation_has_no_worker_success_flag_and_application_exceptions_are_fail() {
    let ir = ir();
    let invocation = Invocation {
        subject: "W.M.identity".into(),
        arguments: vec![VerificationValue::Integer("3".into())],
    };
    let assertion = expr(ExpressionKind::Binary {
        operator: "==".into(),
        left: Box::new(expr(ExpressionKind::Call {
            name: "identity".into(),
            target: Some("W.M.identity".into()),
            arguments: vec![literal("Int", "3")],
        })),
        right: Box::new(literal("Int", "3")),
    });
    let plan = plan(assertion);
    let case = &plan.obligations[0].cases[0];
    let budgets = &plan.budgets;
    assert_eq!(
        next_invocation(&ir, case, &[], budgets),
        Ok(Some(invocation.clone()))
    );
    unknown(&assess(&ir, &plan, &evidence()), "INCOMPLETE_OBSERVATIONS");
    for (observed, passing) in [("3", true), ("4", false)] {
        let observations = vec![InvocationObservation {
            invocation: invocation.clone(),
            observation: Observation::Returned {
                value: VerificationValue::Integer(observed.into()),
            },
        }];
        assert_eq!(next_invocation(&ir, case, &observations, budgets), Ok(None));
        let result = assess(
            &ir,
            &plan,
            &[CaseEvidence {
                case_id: case.id.clone(),
                observations,
            }],
        )
        .result;
        assert_eq!(matches!(result, VerificationResult::Pass), passing);
    }
    for observation in [
        Observation::Threw {
            message: "boom".into(),
        },
        Observation::Unavailable {
            reason: "missing adapter".into(),
        },
    ] {
        let observations = vec![InvocationObservation {
            invocation: invocation.clone(),
            observation,
        }];
        let next = next_invocation(&ir, case, &observations, budgets).unwrap_err();
        let actual = assess(
            &ir,
            &plan,
            &[CaseEvidence {
                case_id: case.id.clone(),
                observations,
            }],
        )
        .result;
        assert_eq!(*next, actual);
        assert!(!matches!(actual, VerificationResult::Pass));
    }
    let report = assess(&ir, &plan, &evidence());
    assert_eq!(
        serde_json::from_str::<PlanResult>(&serde_json::to_string(&report).unwrap()).unwrap(),
        report
    );
}
