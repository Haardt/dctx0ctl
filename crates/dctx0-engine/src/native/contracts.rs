//! Real operation sessions: prepare actual state, observe, centrally assess.
use super::*;
use dctx0_core::{
    InvocationObservation, ReferenceKind, VerificationBudgets, VerificationResult, WorkspaceIr,
};
use dctx0_interpreter::ObservationContext;
use dctx0_verification::{
    OperationAssessment, OperationCaseInput, OperationObservers, OperationPlan, assess_operation,
    operation_ready,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeOperationAssessment {
    pub setup: Vec<InvocationObservation>,
    pub observed: Option<InvocationObservation>,
    pub context: ObservationContext,
    pub assessment: OperationAssessment,
}
impl NativeWorker {
    pub async fn execute_operation_contract(
        &mut self,
        ir: &WorkspaceIr,
        plan: &OperationPlan,
        input: &OperationCaseInput,
        options: (
            &VerificationBudgets,
            OperationObservers,
            &NativeCancellation,
        ),
    ) -> Result<NativeOperationAssessment, NativeFailure> {
        self.session_evidence.clear();
        let deadline = self.limits.case;
        let (budgets, observers, cancel) = options;
        let action = async {
            self.begin_case(&input.id, cancel).await?;
            let mut result = NativeOperationAssessment {
                setup: vec![],
                observed: None,
                context: ObservationContext::default(),
                assessment: OperationAssessment {
                    subject: plan.operation.id.clone(),
                    checked: vec![],
                    open: vec![],
                    result: VerificationResult::Pass,
                },
            };
            for invocation in &input.setup {
                let operation = ir.operations.get(&invocation.subject).ok_or_else(|| {
                    NativeFailure::new(
                        NativeFailureKind::Technical,
                        "Setup operation is undeclared",
                    )
                })?;
                let setup_plan = OperationPlan {
                    precondition_selection: None,
                    operation: operation.clone(),
                    inputs: plan.inputs.clone(),
                    clauses: vec![],
                    cases: vec![],
                };
                let ready = operation_ready(
                    ir,
                    &setup_plan,
                    invocation,
                    &ObservationContext::default(),
                    budgets,
                );
                if ready != VerificationResult::Pass {
                    result.assessment.result = ready;
                    self.end_case(cancel).await?;
                    return Ok(result);
                }
                let delivered = self
                    .invoke_case(invocation.clone(), cancel, &mut cases::reject_mock)
                    .await?;
                let observed = InvocationObservation {
                    invocation: invocation.clone(),
                    observation: delivered.observation,
                };
                let assessed = assess_operation(
                    ir,
                    &setup_plan,
                    &observed,
                    &ObservationContext::default(),
                    budgets,
                );
                let success = matches!(observed.observation,Observation::Returned{ref value} if !matches!(value,dctx0_core::VerificationValue::Failure{..}));
                result.setup.push(observed);
                if assessed.result != VerificationResult::Pass || !success {
                    result.assessment = assessed;
                    if result.assessment.result == VerificationResult::Pass {
                        result.assessment.result=VerificationResult::Unknown{issue:dctx0_core::VerificationIssue{code:"SETUP_REQUIREMENT_UNDELIVERED".into(),message:"A declared setup failure did not establish the case prerequisites".into(),span:None}};
                    }
                    self.end_case(cancel).await?;
                    return Ok(result);
                }
            }
            let entities = ir
                .references
                .iter()
                .filter(|r| {
                    r.from == plan.operation.id
                        && matches!(
                            r.kind,
                            ReferenceKind::Reads | ReferenceKind::Writes | ReferenceKind::State
                        )
                })
                .map(|r| r.target.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if observers.states && !entities.is_empty() {
                result.context.before = Some(self.observe_case(entities.clone(), cancel).await?);
            }
            let ready = operation_ready(ir, plan, &input.invocation, &result.context, budgets);
            if ready != VerificationResult::Pass {
                result.assessment.result = ready;
                self.end_case(cancel).await?;
                return Ok(result);
            }
            let delivered = self
                .invoke_case(input.invocation.clone(), cancel, &mut cases::reject_mock)
                .await?;
            if observers.states && !entities.is_empty() {
                result.context.after = Some(self.observe_case(entities, cancel).await?);
            }
            result.context.calls = if observers.calls {
                delivered.calls
            } else {
                None
            };
            result.context.events = observers.events.then_some(delivered.events);
            let observed = InvocationObservation {
                invocation: input.invocation.clone(),
                observation: delivered.observation,
            };
            result.assessment = assess_operation(ir, plan, &observed, &result.context, budgets);
            result.observed = Some(observed);
            self.end_case(cancel).await?;
            Ok(result)
        };
        match tokio::time::timeout(deadline, action).await {
            Ok(result) => result,
            Err(_) => Err(self.expired_case().await),
        }
    }
}
