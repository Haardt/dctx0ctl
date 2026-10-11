use dctx0_core::*;
use dctx0_verification::{program_inspection::*, *};
use std::collections::BTreeMap;

#[test]
fn effective_release_requires_bound_review_and_actual_cases_and_preserves_failures() {
    let compilation=dctx0_language::compile(&[Source { file:"review.dctx0".into(),text:"workspace W { value A = Int { range 1..2 }\nproof ArchitectureConformance\nreview Boundary { inspect { implementation }\ncriteria { \"Keep validation at the boundary.\" } }\npolicy Release { require ArchitectureConformance == PASS\nrequire Boundary != REQUEST_CHANGES\nrequire Boundary.highFindings == 0 } }".into() }]);
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
            subjects: ["W".into()].into(),
            tasks: Default::default(),
        },
        &Default::default(),
        Default::default(),
    );
    let artifacts = BTreeMap::from([("app.ts".into(), "hash".into())]);
    let mut cases = case_manifest(&plan)
        .required_cases
        .into_iter()
        .map(|id| VerificationCaseRecord {
            id,
            subject: "W.A".into(),
            kind: VerificationCaseKind::Contract,
            bindings: Default::default(),
            setup: vec![],
            arguments: vec![],
            events: None,
            resources: None,
            observations: vec![],
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        })
        .collect::<Vec<_>>();
    let subjects = ["W.ArchitectureConformance", "W.Boundary"];
    let mut reports = subjects
        .into_iter()
        .map(|subject| ProgramInspection {
            inspection: VerificationAttestation {
                schema_version: 1,
                actor: "reviewer".into(),
                subject: subject.into(),
                snapshot: "snapshot".into(),
                plan_hash: "plan".into(),
                requirements: inspection_requirements(&plan, subject).unwrap(),
                implementation_artifacts: artifacts.clone(),
                report: "Inspected application boundary and dependencies.".into(),
                limitations: vec!["Agent judgement".into()],
                result: VerificationResult::Pass,
            },
            review: (subject == "W.Boundary").then_some(InspectedReview {
                approved: true,
                findings: vec![],
            }),
        })
        .collect::<Vec<_>>();
    let assess = |cases: &[VerificationCaseRecord], reports: &[ProgramInspection]| {
        assess_with_inspections(&ir, &plan, cases, "plan", &artifacts, reports)
    };
    let complete = assess(&cases, &reports);
    assert_eq!(complete.result, VerificationResult::Pass);
    assert!(complete.open_obligations.is_empty());
    assert!(
        complete
            .subjects
            .iter()
            .all(|subject| subject.result.basis == VerificationEvidenceBasis::Agent)
    );
    assert!(matches!(
        assess(&[], &reports).result,
        VerificationResult::Unknown { .. }
    ));
    let original = reports[1].clone();
    reports[1].review = None;
    assert!(matches!(
        assess(&cases, &reports).result,
        VerificationResult::Unknown { .. }
    ));
    reports[1] = original;
    reports[1]
        .review
        .as_mut()
        .unwrap()
        .findings
        .push(InspectionFinding {
            id: "high".into(),
            severity: FindingSeverity::High,
            description: "Unresolved boundary defect".into(),
        });
    assert!(matches!(
        assess(&cases, &reports).result,
        VerificationResult::Fail { .. }
    ));
    reports[1].review.as_mut().unwrap().findings.clear();
    reports[0].inspection.snapshot = "stale".into();
    assert!(matches!(
        assess(&cases, &reports).result,
        VerificationResult::Unknown { .. }
    ));
    reports[0].inspection.snapshot = "snapshot".into();
    cases[0].result = VerificationResult::Fail {
        issue: VerificationIssue {
            code: "ACTUAL_MUTATION".into(),
            message: "Counterexample".into(),
            span: None,
        },
    };
    assert_eq!(assess(&cases, &reports).result, cases[0].result);
}
