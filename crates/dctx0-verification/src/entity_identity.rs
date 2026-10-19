//! Entity-key evidence derived centrally from actual operation observations.
//! This bounded check proves observed identity, not a universal database theorem.
use crate::ProgramPlan;
use dctx0_core::*;
use dctx0_interpreter::{ValueContract, check_value};
use std::collections::{BTreeMap, BTreeSet};

fn unknown(code: &str, subject: &str) -> VerificationResult {
    VerificationResult::Unknown {
        issue: VerificationIssue {
            code: code.into(),
            message: subject.into(),
            span: None,
        },
    }
}

pub fn entity_identity_case(
    ir: &WorkspaceIr,
    subject: &str,
    cases: &[VerificationCaseRecord],
    budgets: &VerificationBudgets,
) -> VerificationCaseRecord {
    let sources = cases
        .iter()
        .filter(|case| {
            case.kind == VerificationCaseKind::Operation
                && case
                    .states
                    .iter()
                    .any(|state| state.entities.contains_key(subject))
        })
        .collect::<Vec<_>>();
    let states = sources
        .iter()
        .flat_map(|case| &case.states)
        .filter_map(|state| {
            state
                .entities
                .get(subject)
                .map(|rows| VerificationStateEvidence {
                    phase: state.phase,
                    phase_recorded: state.phase_recorded,
                    entities: [(subject.into(), rows.clone())].into(),
                })
        })
        .collect::<Vec<_>>();
    let mut record = VerificationCaseRecord {
        id: format!("{subject}:identity"),
        subject: subject.into(),
        kind: VerificationCaseKind::EntityIdentity,
        bindings: [(
            "source_cases".into(),
            VerificationValue::List(
                sources
                    .iter()
                    .map(|case| VerificationValue::String(case.id.clone().into()))
                    .collect(),
            ),
        )]
        .into(),
        setup: vec![],
        arguments: vec![],
        observations: vec![],
        events: None,
        resources: None,
        states,
        result: unknown("ENTITY_IDENTITY_OBSERVATION_REQUIRED", subject),
        elapsed_us: 0,
    };
    let Some(data) = ir.data.get(subject).filter(|data| !data.keys.is_empty()) else {
        record.result = unknown("ENTITY_KEY_DECLARATION_REQUIRED", subject);
        return record;
    };
    let ty = TypeRef {
        name: subject.into(),
        symbol_id: Some(subject.into()),
        arguments: vec![],
    };
    let mut populated = false;
    let mut incomplete = sources.iter().any(|case| {
        case.result != VerificationResult::Pass
            || !case
                .observations
                .iter()
                .any(|observed| observed.invocation.subject == case.subject)
            || ![StatePhase::Before, StatePhase::After].iter().all(|phase| {
                case.states
                    .iter()
                    .any(|state| state.phase == *phase && state.entities.contains_key(subject))
            })
    });
    for state in &record.states {
        incomplete |= state.phase_recorded != Some(true);
        let mut identities = Vec::new();
        for row in &state.entities[subject] {
            populated = true;
            match check_value(ir, &ty, row, budgets) {
                ValueContract::Rejected { .. } => {
                    record.result = VerificationResult::Fail {
                        issue: VerificationIssue {
                            code: "INVALID_ENTITY_IDENTITY_ROW".into(),
                            message: subject.into(),
                            span: None,
                        },
                    };
                    return record;
                }
                ValueContract::Unknown { .. } => {
                    incomplete = true;
                    continue;
                }
                ValueContract::Accepted => {}
            }
            let VerificationValue::Record(fields) = row else {
                incomplete = true;
                continue;
            };
            let Some(identity) = data
                .keys
                .iter()
                .map(|key| fields.get(key).cloned())
                .collect::<Option<Vec<_>>>()
            else {
                incomplete = true;
                continue;
            };
            if identities.contains(&identity) {
                record.result = VerificationResult::Fail {
                    issue: VerificationIssue {
                        code: "DUPLICATE_ENTITY_IDENTITY".into(),
                        message: subject.into(),
                        span: None,
                    },
                };
                return record;
            }
            identities.push(identity);
        }
    }
    record.result = if !populated {
        unknown("ENTITY_IDENTITY_OBSERVATION_REQUIRED", subject)
    } else if incomplete {
        unknown("ENTITY_IDENTITY_SOURCE_INCOMPLETE", subject)
    } else {
        VerificationResult::Pass
    };
    record
}

/// Reconstruct the proof from source cases; copied or forged identity summaries
/// cannot replace their actual recorded operation observations.
pub fn validate_entity_identity_cases(
    ir: &WorkspaceIr,
    plan: &ProgramPlan,
    cases: &[VerificationCaseRecord],
) -> bool {
    let expected = plan
        .contracts
        .iter()
        .filter(|contract| !contract.identity_requirements.is_empty())
        .map(|contract| {
            let record = entity_identity_case(ir, &contract.subject, cases, &plan.budgets);
            (record.id.clone(), record)
        })
        .collect::<BTreeMap<_, _>>();
    let actual = cases
        .iter()
        .filter(|case| case.kind == VerificationCaseKind::EntityIdentity)
        .collect::<Vec<_>>();
    actual.len() == expected.len()
        && actual
            .iter()
            .map(|case| &case.id)
            .collect::<BTreeSet<_>>()
            .len()
            == actual.len()
        && actual
            .iter()
            .all(|case| expected.get(&case.id) == Some(*case))
}
