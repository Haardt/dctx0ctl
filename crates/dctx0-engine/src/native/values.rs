//! Actual project factories/decoders; Rust interprets their observations.
use super::*;
impl NativeWorker {
    pub async fn execute_value_case(
        &mut self,
        id: &str,
        subject: &str,
        value: &dctx0_core::VerificationValue,
        cancel: &NativeCancellation,
    ) -> Result<ValueCheck, NativeFailure> {
        self.session_evidence.clear();
        let deadline = self.limits.case;
        let action = async {
            self.begin_case(id, cancel).await?;
            let reply = self
                .request(
                    WorkerRequest::Validate {
                        subject: subject.into(),
                        value: value.clone(),
                    },
                    cancel,
                    &mut cases::reject_mock,
                )
                .await?;
            let WorkerReply::ValueChecked { outcome } = reply else {
                self.abort().await?;
                return Err(NativeFailure::new(
                    NativeFailureKind::Protocol,
                    "Value check response has the wrong kind",
                ));
            };
            self.session_evidence
                .push(NativeSessionEvidence::ValueChecked {
                    subject: subject.into(),
                    value: value.clone(),
                    outcome: outcome.clone(),
                });
            self.end_case(cancel).await?;
            Ok(outcome)
        };
        match tokio::time::timeout(deadline, action).await {
            Ok(result) => result,
            Err(_) => Err(self.expired_case().await),
        }
    }
}
