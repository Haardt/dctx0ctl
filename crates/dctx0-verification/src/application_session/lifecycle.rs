//! A lifecycle fixture lives across its complete path. Transition states come
//! from independent actual entity snapshots, not return values or step labels.
use super::*;
use crate::{
    LifecycleCaseInput, LifecycleObservation, LifecyclePath, LifecyclePlan, assess_lifecycle,
    lifecycle_input_contract, lifecycle_ready,
};
use dctx0_interpreter::ObservationContext;

pub(super) struct LifecycleCase {
    plan: LifecyclePlan,
    path: LifecyclePath,
    input: Option<LifecycleCaseInput>,
    setup: usize,
    position: usize,
    operation: Option<operation::OperationCase>,
    scratch: Option<VerificationCaseRecord>,
    pending: Option<ApplicationDirective>,
    context: ObservationContext,
    observed: Option<InvocationObservation>,
    transitions: Vec<LifecycleObservation>,
}
impl LifecycleCase {
    pub fn find(
        program: &crate::ProgramPlan,
        inputs: &BTreeMap<String, LifecycleCaseInput>,
        id: &str,
    ) -> Option<Self> {
        for plan in &program.lifecycles {
            if let Some(path) = plan.paths.iter().find(|path| path.id == id) {
                return Some(Self {
                    plan: plan.clone(),
                    path: path.clone(),
                    input: inputs.get(id).cloned(),
                    setup: 0,
                    position: 0,
                    operation: None,
                    scratch: None,
                    pending: None,
                    context: Default::default(),
                    observed: None,
                    transitions: vec![],
                });
            }
        }
        None
    }
    fn verdict(result: VerificationResult) -> ApplicationDirective {
        ApplicationDirective::Verdict { result }
    }
    fn invocation(&self) -> Invocation {
        let input = self.input.as_ref().expect("validated lifecycle input");
        Invocation {
            subject: self.plan.transitions[self.path.transitions[self.position]]
                .operation
                .clone(),
            arguments: input.arguments[self.position].clone(),
        }
    }
    pub fn start(
        &mut self,
        ir: &WorkspaceIr,
        program: &crate::ProgramPlan,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        record.subject = self.plan.subject.clone();
        let Some(input) = &self.input else {
            return Self::verdict(unknown("LIFECYCLE_INPUT_REQUIRED", &self.path.id));
        };
        let valid = lifecycle_input_contract(ir, &self.plan, &self.path, input, &program.budgets);
        if valid != VerificationResult::Pass {
            return Self::verdict(valid);
        }
        record
            .bindings
            .insert("keys".into(), VerificationValue::List(input.keys.clone()));
        record.arguments = input.arguments.clone();
        self.advance(ir, program, record)
    }
    fn advance(
        &mut self,
        ir: &WorkspaceIr,
        program: &crate::ProgramPlan,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        let input = self.input.as_ref().expect("validated lifecycle input");
        if self.setup < input.setup.len() {
            return self.begin_operation(
                ir,
                program,
                input.setup[self.setup].clone(),
                true,
                record,
            );
        }
        if self.position == self.path.transitions.len() {
            return Self::verdict(
                assess_lifecycle(
                    ir,
                    &self.plan,
                    &self.path,
                    &input.keys,
                    &self.transitions,
                    &program.budgets,
                )
                .result,
            );
        }
        self.context = Default::default();
        self.observed = None;
        let pending = ApplicationDirective::ReadState {
            entities: vec![self.plan.entity.clone()],
            phase: StatePhase::Before,
            invocation: self.invocation(),
        };
        self.pending = Some(pending.clone());
        pending
    }
    fn begin_operation(
        &mut self,
        ir: &WorkspaceIr,
        program: &crate::ProgramPlan,
        invocation: Invocation,
        setup: bool,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        let Some(mut operation) = operation::OperationCase::local(program, invocation, setup)
        else {
            return Self::verdict(unknown(
                "LIFECYCLE_OPERATION_REQUIRED",
                "Lifecycle step is outside the projected operation closure",
            ));
        };
        let mut scratch = record.clone();
        scratch.states.clear();
        scratch.observations.clear();
        scratch.arguments.clear();
        scratch.setup.clear();
        scratch.bindings.clear();
        scratch.events = None;
        scratch.resources = None;
        let directive = operation.start(ir, &program.budgets, &mut scratch);
        self.operation = Some(operation);
        self.scratch = Some(scratch);
        self.operation_directive(ir, program, directive, record)
    }
    fn operation_directive(
        &mut self,
        ir: &WorkspaceIr,
        program: &crate::ProgramPlan,
        mut directive: ApplicationDirective,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        let setup = self.setup < self.input.as_ref().expect("validated input").setup.len();
        if let ApplicationDirective::Verdict { result } = &directive {
            if *result != VerificationResult::Pass {
                return directive;
            }
            let scratch = self.scratch.take().expect("active operation record");
            record.states.extend(scratch.states);
            record.observations.extend(scratch.observations.clone());
            self.operation = None;
            if setup {
                let invocation = &self.input.as_ref().expect("validated input").setup[self.setup];
                if !scratch.observations.iter().any(|observed| observed.invocation == *invocation && matches!(&observed.observation, Observation::Returned { value } if !matches!(value, VerificationValue::Failure { .. }))) {
                    return Self::verdict(unknown("SETUP_REQUIREMENT_UNDELIVERED", "Lifecycle setup did not establish its prerequisite"));
                }
                record.setup.push(invocation.clone());
                self.setup += 1;
                return self.advance(ir, program, record);
            }
            directive = ApplicationDirective::ReadState {
                entities: vec![self.plan.entity.clone()],
                phase: StatePhase::After,
                invocation: self.invocation(),
            };
        } else if setup && let ApplicationDirective::Invoke { phase, .. } = &mut directive {
            *phase = ApplicationInvocationPhase::Setup;
        }
        self.pending = Some(directive.clone());
        directive
    }
    pub fn observe(
        &mut self,
        ir: &WorkspaceIr,
        program: &crate::ProgramPlan,
        mut observation: ApplicationObservation,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        if let ApplicationObservation::BindingError { message } = observation {
            return Self::verdict(unknown("APPLICATION_BINDING_ERROR", message));
        }
        if self.operation.is_some() {
            if let ApplicationObservation::Invoked {
                invocation,
                observation,
                calls,
                events,
                resources,
            } = &observation
            {
                if self.setup == self.input.as_ref().expect("validated input").setup.len()
                    && *invocation == self.invocation()
                {
                    self.observed = Some(InvocationObservation {
                        invocation: invocation.clone(),
                        observation: observation.clone(),
                    });
                    self.context.calls = calls.clone();
                    self.context.events = events.clone();
                    self.context.resources = resources.clone();
                }
            }
            // The external setup phase is a fixture-routing distinction. The
            // internal operation evaluator still assesses this one real call.
            let directive = self.operation.as_mut().expect("active operation").observe(
                ir,
                &program.budgets,
                observation,
                self.scratch.as_mut().expect("active record"),
            );
            return self.operation_directive(ir, program, directive, record);
        }
        match (self.pending.as_ref(), &mut observation) {
            (
                Some(ApplicationDirective::ReadState {
                    phase,
                    invocation,
                    entities,
                }),
                ApplicationObservation::State {
                    phase: actual_phase,
                    invocation: actual_invocation,
                    entities: actual,
                    complete,
                },
            ) if phase == actual_phase
                && invocation == actual_invocation
                && actual.keys().eq(entities.iter()) =>
            {
                if !*complete {
                    return Self::verdict(unknown(
                        "APPLICATION_STATE_INCOMPLETE",
                        "Lifecycle snapshot is incomplete",
                    ));
                }
                record.states.push(VerificationStateEvidence {
                    phase: *phase,
                    phase_recorded: Some(true),
                    entities: actual.clone(),
                });
                if *phase == StatePhase::Before {
                    self.context.before = Some(actual.clone());
                    let ready = lifecycle_ready(
                        ir,
                        &self.plan,
                        self.path.transitions[self.position],
                        &self.input.as_ref().expect("validated input").keys,
                        invocation,
                        &self.context,
                        &program.budgets,
                    );
                    if ready != VerificationResult::Pass {
                        return Self::verdict(ready);
                    }
                    self.begin_operation(ir, program, invocation.clone(), false, record)
                } else {
                    self.context.after = Some(actual.clone());
                    self.context.phase = StatePhase::After;
                    let Some(invocation) = self.observed.take() else {
                        return Self::verdict(unknown(
                            "APPLICATION_CASE_INCOMPLETE",
                            "Lifecycle production observation is absent",
                        ));
                    };
                    self.transitions.push(LifecycleObservation {
                        invocation,
                        context: self.context.clone(),
                    });
                    self.position += 1;
                    self.advance(ir, program, record)
                }
            }
            _ => Self::verdict(failed(
                "APPLICATION_STATE_SCOPE",
                "Lifecycle snapshot scope, phase or invocation differs",
            )),
        }
    }
}
