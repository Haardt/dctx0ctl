use dctx0_core::{VerificationIssue, VerificationResult as R};
use dctx0_verification::suites::*;
use std::collections::{BTreeMap, BTreeSet};

fn failure() -> R {
    R::Fail {
        issue: VerificationIssue {
            code: "ACTUAL_FAILURE".into(),
            message: "mutation detected".into(),
            span: None,
        },
    }
}

#[test]
fn reference_catalog_requires_all_categories_and_preserves_failure() {
    use ProofCategory::*;
    assert_eq!(named_proof_categories("TypeConformance"), &[Type, Data]);
    assert_eq!(named_proof_categories("Type"), &[Type, Data]);
    for (name, category) in [
        ("SurfaceConformance", Surface),
        ("ArchitectureConformance", Architecture),
        ("Architecture", Architecture),
        ("OwnershipConformance", Ownership),
        ("EffectConformance", Effects),
    ] {
        assert_eq!(
            assess_proof_suite(name, &BTreeMap::from([(category, R::Pass)])),
            R::Pass
        );
    }
    assert!(matches!(
        assess_proof_suite("invented", &BTreeMap::new()),
        R::Unknown { .. }
    ));
    assert!(matches!(
        assess_proof_suite("TypeConformance", &BTreeMap::from([(Type, R::Pass)])),
        R::Unknown { .. }
    ));
    assert_eq!(
        assess_proof_suite("TypeConformance", &BTreeMap::from([(Data, failure())])),
        failure()
    );
    let unknown = R::Unknown {
        issue: VerificationIssue {
            code: "UNOBSERVED".into(),
            message: "no observer".into(),
            span: None,
        },
    };
    assert_eq!(
        assess_proof_suite(
            "TypeConformance",
            &BTreeMap::from([(Type, unknown.clone()), (Data, R::Pass)])
        ),
        unknown
    );
    assert_eq!(
        assess_proof_suite(
            "TypeConformance",
            &BTreeMap::from([(Type, R::Pass), (Data, R::Pass)])
        ),
        R::Pass
    );
}

#[test]
fn suite_membership_requires_actual_unique_complete_execution() {
    let expected = BTreeSet::from(["a".into(), "b".into()]);
    let cases = vec![("a".into(), R::Pass), ("b".into(), R::Pass)];
    assert_eq!(assess_test_suite(&expected, &cases), R::Pass);
    assert_eq!(assess_test_suite(&BTreeSet::new(), &cases), R::Pass);
    for cases in [
        vec![],
        vec![("a".into(), R::Pass)],
        vec![("a".into(), R::Pass), ("a".into(), R::Pass)],
        vec![("a".into(), R::Pass), ("c".into(), R::Pass)],
        vec![("".into(), R::Pass)],
    ] {
        assert!(matches!(
            assess_test_suite(&expected, &cases),
            R::Unknown { .. }
        ));
    }
    assert!(matches!(
        assess_test_suite(&BTreeSet::new(), &[]),
        R::Unknown { .. }
    ));
    assert_eq!(
        assess_test_suite(&expected, &[("a".into(), R::Pass), ("b".into(), failure())]),
        failure()
    );
    let unobserved = R::Unknown {
        issue: VerificationIssue {
            code: "OBSERVER_UNAVAILABLE".into(),
            message: "A mandatory case has no actual observer".into(),
            span: None,
        },
    };
    assert_eq!(
        assess_test_suite(
            &expected,
            &[("a".into(), R::Pass), ("b".into(), unobserved.clone())]
        ),
        unobserved
    );
}

