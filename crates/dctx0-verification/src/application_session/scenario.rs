//! Native scenario steps reuse the central ordered scenario evaluator. A fixture
//! keeps its resources across steps; prepared replies never constitute calls.
use super::*;
use crate::{Scenario, ScenarioStep};

pub(super) struct ScenarioCase<'a> {
    scenario: Scenario<'a>,
    pending: Option<Invocation>,
    bindings: BTreeMap<String, VerificationValue>,
    collaborators: Vec<InvocationObservation>,
}
impl<'a> ScenarioCase<'a> {
    pub fn find(ir: &'a WorkspaceIr, program: &'a crate::ProgramPlan, id: &str) -> Option<Self> {
        let plan = program.examples.iter().find(|plan| plan.subject == id)?;
        Some(Self {
            scenario: Scenario::new(ir, plan, &program.budgets),
            pending: None,
            bindings: plan.bindings.clone(),
            collaborators: vec![],
        })
    }
    pub fn advance(&mut self, record: &mut VerificationCaseRecord) -> ApplicationDirective {
        record.bindings = self.bindings.clone();
        match self.scenario.advance() {
            ScenarioStep::Invoke(invocation) => {
                self.pending = Some(invocation.clone());
                ApplicationDirective::Invoke {
                    invocation,
                    phase: ApplicationInvocationPhase::Production,
                }
            }
            ScenarioStep::Complete(evidence) => {
                self.pending = None;
                record.subject = evidence.subject;
                record.observations = evidence.invocations;
                // Events here are actual emissions collected by the central
                // evaluator, never reconstructed from its expectations.
                record.events = Some(evidence.events);
                ApplicationDirective::Verdict {
                    result: evidence.result,
                }
            }
        }
    }
    pub fn mocks(&self) -> Vec<String> {
        self.scenario.collaborator_subjects()
    }
    pub fn collaborator(&mut self, invocation: &Invocation) -> Option<Observation> {
        self.pending.as_ref()?;
        let observation = self.scenario.collaborator(invocation);
        self.collaborators.push(InvocationObservation {
            invocation: invocation.clone(),
            observation: observation.clone(),
        });
        Some(observation)
    }
    pub fn observe(
        &mut self,
        observed: ApplicationObservation,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        match observed {
            ApplicationObservation::Invoked {
                invocation,
                observation,
                calls,
                events,
                resources,
            } => {
                if self.pending.as_ref() != Some(&invocation) {
                    return ApplicationDirective::Verdict {
                        result: failed(
                            "APPLICATION_INPUT_CHANGED",
                            "Scenario invocation differs from its pending step",
                        ),
                    };
                }
                self.pending = None;
                let subjects = self.mocks();
                if !subjects.is_empty() {
                    let Some(calls) = &calls else {
                        return ApplicationDirective::Verdict {
                            result: unknown(
                                "MOCK_CALL_OBSERVER_REQUIRED",
                                "Scenario collaborator requests need a real call observer",
                            ),
                        };
                    };
                    let mut remaining = std::mem::take(&mut self.collaborators);
                    let consistent = calls
                        .iter()
                        .filter(|call| subjects.contains(&call.invocation.subject))
                        .all(|call| {
                            if let Some(index) = remaining.iter().position(|actual| actual == call)
                            {
                                remaining.swap_remove(index);
                                true
                            } else {
                                false
                            }
                        })
                        && remaining.is_empty();
                    if !consistent {
                        return ApplicationDirective::Verdict {
                            result: failed(
                                "MOCK_OBSERVATION_MISMATCH",
                                "Scenario calls differ from actual collaborator requests",
                            ),
                        };
                    }
                }
                record.resources = match (record.resources.take(), resources) {
                    (Some(mut all), Some(next)) => {
                        all.extend(next);
                        Some(all)
                    }
                    (None, Some(next)) if record.observations.is_empty() => Some(next),
                    _ => None,
                };
                record.observations.push(InvocationObservation {
                    invocation,
                    observation: observation.clone(),
                });
                // Missing event observers are not credited as empty emissions.
                let Some(events) = events else {
                    return ApplicationDirective::Verdict {
                        result: unknown(
                            "APPLICATION_EVENT_OBSERVER",
                            "Scenario event observer is absent",
                        ),
                    };
                };
                self.scenario.deliver_observed(observation, events, calls);
                self.advance(record)
            }
            ApplicationObservation::BindingError { message } => ApplicationDirective::Verdict {
                result: unknown("APPLICATION_BINDING_ERROR", message),
            },
            _ => ApplicationDirective::Verdict {
                result: failed(
                    "APPLICATION_OBSERVATION_ORDER",
                    "Scenario requires its pending production observation",
                ),
            },
        }
    }
}
