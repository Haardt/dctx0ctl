use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PLANNER_VERSION: u32 = 2;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Task planning: {0}")]
    Invalid(String),
    #[error("Task not found: T{0}")]
    Missing(i64),
    #[error("Conflict: {0}")]
    Conflict(String),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskKind {
    Contract,
    Operation,
    Adapter,
    Integration,
    Review,
    Retire,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkState {
    Open,
    Progress,
    Done,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Unverified,
    Pass,
    Fail,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Condition {
    Delivered,
    CheckPass,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prerequisite {
    pub key: String,
    pub condition: Condition,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Explicit responsibility for manual requirements and inspection suites.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_obligations: Vec<String>,
    pub key: String,
    pub kind: TaskKind,
    pub subjects: Vec<String>,
    pub title: String,
    pub contract: String,
    pub facts: Vec<String>,
    pub types: Vec<String>,
    pub requirements_hash: String,
    pub work_order_hash: String,
    pub prerequisites: Vec<Prerequisite>,
    pub wave: usize,
    pub acceptance: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatePlan {
    pub snapshot_id: String,
    pub tasks: Vec<Candidate>,
    pub obligations: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub task_id: i64,
    pub candidate: Candidate,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: i64,
    pub parent: Option<i64>,
    pub snapshot_id: String,
    pub check_run_id: i64,
    pub planner_version: u32,
    pub hash: String,
    pub members: Vec<Member>,
    pub obligations: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adjustment: Option<AdjustmentReason>,
    pub id: i64,
    pub candidate: Candidate,
    pub origin_snapshot: String,
    pub origin_check: i64,
    pub reason: String,
    pub before_snapshot: Option<String>,
    pub changes: Vec<RequirementChange>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskResult {
    pub summary: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub implementation: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckEvidence {
    pub requirements_hash: String,
    pub implementation: String,
    pub verifier: String,
    pub verifier_version: String,
    pub command: String,
    pub started_at: String,
    pub finished_at: String,
    pub report: String,
    pub status: CheckStatus,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub severity: String,
    pub description: String,
    pub task_ids: Vec<i64>,
    pub action_required: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewReport {
    pub reviewer: String,
    pub judgement: String,
    pub summary: String,
    pub findings: Vec<Finding>,
    /// Optional source-bound conformance evidence; legacy reviews remain readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conformance: Option<ConformanceReview>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewedRequirement {
    pub report: String,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConformanceReview {
    pub snapshot: String,
    pub requirements_hash: String,
    pub spec_source_hash: String,
    pub implementation_artifacts: std::collections::BTreeMap<String, String>,
    pub subjects: std::collections::BTreeMap<String, ReviewedRequirement>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_bundle: Option<dctx0_core::ExecutionBundle>,
    pub work: WorkState,
    pub version: i64,
    pub actor: Option<String>,
    pub claim: Option<i64>,
    pub reservation: Option<i64>,
    pub result: Option<TaskResult>,
    pub check: CheckStatus,
    pub evidence: Vec<CheckEvidence>,
    pub reviews: Vec<ReviewReport>,
    pub ever_started: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            execution_bundle: None,
            work: WorkState::Open,
            version: 0,
            actor: None,
            claim: None,
            reservation: None,
            result: None,
            check: CheckStatus::Unverified,
            evidence: vec![],
            reviews: vec![],
            ever_started: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seq: i64,
    pub task_id: Option<i64>,
    pub kind: String,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryLink {
    pub from: i64,
    pub to: i64,
    pub kind: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub project: String,
    pub plan: Plan,
    pub event_seq: i64,
    pub states: BTreeMap<i64, State>,
    pub definitions: BTreeMap<i64, Definition>,
    pub links: Vec<HistoryLink>,
    pub events: Vec<Event>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filters {
    pub wave: Option<usize>,
    pub state: Option<WorkState>,
    pub check: Option<CheckStatus>,
    pub kind: Option<TaskKind>,
    pub subject: Option<String>,
    pub domain: Option<String>,
    pub actor: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Overview {
    pub task_id: i64,
    pub title: String,
    pub kind: TaskKind,
    pub subjects: Vec<String>,
    pub wave: usize,
    pub work: WorkState,
    pub check: CheckStatus,
    pub version: i64,
    pub actor: Option<String>,
    pub claim: Option<i64>,
    pub reservation: Option<i64>,
    pub ready: bool,
    pub delivered: bool,
    pub blockers: Vec<String>,
    pub result_summary: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyView {
    pub task_id: i64,
    pub prerequisite_id: i64,
    pub condition: Condition,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slice {
    pub anchor: i64,
    pub tasks: Vec<Overview>,
    pub dependencies: Vec<DependencyView>,
    pub open_work: Vec<i64>,
    pub boundary: Vec<i64>,
    pub blockers: Vec<String>,
    pub closed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub task_id: Option<i64>,
    pub group_id: Option<i64>,
    pub state: Option<State>,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDiff {
    pub before: i64,
    pub after: i64,
    pub added: Vec<i64>,
    pub kept: Vec<i64>,
    pub removed: Vec<i64>,
    pub links: Vec<HistoryLink>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor {
    pub plan_id: i64,
    pub event_seq: i64,
    pub query_hash: String,
    pub offset: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub schema_version: u32,
    pub plan_id: i64,
    pub snapshot_id: String,
    pub event_seq: i64,
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
    pub complete: bool,
}
#[derive(Debug, Clone)]
pub enum Action {
    Start {
        task: i64,
        actor: String,
        reservation: Option<i64>,
    },
    ClaimNext {
        actor: String,
        filters: Filters,
    },
    Release {
        task: i64,
        actor: String,
        claim: i64,
        version: i64,
        reason: String,
    },
    Complete {
        task: i64,
        actor: String,
        claim: i64,
        version: i64,
        result: TaskResult,
    },
    ClaimSlice {
        anchor: i64,
        actor: String,
        domain: Option<String>,
    },
    ReleaseSlice {
        group: i64,
        actor: String,
        reason: String,
    },
    RecordCheck {
        task: i64,
        evidence: CheckEvidence,
    },
    RecordReview {
        task: i64,
        actor: String,
        claim: i64,
        version: i64,
        report: ReviewReport,
    },
    Note {
        task: i64,
        note: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementChange {
    pub subject: String,
    pub facet: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl TaskKind {
    pub fn acceptance(self) -> Vec<String> {
        let items = match self {
            Self::Contract => [
                "Publish the exact parameters, fields, variants, failures, keys and refinements.",
                "Document representation and artifacts; test constraints and failure boundaries.",
            ],
            Self::Operation => [
                "Implement this surface operation's modeled input, result, failure, effect and state behavior.",
                "Cover supplied examples, pre/postconditions and applicable invariants; document artifacts and decisions.",
            ],
            Self::Adapter => [
                "Implement only this port operation behind its declared application contract.",
                "Preserve modeled state/effects and failures; test interactions and unavailable dependencies.",
            ],
            Self::Integration => [
                "Compose delivered operations, adapters and required contracts; preserve ownership and module boundaries.",
                "Cover supplied cross-operation examples, invariants and lifecycles; document wiring and external setup.",
            ],
            Self::Review => [
                "Inspect supplied implementation subjects against the stated criteria and answer the review questions.",
                "Deliver a structured reviewer/judgement/findings report with affected task IDs; testing evidence stays separate.",
            ],
            Self::Retire => [
                "Remove obsolete implementation only after adapting or removing its former consumers.",
                "Preserve unrelated behavior and document removed artifacts and replacement boundaries.",
            ],
        };
        items.into_iter().map(str::to_owned).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AdjustmentReason {
    ReviewRenewal {
        source: i64,
        source_hash: String,
    },
    FailedCheck {
        source: i64,
    },
    ReviewFinding {
        source: i64,
        finding: String,
    },
    Profile {
        source: i64,
        desired: dctx0_core::ExecutionBundle,
    },
}
impl Definition {
    /// Old schema-2 follow-ups retain their historical textual encoding.
    pub fn is_adjustment(&self) -> bool {
        self.adjustment.is_some() || self.reason.starts_with("Follow-up:")
    }
}
