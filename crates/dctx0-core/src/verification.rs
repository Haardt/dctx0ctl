//! Language-neutral testing wire models; independent of profiles and targets.
use crate::{Span, StatePhase};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const VERIFICATION_SCHEMA_VERSION: u32 = 1;
pub const VERIFICATION_PROTOCOL_VERSION: u32 = 1;

/// Explicit human/agent inspection, distinct from automatic execution evidence.
/// Consumers must validate bindings before applying its result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationAttestation {
    pub schema_version: u32,
    pub actor: String,
    pub subject: String,
    pub snapshot: String,
    pub plan_hash: String,
    pub requirements: BTreeSet<String>,
    pub implementation_artifacts: BTreeMap<String, String>,
    pub report: String,
    pub limitations: Vec<String>,
    pub result: VerificationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationEvidenceBasis {
    Automatic,
    Agent,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttestedVerificationResult {
    pub automatic: VerificationResult,
    pub effective: VerificationResult,
    pub basis: VerificationEvidenceBasis,
}

/// Numbers cross process boundaries as exact text, never JSON floating point.
/// Decimal text is a finite base-10 literal; validation belongs to the interpreter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum VerificationValue {
    Unit,
    Boolean(bool),
    Integer(String),
    Decimal(String),
    /// Finite IEEE-754 value encoded as text; never confused with exact Decimal.
    Float(String),
    String(String),
    Record(BTreeMap<String, VerificationValue>),
    List(Vec<VerificationValue>),
    Option(Option<Box<VerificationValue>>),
    Variant {
        declaration: String,
        name: String,
        payload: Option<Box<VerificationValue>>,
    },
    Failure {
        declaration: String,
        payload: Option<Box<VerificationValue>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invocation {
    pub subject: String,
    pub arguments: Vec<VerificationValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Observation {
    Returned { value: VerificationValue },
    Threw { message: String },
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationObservation {
    pub invocation: Invocation,
    pub observation: Observation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationEvent {
    pub subject: String,
    pub payload: VerificationValue,
}

/// Actual access to a declared resource, supplied by an adapter observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceAccess {
    Read,
    Write,
    Observe,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    pub resource: String,
    pub access: ResourceAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationIssue {
    pub code: String,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "UPPERCASE")]
pub enum VerificationResult {
    Pass,
    Fail { issue: VerificationIssue },
    Unknown { issue: VerificationIssue },
}

/// Execution limits are distinct from bounded generated-corpus selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationBudgets {
    pub seed: u64,
    pub generated_cases: usize,
    pub collection_size: usize,
    pub recursion_depth: usize,
    pub value_bytes: usize,
    pub shrink_steps: usize,
    pub evaluation_steps: usize,
}
impl Default for VerificationBudgets {
    fn default() -> Self {
        Self {
            seed: 42,
            generated_cases: 100,
            collection_size: 4,
            recursion_depth: 8,
            value_bytes: 64 * 1024,
            shrink_steps: 100,
            evaluation_steps: 100_000,
        }
    }
}

/// Immutable native-run reporting models. These are not task transition authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationArtifactRef {
    pub hash: String,
    pub media_type: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationCaseKind {
    Contract,
    EntityIdentity,
    Operation,
    Example,
    Assertion,
    Lifecycle,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationStateEvidence {
    pub phase: StatePhase,
    /// Missing in older reports whose phase labels were inferred from order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_recorded: Option<bool>,
    pub entities: BTreeMap<String, Vec<VerificationValue>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCaseRecord {
    pub id: String,
    pub subject: String,
    pub kind: VerificationCaseKind,
    pub bindings: BTreeMap<String, VerificationValue>,
    pub setup: Vec<Invocation>,
    pub arguments: Vec<Vec<VerificationValue>>,
    pub observations: Vec<InvocationObservation>,
    pub events: Option<Vec<VerificationEvent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<ResourceObservation>>,
    pub states: Vec<VerificationStateEvidence>,
    pub result: VerificationResult,
    pub elapsed_us: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationRunOrigin {
    Native,
    Imported,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationTaskPin {
    pub plan_id: i64,
    pub event_sequence: i64,
    pub requirements: BTreeMap<i64, String>,
    pub profile_bundles: BTreeMap<i64, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationProvenance {
    pub snapshot: String,
    pub check_occurrence: i64,
    pub plan_hash: String,
    pub implementation: BTreeMap<String, String>,
    pub dependencies: BTreeMap<String, String>,
    pub adapter_hash: String,
    pub configuration_hash: String,
    pub harness_hash: String,
    pub interpreter_version: String,
    pub generator_version: String,
    pub protocol_version: u32,
    pub target: String,
    pub runtime_version: String,
    pub build_configuration: BTreeMap<String, String>,
    pub task_pin: Option<VerificationTaskPin>,
    /// Unknown/unreliably captured inputs prevent complete adoption evidence.
    pub incomplete: Vec<VerificationIssue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRun {
    pub schema_version: u32,
    pub origin: VerificationRunOrigin,
    /// Lossless UTC Unix nanoseconds distinguish actual execution occurrences.
    pub started_at_unix_ns: String,
    pub elapsed_us: u64,
    pub provenance: VerificationProvenance,
    pub budgets: VerificationBudgets,
    pub selected_subjects: std::collections::BTreeSet<String>,
    pub required_subjects: std::collections::BTreeSet<String>,
    pub not_selected: Vec<String>,
    pub required_cases: std::collections::BTreeSet<String>,
    pub cases: Vec<VerificationCaseRecord>,
    pub open_obligations: Vec<VerificationIssue>,
    pub bounded_obligations: Vec<String>,
    pub artifacts: Vec<VerificationArtifactRef>,
    pub result: VerificationResult,
}
impl VerificationRun {
    /// Necessary global gate for task-specific delivery, never a task proof.
    /// Manual reviews remain open; native cases and per-task coverage must
    /// independently be checked by the publisher.
    pub fn permits_task_specific_proof(&self) -> bool {
        let manual = |issue: &VerificationIssue| {
            matches!(
                issue.code.as_str(),
                "MANUAL_REQUIREMENT" | "SUITE_INSPECTION_REQUIRED"
            )
        };
        match &self.result {
            VerificationResult::Pass => self.open_obligations.is_empty(),
            VerificationResult::Unknown { issue } => {
                manual(issue)
                    && self.open_obligations.contains(issue)
                    && self.open_obligations.iter().all(manual)
            }
            VerificationResult::Fail { .. } => false,
        }
    }
}

/// Immutable mandatory case inventory and the requirements each case exercises.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCaseManifest {
    pub schema_version: u32,
    pub required_cases: BTreeSet<String>,
    pub case_requirements: BTreeMap<String, BTreeSet<String>>,
    pub open_obligations: Vec<VerificationIssue>,
    pub bounded_obligations: Vec<String>,
}
impl VerificationCaseManifest {
    pub fn register(&mut self, id: String) {
        if !self.required_cases.insert(id.clone()) {
            self.open_obligations.push(VerificationIssue {
                code: "DUPLICATE_CASE_ID".into(),
                message: id,
                span: None,
            });
        }
    }
}
