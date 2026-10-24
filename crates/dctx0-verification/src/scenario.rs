//! Ordered examples with observed collaborators. Pure state, no target or I/O.
use crate::{ExampleAction, ExamplePlan, Expectation, MockOutcome};
use dctx0_core::*;
use dctx0_interpreter::{
    Evaluation, ObservationContext, ValueContract, check_value, evaluate, evaluate_observed,
    modeled_value,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioEvidence {
    pub subject: String,
    pub invocations: Vec<InvocationObservation>,
    pub collaborators: Vec<InvocationObservation>,
    pub events: Vec<VerificationEvent>,
    pub checks: Vec<String>,
    /// Given configures behavior, not an invented exact-one assertion.
    pub unused_mocks: Vec<String>,
    pub result: VerificationResult,
}
pub enum ScenarioStep {
    Invoke(Invocation),
    Complete(Box<ScenarioEvidence>),
}
struct PreparedMock {
    source: String,
    invocation: Invocation,
    observation: Observation,
    calls: usize,
}
// Incomplete prefixes cannot be repaired by later observed steps.
enum CallTrace {
    Pending,
    Observed(Vec<InvocationObservation>),
    Incomplete,
}
pub struct Scenario<'a> {
    ir: &'a WorkspaceIr,
    plan: &'a ExamplePlan,
    budgets: &'a VerificationBudgets,
    variables: BTreeMap<String, VerificationValue>,
    cursor: usize,
    executed: BTreeSet<(u8, usize)>,
    pending: Option<(Invocation, String)>,
    last: Option<Observation>,
    calls: CallTrace,
    mocks: Vec<PreparedMock>,
    evidence: ScenarioEvidence,
}
fn unknown(code: &str, message: &str) -> Box<VerificationResult> {
    Box::new(VerificationResult::Unknown {
        issue: VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: None,
        },
    })
}
fn failed(code: &str, message: &str) -> Box<VerificationResult> {
    Box::new(VerificationResult::Fail {
        issue: VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: None,
        },
    })
}
fn contract(result: ValueContract) -> Result<(), Box<VerificationResult>> {
    match result {
        ValueContract::Accepted => Ok(()),
        ValueContract::Rejected { reason } => Err(failed("SCENARIO_CONTRACT", &reason)),
        ValueContract::Unknown { issue } => Err(Box::new(VerificationResult::Unknown { issue })),
    }
}
impl<'a> Scenario<'a> {
    pub fn new(
        ir: &'a WorkspaceIr,
        plan: &'a ExamplePlan,
        budgets: &'a VerificationBudgets,
    ) -> Self {
        Self {
            ir,
            plan,
            budgets,
            variables: plan.bindings.clone(),
            cursor: 0,
            executed: BTreeSet::new(),
            pending: None,
            last: None,
            calls: CallTrace::Pending,
            mocks: Vec::new(),
            evidence: ScenarioEvidence {
                subject: plan.subject.clone(),
                invocations: Vec::new(),
                collaborators: Vec::new(),
                events: Vec::new(),
                checks: Vec::new(),
                unused_mocks: Vec::new(),
                result: plan
                    .issues
                    .first()
                    .map_or(VerificationResult::Pass, |issue| {
                        VerificationResult::Unknown {
                            issue: issue.clone(),
                        }
                    }),
            },
        }
    }
    fn stop(&mut self, result: &VerificationResult) {
        if matches!(self.evidence.result, VerificationResult::Pass)
            || matches!(*result, VerificationResult::Fail { .. })
                && !matches!(self.evidence.result, VerificationResult::Fail { .. })
        {
            self.evidence.result = result.clone();
        }
    }
    fn value(&self, expression: &Expression) -> Result<VerificationValue, Box<VerificationResult>> {
        self.evaluated(
            expression,
            evaluate(self.ir, expression, &self.variables, &[], self.budgets),
        )
    }
    fn assertion_value(
        &self,
        expression: &Expression,
    ) -> Result<VerificationValue, Box<VerificationResult>> {
        let context = ObservationContext {
            calls: match &self.calls {
                CallTrace::Observed(calls) => Some(calls.clone()),
                CallTrace::Pending | CallTrace::Incomplete => None,
            },
            ..Default::default()
        };
        self.evaluated(
            expression,
            evaluate_observed(
                self.ir,
                expression,
                &self.variables,
                &[],
                &context,
                self.budgets,
            ),
        )
    }
    fn evaluated(
        &self,
        expression: &Expression,
        evaluation: Evaluation,
    ) -> Result<VerificationValue, Box<VerificationResult>> {
        let value = match evaluation {
            Evaluation::Complete(value) => value,
            Evaluation::Unknown(issue) => {
                return Err(Box::new(VerificationResult::Unknown { issue }));
            }
            _ => {
                return Err(unknown(
                    "SCENARIO_EXPRESSION_CALL",
                    "Nested actual calls need a separate expression continuation",
                ));
            }
        };
        modeled_value(self.ir, &expression.ty, value, self.budgets).map_err(|value| match value {
            ValueContract::Unknown { issue } => Box::new(VerificationResult::Unknown { issue }),
            other => unknown(
                "MODELED_VALUE_CONTRACT",
                &format!("Invalid modeled value: {other:?}"),
            ),
        })
    }
    fn invocation(&self, expression: &Expression) -> Result<Invocation, Box<VerificationResult>> {
        let ExpressionKind::Call {
            target: Some(subject),
            arguments,
            ..
        } = &expression.value
        else {
            return Err(unknown(
                "SCENARIO_INVOCATION",
                "An actual operation invocation is required",
            ));
        };
        let operation =
            self.ir.operations.get(subject).ok_or_else(|| {
                unknown("SCENARIO_OPERATION", "Operation declaration is unavailable")
            })?;
        if operation.parameters.len() != arguments.len() {
            return Err(unknown(
                "SCENARIO_ARITY",
                "Invocation arguments do not match the declaration",
            ));
        }
        let arguments = arguments
            .iter()
            .zip(&operation.parameters)
            .map(|(expression, parameter)| {
                let value = self.value(expression)?;
                contract(check_value(self.ir, &parameter.ty, &value, self.budgets))?;
                Ok(value)
            })
            .collect::<Result<_, Box<VerificationResult>>>()?;
        Ok(Invocation {
            subject: subject.clone(),
            arguments,
        })
    }
    fn prepare(&mut self, index: usize) -> Result<(), Box<VerificationResult>> {
        let step = self
            .plan
            .mocks
            .get(index)
            .ok_or_else(|| unknown("SCENARIO_ORDER", "Mock action refers to a missing step"))?;
        let invocation = self.invocation(&step.invocation)?;
        let observation = match &step.outcome {
            Some(MockOutcome::Returned { expression }) => Observation::Returned {
                value: self.value(expression)?,
            },
            Some(MockOutcome::Failed { expression }) => {
                let declaration = expression.ty.symbol_id.clone().ok_or_else(|| {
                    unknown("SCENARIO_FAILURE", "Mock failure has no declaration")
                })?;
                Observation::Returned {
                    value: VerificationValue::Failure {
                        declaration,
                        payload: Some(Box::new(self.value(expression)?)),
                    },
                }
            }
            None => return Err(unknown("SCENARIO_MOCK_OUTCOME", "Mock outcome is missing")),
        };
        self.check_observation(&invocation, &observation)?;
        self.mocks.push(PreparedMock {
            source: step.source_id.clone(),
            invocation,
            observation,
            calls: 0,
        });
        Ok(())
    }
    fn check_observation(
        &self,
        invocation: &Invocation,
        observation: &Observation,
    ) -> Result<(), Box<VerificationResult>> {
        match observation {
            Observation::Returned { value } => {
                let operation = &self.ir.operations[&invocation.subject];
                let ty = if matches!(value, VerificationValue::Failure { .. }) {
                    TypeRef {
                        name: "Failure".into(),
                        symbol_id: None,
                        arguments: operation.failures.clone(),
                    }
                } else {
                    operation.result.clone()
                };
                contract(check_value(self.ir, &ty, value, self.budgets))
            }
            Observation::Threw { message } => Err(unknown("APPLICATION_EXCEPTION", message)),
            Observation::Unavailable { reason } => Err(unknown("OPERATION_UNAVAILABLE", reason)),
        }
    }
    /// Called only when the native consumer actually invokes a collaborator.
    /// Matching does not invent a call count: each actual request is retained.
    pub fn collaborator(&mut self, invocation: &Invocation) -> Observation {
        let matching = self
            .mocks
            .iter()
            .enumerate()
            .filter(|(_, mock)| &mock.invocation == invocation)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let observation = match matching.as_slice() {
            [index] => {
                let mock = &mut self.mocks[*index];
                mock.calls += 1;
                mock.observation.clone()
            }
            _ => {
                self.stop(&failed(
                    "UNEXPECTED_COLLABORATOR",
                    "Actual collaborator arguments have no unique prepared behavior",
                ));
                Observation::Unavailable {
                    reason: "No unique prepared collaborator behavior".into(),
                }
            }
        };
        self.evidence.collaborators.push(InvocationObservation {
            invocation: invocation.clone(),
            observation: observation.clone(),
        });
        observation
    }
    pub fn deliver(&mut self, observation: Observation, events: Vec<VerificationEvent>) {
        self.deliver_observed(observation, events, None);
    }
    /// Deliver one actual step. None is a missing observer, not an empty trace.
    /// Call assertions count the complete executed prefix of this example.
    pub fn deliver_observed(
        &mut self,
        observation: Observation,
        events: Vec<VerificationEvent>,
        calls: Option<Vec<InvocationObservation>>,
    ) {
        let Some((invocation, binding)) = self.pending.take() else {
            self.stop(&unknown(
                "SCENARIO_PROTOCOL",
                "Observation has no pending invocation",
            ));
            return;
        };
        self.calls = match (
            std::mem::replace(&mut self.calls, CallTrace::Incomplete),
            calls,
        ) {
            (CallTrace::Pending, Some(calls)) => CallTrace::Observed(calls),
            (CallTrace::Observed(mut previous), Some(calls)) => {
                previous.extend(calls);
                CallTrace::Observed(previous)
            }
            _ => CallTrace::Incomplete,
        };
        if let Err(result) = self.check_observation(&invocation, &observation) {
            self.stop(&result);
        }
        if let Observation::Returned { value } = &observation {
            self.variables.insert(binding, value.clone());
        }
        for event in &events {
            let ty = TypeRef {
                name: event.subject.clone(),
                symbol_id: Some(event.subject.clone()),
                arguments: Vec::new(),
            };
            if !self
                .ir
                .symbols
                .get(&event.subject)
                .is_some_and(|symbol| symbol.kind == StatementKind::Event)
            {
                self.stop(&failed(
                    "UNDECLARED_EVENT",
                    "Observed event is not a declared event",
                ));
            } else if let Err(result) =
                contract(check_value(self.ir, &ty, &event.payload, self.budgets))
            {
                self.stop(&result);
            }
        }
        self.evidence.events.extend(events);
        self.evidence.invocations.push(InvocationObservation {
            invocation,
            observation: observation.clone(),
        });
        self.last = Some(observation);
    }
    fn check(&mut self, index: usize) -> Result<(), Box<VerificationResult>> {
        let expectation = self.plan.expectations.get(index).ok_or_else(|| {
            unknown(
                "SCENARIO_ORDER",
                "Check action refers to a missing expectation",
            )
        })?;
        let source = match expectation {
            Expectation::Success { source_id } => {
                if !matches!(self.last, Some(Observation::Returned { ref value }) if !matches!(value, VerificationValue::Failure { .. }))
                {
                    return Err(failed(
                        "EXPECTED_SUCCESS",
                        "The actual operation did not return success",
                    ));
                }
                source_id
            }
            Expectation::Failure {
                source_id,
                declaration,
                payload,
            } => {
                let Some(declaration) = declaration else {
                    return Err(unknown(
                        "EXPECTED_FAILURE_TYPE",
                        "Expected failure declaration is unavailable",
                    ));
                };
                let expected = payload
                    .as_ref()
                    .map(|expression| self.value(expression))
                    .transpose()?;
                if !matches!(&self.last, Some(Observation::Returned { value: VerificationValue::Failure { declaration: actual, payload } }) if actual == declaration && expected.as_ref().is_none_or(|expected| payload.as_deref() == Some(expected)))
                {
                    return Err(failed(
                        "EXPECTED_FAILURE",
                        "The actual failure does not match the modeled expectation",
                    ));
                }
                source_id
            }
            Expectation::Emitted {
                source_id,
                declaration,
                payload,
            } => {
                let expected = payload
                    .as_ref()
                    .map(|expression| self.value(expression))
                    .transpose()?;
                if !self.evidence.events.iter().any(|event| {
                    &event.subject == declaration
                        && expected
                            .as_ref()
                            .is_none_or(|value| &event.payload == value)
                }) {
                    return Err(failed(
                        "EXPECTED_EVENT",
                        "Expected event/payload was not observed",
                    ));
                }
                source_id
            }
            Expectation::Assertion {
                source_id,
                expression,
            } => {
                match self.assertion_value(expression)? {
                    VerificationValue::Boolean(true) => {}
                    VerificationValue::Boolean(false) => {
                        return Err(failed(
                            "ASSERTION_FAILED",
                            "Actual example result violates the assertion",
                        ));
                    }
                    _ => {
                        return Err(unknown(
                            "NON_BOOLEAN_ASSERTION",
                            "Example assertion is not Boolean",
                        ));
                    }
                }
                source_id
            }
        };
        self.evidence.checks.push(source.clone());
        Ok(())
    }
    pub fn advance(&mut self) -> ScenarioStep {
        if self.pending.is_some() {
            self.stop(&unknown(
                "SCENARIO_PROTOCOL",
                "Pending invocation requires an observation",
            ));
        }
        while matches!(self.evidence.result, VerificationResult::Pass)
            && self.cursor < self.plan.order.len()
        {
            let action = self.plan.order[self.cursor].clone();
            self.cursor += 1;
            let identity = match action {
                ExampleAction::PrepareMock(index) => (0, index),
                ExampleAction::Invoke(index) => (1, index),
                ExampleAction::Check(index) => (2, index),
            };
            if !self.executed.insert(identity) {
                self.stop(&unknown(
                    "DUPLICATE_SCENARIO_ACTION",
                    "A mandatory scenario action appears more than once",
                ));
                break;
            }
            let result = match action {
                ExampleAction::PrepareMock(index) => self.prepare(index),
                ExampleAction::Invoke(index) => match self.plan.invocations.get(index) {
                    None => Err(unknown(
                        "SCENARIO_ORDER",
                        "Invoke action refers to a missing step",
                    )),
                    Some(step) => match self.invocation(&step.invocation) {
                        Err(result) => Err(result),
                        Ok(invocation) => {
                            self.pending = Some((invocation.clone(), step.binding.clone()));
                            return ScenarioStep::Invoke(invocation);
                        }
                    },
                },
                ExampleAction::Check(index) => self.check(index),
            };
            if let Err(result) = result {
                self.stop(&result);
            }
        }
        self.evidence.unused_mocks = self
            .mocks
            .iter()
            .filter(|mock| mock.calls == 0)
            .map(|mock| mock.source.clone())
            .collect();
        if !self.evidence.unused_mocks.is_empty()
            && matches!(self.evidence.result, VerificationResult::Pass)
        {
            self.stop(&unknown(
                "UNEXERCISED_COLLABORATOR",
                "Prepared collaborators were not exercised; consumer evidence is incomplete",
            ));
        }
        if self.evidence.invocations.is_empty()
            && matches!(self.evidence.result, VerificationResult::Pass)
        {
            self.stop(&unknown(
                "EMPTY_SCENARIO",
                "No actual operation was executed",
            ));
        }
        if matches!(self.evidence.result, VerificationResult::Pass)
            && self.executed.len()
                != self.plan.mocks.len()
                    + self.plan.invocations.len()
                    + self.plan.expectations.len()
        {
            self.stop(&unknown(
                "MISSING_SCENARIO_ACTION",
                "Mandatory mock, invocation or expectation actions were omitted",
            ));
        }
        ScenarioStep::Complete(Box::new(self.evidence.clone()))
    }
}
