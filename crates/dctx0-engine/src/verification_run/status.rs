//! Project status from compact metadata and exact captured-input inventories.
use super::*;
use dctx0_store::{Store, VerificationRunContext, VerificationRunSummary};
use dctx0_tasks::{CheckStatus, Session, WorkPin, WorkState};

#[derive(Debug, Default, Serialize)]
pub struct StatusTaskCounts {
    pub total: usize,
    pub done_pass: usize,
    pub done_without_pass: usize,
    pub open: usize,
    pub in_progress: usize,
    pub blocked: usize,
    /// These three counts partition historical Done/PASS tasks. Unknown includes
    /// unbound agent evidence and pruned native inputs; it never means current.
    pub evidence_current: usize,
    pub evidence_stale: usize,
    pub evidence_unknown: usize,
    /// INSPECTED evidence with independently executed native cases.
    pub inspected_current: usize,
    pub inspected_stale: usize,
    /// Authored agent inspections whose exact retained native/source binding
    /// was independently checked during this read. Not automatic native proof.
    pub agent_current: usize,
    pub agent_stale: usize,
}

#[derive(Debug, Serialize)]
pub struct StatusCause {
    pub code: String,
    pub affected: usize,
    pub next_step: String,
}

#[derive(Debug, Serialize)]
pub struct StatusRunSummary {
    pub id: String,
    pub target: String,
    pub snapshot: String,
    pub check_occurrence: i64,
    pub result: VerificationResult,
    pub required_case_count: usize,
    pub case_count: usize,
    pub missing_case_count: usize,
    pub outcomes: [u64; 3],
    pub open_obligation_count: usize,
    pub details_available: bool,
    pub selected_subject_count: usize,
    pub causes: Option<BTreeMap<VerificationCause, usize>>,
}
impl From<&VerificationRunSummary> for StatusRunSummary {
    fn from(summary: &VerificationRunSummary) -> Self {
        Self {
            id: summary.id.clone(),
            target: summary.target.clone(),
            snapshot: summary.snapshot.clone(),
            check_occurrence: summary.check_occurrence,
            result: summary.result.clone(),
            required_case_count: summary.required_case_count,
            case_count: summary.case_count,
            missing_case_count: summary.missing_case_count,
            outcomes: summary.outcomes,
            open_obligation_count: summary.open_obligation_count,
            details_available: summary.details_available,
            selected_subject_count: summary.selected_subjects.len(),
            causes: summary.causes.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StatusRun {
    pub summary: StatusRunSummary,
    pub sources_current: Option<bool>,
    pub requirements_current: Option<bool>,
    pub task_pin_present: Option<bool>,
    pub task_event_pin_current: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct StatusProgress {
    pub baseline: WorkPin,
    pub newly_done: usize,
    pub newly_pass: usize,
    pub added_done: usize,
    pub added_pass: usize,
    pub lost_pass: usize,
}

#[derive(Debug, Serialize)]
pub struct ProjectStatus {
    pub schema_version: u32,
    pub snapshot: Option<String>,
    pub check_occurrence: Option<i64>,
    pub spec_current: Option<bool>,
    pub task_pin: Option<WorkPin>,
    pub task_plan_current: Option<bool>,
    pub tasks: StatusTaskCounts,
    pub latest_run: Option<StatusRun>,
    /// Retained historical coverage, separate from the latest attempt and from
    /// live validity (which read-only status cannot establish).
    pub historical_complete_baseline: Option<StatusRunSummary>,
    pub progress: Option<StatusProgress>,
    pub next_causes: Vec<StatusCause>,
    pub concurrent_change: bool,
    pub issues: Vec<VerificationIssue>,
}

fn status_input<T>(
    result: Result<T>,
    code: &str,
    issues: &mut Vec<VerificationIssue>,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            issues.push(issue(code, error.to_string()));
            None
        }
    }
}

pub(super) fn requirements_current(
    context: &VerificationRunContext,
    session: &Session,
) -> Option<bool> {
    let pin = context.provenance.task_pin.as_ref()?;
    Some(
        session.plan.snapshot_id == context.provenance.snapshot
            && session.plan.check_run_id == context.provenance.check_occurrence
            && pin.requirements.iter().all(|(id, hash)| {
                session
                    .plan
                    .members
                    .iter()
                    .any(|m| m.task_id == *id && m.candidate.requirements_hash == *hash)
            }),
    )
}

pub(super) fn sources_current(
    store: &Store,
    id: &str,
    context: &VerificationRunContext,
    inputs: Option<&BTreeMap<String, String>>,
) -> Result<Option<bool>> {
    let Some(inputs) = inputs else {
        return Ok(None);
    };
    let artifacts = context
        .artifacts
        .iter()
        .filter(|a| a.media_type == "application/vnd.dctx0.captured-input-inventory+json")
        .collect::<Vec<_>>();
    let [reference] = artifacts.as_slice() else {
        return Ok(None);
    };
    let artifact = store.verification_artifact(id, &reference.hash)?;
    let expected: BTreeMap<String, String> = serde_json::from_slice(&artifact.contents)?;
    Ok(Some(&expected == inputs))
}

struct EvidenceBinding {
    implementation: String,
    requirements: BTreeMap<i64, String>,
    current: Option<bool>,
}

fn evidence_binding(
    store: &Store,
    project: &str,
    id: &str,
    session: &Session,
    inputs: Option<&BTreeMap<String, String>>,
    spec_current: Option<bool>,
) -> Result<Option<EvidenceBinding>> {
    let summary = store.verification_run_summary(project, id)?;
    if summary.origin != VerificationRunOrigin::Native || !summary.details_available {
        return Ok(None);
    }
    let Some(context) = store.verification_run_context(project, id)? else {
        return Ok(None);
    };
    let current = match (
        sources_current(store, id, &context, inputs)?,
        requirements_current(&context, session),
    ) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) if spec_current == Some(true) => Some(true),
        _ if spec_current == Some(false) => Some(false),
        _ => None,
    };
    let identities = context
        .artifacts
        .iter()
        .filter(|artifact| {
            matches!(
                artifact.media_type.as_str(),
                "application/vnd.dctx0.verifier-identity+json"
                    | "application/vnd.dctx0.runtime-identity+json"
                    | "application/vnd.dctx0.assessment-version+json"
                    | "application/vnd.dctx0.native-limits+json"
                    | "application/vnd.dctx0.dependency-resolution-limits+json"
            )
        })
        .map(|artifact| (&artifact.media_type, &artifact.hash))
        .collect::<BTreeMap<_, _>>();
    let implementation = dctx0_changes::hash(&(&context.provenance, identities));
    Ok(context.provenance.task_pin.map(|pin| EvidenceBinding {
        implementation,
        requirements: pin.requirements,
        current,
    }))
}

