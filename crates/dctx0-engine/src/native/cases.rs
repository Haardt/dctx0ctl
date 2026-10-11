//! Native pure-expression continuation. Central Rust semantics decide the result.
use super::*;
use dctx0_core::{InvocationObservation, VerificationIssue, VerificationResult, WorkspaceIr};
use dctx0_interpreter::{ValueContract, check_value};
use dctx0_verification::{
    CaseEvidence, CaseResult, Obligation, VerificationCase, VerificationPlan, VerificationScope,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeCaseResult {
    pub evidence: CaseEvidence,
    pub assessment: CaseResult,
}
fn issue(code: &str, message: &str, case: &VerificationCase) -> VerificationResult {
    let issue = VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: Some(case.assertion.span.clone()),
    };
    VerificationResult::Fail { issue }
}
impl NativeWorker {
    /// Evaluates one explicit case; not a complete plan, provenance or adoption proof.
    pub async fn execute_pure_case(
        &mut self,
        ir: &WorkspaceIr,
        plan: &VerificationPlan,
        case: &VerificationCase,
        cancellation: &NativeCancellation,
    ) -> Result<NativeCaseResult, NativeFailure> {
        self.clear_evidence();
        match tokio::time::timeout(
            self.limits.case,
            self.pure_case_session(ir, plan, case, cancellation),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(self.expired_case().await),
        }
    }
    async fn pure_case_session(
        &mut self,
        ir: &WorkspaceIr,
        plan: &VerificationPlan,
        case: &VerificationCase,
        cancellation: &NativeCancellation,
    ) -> Result<NativeCaseResult, NativeFailure> {
        self.begin_case(&case.id, cancellation).await?;
        let mut evidence = CaseEvidence {
            case_id: case.id.clone(),
            observations: Vec::new(),
        };
        let assessment = self
            .evaluate_native_case(ir, plan, case, &mut evidence, cancellation)
            .await;
        // End even when a modeled expression or a native return cannot be evaluated.
        if self.group_active {
            self.end_case(cancellation).await?;
        }
        Ok(NativeCaseResult {
            evidence,
            assessment: CaseResult {
                case_id: case.id.clone(),
                result: assessment?,
            },
        })
    }
    async fn evaluate_native_case(
        &mut self,
        ir: &WorkspaceIr,
        plan: &VerificationPlan,
        case: &VerificationCase,
        evidence: &mut CaseEvidence,
        cancellation: &NativeCancellation,
    ) -> Result<VerificationResult, NativeFailure> {
        loop {
            let invocation = match dctx0_verification::next_invocation(
                ir,
                case,
                &evidence.observations,
                &plan.budgets,
            ) {
                Ok(Some(call)) => call,
                Ok(None) => {
                    let mut single = plan.clone();
                    single.obligations = vec![Obligation {
                        id: case.id.clone(),
                        subject: "native-case".into(),
                        cases: vec![case.clone()],
                        unsupported: None,
                        bounded_corpus: false,
                    }];
                    single.scope = VerificationScope {
                        subjects: std::collections::BTreeSet::from(["native-case".into()]),
                        tasks: Default::default(),
                    };
                    return Ok(dctx0_verification::assess(
                        ir,
                        &single,
                        std::slice::from_ref(evidence),
                    )
                    .result);
                }
                Err(result) => return Ok(*result),
            };
            let session::Invoked {
                observation,
                events,
                ..
            } = self
                .invoke_case(invocation.clone(), cancellation, &mut reject_mock)
                .await?;
            let contract = if let dctx0_core::Observation::Returned { value } = &observation {
                let operation = &ir.operations[&invocation.subject];
                Some(check_value(ir, &operation.result, value, &plan.budgets))
            } else {
                None
            };
            evidence.observations.push(InvocationObservation {
                invocation,
                observation,
            });
            if !events.is_empty() {
                return Ok(issue(
                    "PURE_OPERATION_EMITTED",
                    "Pure operation emitted actual events",
                    case,
                ));
            }
            match contract {
                Some(ValueContract::Rejected { reason }) => {
                    return Ok(issue("NATIVE_RETURN_CONTRACT", &reason, case));
                }
                Some(ValueContract::Unknown { issue }) => {
                    return Ok(VerificationResult::Unknown { issue });
                }
                _ => {}
            }
        }
    }
}
pub(super) fn reject_mock(_invocation: &Invocation) -> Result<Observation, NativeFailure> {
    Err(NativeFailure::new(
        NativeFailureKind::Protocol,
        "Pure-expression case requested an unconfigured collaborator",
    ))
}
