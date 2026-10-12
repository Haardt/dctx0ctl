//! Separate native transport contract; profile manifests remain unchanged.
mod dependencies;
mod inspection;
use dctx0_core::{Invocation, Observation, VerificationValue};
pub use dependencies::*;
pub use inspection::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const SDK_VERSION: u32 = 1;
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;
pub const MAX_REQUEST_ID: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessRequest {
    pub adapter_import: String,
    pub subjects: BTreeSet<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub contents: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRecipe {
    pub program: String,
    pub arguments: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessBundle {
    pub sdk_version: u32,
    pub generator_version: String,
    pub target: String,
    pub artifacts: Vec<Artifact>,
    pub recipe: ExecutionRecipe,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<ExecutionRecipe>,
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Harness input: {0}")]
    Invalid(String),
    #[error("Protocol: {0}")]
    Protocol(String),
}
pub trait HarnessGenerator {
    fn runtime(&self) -> &str {
        "bun"
    }
    fn inspect_dependencies(
        &self,
        _sources: &[InspectionSource],
    ) -> Result<DependencyInspection, Error> {
        Err(Error::Invalid(
            "Target dependency inspection is unavailable".into(),
        ))
    }
    fn module_observer_artifacts(
        &self,
        _sources: &[InspectionSource],
        _project_root_relative: &str,
    ) -> Result<Vec<Artifact>, Error> {
        Ok(vec![])
    }
    fn generate(&self, request: &HarnessRequest) -> Result<HarnessBundle, Error>;
    /// Setup preserves existing developer adapters; scaffolds are unavailable.
    fn scaffold(&self) -> Artifact;
    /// Additional editable setup files; repeated setup preserves them.
    fn setup_artifacts(&self) -> Vec<Artifact> {
        Vec::new()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectDescription {
    pub subject: String,
    pub role: String,
    pub artifact: String,
    pub entry_point: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterDescription {
    pub protocol_version: u32,
    pub subjects: Vec<SubjectDescription>,
    pub artifacts: Vec<String>,
    pub capabilities: Vec<String>,
    pub representations: BTreeMap<String, String>,
}
pub use dctx0_core::VerificationEvent as EmittedEvent;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ValueCheck {
    Accepted { value: VerificationValue },
    Rejected { reason: String },
    Unavailable { reason: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkerRequest {
    Metrics,
    ObserveDependencies,
    Describe,
    ResolveDependency {
        mode: DependencyResolution,
        specifier: String,
        importer: String,
    },
    Begin {
        case_id: String,
    },
    Invoke {
        invocation: Invocation,
    },
    Validate {
        subject: String,
        value: VerificationValue,
    },
    Observe {
        entities: Vec<String>,
    },
    End,
    Shutdown,
    MockReply {
        request_id: u64,
        observation: Observation,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkerReply {
    DependenciesObserved {
        evidence: ModuleObserverEvidence,
    },
    DependencyResolved {
        path: String,
    },
    DependencyUnresolved {
        reason: String,
    },
    Measured {
        peak_rss_bytes: Option<u64>,
        runtime_version: String,
        platform: String,
    },
    Described {
        description: AdapterDescription,
    },
    Begun {
        case_id: String,
    },
    Invoked {
        observation: Observation,
        events: Vec<EmittedEvent>,
        /// None means no real call observer; an observed empty trace is Some([]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        calls: Option<Vec<dctx0_core::InvocationObservation>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resources: Option<Vec<dctx0_core::ResourceObservation>>,
    },
    ValueChecked {
        outcome: ValueCheck,
    },
    Observed {
        states: BTreeMap<String, Vec<VerificationValue>>,
    },
    Ended,
    Bye,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkerMessage {
    Reply {
        id: u64,
        reply: WorkerReply,
    },
    MockCall {
        request_id: u64,
        invocation: Invocation,
    },
    TechnicalError {
        id: Option<u64>,
        category: String,
        message: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
    pub protocol_version: u32,
    pub id: u64,
    pub request: WorkerRequest,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageEnvelope {
    pub protocol_version: u32,
    pub message: WorkerMessage,
}

/// Reject version, framing, size and schema violations before using observations.
pub fn decode_message(bytes: &[u8], limit: usize) -> Result<MessageEnvelope, Error> {
    if bytes.len() > limit || bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Error::Protocol("Message byte limit exceeded".into()));
    }
    let raw: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| Error::Protocol(e.to_string()))?;
    let message: MessageEnvelope =
        serde_json::from_slice(bytes).map_err(|e| Error::Protocol(e.to_string()))?;
    // Round-trip shape comparison also catches ignored unit/adjacent-variant fields.
    // Nullable wire fields must be explicit; arbitrary record/map keys are retained.
    if serde_json::to_value(&message).expect("typed messages serialize") != raw {
        return Err(Error::Protocol("Unexpected or omitted wire fields".into()));
    }
    if message.protocol_version != SDK_VERSION {
        return Err(Error::Protocol("Unsupported protocol version".into()));
    }
    let id = match &message.message {
        WorkerMessage::Reply { id, .. } => Some(*id),
        WorkerMessage::MockCall { request_id, .. } => Some(*request_id),
        WorkerMessage::TechnicalError { .. } => None,
    };
    if id.is_some_and(|id| id == 0 || id > MAX_REQUEST_ID) {
        return Err(Error::Protocol("Invalid message ID".into()));
    }
    Ok(message)
}

pub fn encode_request(request: &RequestEnvelope, limit: usize) -> Result<Vec<u8>, Error> {
    if request.protocol_version != SDK_VERSION || request.id == 0 || request.id > MAX_REQUEST_ID {
        return Err(Error::Protocol("Invalid request version or ID".into()));
    }
    let mut bytes = serde_json::to_vec(request).expect("typed wire requests serialize");
    if bytes.len() > limit || bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Error::Protocol("Request byte limit exceeded".into()));
    }
    bytes.push(b'\n');
    Ok(bytes)
}
