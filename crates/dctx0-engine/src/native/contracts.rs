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
        self.clear_evidence();
        let deadline = self.limits.case;
        let (budgets, observers, cancel) = options;
        let action = async {
            let mut mocks =
                dctx0_verification::OperationMockSession::new(ir, &input.mocks, budgets);
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
            if let Some(issue) = mocks.issues.first() {
                result.assessment.result = VerificationResult::Unknown {
                    issue: issue.clone(),
                };
                result.assessment.open = mocks.issues.clone();
                self.end_case(cancel).await?;
                return Ok(result);
            }
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
                                ReferenceKind::Reads | ReferenceKind::Writes | ReferenceKind::State
                            )
                    })
                    .map(|reference| reference.target.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                if observers.states && !setup_plan.clauses.is_empty() && !entities.is_empty() {
                    setup_context.before = Some(
                        self.observe_case(entities, dctx0_core::StatePhase::Before, cancel)
                            .await?,
                    );
                }
                let ready = operation_ready(ir, &setup_plan, invocation, &setup_context, budgets);
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
                let assessed =
                    assess_operation(ir, &setup_plan, &observed, &setup_context, budgets);
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
                result.context.before = Some(
                    self.observe_case(entities.clone(), dctx0_core::StatePhase::Before, cancel)
                        .await?,
                );
            }
            let ready = operation_ready(ir, plan, &input.invocation, &result.context, budgets);
            if ready != VerificationResult::Pass {
                result.assessment.result = ready;
                self.end_case(cancel).await?;
                return Ok(result);
            }
            let delivered = self
                .invoke_case(input.invocation.clone(), cancel, &mut |invocation| {
                    Ok(mocks.respond(invocation))
                })
                .await;
            if let Some(issue) = mocks.issues.first() {
                result.assessment.result = if issue.code == "UNEXPECTED_MOCK_CALL" {
                    VerificationResult::Fail {
                        issue: issue.clone(),
                    }
                } else {
                    VerificationResult::Unknown {
                        issue: issue.clone(),
                    }
                };
                result.assessment.open.extend(mocks.issues.clone());
                result.context.calls = Some(mocks.observed.clone());
                // A rejected adapter promise already aborts and reaps the worker.
                // Preserve the centrally observed violation alongside that failure.
                if let Err(error) = &delivered {
                    result.assessment.open.push(dctx0_core::VerificationIssue {
                        code: "MOCK_INVOCATION_ABORTED".into(),
                        message: error.to_string(),
                        span: None,
                    });
                } else {
                    self.end_case(cancel).await?;
                }
                return Ok(result);
            }
            let delivered = delivered?;
            let recorded = mocks.check_recorded_calls(delivered.calls.as_deref());
            if recorded != VerificationResult::Pass {
                result.assessment.result = recorded;
                result.context.calls = Some(mocks.observed.clone());
                result.observed = Some(InvocationObservation {
                    invocation: input.invocation.clone(),
                    observation: delivered.observation,
                });
                self.end_case(cancel).await?;
                return Ok(result);
            }
            if observers.states && !entities.is_empty() {
                result.context.after = Some(
                    self.observe_case(entities, dctx0_core::StatePhase::After, cancel)
                        .await?,
                );
            }
            result.context.calls = if observers.calls {
                delivered.calls
            } else {
                None
            };
            result.context.resources = delivered.resources;
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
