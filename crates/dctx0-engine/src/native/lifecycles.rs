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
        self.clear_evidence();
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
        let input_result =
            dctx0_verification::lifecycle_input_contract(ir, plan, path, input, budgets);
        if input_result != VerificationResult::Pass {
            let mut assessment = assess_lifecycle(ir, plan, path, &input.keys, &[], budgets);
            assessment.result = input_result;
            return Ok(NativeLifecycleResult {
                setup: vec![],
                observations: vec![],
                assessment,
                blocked_context: None,
            });
        }
        self.begin_case(&path.id, cancel).await?;
        let mut result = NativeLifecycleResult {
            setup: vec![],
            observations: vec![],
            assessment: assess_lifecycle(ir, plan, path, &input.keys, &[], budgets),
            blocked_context: None,
        };
        for invocation in &input.setup {
            let operation = &ir.operations[&invocation.subject];
            let setup_plan = dctx0_verification::OperationPlan {
                purity_requirements: vec![],
                precondition_selection: None,
                operation: operation.clone(),
                inputs: dctx0_verification::InputCorpus {
                    bindings: vec![],
                    exhaustive: false,
                    issues: vec![],
                },
                clauses: ir
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.owner == operation.id
                            && node.kind == dctx0_core::StatementKind::Requires
                    })
                    .cloned()
                    .collect(),
                cases: vec![],
                generated_preparations: vec![],
            };
            let mut setup_context = ObservationContext::default();
            let entities = ir
                .references
                .iter()
                .filter(|reference| {
                    reference.from == operation.id
                        && matches!(
                            reference.kind,
                            dctx0_core::ReferenceKind::Reads
                                | dctx0_core::ReferenceKind::Writes
                                | dctx0_core::ReferenceKind::State
                        )
                })
                .map(|reference| reference.target.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if !setup_plan.clauses.is_empty() && !entities.is_empty() {
                setup_context.before = Some(
                    self.observe_case(entities, dctx0_core::StatePhase::Before, cancel)
                        .await?,
                );
            }
            let ready = dctx0_verification::operation_ready(
                ir,
                &setup_plan,
                invocation,
                &setup_context,
                budgets,
            );
            if ready != VerificationResult::Pass {
                result.assessment.result = ready;
                result.blocked_context = Some(setup_context);
                self.end_case(cancel).await?;
                return Ok(result);
            }
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
            let before = self
                .observe_case(
                    vec![plan.entity.clone()],
                    dctx0_core::StatePhase::Before,
                    cancel,
                )
                .await?;
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
            let after = self
                .observe_case(
                    vec![plan.entity.clone()],
                    dctx0_core::StatePhase::After,
                    cancel,
                )
                .await?;
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
                    resources: delivered.resources,
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
