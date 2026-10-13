//! Reviewed Rust response projections; serde decodes representation, not expected behavior.
#![allow(dead_code, non_snake_case)]
use crate::contract::Value;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompilerCompilation {
    files: Vec<String>,
    lines: Vec<i64>,
    columns: Vec<i64>,
    accepted: bool,
    diagnostics: i64,
    symbols: i64,
    operations: i64,
}
#[derive(Deserialize)]
enum EvidenceStatus {
    Passed,
    Failed,
    Unknown,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceCaseRequirement {
    caseId: String,
    requirements: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceCaseOutcome {
    caseId: String,
    status: EvidenceStatus,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceCoverage {
    status: EvidenceStatus,
    requiredCases: Vec<String>,
    missingRequirements: Vec<String>,
    unexecutedCases: Vec<String>,
}
#[derive(Deserialize)]
enum TasksWorkState {
    Open,
    Progress,
    Done,
}
#[derive(Deserialize)]
enum TasksCheckState {
    Unverified,
    Passed,
    Failed,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TasksTaskState {
    taskId: i64,
    work: TasksWorkState,
    check: TasksCheckState,
    version: i64,
    claim: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TasksTransitionConflict {
    taskId: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TasksClaimRace {
    claims: i64,
    distinct: i64,
    rejected: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistorySnapshot {
    id: String,
    sourceHash: String,
    checkOccurrence: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryCheckReport {
    accepted: bool,
    diagnostics: i64,
    publishedSnapshot: Option<HistorySnapshot>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryMissingSnapshot {
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilesDocument {
    path: String,
    content: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilesSetup {
    created: Vec<String>,
    preserved: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilesInvalidRequest {
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilesAdaptation {
    oldTask: i64,
    newTask: i64,
    frozenHash: String,
    retainedHash: String,
    editUnchanged: bool,
    oldDone: bool,
    newOpen: bool,
    newUnverified: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRunSummary {
    runId: String,
    mandatoryCases: i64,
    recordedCases: i64,
    openObligations: i64,
    provenanceGaps: i64,
    status: EvidenceStatus,
    adoptedTasks: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeInvalidRequest {
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeReplaySummary {
    originalRun: String,
    originalCase: String,
    runId: String,
    status: EvidenceStatus,
    provenanceChanges: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeTaskCounts {
    done: i64,
    passed: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditorsDiagnostic {
    code: String,
    message: String,
    line: i64,
    column: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditorsSemanticToken {
    line: i64,
    column: i64,
    length: i64,
    tokenType: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditorsInvalidRequest {
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DistributionInvocationResult {
    exitCode: i64,
    stdout: String,
    stderr: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DistributionPackages {
    skillsExit: i64,
    repeatExit: i64,
    editorExit: i64,
    zedExit: i64,
    skillPreserved: bool,
    editorPreserved: bool,
    jsonValid: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteSnapshotView { sourceFiles:i64,tasks:i64,integerPins:bool,sourcePinned:bool,unverified:bool }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteExportView { deterministic:bool,selfContained:bool,preservesUnmanaged:bool,databaseUnchanged:bool }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteEmptyView { rejected:bool,databaseCreated:bool }
pub fn validate(subject: &str, value: Value) -> Result<(), String> {
    match subject {
        "Dctx0.ApiInt" => serde_json::from_value::<i64>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Compiler.Compilation" => serde_json::from_value::<CompilerCompilation>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Evidence.Status" => serde_json::from_value::<EvidenceStatus>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Evidence.CaseRequirement" => {
            serde_json::from_value::<EvidenceCaseRequirement>(value)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        "Dctx0.Evidence.CaseOutcome" => serde_json::from_value::<EvidenceCaseOutcome>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Evidence.Coverage" => serde_json::from_value::<EvidenceCoverage>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Tasks.WorkState" => serde_json::from_value::<TasksWorkState>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Tasks.CheckState" => serde_json::from_value::<TasksCheckState>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Tasks.TaskState" => serde_json::from_value::<TasksTaskState>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Tasks.TransitionConflict" => {
            serde_json::from_value::<TasksTransitionConflict>(value)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        "Dctx0.Tasks.ClaimRace" => serde_json::from_value::<TasksClaimRace>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.History.Snapshot" => serde_json::from_value::<HistorySnapshot>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.History.CheckReport" => serde_json::from_value::<HistoryCheckReport>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.History.MissingSnapshot" => serde_json::from_value::<HistoryMissingSnapshot>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Profiles.Document" => serde_json::from_value::<ProfilesDocument>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Profiles.Setup" => serde_json::from_value::<ProfilesSetup>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Profiles.InvalidRequest" => serde_json::from_value::<ProfilesInvalidRequest>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Profiles.Adaptation" => serde_json::from_value::<ProfilesAdaptation>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Native.RunSummary" => serde_json::from_value::<NativeRunSummary>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Native.InvalidRequest" => serde_json::from_value::<NativeInvalidRequest>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Native.ReplaySummary" => serde_json::from_value::<NativeReplaySummary>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Native.TaskCounts" => serde_json::from_value::<NativeTaskCounts>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Editors.Diagnostic" => serde_json::from_value::<EditorsDiagnostic>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Editors.SemanticToken" => serde_json::from_value::<EditorsSemanticToken>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Editors.InvalidRequest" => serde_json::from_value::<EditorsInvalidRequest>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Distribution.InvocationResult" => {
            serde_json::from_value::<DistributionInvocationResult>(value)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        "Dctx0.Distribution.Packages" => serde_json::from_value::<DistributionPackages>(value)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "Dctx0.Site.SnapshotView" => serde_json::from_value::<SiteSnapshotView>(value).map(|_| ()).map_err(|e|e.to_string()),
        "Dctx0.Site.ExportView" => serde_json::from_value::<SiteExportView>(value).map(|_| ()).map_err(|e|e.to_string()),
        "Dctx0.Site.EmptyView" => serde_json::from_value::<SiteEmptyView>(value).map(|_| ()).map_err(|e|e.to_string()),
        _ => Err(format!("Unbound projection type: {subject}")),
    }
}
