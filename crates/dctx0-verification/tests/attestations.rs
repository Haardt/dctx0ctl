use dctx0_core::{
    VerificationAttestation, VerificationEvidenceBasis as Basis, VerificationIssue,
    VerificationResult as R,
};
use dctx0_verification::attestations::*;

fn pending() -> R {
    R::Unknown {
        issue: VerificationIssue {
            code: "REVIEW_REQUIRED".into(),
            message: "architecture inspection".into(),
            span: None,
        },
    }
}
fn report() -> VerificationAttestation {
    VerificationAttestation {
        schema_version: 1,
        actor: "reviewer".into(),
        subject: "Architecture".into(),
        snapshot: "snapshot".into(),
        plan_hash: "plan".into(),
        requirements: ["ownership".into(), "architecture".into()].into(),
        implementation_artifacts: [("app.ts".into(), "content-hash".into())].into(),
        report: "Inspected imports and composition; domain does not import persistence.".into(),
        limitations: vec!["Reflection is not exhaustively decidable".into()],
        result: R::Pass,
    }
}
#[test]
fn bound_agent_pass_resolves_unknown_without_relabeling_it_as_automatic() {
    let report = report();
    let binding = AttestationBinding {
        subject: &report.subject,
        snapshot: &report.snapshot,
        plan_hash: &report.plan_hash,
        requirements: &report.requirements,
        implementation_artifacts: &report.implementation_artifacts,
    };
    let result = apply_attestation(&pending(), &report, &binding);
    assert_eq!(result.automatic, pending());
    assert_eq!(result.effective, R::Pass);
    assert_eq!(result.basis, Basis::Agent);
    assert_eq!(
        apply_attestation(&R::Pass, &report, &binding).basis,
        Basis::Mixed
    );
    let fail = R::Fail {
        issue: VerificationIssue {
            code: "MUTATION".into(),
            message: "actual counterexample".into(),
            span: None,
        },
    };
    assert_eq!(apply_attestation(&fail, &report, &binding).effective, fail);
    let mut changed = report.clone();
    changed.result = fail.clone();
    assert_eq!(
        apply_attestation(&R::Pass, &changed, &binding).effective,
        fail
    );
    changed.result = pending();
    assert_eq!(
        apply_attestation(&R::Pass, &changed, &binding).effective,
        R::Pass
    );
}
#[test]
fn stale_incomplete_or_unexplained_attestations_have_no_effect() {
    let original = report();
    let binding = AttestationBinding {
        subject: &original.subject,
        snapshot: &original.snapshot,
        plan_hash: &original.plan_hash,
        requirements: &original.requirements,
        implementation_artifacts: &original.implementation_artifacts,
    };
    for change in 0..9 {
        let mut report = original.clone();
        match change {
            0 => report.schema_version = 2,
            1 => report.actor.clear(),
            2 => report.report = "  ".into(),
            3 => report.subject = "Other".into(),
            4 => report.snapshot = "new-spec".into(),
            5 => report.plan_hash = "new-plan".into(),
            6 => {
                report.requirements.remove("ownership");
            }
            7 => {
                report
                    .implementation_artifacts
                    .insert("app.ts".into(), "new-code".into());
            }
            _ => report.implementation_artifacts.clear(),
        }
        let result = apply_attestation(&pending(), &report, &binding);
        assert_eq!(result.effective, pending());
        assert_eq!(result.basis, Basis::Automatic);
    }
}