impl Project {
    /// Read-only: no compiler check, run reconstruction, adoption or saved cursor.
    /// An explicit baseline refers to a real saved plan/event, never last-query
    /// time. Without one, progress is measured from the preceding run's task pin.
    pub fn status(
        &self,
        application: Option<&Path>,
        since: Option<(i64, i64)>,
    ) -> Result<ProjectStatus> {
        if !self.database.try_exists()? {
            if since.is_some() {
                return Err(Error::Invalid(
                    "Progress baseline requires a published task plan".into(),
                ));
            }
            return Ok(ProjectStatus {
                schema_version: 1,
                snapshot: None,
                check_occurrence: None,
                spec_current: None,
                task_pin: None,
                task_plan_current: None,
                tasks: Default::default(),
                latest_run: None,
                historical_complete_baseline: None,
                progress: None,
                next_causes: vec![StatusCause {
                    code: "SPEC_CHECK_REQUIRED".into(),
                    affected: 0,
                    next_step:
                        "Publish a successful check, then tasks plan; no local history exists"
                            .into(),
                }],
                concurrent_change: false,
                issues: vec![],
            });
        }
        let store = self.store(AccessMode::ReadOnly)?;
        let key = self.key();
        let snapshot = store.latest(&key)?;
        let check = snapshot
            .as_ref()
            .map(|_| store.task_check(&key))
            .transpose()?;
        let session = store
            .task_head(&key)?
            .map(|id| store.task_metadata_session(&key, Some(id), None))
            .transpose()?;
        let mut issues = vec![];
        let config = status_input(
            self.optional_verification_configuration(application),
            "STATUS_CONFIGURATION_UNAVAILABLE",
            &mut issues,
        )
        .flatten();
        let inputs = config.as_ref().and_then(|(root, config)| {
            status_input(
                Capture::inventory_with_budget(root, config.capture_budgets()),
                "STATUS_SOURCE_FRESHNESS_UNAVAILABLE",
                &mut issues,
            )
        });
        let spec_current = snapshot.as_ref().and_then(|snapshot| {
            status_input(
                self.sources()
                    .map(|sources| dctx0_changes::source_hash(&sources) == snapshot.source_hash),
                "STATUS_SPEC_FRESHNESS_UNAVAILABLE",
                &mut issues,
            )
        });
        let mut report = ProjectStatus {
            schema_version: 1,
            snapshot: snapshot.as_ref().map(|s| s.id.clone()),
            check_occurrence: check.as_ref().map(|(check, _)| *check),
            spec_current,
            task_pin: session.as_ref().map(WorkPin::of),
            task_plan_current: session.as_ref().map(|s| {
                check.as_ref().is_some_and(|(c, snap)| {
                    *c == s.plan.check_run_id && *snap == s.plan.snapshot_id
                })
            }),
            tasks: Default::default(),
            latest_run: None,
            historical_complete_baseline: None,
            progress: None,
            next_causes: vec![],
            concurrent_change: false,
            issues,
        };
        let mut latest = vec![];
        let mut offset = 0;
        loop {
            let page = store.verification_run_summaries(&key, offset, 200)?;
            let complete = page.len() < 200;
            latest.extend(
                page.into_iter()
                    .filter(|summary| {
                        config.as_ref().is_none_or(|(_, config)| {
                            let mut subjects = config.scope.subjects.clone();
                            if let Some(session) = &session {
                                for task in &config.scope.tasks {
                                    if let Ok(member) = session.member(*task) {
                                        subjects.extend(member.candidate.subjects.clone());
                                    }
                                }
                            }
                            summary.target == config.target
                                && summary.selected_subjects.iter().any(|subject| {
                                    subjects.iter().any(|selected| {
                                        selected == subject
                                            || subject.starts_with(&format!("{selected}."))
                                            || selected.starts_with(&format!("{subject}."))
                                    })
                                })
                        })
                    })
                    .take(2 - latest.len()),
            );
            if latest.len() >= 2 || complete {
                break;
            }
            offset += 200;
        }
        if let Some((_, config)) = &config {
            report.historical_complete_baseline = store
                .verification_complete_baseline(&key, &config.target, &config.scope.subjects)?
                .as_ref()
                .map(Into::into);
        }
        if let Some(summary) = latest.first() {
            let context = store.verification_run_context(&key, &summary.id)?;
            report.latest_run = Some(StatusRun {
                sources_current: context
                    .as_ref()
                    .filter(|_| summary.details_available)
                    .map(|c| sources_current(&store, &summary.id, c, inputs.as_ref()))
                    .transpose()?
                    .flatten(),
                requirements_current: context
                    .as_ref()
                    .and_then(|c| session.as_ref().and_then(|s| requirements_current(c, s))),
                task_pin_present: context.as_ref().map(|c| c.provenance.task_pin.is_some()),
                task_event_pin_current: context
                    .as_ref()
                    .and_then(|c| c.provenance.task_pin.as_ref())
                    .and_then(|p| {
                        session
                            .as_ref()
                            .map(|s| p.plan_id == s.plan.id && p.event_sequence == s.event_seq)
                    }),
                summary: summary.into(),
            });
        }
        if let Some(session) = &session {
            for row in session.overviews() {
                report.tasks.total += 1;
                match row.work {
                    WorkState::Open => report.tasks.open += 1,
                    WorkState::Progress => report.tasks.in_progress += 1,
                    WorkState::Done if row.check == CheckStatus::Pass => {
                        report.tasks.done_pass += 1
                    }
                    WorkState::Done => report.tasks.done_without_pass += 1,
                }
                report.tasks.blocked +=
                    usize::from(row.work != WorkState::Done && !row.blockers.is_empty());
            }
            let references = store
                .task_evidence_references(&key, session.event_seq)?
                .unwrap_or_default();
            let hashes = references.iter().map(|r| r.report_hash.clone()).collect();
            let runs = store.verification_report_runs(&key, &hashes)?;
            let active = session
                .plan
                .members
                .iter()
                .map(|m| (m.task_id, &m.candidate.requirements_hash))
                .collect::<BTreeMap<_, _>>();
            let mut freshness = BTreeMap::new();
            let mut inspected_artifacts = BTreeMap::new();
            let mut agent = super::status_agent::AgentFreshness::new(
                self,
                &store,
                session,
                config.as_ref().map(|(root, _)| root.as_path()),
                inputs.as_ref(),
                spec_current,
            );
            for reference in &references {
                if active.get(&reference.task_id).copied() != Some(&reference.requirements_hash)
                    || !session
                        .states
                        .get(&reference.task_id)
                        .is_some_and(|s| s.work == WorkState::Done && s.check == CheckStatus::Pass)
                    || reference.status != "PASS"
                {
                    continue;
                }
                if !matches!(
                    reference.verifier.as_str(),
                    "dctx0-native-verifier" | "dctx0-inspected-verifier"
                ) {
                    match agent.current(reference) {
                        Some(true) => {
                            report.tasks.evidence_current += 1;
                            report.tasks.agent_current += 1;
                        }
                        Some(false) => {
                            report.tasks.evidence_stale += 1;
                            report.tasks.agent_stale += 1;
                        }
                        None => {}
                    }
                    continue;
                }
                let inspected = reference.verifier == "dctx0-inspected-verifier";
                let id = if inspected {
                    reference.native_run.as_ref()
                } else {
                    runs.get(&reference.report_hash)
                };
                let Some(id) = id else { continue };
                if !inspected
                    && !store.verification_task_linked(
                        &key,
                        id,
                        reference.task_id,
                        &reference.requirements_hash,
                    )?
                {
                    continue;
                }
                if inspected {
                    let Some(hash) = &reference.inspection_artifact else {
                        continue;
                    };
                    if !store.verification_inspected_task_linked(
                        &key,
                        id,
                        reference.task_id,
                        &reference.requirements_hash,
                        &reference.report_hash,
                    )? {
                        continue;
                    }
                    let Some(context) = store.verification_run_context(&key, id)? else {
                        continue;
                    };
                    if !context.artifacts.iter().any(|a| {
                        a.hash == *hash && a.media_type == dctx0_tasks::TASK_INSPECTIONS_MEDIA_TYPE
                    }) {
                        continue;
                    }
                    let cache_key = (id.clone(), hash.clone());
                    if !inspected_artifacts.contains_key(&cache_key) {
                        let batch =
                            store
                                .verification_artifact(id, hash)
                                .ok()
                                .and_then(|artifact| {
                                    (artifact.reference.hash == *hash
                                    && artifact.reference.media_type
                                        == dctx0_tasks::TASK_INSPECTIONS_MEDIA_TYPE)
                                    .then(|| {
                                        serde_json::from_slice::<dctx0_tasks::InspectedDoneBatch>(
                                            &artifact.contents,
                                        )
                                        .ok()
                                    })
                                    .flatten()
                                });
                        inspected_artifacts.insert(cache_key.clone(), batch);
                    }
                    let Some(batch) = inspected_artifacts[&cache_key].as_ref() else {
                        continue;
                    };
                    let Ok(member) = session.member(reference.task_id) else {
                        continue;
                    };
                    if batch.snapshot != context.provenance.snapshot
                        || batch.check_occurrence != context.provenance.check_occurrence
                        || context.provenance.task_pin.as_ref() != Some(&batch.task_pin)
                        || !batch.valid_catalog(&context.provenance.implementation)
                        || !batch.validate_task(
                            reference.task_id,
                            &member.candidate,
                            &context.provenance.implementation,
                        )
                    {
                        continue;
                    }
                }
                if !freshness.contains_key(id) {
                    freshness.insert(
                        id.clone(),
                        evidence_binding(&store, &key, id, session, inputs.as_ref(), spec_current)?,
                    );
                }
                let Some(binding) = freshness[id].as_ref() else {
                    continue;
                };
                if binding.implementation != reference.implementation
                    || binding.requirements.get(&reference.task_id)
                        != Some(&reference.requirements_hash)
                {
                    continue;
                }
                match binding.current {
                    Some(true) => {
                        report.tasks.evidence_current += 1;
                        if inspected {
                            report.tasks.inspected_current += 1;
                        }
                    }
                    Some(false) => {
                        report.tasks.evidence_stale += 1;
                        if inspected {
                            report.tasks.inspected_stale += 1;
                        }
                    }
                    None => {}
                }
            }
            report.tasks.evidence_unknown = report
                .tasks
                .done_pass
                .saturating_sub(report.tasks.evidence_current + report.tasks.evidence_stale);
            let baseline = if let Some(since) = since {
                Some(since)
            } else if let Some(previous) = latest.get(1) {
                store
                    .verification_run_context(&key, &previous.id)?
                    .and_then(|c| c.provenance.task_pin.map(|p| (p.plan_id, p.event_sequence)))
            } else {
                None
            };
            if let Some((plan, event)) = baseline {
                let before = store.task_metadata_session(&key, Some(plan), Some(event))?;
                let progress = dctx0_tasks::task_progress(&before, session)?;
                report.progress = Some(StatusProgress {
                    baseline: WorkPin::of(&before),
                    newly_done: progress.newly_done.len(),
                    newly_pass: progress.newly_pass.len(),
                    added_done: progress.added_done.len(),
                    added_pass: progress.added_pass.len(),
                    lost_pass: progress.lost_pass.len(),
                });
            }
            report.concurrent_change = store.task_sequence(&key)? != session.event_seq
                || store.task_head(&key)? != Some(session.plan.id);
        } else if since.is_some() {
            return Err(Error::Invalid(
                "Progress baseline requires a published task plan".into(),
            ));
        }
        let mut causes = vec![];
        let mut cause = |condition, code: &str, affected, step: &str| {
            if condition {
                causes.push(StatusCause {
                    code: code.into(),
                    affected,
                    next_step: step.into(),
                })
            }
        };
        for problem in &report.issues {
            cause(
                true,
                &problem.code,
                report.tasks.total,
                "Restore readable configuration/source inputs; source freshness is unknown, while saved task counts remain available",
            );
        }
        cause(
            spec_current != Some(true),
            "SPEC_CHECK_REQUIRED",
            report.tasks.total,
            "Publish a successful check for current specification sources",
        );
        cause(
            report.task_plan_current != Some(true),
            "TASK_PLAN_REQUIRED",
            report.tasks.total,
            "Publish tasks plan against the latest successful check",
        );
        if let Some(run) = &report.latest_run {
            cause(
                !run.summary.details_available,
                "EVIDENCE_DETAILS_PRUNED",
                report.tasks.evidence_unknown,
                "Only historical counts remain; fetch retained evidence or gather new native evidence after preflight",
            );
            cause(
                run.sources_current == Some(false),
                "IMPLEMENTATION_CHANGED",
                report.tasks.evidence_stale,
                "Run verify preflight against the changed source inventory before gathering fresh evidence",
            );
            cause(
                run.task_pin_present == Some(false),
                "TASK_PIN_REQUIRED",
                report.tasks.open,
                "Select matching task IDs or scope; use verify preflight before a delivery run",
            );
            if let Some(diagnosis) =
                dctx0_verification::diagnostics::diagnose_result("latest run", &run.summary.result)
            {
                cause(
                    true,
                    &diagnosis.issue.code,
                    (run.summary.outcomes[1] + run.summary.outcomes[2]) as usize,
                    &diagnosis.next_step,
                );
            }
            cause(
                run.summary.missing_case_count > 0,
                "MANDATORY_CASES_MISSING",
                run.summary.missing_case_count,
                "Inspect verify show --summary, then filter --cause adapter,environment; repair the reported interruption",
            );
            cause(
                run.summary.outcomes[1] > 0,
                "NATIVE_FAILURE",
                run.summary.outcomes[1] as usize,
                "Inspect verify show --outcome FAIL and repair actual production behavior",
            );
            cause(
                run.summary.outcomes[2] > 0,
                "NATIVE_UNKNOWN",
                run.summary.outcomes[2] as usize,
                "Inspect verify show --outcome UNKNOWN for missing bindings, observations or environmental prerequisites",
            );
            cause(
                run.summary.open_obligation_count > 0,
                "OPEN_OBLIGATIONS",
                run.summary.open_obligation_count,
                "Inspect verify show --summary; supply the required observations or authored source reviews",
            );
        } else {
            cause(
                true,
                "NATIVE_EVIDENCE_REQUIRED",
                report.tasks.open,
                "Run verify preflight before native verification",
            );
        }
        cause(
            report.tasks.blocked > 0,
            "TASK_PREREQUISITES",
            report.tasks.blocked,
            "Use tasks next to inspect dependency-ready work and deliver its prerequisites",
        );
        cause(
            report.concurrent_change,
            "CONCURRENT_CHANGE",
            report.tasks.total,
            "Task events changed during this read-only status; query again",
        );
        report.next_causes = causes.into_iter().take(3).collect();
        Ok(report)
    }
}
