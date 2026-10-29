//! Single real pure call. Signature and observed effects are assessed in Rust.
use super::*;
use dctx0_core::{
    OperationIr, VerificationBudgets, VerificationIssue, VerificationResult, WorkspaceIr,
};
use dctx0_interpreter::{ValueContract, check_value};
use dctx0_verification::{InputCorpus, OperationPlan, assess_operation_continued, operation_ready};
impl NativeWorker {
    pub async fn execute_pure_operation(
        &mut self,
        ir: &WorkspaceIr,
        operation: &OperationIr,
        invocation: Invocation,
        budgets: &VerificationBudgets,
        cancel: &NativeCancellation,
    ) -> Result<VerificationResult, NativeFailure> {
        self.clear_evidence();
        if !operation.pure
            || !operation.failures.is_empty()
            || invocation.subject != operation.id
            || invocation.arguments.len() != operation.parameters.len()
            || ir.operations.get(&operation.id) != Some(operation)
        {
            return Err(NativeFailure::new(
                NativeFailureKind::Technical,
                "Pure operation case does not match a supported declared signature",
            ));
        }
        for (parameter, value) in operation.parameters.iter().zip(&invocation.arguments) {
            if !matches!(
                check_value(ir, &parameter.ty, value, budgets),
                ValueContract::Accepted
            ) {
                return Err(NativeFailure::new(
                    NativeFailureKind::Technical,
                    "Pure operation input is not a supported valid contract value",
                ));
            }
        }
        let plan = OperationPlan {
            purity_requirements: vec![],
            precondition_selection: None,
            operation: operation.clone(),
            inputs: InputCorpus {
                bindings: vec![],
                exhaustive: false,
                issues: vec![],
            },
            clauses: ir
                .nodes
                .iter()
                .filter(|node| {
                    node.owner == operation.id
                        && matches!(
                            node.kind,
                            dctx0_core::StatementKind::Requires
                                | dctx0_core::StatementKind::Ensures
                                | dctx0_core::StatementKind::State
                                | dctx0_core::StatementKind::Read
                                | dctx0_core::StatementKind::Write
                                | dctx0_core::StatementKind::Call
                                | dctx0_core::StatementKind::Emit
                                | dctx0_core::StatementKind::Observe
                        )
                })
                .cloned()
                .collect(),
            cases: vec![],
            generated_preparations: vec![],
        };
        let ready = operation_ready(ir, &plan, &invocation, &Default::default(), budgets);
        if ready != VerificationResult::Pass {
            return Ok(ready);
        }
        let deadline = self.limits.case;
        let action = async {
            self.begin_case(&operation.id, cancel).await?;
            let delivered = self
                .invoke_case(invocation.clone(), cancel, &mut cases::reject_mock)
                .await?;
            let issue = |code: &str, message: String| VerificationIssue {
                code: code.into(),
                message,
                span: None,
            };
            let result = if !delivered.events.is_empty() {
                VerificationResult::Fail {
                    issue: issue(
                        "PURE_OPERATION_EMITTED",
                        "Pure operation emitted actual events".into(),
                    ),
                }
            } else {
                match delivered.observation {
                    Observation::Returned { value } => {
                        match check_value(ir, &operation.result, &value, budgets) {
                            ValueContract::Accepted => {
                                self.assess_native_clauses(
                                    ir,
                                    &plan,
                                    &dctx0_core::InvocationObservation {
                                        invocation: invocation.clone(),
                                        observation: Observation::Returned { value },
                                    },
                                    &dctx0_interpreter::ObservationContext {
                                        calls: delivered.calls,
                                        resources: delivered.resources,
                                        events: Some(delivered.events),
                                        ..Default::default()
                                    },
                                    budgets,
                                    cancel,
                                )
                                .await?
                                .result
                            }
                            ValueContract::Rejected { reason } => VerificationResult::Fail {
                                issue: issue("NATIVE_RETURN_CONTRACT", reason),
                            },
                            ValueContract::Unknown { issue } => {
                                VerificationResult::Unknown { issue }
                            }
                        }
                    }
                    Observation::Threw { message } => VerificationResult::Unknown {
                        issue: issue("APPLICATION_EXCEPTION", message),
                    },
                    Observation::Unavailable { reason } => VerificationResult::Unknown {
                        issue: issue("NATIVE_OPERATION_UNAVAILABLE", reason),
                    },
                }
            };
            self.end_case(cancel).await?;
            Ok(result)
        };
        match tokio::time::timeout(deadline, action).await {
            Ok(result) => result,
            Err(_) => Err(self.expired_case().await),
        }
    }
    pub(super) async fn assess_native_clauses(
        &mut self,
        ir: &WorkspaceIr,
        plan: &OperationPlan,
        observed: &dctx0_core::InvocationObservation,
        context: &dctx0_interpreter::ObservationContext,
        budgets: &VerificationBudgets,
        cancel: &NativeCancellation,
    ) -> Result<dctx0_verification::OperationAssessment, NativeFailure> {
        let mut tapes =
            std::collections::BTreeMap::<String, Vec<dctx0_core::InvocationObservation>>::new();
        loop {
            let assessed = assess_operation_continued(ir, plan, observed, context, budgets, &tapes);
            let Some(pending) = assessed.pending_call.clone() else {
                return Ok(assessed);
            };
            let finish = |result: VerificationResult| {
                let mut report = assessed.clone();
                report.pending_call = None;
                if let VerificationResult::Unknown { issue } = &result {
                    report.open.push(issue.clone());
                }
                report.result = result;
                report
            };
            let issue = |code: &str, message: String| VerificationIssue {
                code: code.into(),
                message,
                span: None,
            };
            if tapes.values().map(Vec::len).sum::<usize>() >= budgets.evaluation_steps {
                return Ok(finish(VerificationResult::Unknown {
                    issue: issue(
                        "EVALUATION_BUDGET",
                        "Pure clause continuation exhausted its invocation budget".into(),
                    ),
                }));
            }
            let operation = &ir.operations[&pending.invocation.subject];
            if !operation.pure
                || operation.parameters.len() != pending.invocation.arguments.len()
                || operation
                    .parameters
                    .iter()
                    .zip(&pending.invocation.arguments)
                    .any(|(parameter, value)| {
                        !matches!(
                            check_value(ir, &parameter.ty, value, budgets),
                            ValueContract::Accepted
                        )
                    })
            {
                return Ok(finish(VerificationResult::Unknown {
                    issue: issue(
                        "CLAUSE_CALL_INPUT",
                        "Clause invocation does not match a valid pure signature".into(),
                    ),
                }));
            }
            let delivered = self
                .invoke_case(pending.invocation.clone(), cancel, &mut cases::reject_mock)
                .await?;
            if !delivered.events.is_empty()
                || delivered.resources.as_ref().is_some_and(|resources| {
                    resources
                        .iter()
                        .any(|resource| resource.access == dctx0_core::ResourceAccess::Write)
                })
            {
                return Ok(finish(VerificationResult::Fail {
                    issue: issue(
                        "PURE_CLAUSE_EFFECT",
                        "Pure clause oracle emitted events or wrote a resource".into(),
                    ),
                }));
            }
            if let Observation::Returned { value } = &delivered.observation {
                match check_value(ir, &operation.result, value, budgets) {
                    ValueContract::Accepted => {}
                    ValueContract::Rejected { reason } => {
                        return Ok(finish(VerificationResult::Fail {
                            issue: issue("NATIVE_RETURN_CONTRACT", reason),
                        }));
                    }
                    ValueContract::Unknown { issue } => {
                        return Ok(finish(VerificationResult::Unknown { issue }));
                    }
                }
            }
            tapes
                .entry(pending.key)
                .or_default()
                .push(dctx0_core::InvocationObservation {
                    invocation: pending.invocation,
                    observation: delivered.observation,
                });
        }
    }
}
