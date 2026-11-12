//! Ordered pure-call tapes for the existing central assertion cases.
use super::*;
use crate::{
    CaseEvidence, Obligation, VerificationCase, VerificationPlan, assess, next_invocation,
};
use dctx0_interpreter::{ValueContract, check_value};
pub(super) struct AssertionCase {
    case: VerificationCase,
    subject: String,
    pending: Option<Invocation>,
    tape: Vec<InvocationObservation>,
}
impl AssertionCase {
    pub fn find(program: &crate::ProgramPlan, id: &str) -> Option<Self> {
        for property in &program.properties {
            for case in property.assertions.iter().flat_map(|a| &a.cases) {
                if case.id == id {
                    return Some(Self {
                        case: case.clone(),
                        subject: property.subject.clone(),
                        pending: None,
                        tape: vec![],
                    });
                }
            }
        }
        None
    }
    pub fn advance(
        &mut self,
        ir: &WorkspaceIr,
        program: &crate::ProgramPlan,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        record.subject = self.subject.clone();
        record.bindings = self.case.variables.clone();
        match next_invocation(ir, &self.case, &self.tape, &program.budgets) {
            Ok(Some(invocation)) => {
                if self.tape.len() >= program.budgets.evaluation_steps {
                    return ApplicationDirective::Verdict {
                        result: unknown(
                            "EVALUATION_BUDGET",
                            "Assertion continuation budget exhausted",
                        ),
                    };
                }
                let valid = ir.operations.get(&invocation.subject).is_some_and(|op| {
                    op.pure
                        && op.parameters.len() == invocation.arguments.len()
                        && op
                            .parameters
                            .iter()
                            .zip(&invocation.arguments)
                            .all(|(p, v)| {
                                matches!(
                                    check_value(ir, &p.ty, v, &program.budgets),
                                    ValueContract::Accepted
                                )
                            })
                });
                if !valid {
                    return ApplicationDirective::Verdict {
                        result: unknown(
                            "CLAUSE_CALL_INPUT",
                            "Assertion continuation does not match a valid pure signature",
                        ),
                    };
                }
                self.pending = Some(invocation.clone());
                ApplicationDirective::Invoke {
                    invocation,
                    phase: ApplicationInvocationPhase::Oracle,
                }
            }
            Ok(None) => {
                let plan = VerificationPlan {
                    schema_version: 1,
                    snapshot: program.snapshot.clone(),
                    check_occurrence: program.check_occurrence,
                    scope: crate::VerificationScope {
                        subjects: BTreeSet::from([self.subject.clone()]),
                        tasks: BTreeSet::new(),
                    },
                    budgets: program.budgets.clone(),
                    obligations: vec![Obligation {
                        id: self.case.id.clone(),
                        subject: self.subject.clone(),
                        cases: vec![self.case.clone()],
                        unsupported: None,
                        bounded_corpus: false,
                    }],
                };
                ApplicationDirective::Verdict {
                    result: assess(
                        ir,
                        &plan,
                        &[CaseEvidence {
                            case_id: self.case.id.clone(),
                            observations: self.tape.clone(),
                        }],
                    )
                    .result,
                }
            }
            Err(result) => ApplicationDirective::Verdict { result: *result },
        }
    }
    pub fn observe(
        &mut self,
        ir: &WorkspaceIr,
        program: &crate::ProgramPlan,
        observation: ApplicationObservation,
        record: &mut VerificationCaseRecord,
    ) -> ApplicationDirective {
        let result = match observation {
            ApplicationObservation::Invoked {
                invocation,
                observation,
                events,
                resources,
                ..
            } if self.pending.as_ref() == Some(&invocation) => {
                if events.as_ref().is_some_and(|e| !e.is_empty())
                    || resources
                        .as_ref()
                        .is_some_and(|r| r.iter().any(|r| r.access == ResourceAccess::Write))
                {
                    Some(failed(
                        "PURE_CLAUSE_EFFECT",
                        "Pure assertion call emitted an event or wrote a resource",
                    ))
                } else {
                    let contract = if let Observation::Returned { value } = &observation {
                        match check_value(
                            ir,
                            &ir.operations[&invocation.subject].result,
                            value,
                            &program.budgets,
                        ) {
                            ValueContract::Accepted => None,
                            ValueContract::Rejected { reason } => {
                                Some(failed("NATIVE_RETURN_CONTRACT", reason))
                            }
                            ValueContract::Unknown { issue } => {
                                Some(VerificationResult::Unknown { issue })
                            }
                        }
                    } else {
                        None
                    };
                    let observed = InvocationObservation {
                        invocation,
                        observation,
                    };
                    record.observations.push(observed.clone());
                    self.tape.push(observed);
                    self.pending = None;
                    contract
                }
            }
            ApplicationObservation::BindingError { message } => {
                Some(unknown("APPLICATION_BINDING_ERROR", message))
            }
            _ => Some(failed(
                "APPLICATION_INPUT_CHANGED",
                "Assertion observation differs from its pending pure invocation",
            )),
        };
        if let Some(result) = result {
            ApplicationDirective::Verdict { result }
        } else {
            self.advance(ir, program, record)
        }
    }
}
