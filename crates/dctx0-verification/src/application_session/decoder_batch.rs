use super::*;

const MAX_CASES: usize = 16;
// Covers the protocol envelope, correlation ID, newline and reply header.
const FRAME_OVERHEAD: usize = 96;

impl ApplicationSession<'_> {
    pub(super) fn batch_allowed(&self, test: &str) -> bool {
        self.boundary == ApplicationValueBoundary::JsonDecoder
            && self.group.stateless_decoders.contains(test)
    }

    pub(super) fn single_decoder_request_allowed(&self, context: u64) -> bool {
        // A single large observation can use the legacy frame if it is next in
        // its packet. Later entries cannot overtake it.
        self.active.get(&context).is_none_or(|active| {
            !active.batched
                || !self
                    .active
                    .range(..context)
                    .any(|(_, earlier)| earlier.batched && earlier.test == active.test)
        })
    }

    pub(super) fn next_decoder_batch(&mut self, test: &str, limit: usize) -> ApplicationReply {
        if !self.batch_allowed(test)
            || !self.can_start_case(test)
            || limit > self.group.resources.max_message_bytes as usize
            || limit <= FRAME_OVERHEAD
        {
            return self.fault("APPLICATION_DECODER_BATCH_NOT_AVAILABLE", test);
        }
        let mut cases: Vec<DecoderBatchCase> = Vec::new();
        let mut bytes = FRAME_OVERHEAD;
        for _ in 0..MAX_CASES {
            let ApplicationReply::Case {
                context,
                id,
                form,
                directive,
                mocks,
            } = self.issue_case(test)
            else {
                break;
            };
            let case = DecoderBatchCase {
                context,
                id,
                directive,
            };
            bytes += serde_json::to_vec(&case)
                .expect("serializable decoder case")
                .len()
                + 1;
            if bytes > limit {
                if cases.is_empty() {
                    return ApplicationReply::Case {
                        context,
                        id: case.id,
                        form,
                        directive: case.directive,
                        mocks,
                    };
                }
                self.active.remove(&context);
                self.case_cursor.insert(
                    test.into(),
                    cases.last().expect("nonempty packet").id.clone(),
                );
                break;
            }
            self.active.get_mut(&context).expect("issued case").batched = true;
            cases.push(case);
        }
        if cases.is_empty() {
            ApplicationReply::NoMoreCases
        } else {
            ApplicationReply::DecoderBatch { cases }
        }
    }

    pub(super) fn complete_decoder_batch(
        &mut self,
        test: &str,
        cases: Vec<DecoderBatchObservation>,
    ) -> ApplicationReply {
        let mut pending = self
            .active
            .iter()
            .filter(|(_, case)| case.batched && case.test == test);
        if cases.is_empty()
            || cases.len() > MAX_CASES
            || !cases.iter().all(|entry| {
                pending.next().is_some_and(|(context, active)| {
                    entry.context == *context
                        && match &entry.observation {
                            Some(ApplicationObservation::Decoded { .. }) => {
                                active.verdict.is_none()
                            }
                            None => active.verdict.is_some(),
                            _ => false,
                        }
                })
            })
        {
            return self.fault("APPLICATION_DECODER_BATCH_ORDER", test);
        }
        let cases = cases
            .into_iter()
            .map(|entry| {
                if let Some(observation) = entry.observation {
                    self.observe(entry.context, observation);
                }
                let ApplicationReply::CaseClosed { result, .. } =
                    self.close_case(entry.context, None)
                else {
                    unreachable!("validated decoder context");
                };
                DecoderBatchVerdict {
                    context: entry.context,
                    status: match result {
                        VerificationResult::Pass => ApplicationAssessmentStatus::Pass,
                        VerificationResult::Fail { .. } => ApplicationAssessmentStatus::Fail,
                        VerificationResult::Unknown { .. } => ApplicationAssessmentStatus::Unknown,
                    },
                }
            })
            .collect();
        ApplicationReply::DecoderBatchClosed { cases }
    }
}
