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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resources: Option<Vec<dctx0_core::ResourceObservation>>,
    },
    Observed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phase: Option<dctx0_core::StatePhase>,
        states: std::collections::BTreeMap<String, Vec<dctx0_core::VerificationValue>>,
    },
    Ended,
}
pub(super) struct Invoked {
    pub observation: Observation,
    pub events: Vec<EmittedEvent>,
    pub calls: Option<Vec<dctx0_core::InvocationObservation>>,
    pub resources: Option<Vec<dctx0_core::ResourceObservation>>,
}
impl NativeWorker {
    pub(super) fn clear_evidence(&mut self) {
        self.session_evidence.clear();
        self.evidence_bytes = 0;
    }
    pub(super) async fn retain_evidence(
        &mut self,
        entry: NativeSessionEvidence,
    ) -> Result<(), NativeFailure> {
        let bytes = serde_json::to_vec(&entry)
            .map_err(|error| NativeFailure::new(NativeFailureKind::Technical, error.to_string()))?
            .len();
        if self.session_evidence.len() >= self.limits.evidence_entries
            || bytes
                > self
                    .limits
                    .evidence_bytes
                    .saturating_sub(self.evidence_bytes)
        {
            let mut error = NativeFailure::new(
                NativeFailureKind::MessageLimit,
                "Session evidence exceeds its byte or entry budget; retained prefix is incomplete",
            );
            if let Err(cleanup) = self.abort().await {
                error.cleanup_error = Some(cleanup.to_string());
            }
            return Err(error);
        }
        self.evidence_bytes += bytes;
        self.session_evidence.push(entry);
        Ok(())
    }
    /// The latest case's acknowledged prefix, including after a failed cleanup.
    pub fn session_evidence(&self) -> &[NativeSessionEvidence] {
        &self.session_evidence
    }
    pub(super) async fn begin_case(
        &mut self,
        id: &str,
        cancel: &NativeCancellation,
    ) -> Result<(), NativeFailure> {
        self.clear_evidence();
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
        self.retain_evidence(NativeSessionEvidence::Begun { case_id: id.into() })
            .await?;
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
        self.retain_evidence(NativeSessionEvidence::Ended).await?;
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
                resources,
            } => {
                self.retain_evidence(NativeSessionEvidence::Invoked {
                    invocation,
                    observation: observation.clone(),
                    events: events.clone(),
                    calls: calls.clone(),
                    resources: resources.clone(),
                })
                .await?;
                Ok(Invoked {
                    observation,
                    events,
                    calls,
                    resources,
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
        phase: dctx0_core::StatePhase,
        cancel: &NativeCancellation,
    ) -> Result<std::collections::BTreeMap<String, Vec<dctx0_core::VerificationValue>>, NativeFailure>
    {
        match self
            .request(
                WorkerRequest::Observe {
                    entities: entities.clone(),
                },
                cancel,
                &mut cases::reject_mock,
            )
            .await?
        {
            WorkerReply::Observed { states } => {
                if entities.iter().any(|entity| !states.contains_key(entity)) {
                    self.abort().await?;
                    return Err(NativeFailure::new(
                        NativeFailureKind::Adapter,
                        "State observer omitted a requested entity; an explicit empty list is required for observed empty state",
                    ));
                }
                self.retain_evidence(NativeSessionEvidence::Observed {
                    phase: Some(phase),
                    states: states.clone(),
                })
                .await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn concrete_observer_keys_distinguish_missing_from_observed_empty() {
        let python =
            std::env::var("DCTX0_TEST_PYTHON").expect("Coupled runner supplies Python fixture");
        for missing in [true, false] {
            let states = if missing { "{}" } else { "{'W.Member':[]}" };
            let code = format!(
                "import json,sys\np=json.loads(sys.stdin.readline())\nprint(json.dumps({{'protocol_version':1,'message':{{'kind':'reply','id':p['id'],'reply':{{'kind':'observed','states':{states}}}}}}}),flush=True)\nsys.stdin.read()\n"
            );
            let recipe = ExecutionRecipe {
                program: python.clone(),
                arguments: vec!["-u".into(), "-c".into(), code],
            };
            let mut worker = NativeWorker::spawn(&recipe, Path::new("."), NativeLimits::default())
                .await
                .unwrap();
            let result = worker
                .observe_case(
                    vec!["W.Member".into()],
                    dctx0_core::StatePhase::After,
                    &NativeCancellation::default(),
                )
                .await;
            if missing {
                assert_eq!(result.unwrap_err().kind, NativeFailureKind::Adapter);
            } else {
                assert!(result.unwrap()["W.Member"].is_empty());
            }
            worker.abort().await.unwrap();
        }
    }
    #[test]
    fn legacy_observation_journals_keep_their_unknown_phase() {
        let legacy: NativeSessionEvidence =
            serde_json::from_str(r#"{"kind":"observed","states":{}}"#).unwrap();
        assert!(matches!(
            legacy,
            NativeSessionEvidence::Observed { phase: None, .. }
        ));
        for phase in [
            dctx0_core::StatePhase::Before,
            dctx0_core::StatePhase::After,
        ] {
            let entry = NativeSessionEvidence::Observed {
                phase: Some(phase),
                states: Default::default(),
            };
            let restored: NativeSessionEvidence =
                serde_json::from_slice(&serde_json::to_vec(&entry).unwrap()).unwrap();
            assert!(
                matches!(restored,NativeSessionEvidence::Observed { phase:Some(actual),.. } if actual == phase)
            );
        }
    }
}
