//! Read-only explanations using the native publication proof evaluator.
use super::adoption::evaluate;
use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct AdoptionPreviewTask {
    pub task_id: i64,
    pub eligible: bool,
    pub blockers: Vec<VerificationIssue>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationAdoptionPreview {
    pub run_id: String,
    pub pin: dctx0_tasks::WorkPin,
    pub tasks: Vec<AdoptionPreviewTask>,
    pub issues: Vec<VerificationIssue>,
}
impl Project {
    /// Current-source inspection only. This receipt never authorizes publication.
    pub fn verification_adoption_preview(
        &self,
        application: Option<&Path>,
        run_id: &str,
    ) -> Result<VerificationAdoptionPreview> {
        let session = self.task_session(None, None)?;
        let mut report = VerificationAdoptionPreview {
            run_id: run_id.into(),
            pin: dctx0_tasks::WorkPin::of(&session),
            tasks: vec![],
            issues: vec![],
        };
        let store = self.store(AccessMode::ReadOnly)?;
        let summary = store.verification_run_summary(&self.key(), run_id)?;
        let mut eligible = BTreeSet::new();
        let mut blocked = BTreeMap::<i64, Vec<VerificationIssue>>::new();
        let mut selected = BTreeSet::new();
        if !summary.details_available {
            report.issues.push(issue(
                "ADOPTION_DETAILS_PRUNED",
                "Historical summaries cannot authorize new adoption",
            ));
        } else {
            let saved = self.saved_verification_run(run_id)?;
            let run = &saved.run;
            if let Some(pin) = &run.provenance.task_pin {
                selected.extend(pin.requirements.keys().copied());
            }
            if run.origin != VerificationRunOrigin::Native {
                report.issues.push(issue(
                    "ADOPTION_NATIVE_ORIGIN_REQUIRED",
                    "Imported evidence cannot authorize automatic task delivery",
                ));
            } else {
                let (application, _) = self.verification_configuration(application)?;
                let artifacts = run
                    .artifacts
                    .iter()
                    .map(|reference| {
                        let artifact = store.verification_artifact(run_id, &reference.hash)?;
                        if artifact.reference != *reference {
                            return Err(Error::Invalid("Native artifact identity mismatch".into()));
                        }
                        Ok(artifact)
                    })
                    .collect::<Result<Vec<_>>>()?;
                let unique = |kind: &str| -> Result<&VerificationArtifact> {
                    let found = artifacts
                        .iter()
                        .filter(|a| a.reference.media_type == kind)
                        .collect::<Vec<_>>();
                    match found.as_slice() {
                        [artifact] => Ok(*artifact),
                        _ => Err(Error::Invalid(format!("One {kind} artifact required"))),
                    }
                };
                // Missing/invalid source inventory is a blocker, never inferred freshness.
                match unique("application/vnd.dctx0.captured-input-inventory+json").and_then(|a| {
                    Ok(serde_json::from_slice::<BTreeMap<String, String>>(
                        &a.contents,
                    )?)
                }) {
                    Ok(inputs) => match Capture::inventory(&application) {
                        Ok(current) if current == inputs => {}
                        Ok(_) => report.issues.push(issue(
                            "ADOPTION_IMPLEMENTATION_CHANGED",
                            "Implementation, adapter, configuration or dependency inputs changed",
                        )),
                        Err(error) => report
                            .issues
                            .push(issue("ADOPTION_FRESHNESS_UNAVAILABLE", error.to_string())),
                    },
                    Err(error) => report.issues.push(issue(
                        "ADOPTION_INPUT_INVENTORY_REQUIRED",
                        error.to_string(),
                    )),
                }
                let (check, snapshot_id) = store.task_check(&self.key())?;
                if check != run.provenance.check_occurrence
                    || snapshot_id != run.provenance.snapshot
                {
                    report.issues.push(issue(
                        "ADOPTION_HISTORICAL_CHECK",
                        "Latest published check does not match native evidence",
                    ));
                }
                let snapshot = self.load(&run.provenance.snapshot)?;
                if dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash {
                    report.issues.push(issue(
                        "ADOPTION_SPEC_CHANGED",
                        "Specification sources differ from executed requirements",
                    ));
                }
                if artifacts
                    .iter()
                    .filter(|a| {
                        a.reference.media_type == "application/vnd.dctx0.runtime-identity+json"
                    })
                    .count()
                    != 1
                {
                    report.issues.push(issue(
                        "ADOPTION_RUNTIME_IDENTITY_REQUIRED",
                        "One current native runtime identity is required",
                    ));
                }
                for artifact in artifacts.iter().filter(|a| {
                    a.reference.media_type == "application/vnd.dctx0.runtime-identity+json"
                }) {
                    let runtime: crate::NativeRuntimeIdentity =
                        serde_json::from_slice(&artifact.contents)?;
                    match runtime.still_current() {
                        Ok(true) => {}
                        Ok(false) => report.issues.push(issue(
                            "ADOPTION_RUNTIME_CHANGED",
                            "Native runtime identity changed",
                        )),
                        Err(error) => report
                            .issues
                            .push(issue("ADOPTION_RUNTIME_UNAVAILABLE", error.to_string())),
                    }
                }
                match unique("application/vnd.dctx0.plan+json") {
                    Ok(artifact) if artifact.reference.hash == run.provenance.plan_hash => {
                        let plan: ProgramPlan = serde_json::from_slice(&artifact.contents)?;
                        let (_, issues, proposal) =
                            evaluate(self, &application, &plan, &saved, &artifacts, true)?;
                        if let Some(proposal) = proposal {
                            eligible.extend(proposal.tasks);
                            for blocker in proposal.blocked {
                                blocked
                                    .entry(blocker.task_id)
                                    .or_default()
                                    .push(issue(&blocker.code, blocker.reason));
                            }
                        }
                        report.issues.extend(issues);
                    }
                    _ => report.issues.push(issue(
                        "ADOPTION_PLAN_UNAVAILABLE",
                        "Exact native plan artifact required",
                    )),
                }
            }
        }
        for member in &session.plan.members {
            let id = member.task_id;
            let mut blockers = blocked.remove(&id).unwrap_or_default();
            if summary.details_available && !selected.contains(&id) {
                blockers.push(issue(
                    "TASK_OUTSIDE_EVIDENCE_SCOPE",
                    "Task was not pinned in this native run",
                ));
            }
            let state = &session.states[&id];
            if state.work != dctx0_tasks::WorkState::Open
                || state.actor.is_some()
                || state.claim.is_some()
                || state.reservation.is_some()
            {
                blockers.push(issue(
                    "TASK_NOT_UNCLAIMED_OPEN",
                    "Done, ongoing or reserved work is preserved",
                ));
            }
            if !matches!(
                member.candidate.kind,
                dctx0_tasks::TaskKind::Contract | dctx0_tasks::TaskKind::Operation
            ) {
                blockers.push(issue("TASK_KIND_REQUIRES_DEDICATED_EVIDENCE", "Review, retirement, adapter and integration work require their dedicated delivery workflow"));
            }
            let can_adopt =
                eligible.contains(&id) && blockers.is_empty() && report.issues.is_empty();
            if !can_adopt && blockers.is_empty() && report.issues.is_empty() {
                blockers.push(issue(
                    "TASK_EVIDENCE_REQUIRED",
                    "No eligible native proposal for this task",
                ));
            }
            report.tasks.push(AdoptionPreviewTask {
                task_id: id,
                eligible: can_adopt,
                blockers,
            });
        }
        // Detect concurrent task changes; this read-only report is never a reservation.
        if store.task_sequence(&self.key())? != session.event_seq {
            report.issues.push(issue(
                "ADOPTION_CONCURRENT_CHANGE",
                "Task events changed during preview; retry inspection",
            ));
            for task in &mut report.tasks {
                task.eligible = false;
            }
        }
        Ok(report)
    }
}
