//! Prepared collaborator behavior is separate from actual runtime observations.
use dctx0_core::*;
use dctx0_interpreter::{ValueContract, check_value};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationMock {
    pub invocation: Invocation,
    pub outcome: Observation,
}
pub struct OperationMockSession {
    prepared: Vec<OperationMock>,
    pub observed: Vec<InvocationObservation>,
    pub issues: Vec<VerificationIssue>,
}
fn issue(code: &str, message: impl Into<String>) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: None,
    }
}
impl OperationMockSession {
    /// Mock callbacks are observed by Rust. An adapter trace must agree with
    /// them; recording a call cannot substitute for actually requesting it.
    pub fn check_recorded_calls(
        &self,
        calls: Option<&[InvocationObservation]>,
    ) -> VerificationResult {
        if self.prepared.is_empty() {
            return VerificationResult::Pass;
        }
        let Some(calls) = calls else {
            return VerificationResult::Unknown {
                issue: issue(
                    "MOCK_CALL_OBSERVER_REQUIRED",
                    "Prepared mock calls need a matching recorded call trace",
                ),
            };
        };
        let subjects = self
            .prepared
            .iter()
            .map(|entry| entry.invocation.subject.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut recorded = calls
            .iter()
            .filter(|call| subjects.contains(call.invocation.subject.as_str()));
        // Invocation and completion order can differ for concurrent calls.
        // Compare occurrences without introducing an undeclared order rule.
        let mut remaining = self.observed.iter().collect::<Vec<_>>();
        let consistent = recorded.all(|call| {
            if let Some(index) = remaining.iter().position(|actual| *actual == call) {
                remaining.swap_remove(index);
                true
            } else {
                false
            }
        }) && remaining.is_empty();
        if !consistent {
            return VerificationResult::Fail {
                issue: issue(
                    "MOCK_OBSERVATION_MISMATCH",
                    "Recorded mock calls differ from actual Rust-observed collaborator requests",
                ),
            };
        }
        VerificationResult::Pass
    }
    pub fn new(
        ir: &WorkspaceIr,
        prepared: &[OperationMock],
        budgets: &VerificationBudgets,
    ) -> Self {
        let mut session = Self {
            prepared: prepared.to_vec(),
            observed: vec![],
            issues: vec![],
        };
        for (index, entry) in prepared.iter().enumerate() {
            if prepared[..index]
                .iter()
                .any(|previous| previous.invocation == entry.invocation)
            {
                session.issues.push(issue(
                    "AMBIGUOUS_MOCK_REPLY",
                    "Prepared replies repeat the same invocation",
                ));
            }
            let Some(operation) = ir.operations.get(&entry.invocation.subject) else {
                session
                    .issues
                    .push(issue("MOCK_OPERATION_UNKNOWN", &entry.invocation.subject));
                continue;
            };
            if entry.invocation.arguments.len() != operation.parameters.len() {
                session
                    .issues
                    .push(issue("MOCK_ARGUMENT_CONTRACT", &operation.id));
                continue;
            }
            for (parameter, value) in operation.parameters.iter().zip(&entry.invocation.arguments) {
                match check_value(ir, &parameter.ty, value, budgets) {
                    ValueContract::Accepted => {}
                    ValueContract::Rejected { reason } => {
                        session.issues.push(issue("MOCK_ARGUMENT_CONTRACT", reason))
                    }
                    ValueContract::Unknown { issue } => session.issues.push(issue),
                }
            }
            let Observation::Returned { value } = &entry.outcome else {
                session.issues.push(issue(
                    "MOCK_OUTCOME_UNSUPPORTED",
                    "Prepared outcomes must be typed values or declared failures",
                ));
                continue;
            };
            let ty = if matches!(value, VerificationValue::Failure { .. }) {
                TypeRef {
                    name: "Failure".into(),
                    symbol_id: None,
                    arguments: operation.failures.clone(),
                }
            } else {
                operation.result.clone()
            };
            match check_value(ir, &ty, value, budgets) {
                ValueContract::Accepted => {}
                ValueContract::Rejected { reason } => {
                    session.issues.push(issue("MOCK_OUTCOME_CONTRACT", reason))
                }
                ValueContract::Unknown { issue } => session.issues.push(issue),
            }
        }
        session
    }
    pub fn respond(&mut self, invocation: &Invocation) -> Observation {
        let matching = self
            .prepared
            .iter()
            .filter(|entry| &entry.invocation == invocation)
            .collect::<Vec<_>>();
        let outcome = if self.prepared.is_empty() {
            self.issues.push(issue(
                "MOCK_PLAN_REQUIRED",
                "Actual collaborator request requires an explicit prepared mock plan",
            ));
            Observation::Unavailable {
                reason: "MOCK_PLAN_REQUIRED".into(),
            }
        } else if !self.issues.is_empty() {
            Observation::Unavailable {
                reason: "Prepared mock plan is incomplete".into(),
            }
        } else if matching.len() == 1 {
            matching[0].outcome.clone()
        } else {
            let code = if matching.is_empty() {
                "UNEXPECTED_MOCK_CALL"
            } else {
                "AMBIGUOUS_MOCK_REPLY"
            };
            self.issues.push(issue(
                code,
                format!(
                    "Actual call to {} has {} prepared matches",
                    invocation.subject,
                    matching.len()
                ),
            ));
            Observation::Unavailable {
                reason: code.into(),
            }
        };
        self.observed.push(InvocationObservation {
            invocation: invocation.clone(),
            observation: outcome.clone(),
        });
        outcome
    }
}
