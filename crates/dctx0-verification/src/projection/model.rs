//! Execution planning contracts. Stored plans are not execution/adoption evidence.
use crate::{InputCorpus, ValueCorpus, VerificationCase, VerificationScope};
use dctx0_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    Values,
    PureCalls,
    Operations,
    Mocks,
    Events,
    CallObservation,
    ResourceObservation,
    State,
    Lifecycles,
    Inspection,
    Review,
}
impl Capability {
    /// Required adapter capability, separate from human inspection/review work.
    pub fn adapter_capability(&self) -> Option<&'static str> {
        match self {
            Self::Values => Some("values"),
            Self::PureCalls | Self::Operations => Some("invocation"),
            Self::Mocks => Some("mock-injection"),
            Self::Events => Some("events"),
            Self::CallObservation => Some("call-observation"),
            Self::ResourceObservation => Some("resource-observation"),
            Self::State => Some("state"),
            Self::Lifecycles => Some("sessions"),
            Self::Inspection | Self::Review => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractPlan {
    pub subject: String,
    pub ty: TypeRef,
    /// Modeled refinements actually evaluated by the shared value checker.
    /// Historical plans without this field do not gain invented requirement IDs.
    #[serde(default)]
    pub refinement_requirements: Vec<String>,
    /// Record field declarations enforced by the central type checker for
    /// every accepted/rejected factory case. Entity keys are separate evidence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structural_requirements: Vec<String>,
    /// Declared keys require actual populated persistence observations, separately
    /// from factory acceptance and record-field validation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identity_requirements: Vec<String>,
    pub corpus: ValueCorpus,
    #[serde(default = "crate::missing_rejections")]
    pub rejection_corpus: ValueCorpus,
    #[serde(default)]
    pub rejection_generator_version: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationPlan {
    pub operation: OperationIr,
    /// Source requirements checked by the native pure-operation boundary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub purity_requirements: Vec<String>,
    pub inputs: InputCorpus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precondition_selection: Option<crate::PreconditionSelection>,
    pub clauses: Vec<SemanticNode>,
    #[serde(default)]
    pub cases: Vec<crate::OperationCaseInput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generated_preparations: Vec<GeneratedPreparation>,
}
/// Explicit reviewed preparation for one exact generated binding. No application
/// result is supplied: the native operation and Rust assessment remain mandatory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedPreparation {
    pub bindings: BTreeMap<String, VerificationValue>,
    pub setup: Vec<dctx0_core::Invocation>,
}
impl OperationPlan {
    pub fn generated_setup(
        &self,
        bindings: &BTreeMap<String, VerificationValue>,
    ) -> Vec<dctx0_core::Invocation> {
        self.generated_preparations
            .iter()
            .find(|entry| &entry.bindings == bindings)
            .map(|entry| entry.setup.clone())
            .unwrap_or_default()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionPlan {
    pub source_id: String,
    pub cases: Vec<VerificationCase>,
    pub issues: Vec<VerificationIssue>,
    pub bounded_corpus: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyPlan {
    pub subject: String,
    pub quantifiers: Vec<FieldIr>,
    pub assertions: Vec<AssertionPlan>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum MockOutcome {
    Returned { expression: Expression },
    Failed { expression: Expression },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockStep {
    pub source_id: String,
    pub invocation: Expression,
    pub outcome: Option<MockOutcome>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationStep {
    pub source_id: String,
    pub binding: String,
    pub invocation: Expression,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Expectation {
    Success {
        source_id: String,
    },
    Failure {
        source_id: String,
        declaration: Option<String>,
        payload: Option<Expression>,
    },
    Emitted {
        source_id: String,
        declaration: String,
        payload: Option<Expression>,
    },
    Assertion {
        source_id: String,
        expression: Expression,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExampleAction {
    PrepareMock(usize),
    Invoke(usize),
    Check(usize),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExamplePlan {
    pub subject: String,
    pub order: Vec<ExampleAction>,
    pub variables: Vec<FieldIr>,
    pub bindings: BTreeMap<String, VerificationValue>,
    pub mocks: Vec<MockStep>,
    pub invocations: Vec<InvocationStep>,
    pub expectations: Vec<Expectation>,
    pub issues: Vec<VerificationIssue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleState {
    pub name: String,
    pub initial: bool,
    pub terminal: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleTransition {
    pub source_id: String,
    pub operation: String,
    pub from: String,
    pub to: String,
    pub guard: Option<Expression>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecyclePlan {
    pub subject: String,
    pub entity: String,
    pub field: String,
    pub states: Vec<LifecycleState>,
    pub transitions: Vec<LifecycleTransition>,
    pub paths: Vec<LifecyclePath>,
    pub issues: Vec<VerificationIssue>,
}
/// Each path starts a fresh session. Indices refer to the pinned transition list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecyclePath {
    pub id: String,
    pub initial: String,
    pub transitions: Vec<usize>,
    pub final_state: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCheck {
    pub subject: String,
    pub kind: StatementKind,
    pub nodes: Vec<SemanticNode>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuitePlan {
    pub declaration: Symbol,
    pub nodes: Vec<SemanticNode>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramPlan {
    pub schema_version: u32,
    pub snapshot: String,
    pub check_occurrence: u64,
    pub scope: VerificationScope,
    pub closure: BTreeSet<String>,
    pub not_selected: Vec<String>,
    pub budgets: VerificationBudgets,
    pub generator_version: String,
    pub contracts: Vec<ContractPlan>,
    pub operations: Vec<OperationPlan>,
    pub properties: Vec<PropertyPlan>,
    pub examples: Vec<ExamplePlan>,
    pub lifecycles: Vec<LifecyclePlan>,
    pub manual_checks: Vec<ManualCheck>,
    pub suites: Vec<SuitePlan>,
    pub policies: Vec<ManualCheck>,
    pub capabilities: Vec<Capability>,
    pub issues: Vec<VerificationIssue>,
}
/// Reviewed sample data is required for symbolic examples. Arbitrary generated
/// values must not turn an existential example into a universal assertion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionFixtures {
    pub examples: BTreeMap<String, BTreeMap<String, VerificationValue>>,
    #[serde(default)]
    pub operations: BTreeMap<String, Vec<crate::OperationCaseInput>>,
    #[serde(default)]
    pub generated_preparations: BTreeMap<String, Vec<GeneratedPreparation>>,
}
