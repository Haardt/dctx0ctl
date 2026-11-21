use crate::application_plan::TestForm;
use dctx0_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Pinned by the engine, never selected by a test's observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationValueBoundary {
    JsonDecoder,
    ValueConstructor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApplicationRequest {
    Collaborator {
        context: u64,
        invocation: Invocation,
    },
    Register {
        test: String,
    },
    RegisterDecoder {
        test: String,
    },
    NextDecoderBatch {
        test: String,
        max_message_bytes: usize,
    },
    CompleteDecoderBatch {
        test: String,
        cases: Vec<DecoderBatchObservation>,
    },
    Next {
        test: String,
    },
    Observe {
        context: u64,
        observation: ApplicationObservation,
    },
    CloseCase {
        context: u64,
        cleanup_error: Option<String>,
    },
    CloseTest {
        test: String,
    },
    Finish,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApplicationReply {
    CollaboratorResult {
        context: u64,
        observation: Observation,
    },
    Registered,
    DecoderRegistered {
        batch: bool,
        max_message_bytes: usize,
        case_timeout_seconds: u64,
    },
    DecoderBatch {
        cases: Vec<DecoderBatchCase>,
    },
    DecoderBatchClosed {
        cases: Vec<DecoderBatchVerdict>,
    },
    Case {
        context: u64,
        id: String,
        form: TestForm,
        directive: ApplicationDirective,
        mocks: Vec<String>,
    },
    Action {
        context: u64,
        directive: ApplicationDirective,
        mocks: Vec<String>,
    },
    CaseClosed {
        context: u64,
        result: VerificationResult,
    },
    NoMoreCases,
    TestClosed,
    Finished {
        summary: ApplicationAssessmentSummary,
    },
    Rejected {
        issue: VerificationIssue,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderBatchCase {
    pub context: u64,
    pub id: String,
    pub directive: ApplicationDirective,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderBatchObservation {
    pub context: u64,
    /// An already assessed directive needs no native observation.
    pub observation: Option<ApplicationObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderBatchVerdict {
    pub context: u64,
    pub status: ApplicationAssessmentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApplicationDirective {
    Invoke {
        invocation: Invocation,
        phase: ApplicationInvocationPhase,
    },
    ReadState {
        entities: Vec<String>,
        phase: StatePhase,
        invocation: Invocation,
    },
    Construct {
        subject: String,
        ty: TypeRef,
        value: VerificationValue,
    },
    Decode {
        subject: String,
        ty: TypeRef,
        raw: Vec<u8>,
    },
    Verdict {
        result: VerificationResult,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DecoderOutcome {
    Accepted { value: VerificationValue },
    Rejected { reason: String },
    Exception { message: String },
    BindingError { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApplicationObservation {
    Invoked {
        invocation: Invocation,
        observation: Observation,
        calls: Option<Vec<InvocationObservation>>,
        events: Option<Vec<VerificationEvent>>,
        resources: Option<Vec<ResourceObservation>>,
    },
    State {
        entities: std::collections::BTreeMap<String, Vec<VerificationValue>>,
        phase: StatePhase,
        invocation: Invocation,
        complete: bool,
    },
    Constructed {
        input: VerificationValue,
        outcome: DecoderOutcome,
    },
    Decoded {
        raw: Vec<u8>,
        outcome: DecoderOutcome,
    },
    BindingError {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationAssessmentReport {
    pub assessment_version: String,
    pub group: String,
    pub finalized: bool,
    pub required_cases: BTreeSet<String>,
    pub missing_cases: BTreeSet<String>,
    pub missing_tests: BTreeSet<String>,
    pub cases: Vec<VerificationCaseRecord>,
    pub issues: Vec<VerificationIssue>,
    pub result: VerificationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ApplicationAssessmentStatus {
    Pass,
    Fail,
    Unknown,
}

/// The reply stays small even for large finite plans. Complete evidence is read
/// from the engine-owned report; counts never substitute for required cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationAssessmentSummary {
    pub assessment_version: String,
    pub group: String,
    pub finalized: bool,
    pub required_cases: usize,
    pub completed_cases: usize,
    pub missing_cases: usize,
    pub missing_tests: usize,
    pub issues: usize,
    pub status: ApplicationAssessmentStatus,
    pub issue_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationInvocationPhase {
    Setup,
    Production,
    Oracle,
}
