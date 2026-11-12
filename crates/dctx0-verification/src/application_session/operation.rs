//! Local typed continuations over actual operations. Oracle tapes never fulfill
//! production-call effects. Setup inputs resolve only after real setup results.
use super::*;
use crate::{
    OperationCaseInput, OperationPlan, assess_operation_continued, operation_ready,
    resolve_operation_arguments,
};
use dctx0_interpreter::{ObservationContext, ValueContract, check_value};

pub(super) struct OperationCase {
    input: OperationCaseInput,
    bindings: Option<BTreeMap<String, VerificationValue>>,
    plan: OperationPlan,
    setup: Vec<InvocationObservation>,
    invocation: Invocation,
    current_plan: OperationPlan,
    context: ObservationContext,
    observed: Option<InvocationObservation>,
    tapes: BTreeMap<String, Vec<InvocationObservation>>,
    pending: ApplicationDirective,
    oracle_key: Option<String>,
    entities: Vec<String>,
    mocks: Option<crate::OperationMockSession>,
}

impl OperationCase {
    pub fn local(
        program: &crate::ProgramPlan,
        invocation: Invocation,
        setup: bool,
    ) -> Option<Self> {
        let mut operation = program
            .operations
            .iter()
            .find(|plan| plan.operation.id == invocation.subject)?
            .clone();
        operation.cases = vec![OperationCaseInput {
            id: "local-step".into(),
            invocation,
            setup: vec![],
            argument_bindings: vec![],
            mocks: vec![],
        }];
        if setup {
            operation
                .clauses
                .retain(|node| node.kind == StatementKind::Requires);
        }
        let mut local = program.clone();
        local.operations = vec![operation];
        Self::find(&local, "local-step")
    }
    pub fn find(program: &crate::ProgramPlan, id: &str) -> Option<Self> {
        for plan in &program.operations {
            let input = if let Some(input) = plan.cases.iter().find(|input| input.id == id) {
                input.clone()
            } else if let Some(index) = id
                .strip_prefix(&format!("{}:operation:", plan.operation.id))
                .and_then(|index| index.parse::<usize>().ok())
            {
                let bindings = plan.inputs.bindings.get(index)?;
                OperationCaseInput {
                    id: id.into(),
                    invocation: Invocation {
                        subject: plan.operation.id.clone(),
                        arguments: plan
                            .operation
                            .parameters
                            .iter()
                            .map(|p| bindings[&p.name].clone())
                            .collect(),
                    },
                    setup: plan.generated_setup(bindings),
                    argument_bindings: plan.generated_argument_bindings(bindings),
                    mocks: vec![],
                }
            } else {
                continue;
            };
            let bindings = id
                .strip_prefix(&format!("{}:operation:", plan.operation.id))
                .and_then(|i| i.parse::<usize>().ok())
                .and_then(|i| plan.inputs.bindings.get(i))
                .cloned();
            return Some(Self {
                bindings,
                invocation: input.invocation.clone(),
                current_plan: plan.clone(),
                input,
                plan: plan.clone(),
                setup: vec![],
                context: Default::default(),
                observed: None,
                tapes: BTreeMap::new(),
                pending: ApplicationDirective::Verdict {
                    result: unknown("APPLICATION_CASE_INCOMPLETE", id),
                },
                oracle_key: None,
                entities: vec![],
                mocks: None,
            });
        }
        None
    }
    fn verdict(&mut self, result: VerificationResult) -> ApplicationDirective {
        self.pending = ApplicationDirective::Verdict { result };
        self.pending.clone()
    }
    pub fn start(
        &mut self,
        ir: &WorkspaceIr,
        budgets: &VerificationBudgets,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        record.subject = self.input.invocation.subject.clone();
        if let Some(bindings) = &self.bindings {
            record.bindings = bindings.clone();
        }
        if self.mocks.is_none() {
            self.mocks = Some(crate::OperationMockSession::new(
                ir,
                &self.input.mocks,
                budgets,
            ));
        }
        if let Some(issue) = self.mocks.as_ref().and_then(|m| m.issues.first()).cloned() {
            return self.verdict(VerificationResult::Unknown { issue });
        }
        let target = (self.setup.len() < self.input.setup.len()).then_some(self.setup.len());
        self.invocation = match resolve_operation_arguments(ir, &self.input, &self.setup, target) {
            Ok(value) => value,
            Err(message) => return self.verdict(unknown("APPLICATION_SETUP_BINDING", message)),
        };
        self.current_plan = if target.is_some() {
            let Some(operation) = ir.operations.get(&self.invocation.subject) else {
                return self.verdict(unknown(
                    "APPLICATION_SETUP_OPERATION",
                    "Undeclared setup operation",
                ));
            };
            OperationPlan {
                operation: operation.clone(),
                clauses: ir
                    .nodes
                    .iter()
                    .filter(|n| n.owner == operation.id && n.kind == StatementKind::Requires)
                    .cloned()
                    .collect(),
                cases: vec![],
                generated_preparations: vec![],
                inputs: self.plan.inputs.clone(),
                purity_requirements: vec![],
                precondition_selection: None,
            }
        } else {
            self.plan.clone()
        };
        self.context = Default::default();
        self.observed = None;
        self.tapes.clear();
        self.oracle_key = None;
        self.entities = ir
            .references
            .iter()
            .filter(|r| {
                r.from == self.invocation.subject
                    && matches!(
                        r.kind,
                        ReferenceKind::Reads | ReferenceKind::Writes | ReferenceKind::State
                    )
            })
            .map(|r| r.target.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if target.is_none() {
            record.arguments.push(self.invocation.arguments.clone());
        }
        if !self.entities.is_empty() && (target.is_none() || !self.current_plan.clauses.is_empty())
        {
            self.pending = ApplicationDirective::ReadState {
                entities: self.entities.clone(),
                phase: StatePhase::Before,
                invocation: self.invocation.clone(),
            };
            self.pending.clone()
        } else {
            self.invoke(ir, budgets)
        }
    }
    fn invoke(&mut self, ir: &WorkspaceIr, budgets: &VerificationBudgets) -> ApplicationDirective {
        let ready = operation_ready(
            ir,
            &self.current_plan,
            &self.invocation,
            &self.context,
            budgets,
        );
        if ready != VerificationResult::Pass {
            return self.verdict(ready);
        }
        self.pending = ApplicationDirective::Invoke {
            invocation: self.invocation.clone(),
            phase: if self.setup.len() < self.input.setup.len() {
                ApplicationInvocationPhase::Setup
            } else {
                ApplicationInvocationPhase::Production
            },
        };
        self.pending.clone()
    }
    pub fn observe(
        &mut self,
        ir: &WorkspaceIr,
        budgets: &VerificationBudgets,
        observation: ApplicationObservation,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        match (self.pending.clone(), observation) {
            (
                ApplicationDirective::ReadState {
                    entities,
                    phase,
                    invocation,
                },
                ApplicationObservation::State {
                    entities: actual,
                    phase: actual_phase,
                    invocation: actual_invocation,
                    complete,
                },
            ) => {
                if actual_phase != phase
                    || actual_invocation != invocation
                    || actual.keys().ne(entities.iter())
                {
                    return self.verdict(failed(
                        "APPLICATION_STATE_SCOPE",
                        "State scope, phase or invocation differs from its pending observation",
                    ));
                }
                if !complete {
                    return self.verdict(unknown(
                        "APPLICATION_STATE_INCOMPLETE",
                        "State observer did not deliver its complete scope",
                    ));
                }
                record.states.push(VerificationStateEvidence {
                    phase,
                    phase_recorded: Some(true),
                    entities: actual.clone(),
                });
                if phase == StatePhase::Before {
                    self.context.before = Some(actual);
                    self.invoke(ir, budgets)
                } else {
                    self.context.after = Some(actual);
                    self.assess(ir, budgets, record)
                }
            }
            (
                ApplicationDirective::Invoke { invocation, phase },
                ApplicationObservation::Invoked {
                    invocation: actual,
                    observation,
                    calls,
                    events,
                    resources,
                },
            ) => {
                if actual != invocation {
                    return self.verdict(failed(
                        "APPLICATION_INPUT_CHANGED",
                        "Native invocation differs from its centrally selected input",
                    ));
                }
                let observed = InvocationObservation {
                    invocation,
                    observation,
                };
                if phase == ApplicationInvocationPhase::Production {
                    let result = self
                        .mocks
                        .as_ref()
                        .map(|m| m.check_recorded_calls(calls.as_deref()));
                    if let Some(result) = result.filter(|r| *r != VerificationResult::Pass) {
                        return self.verdict(result);
                    }
                    if let Some(issue) = self.mocks.as_ref().and_then(|m| m.issues.first()).cloned()
                    {
                        return self.verdict(VerificationResult::Fail { issue });
                    }
                }
                record.observations.push(observed.clone());
                if phase == ApplicationInvocationPhase::Oracle {
                    if events.as_ref().is_some_and(|e| !e.is_empty())
                        || resources
                            .as_ref()
                            .is_some_and(|r| r.iter().any(|r| r.access == ResourceAccess::Write))
                    {
                        return self.verdict(failed(
                            "PURE_CLAUSE_EFFECT",
                            "Pure clause oracle emitted an event or wrote a resource",
                        ));
                    }
                    if let Observation::Returned { value } = &observed.observation {
                        let operation = &ir.operations[&observed.invocation.subject];
                        match check_value(ir, &operation.result, value, budgets) {
                            ValueContract::Accepted => {}
                            ValueContract::Rejected { reason } => {
                                return self.verdict(failed("NATIVE_RETURN_CONTRACT", reason));
                            }
                            ValueContract::Unknown { issue } => {
                                return self.verdict(VerificationResult::Unknown { issue });
                            }
                        }
                    }
                    if let Some(key) = self.oracle_key.take() {
                        self.tapes.entry(key).or_default().push(observed)
                    }
                    return self.assess(ir, budgets, record);
                }
                if self.current_plan.operation.pure
                    && (events.as_ref().is_some_and(|e| !e.is_empty())
                        || resources
                            .as_ref()
                            .is_some_and(|r| r.iter().any(|r| r.access == ResourceAccess::Write)))
                {
                    return self.verdict(failed(
                        "PURE_OPERATION_EFFECT",
                        "Pure production operation emitted an event or wrote a resource",
                    ));
                }
                self.context.calls = calls;
                self.context.events = events.clone();
                self.context.resources = resources.clone();
                if phase == ApplicationInvocationPhase::Production {
                    record.events = events;
                    record.resources = resources;
                }
                self.observed = Some(observed);
                if phase == ApplicationInvocationPhase::Production && !self.entities.is_empty() {
                    self.pending = ApplicationDirective::ReadState {
                        entities: self.entities.clone(),
                        phase: StatePhase::After,
                        invocation: self.invocation.clone(),
                    };
                    self.pending.clone()
                } else {
                    self.assess(ir, budgets, record)
                }
            }
            (_, ApplicationObservation::BindingError { message }) => {
                self.verdict(unknown("APPLICATION_BINDING_ERROR", message))
            }
            _ => self.verdict(failed(
                "APPLICATION_OBSERVATION_ORDER",
                "Native observation does not match its pending directive",
            )),
        }
    }
    fn assess(
        &mut self,
        ir: &WorkspaceIr,
        budgets: &VerificationBudgets,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        let Some(observed) = &self.observed else {
            return self.verdict(unknown(
                "APPLICATION_CASE_INCOMPLETE",
                "No production observation",
            ));
        };
        let report = assess_operation_continued(
            ir,
            &self.current_plan,
            observed,
            &self.context,
            budgets,
            &self.tapes,
        );
        if let Some(call) = report.pending_call {
            if self.tapes.values().map(Vec::len).sum::<usize>() >= budgets.evaluation_steps {
                return self.verdict(unknown(
                    "EVALUATION_BUDGET",
                    "Pure continuation budget exhausted",
                ));
            }
            let Some(operation) = ir.operations.get(&call.invocation.subject) else {
                return self.verdict(unknown("CLAUSE_CALL_INPUT", "Undeclared oracle"));
            };
            if !operation.pure
                || operation.parameters.len() != call.invocation.arguments.len()
                || operation
                    .parameters
                    .iter()
                    .zip(&call.invocation.arguments)
                    .any(|(p, v)| {
                        !matches!(check_value(ir, &p.ty, v, budgets), ValueContract::Accepted)
                    })
            {
                return self.verdict(unknown(
                    "CLAUSE_CALL_INPUT",
                    "Oracle does not match a valid pure signature",
                ));
            }
            self.oracle_key = Some(call.key);
            self.pending = ApplicationDirective::Invoke {
                invocation: call.invocation,
                phase: ApplicationInvocationPhase::Oracle,
            };
            return self.pending.clone();
        }
        if report.result != VerificationResult::Pass {
            return self.verdict(report.result);
        }
        if self.setup.len() < self.input.setup.len() {
            if !matches!(&observed.observation,Observation::Returned {value} if !matches!(value,VerificationValue::Failure {..}))
            {
                return self.verdict(unknown(
                    "SETUP_REQUIREMENT_UNDELIVERED",
                    "Setup failure did not establish prerequisites",
                ));
            }
            record.setup.push(observed.invocation.clone());
            self.setup.push(observed.clone());
            return self.start(ir, budgets, record);
        }
        self.verdict(report.result)
    }
    pub fn mock_subjects(&self) -> Vec<String> {
        if self.setup.len() < self.input.setup.len()
            || matches!(
                self.pending,
                ApplicationDirective::Invoke {
                    phase: ApplicationInvocationPhase::Oracle,
                    ..
                }
            )
        {
            return vec![];
        }
        self.input
            .mocks
            .iter()
            .map(|mock| mock.invocation.subject.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn collaborator(&mut self, invocation: &Invocation) -> Option<Observation> {
        if !matches!(
            self.pending,
            ApplicationDirective::Invoke {
                phase: ApplicationInvocationPhase::Production,
                ..
            }
        ) {
            return None;
        }
        Some(self.mocks.as_mut()?.respond(invocation))
    }
}
