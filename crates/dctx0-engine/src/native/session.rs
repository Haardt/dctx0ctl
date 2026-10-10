//! Shared session/protocol guards; successful replies alone are no run proof.
use super::*;
/// Acknowledged native observations survive case errors and cancellation.
/// This journal records delivery, never a verification verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NativeSessionEvidence {
    ValueChecked {
        subject: String,
        value: dctx0_core::VerificationValue,
        outcome: ValueCheck,
    },
    Begun {
        case_id: String,
    },
    MockRequested {
        request_id: u64,
        invocation: Invocation,
    },
    MockReplied {
        request_id: u64,
        observation: Observation,
    },
    Invoked {
        invocation: Invocation,
        observation: Observation,
        events: Vec<EmittedEvent>,
        calls: Option<Vec<dctx0_core::InvocationObservation>>,
    },
    Observed {
        states: std::collections::BTreeMap<String, Vec<dctx0_core::VerificationValue>>,
    },
    Ended,
}
pub(super) struct Invoked {
    pub observation: Observation,
    pub events: Vec<EmittedEvent>,
    pub calls: Option<Vec<dctx0_core::InvocationObservation>>,
}
impl NativeWorker {
    /// The latest case's acknowledged prefix, including after a failed cleanup.
    pub fn session_evidence(&self) -> &[NativeSessionEvidence] {
        &self.session_evidence
    }
    pub(super) async fn begin_case(
        &mut self,
        id: &str,
        cancel: &NativeCancellation,
    ) -> Result<(), NativeFailure> {
        self.session_evidence.clear();
        let reply = self
            .request(
                WorkerRequest::Begin { case_id: id.into() },
                cancel,
                &mut cases::reject_mock,
            )
            .await?;
        if !matches!(reply,WorkerReply::Begun {case_id} if case_id==id) {
            self.abort().await?;
            return Err(NativeFailure::new(
                NativeFailureKind::Protocol,
                "Case session acknowledgment mismatch",
            ));
        }
        self.session_evidence
            .push(NativeSessionEvidence::Begun { case_id: id.into() });
        Ok(())
    }
    pub(super) async fn end_case(
        &mut self,
        cancel: &NativeCancellation,
    ) -> Result<(), NativeFailure> {
        if self
            .request(WorkerRequest::End, cancel, &mut cases::reject_mock)
            .await?
            != WorkerReply::Ended
        {
            self.abort().await?;
            return Err(NativeFailure::new(
                NativeFailureKind::Protocol,
                "Case cleanup acknowledgment mismatch",
            ));
        }
        self.session_evidence.push(NativeSessionEvidence::Ended);
        Ok(())
    }
    pub(super) async fn invoke_case(
        &mut self,
        invocation: Invocation,
        cancel: &NativeCancellation,
        mocks: &mut NativeMockHandler<'_>,
    ) -> Result<Invoked, NativeFailure> {
        match self
            .request(
                WorkerRequest::Invoke {
                    invocation: invocation.clone(),
                },
                cancel,
                mocks,
            )
            .await?
        {
            WorkerReply::Invoked {
                observation,
                events,
                calls,
            } => {
                self.session_evidence.push(NativeSessionEvidence::Invoked {
                    invocation,
                    observation: observation.clone(),
                    events: events.clone(),
                    calls: calls.clone(),
                });
                Ok(Invoked {
                    observation,
                    events,
                    calls,
                })
            }
            _ => {
                self.abort().await?;
                Err(NativeFailure::new(
                    NativeFailureKind::Protocol,
                    "Invocation response has the wrong kind",
                ))
            }
        }
    }
    pub(super) async fn observe_case(
        &mut self,
        entities: Vec<String>,
        cancel: &NativeCancellation,
    ) -> Result<std::collections::BTreeMap<String, Vec<dctx0_core::VerificationValue>>, NativeFailure>
    {
        match self
            .request(
                WorkerRequest::Observe { entities },
                cancel,
                &mut cases::reject_mock,
            )
            .await?
        {
            WorkerReply::Observed { states } => {
                self.session_evidence.push(NativeSessionEvidence::Observed {
                    states: states.clone(),
                });
                Ok(states)
            }
            _ => {
                self.abort().await?;
                Err(NativeFailure::new(
                    NativeFailureKind::Protocol,
                    "State observation response has the wrong kind",
                ))
            }
        }
    }
}