#[test]
fn release_rules_do_not_invent_zero_findings_or_pass_missing_negative_requirements() {
    let rule = PolicyRule {
        target: "Review".into(),
        expected: PolicyValue::HighFindings(0),
        equal: true,
    };
    assert!(matches!(
        assess_policy_rule(&rule, &BTreeMap::new()),
        R::Unknown { .. }
    ));
    let outcomes = |status, high_findings| {
        BTreeMap::from([(
            "Review".into(),
            PolicyOutcome {
                status,
                high_findings,
            },
        )])
    };
    assert!(matches!(
        assess_policy_rule(&rule, &outcomes(PolicyStatus::Approve, None)),
        R::Unknown { .. }
    ));
    assert_eq!(
        assess_policy_rule(&rule, &outcomes(PolicyStatus::Approve, Some(0))),
        R::Pass
    );
    assert!(matches!(
        assess_policy_rule(&rule, &outcomes(PolicyStatus::ApproveWithFindings, Some(1))),
        R::Fail { .. }
    ));
    let rule = PolicyRule {
        expected: PolicyValue::Status(PolicyStatus::Fail),
        equal: false,
        ..rule
    };
    for status in [
        PolicyStatus::Unknown,
        PolicyStatus::NotRun,
        PolicyStatus::Stale,
    ] {
        assert!(matches!(
            assess_policy_rule(&rule, &outcomes(status, None)),
            R::Unknown { .. }
        ));
    }
    assert_eq!(
        assess_policy_rule(&rule, &outcomes(PolicyStatus::Pass, None)),
        R::Pass
    );
    assert!(matches!(
        assess_policy_rule(&rule, &outcomes(PolicyStatus::Fail, None)),
        R::Fail { .. }
    ));
    let rule = PolicyRule {
        expected: PolicyValue::Status(PolicyStatus::Approve),
        equal: true,
        ..rule
    };
    assert_eq!(
        assess_policy_rule(&rule, &outcomes(PolicyStatus::Approve, Some(0))),
        R::Pass
    );
    assert!(matches!(
        assess_policy_rule(&rule, &outcomes(PolicyStatus::RequestChanges, Some(0))),
        R::Fail { .. }
    ));
}

#[test]
fn cinema_release_uses_all_fifteen_existing_rules_and_resolved_targets() {
    use dctx0_core::{Source, StatementKind};
    use dctx0_verification::{VerificationScope, project_program};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema/dctx0");
    let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|file| Source {
        file: file.into(),
        text: std::fs::read_to_string(root.join(file)).unwrap(),
    });
    let compilation = dctx0_language::compile(&sources);
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    let ir = compilation.ir.unwrap();
    let plan = project_program(
        &ir,
        "snapshot",
        1,
        VerificationScope {
            subjects: BTreeSet::from(["HomeCinema.MovieNight".into()]),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        Default::default(),
    );
    let nodes = plan.policies[0]
        .nodes
        .iter()
        .filter(|node| node.kind == StatementKind::Require)
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 15);
    let rules = nodes
        .iter()
        .map(|node| project_policy_rule(&ir, node).unwrap())
        .collect::<Vec<_>>();
    let mut outcomes = BTreeMap::new();
    for rule in &rules {
        outcomes.insert(
            rule.target.clone(),
            PolicyOutcome {
                status: if ir.symbols[&rule.target].kind == StatementKind::Review {
                    PolicyStatus::Approve
                } else {
                    PolicyStatus::Pass
                },
                high_findings: Some(0),
            },
        );
    }
    assert_eq!(
        assess_program_policies(&ir, &plan, &outcomes)[0].result,
        R::Pass
    );
    let review = "HomeCinema.MovieNight.Catalog.CatalogArchitecture";
    outcomes.get_mut(review).unwrap().high_findings = None;
    assert!(matches!(
        assess_program_policies(&ir, &plan, &outcomes)[0].result,
        R::Unknown { .. }
    ));
    outcomes.get_mut(review).unwrap().status = PolicyStatus::RequestChanges;
    assert!(matches!(
        assess_program_policies(&ir, &plan, &outcomes)[0].result,
        R::Fail { .. }
    ));
    let mut malformed = nodes[0].clone();
    malformed.terms[1].text = ">=".into();
    assert!(project_policy_rule(&ir, &malformed).is_err());
    let mut malformed_count = (*nodes
        .iter()
        .find(|node| node.terms[0].text.ends_with(".highFindings"))
        .unwrap())
    .clone();
    malformed_count.terms[2].text = "not-a-number".into();
    assert!(project_policy_rule(&ir, &malformed_count).is_err());
    malformed.terms.clear();
    assert!(project_policy_rule(&ir, &malformed).is_err());
    assert!(matches!(
        assess_program_policies(&ir, &plan, &BTreeMap::new())[0].result,
        R::Unknown { .. }
    ));
}
