//! Connected native lifecycle sessions. Rust owns every state/guard verdict.
use super::*;
use dctx0_core::{InvocationObservation, VerificationBudgets, VerificationResult, WorkspaceIr};
use dctx0_interpreter::{ObservationContext, ValueContract, check_value};
use dctx0_verification::{
    LifecycleAssessment, LifecycleCaseInput, LifecycleObservation, LifecyclePath, LifecyclePlan,
    assess_lifecycle, lifecycle_ready,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeLifecycleResult {
    pub setup: Vec<InvocationObservation>,
    pub observations: Vec<LifecycleObservation>,
    pub assessment: LifecycleAssessment,
    pub blocked_context: Option<ObservationContext>,
}
fn input_contract(
    ir: &WorkspaceIr,
    invocation: &Invocation,
    budgets: &VerificationBudgets,
) -> Result<(), NativeFailure> {
    let operation = ir.operations.get(&invocation.subject).ok_or_else(|| {
        NativeFailure::new(
            NativeFailureKind::Technical,
            "Lifecycle input has no declared operation",
        )
    })?;
    if operation.parameters.len() != invocation.arguments.len() {
        return Err(NativeFailure::new(
            NativeFailureKind::Technical,
            "Lifecycle input has the wrong argument count",
        ));
    }
    for (parameter, value) in operation.parameters.iter().zip(&invocation.arguments) {
        if let result @ (ValueContract::Rejected { .. } | ValueContract::Unknown { .. }) =
            check_value(ir, &parameter.ty, value, budgets)
        {
            return Err(NativeFailure::new(
                NativeFailureKind::Technical,
                format!("Invalid lifecycle input: {result:?}"),
            ));
        }
    }
    Ok(())
}
impl NativeWorker {
    pub async fn execute_lifecycle(
        &mut self,
        ir: &WorkspaceIr,
        plan: &LifecyclePlan,
        path: &LifecyclePath,
        input: &LifecycleCaseInput,
        budgets: &VerificationBudgets,
        cancel: &NativeCancellation,
    ) -> Result<NativeLifecycleResult, NativeFailure> {
        self.session_evidence.clear();
        match tokio::time::timeout(
            self.limits.case,
            self.lifecycle_session(ir, plan, path, input, budgets, cancel),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(self.expired_case().await),
        }
    }
    async fn lifecycle_session(
        &mut self,
        ir: &WorkspaceIr,
        plan: &LifecyclePlan,
        path: &LifecyclePath,
        input: &LifecycleCaseInput,
        budgets: &VerificationBudgets,
        cancel: &NativeCancellation,
    ) -> Result<NativeLifecycleResult, NativeFailure> {
        if !plan.paths.contains(path)
            || input.arguments.len() != path.transitions.len()
            || path
                .transitions
                .iter()
                .any(|index| *index >= plan.transitions.len())
        {
            return Err(NativeFailure::new(
                NativeFailureKind::Technical,
                "Lifecycle input does not cover the pinned path",
            ));
        }
        let invocations = path
            .transitions
            .iter()
            .zip(&input.arguments)
            .map(|(index, arguments)| Invocation {
                subject: plan.transitions[*index].operation.clone(),
                arguments: arguments.clone(),
            })
            .collect::<Vec<_>>();
        for invocation in input.setup.iter().chain(&invocations) {
            input_contract(ir, invocation, budgets)?;
        }
        self.begin_case(&path.id, cancel).await?;
        let mut result = NativeLifecycleResult {
            setup: vec![],
            observations: vec![],
            assessment: assess_lifecycle(ir, plan, path, &input.keys, &[], budgets),
            blocked_context: None,
        };
        for invocation in &input.setup {
            let delivered = self
                .invoke_case(invocation.clone(), cancel, &mut cases::reject_mock)
                .await?;
            let success = matches!(&delivered.observation,Observation::Returned {value} if !matches!(value,dctx0_core::VerificationValue::Failure {..}));
            result.setup.push(InvocationObservation {
                invocation: invocation.clone(),
                observation: delivered.observation,
            });
            if let Observation::Returned { value } = &result
                .setup
                .last()
                .expect("Setup observation was just pushed")
                .observation
            {
                if success {
                    match check_value(
                        ir,
                        &ir.operations[&invocation.subject].result,
                        value,
                        budgets,
                    ) {
                        ValueContract::Accepted => {}
                        ValueContract::Rejected { reason } => {
                            result.assessment.result = VerificationResult::Fail {
                                issue: dctx0_core::VerificationIssue {
                                    code: "LIFECYCLE_SETUP_CONTRACT".into(),
                                    message: reason,
                                    span: None,
                                },
                            };
                            self.end_case(cancel).await?;
                            return Ok(result);
                        }
                        ValueContract::Unknown { issue } => {
                            result.assessment.result = VerificationResult::Unknown { issue };
                            self.end_case(cancel).await?;
                            return Ok(result);
                        }
                    }
                }
            }
            if !success {
                self.end_case(cancel).await?;
                return Ok(result);
            }
        }
        for (position, invocation) in invocations.into_iter().enumerate() {
            let before = self.observe_case(vec![plan.entity.clone()], cancel).await?;
            let context = ObservationContext {
                before: Some(before.clone()),
                ..Default::default()
            };
            let ready = lifecycle_ready(
                ir,
                plan,
                path.transitions[position],
                &input.keys,
                &invocation,
                &context,
                budgets,
            );
            if ready != VerificationResult::Pass {
                result.assessment.result = ready;
                result.blocked_context = Some(context);
                break;
            }
            let delivered = self
                .invoke_case(invocation.clone(), cancel, &mut cases::reject_mock)
                .await?;
            let after = self.observe_case(vec![plan.entity.clone()], cancel).await?;
            result.observations.push(LifecycleObservation {
                invocation: InvocationObservation {
                    invocation,
                    observation: delivered.observation,
                },
                context: ObservationContext {
                    phase: dctx0_core::StatePhase::After,
                    before: Some(before),
                    after: Some(after),
                    calls: delivered.calls,
                    events: Some(delivered.events),
                },
            });
            // Assess the complete delivered prefix; missing later steps stay open.
            result.assessment =
                assess_lifecycle(ir, plan, path, &input.keys, &result.observations, budgets);
            if matches!(result.assessment.result, VerificationResult::Fail { .. }) {
                break;
            }
        }
        self.end_case(cancel).await?;
        Ok(result)
    }
}
