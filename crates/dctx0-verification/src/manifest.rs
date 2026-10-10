//! Mandatory case inventory shared by execution, replay and task evidence mapping.
use crate::{OperationPlan, ProgramPlan};
use dctx0_core::{StatementKind, VerificationIssue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseManifest {
    pub schema_version: u32,
    pub required_cases: BTreeSet<String>,
    /// Requirements exercised by a case; only actual execution can discharge them.
    pub case_requirements: BTreeMap<String, BTreeSet<String>>,
    pub open_obligations: Vec<VerificationIssue>,
    pub bounded_obligations: Vec<String>,
}
impl CaseManifest {
    fn register(&mut self, id: String) {
        if !self.required_cases.insert(id.clone()) {
            self.open_obligations.push(issue("DUPLICATE_CASE_ID", id));
        }
    }
}
fn issue(code: &str, message: impl Into<String>) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: None,
    }
}
/// Pure projection. No worker result or target controls mandatory case selection.
pub fn case_manifest(plan: &ProgramPlan) -> CaseManifest {
    let mut manifest = CaseManifest {
        schema_version: 1,
        required_cases: BTreeSet::new(),
        case_requirements: BTreeMap::new(),
        open_obligations: vec![],
        bounded_obligations: vec![],
    };
    for contract in &plan.contracts {
        manifest
            .open_obligations
            .extend(contract.corpus.issues.clone());
        manifest
            .open_obligations
            .extend(contract.rejection_corpus.issues.clone());
        if contract.rejection_generator_version != crate::REJECTION_GENERATOR_VERSION {
            manifest
                .open_obligations
                .push(issue("REJECTION_GENERATOR_VERSION", &contract.subject));
        }
        if !contract.corpus.exhaustive {
            manifest.bounded_obligations.push(contract.subject.clone());
        }
        for index in 0..contract.corpus.values.len() {
            manifest.register(format!("{}:value:{index}", contract.subject));
        }
        for index in 0..contract.rejection_corpus.values.len() {
            manifest.register(format!("{}:rejection:{index}", contract.subject));
        }
        if contract.corpus.values.is_empty() {
            manifest.open_obligations.push(issue(
                "EMPTY_CONTRACT_CORPUS",
                format!("{} has no valid cases", contract.subject),
            ));
        }
    }
    for operation in &plan.operations {
        manifest
            .open_obligations
            .extend(operation.inputs.issues.clone());
        for case in &operation.cases {
            manifest.register(case.id.clone());
        }
        if supports_generated_operation(operation) {
            for index in 0..operation.inputs.bindings.len() {
                manifest.register(format!("{}:operation:{index}", operation.operation.id));
            }
            if !operation.inputs.exhaustive {
                manifest
                    .bounded_obligations
                    .push(operation.operation.id.clone());
            }
            if operation.inputs.bindings.is_empty() {
                manifest
                    .open_obligations
                    .push(issue("EMPTY_OPERATION_CORPUS", &operation.operation.id));
            }
        } else {
            manifest.register(format!("{}:operation", operation.operation.id));
            manifest.open_obligations.push(issue("OPERATION_CLOSURE_INCOMPLETE", format!("{} needs complete generated pre/postcondition and effect assessment; example coverage alone is not a full operation proof", operation.operation.id)));
        }
        if !operation.cases.is_empty() {
            manifest
                .bounded_obligations
                .push(format!("{}:reviewed-cases", operation.operation.id));
        }
    }
    for property in &plan.properties {
        for assertion in &property.assertions {
            manifest.open_obligations.extend(assertion.issues.clone());
            if assertion.bounded_corpus {
                manifest
                    .bounded_obligations
                    .push(assertion.source_id.clone());
            }
            for case in &assertion.cases {
                manifest.register(case.id.clone());
            }
            if assertion.cases.is_empty() {
                manifest
                    .open_obligations
                    .push(issue("EMPTY_ASSERTION_CORPUS", &assertion.source_id));
            }
        }
    }
    for example in &plan.examples {
        manifest.register(example.subject.clone());
        manifest.open_obligations.extend(example.issues.clone());
    }
    for lifecycle in &plan.lifecycles {
        manifest.open_obligations.extend(lifecycle.issues.clone());
        for path in &lifecycle.paths {
            manifest.register(path.id.clone());
        }
        if lifecycle.paths.is_empty() {
            manifest
                .open_obligations
                .push(issue("EMPTY_LIFECYCLE_PATHS", &lifecycle.subject));
        }
    }
    for manual in plan.manual_checks.iter().chain(&plan.policies) {
        manifest.open_obligations.push(issue(
            "MANUAL_REQUIREMENT",
            format!(
                "{}: {:?} has no automatically asserted meaning",
                manual.subject, manual.kind
            ),
        ));
    }
    for suite in &plan.suites {
        manifest
            .open_obligations
            .push(issue("SUITE_INSPECTION_REQUIRED", &suite.declaration.id));
    }
    for contract in &plan.contracts {
        let mut requirements = [contract.subject.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        requirements.extend(contract.refinement_requirements.clone());
        for (kind, size) in [
            ("value", contract.corpus.values.len()),
            ("rejection", contract.rejection_corpus.values.len()),
        ] {
            for index in 0..size {
                manifest.case_requirements.insert(
                    format!("{}:{kind}:{index}", contract.subject),
                    requirements.clone(),
                );
            }
        }
    }
    for operation in &plan.operations {
        let mut requirements = [operation.operation.id.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        requirements.extend(operation.clauses.iter().map(|n| n.id.clone()));
        for case in &operation.cases {
            manifest
                .case_requirements
                .insert(case.id.clone(), requirements.clone());
        }
        if supports_generated_operation(operation) {
            for index in 0..operation.inputs.bindings.len() {
                manifest.case_requirements.insert(
                    format!("{}:operation:{index}", operation.operation.id),
                    requirements.clone(),
                );
            }
        } else {
            manifest.case_requirements.insert(
                format!("{}:operation", operation.operation.id),
                requirements,
            );
        }
    }
    for property in &plan.properties {
        for assertion in &property.assertions {
            for case in &assertion.cases {
                manifest.case_requirements.insert(
                    case.id.clone(),
                    [property.subject.clone(), assertion.source_id.clone()].into(),
                );
            }
        }
    }
    for example in &plan.examples {
        manifest
            .case_requirements
            .insert(example.subject.clone(), [example.subject.clone()].into());
    }
    for lifecycle in &plan.lifecycles {
        for path in &lifecycle.paths {
            manifest
                .case_requirements
                .insert(path.id.clone(), [lifecycle.subject.clone()].into());
        }
    }
    manifest
}
pub fn supports_generated_operation(operation: &OperationPlan) -> bool {
    operation.operation.pure
        && operation
            .clauses
            .iter()
            .all(|node| matches!(node.kind, StatementKind::Requires | StatementKind::Ensures))
        && operation.operation.failures.is_empty()
}
