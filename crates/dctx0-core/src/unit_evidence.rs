//! Versioned technical evidence, independent of task/publication identities.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const UNIT_EVIDENCE_VERSION: u32 = 1;
pub const UNIT_BINDING_MEDIA_TYPE: &str = "application/vnd.dctx0.unit-binding+json";
pub const UNIT_COMPLETION_MEDIA_TYPE: &str = "application/vnd.dctx0.unit-completion+json";

/// A direct compiler-checked decoder root, not an executable source expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDecoderBinding {
    pub subject: String,
    pub import_path: String,
    pub function: String,
}

/// A proven execution boundary. Unknown targets retain their complete cohort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UnitDependencyBoundary {
    Complete {
        analyzer: String,
        version: String,
        roots: BTreeMap<String, BTreeSet<String>>,
        discovery: BTreeMap<String, String>,
    },
    Unknown {
        group: String,
        reasons: BTreeSet<String>,
    },
}

/// An atomic technical unit; sessions and shared fixtures must be merged first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationUnit {
    pub id: String,
    pub cases: BTreeSet<String>,
    pub boundary: UnitDependencyBoundary,
    pub execute_always: bool,
}

/// Canonical identity. Every digest names semantic content, never a task or PID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitEvidenceBindingV1 {
    pub schema_version: u32,
    pub unit: VerificationUnit,
    pub logical_cases: BTreeMap<String, String>,
    pub requirements: BTreeMap<String, String>,
    pub dependencies: BTreeMap<String, String>,
    pub fixtures: BTreeMap<String, String>,
    pub build_inputs: BTreeMap<String, String>,
    pub environment: BTreeMap<String, Option<String>>,
    pub semantics: BTreeMap<String, String>,
}

/// Supplementary immutable proof; the original native run is never rewritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitCompletionProofV1 {
    pub schema_version: u32,
    pub native_run: String,
    pub unit: String,
    pub fingerprint: String,
    pub generation: u64,
    pub completed: bool,
    pub cleanup_completed: bool,
    pub dependencies_complete: bool,
    pub case_bodies: BTreeMap<String, String>,
    pub artifacts: BTreeSet<String>,
}

/// Written by the native orchestrator before the run is content-addressed.
/// Keeping the run ID out avoids a circular hash. Migration needs separate,
/// source-validated supplementary evidence; it cannot invent this receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitNativeCompletionV1 {
    pub schema_version: u32,
    pub unit: String,
    pub fingerprint: String,
    pub generation: u64,
    pub completed: bool,
    pub cleanup_completed: bool,
    pub dependencies_complete: bool,
    pub case_bodies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitAttemptOutcome {
    Started,
    Pass,
    Fail,
    Unknown,
}

/// Generation is allocated durably before starting any native process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitAttemptV1 {
    pub unit: String,
    pub fingerprint: String,
    pub generation: u64,
    pub outcome: UnitAttemptOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitExecutionAction {
    Reuse,
    ExecuteNew,
    ExecuteChanged,
    ExecuteUnbound,
    ExecuteAlways,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitPlanReason {
    pub code: String,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitExecutionDecision {
    pub unit: String,
    pub cases: BTreeSet<String>,
    pub fingerprint: String,
    pub action: UnitExecutionAction,
    pub source: Option<UnitCompletionProofV1>,
    pub reasons: Vec<UnitPlanReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitExecutionPlan {
    pub schema_version: u32,
    pub required_cases: BTreeSet<String>,
    pub decisions: Vec<UnitExecutionDecision>,
    pub removed_cases: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitCoverage {
    ReusedPass,
    FreshPass,
    Fail,
    Unknown,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComposedUnitEvidenceV1 {
    pub binding: UnitEvidenceBindingV1,
    pub coverage: UnitCoverage,
    pub proof: Option<UnitCompletionProofV1>,
}

/// A composition has multiple explicit native sources and no invented runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCompositionV1 {
    pub schema_version: u32,
    pub required_cases: BTreeSet<String>,
    pub units: Vec<ComposedUnitEvidenceV1>,
    pub removed_cases: BTreeSet<String>,
}
