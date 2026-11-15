//! Private native-execution publication path. Imported reports never invoke it.
use super::*;
use dctx0_store::{TaskDelivery, TaskDeliveryPublication};
use dctx0_target_sdk::AdapterDescription;
use dctx0_tasks::{
    AdoptionProposal, AdoptionTaskProof, CheckEvidence, CheckStatus, TaskKind, TaskResult,
    WorkState, propose_adoption, task_case_coverage,
};
/// Capture state versions before either hybrid backend starts. The store will
/// recheck these together with the plan, profile and event pins on publication.
pub(super) fn capture_done_versions(
    project: &Project,
    pin: Option<&VerificationTaskPin>,
) -> Result<BTreeMap<i64, i64>> {
    let Some(pin) = pin else {
        return Ok(BTreeMap::new());
    };
    let session = project.task_metadata_session(None, None)?;
    if session.plan.id != pin.plan_id || session.event_seq != pin.event_sequence {
        return Err(Error::Invalid(
            "Task plan or event changed while capturing hybrid task versions".into(),
        ));
    }
    let mut versions = BTreeMap::new();
    for (id, requirements) in &pin.requirements {
        let member = session.member(*id)?;
        if &member.candidate.requirements_hash != requirements {
            return Err(Error::Invalid(format!(
                "T{id}: Requirements changed while capturing hybrid task versions"
            )));
        }
        let state = &session.states[id];
        if matches!(
            member.candidate.kind,
            TaskKind::Contract | TaskKind::Operation
        ) && !session.definitions[id].is_adjustment()
            && state.work == WorkState::Done
            && state.check == CheckStatus::Unverified
            && state.result.is_some()
            && state.actor.is_none()
            && state.claim.is_none()
            && state.reservation.is_none()
        {
            versions.insert(*id, state.version);
        }
    }
    Ok(versions)
}

/// The caller owns the project lock and has checked actual source and runtime
/// freshness. Done versions must have been captured before either backend ran.
pub(super) fn publish_hybrid(
    project: &Project,
    application: &Path,
    plan: &ProgramPlan,
    saved: &StoredVerificationRun,
    artifacts: &[VerificationArtifact],
    done_versions: &BTreeMap<i64, i64>,
) -> Result<(Vec<i64>, Vec<VerificationIssue>, Option<AdoptionProposal>)> {
    evaluate_inner(
        project,
        application,
        plan,
        saved,
        artifacts,
        false,
        Some(done_versions),
        false,
    )
}

pub(super) fn publish(
    project: &Project,
    application: &Path,
    plan: &ProgramPlan,
    saved: &StoredVerificationRun,
    artifacts: &[VerificationArtifact],
) -> Result<(Vec<i64>, Vec<VerificationIssue>, Option<AdoptionProposal>)> {
    evaluate(project, application, plan, saved, artifacts, false)
}
pub(super) fn publish_incremental(
    project: &Project,
    application: &Path,
    plan: &ProgramPlan,
    saved: &StoredVerificationRun,
    artifacts: &[VerificationArtifact],
    done_versions: &BTreeMap<i64, i64>,
) -> Result<(Vec<i64>, Vec<VerificationIssue>, Option<AdoptionProposal>)> {
    // Empty completed-task pins do not request a Done/Unverified transition.
    // Fresh Open-task delivery keeps its existing task-specific proof rules;
    // unrelated manual UNKNOWN obligations retain their original status.
    let done_versions = (!done_versions.is_empty()).then_some(done_versions);
    evaluate_inner(
        project,
        application,
        plan,
        saved,
        artifacts,
        false,
        done_versions,
        true,
    )
}
pub(super) fn evaluate(
    project: &Project,
    application: &Path,
    plan: &ProgramPlan,
    saved: &StoredVerificationRun,
    artifacts: &[VerificationArtifact],
    preview: bool,
) -> Result<(Vec<i64>, Vec<VerificationIssue>, Option<AdoptionProposal>)> {
    evaluate_inner(
        project,
        application,
        plan,
        saved,
        artifacts,
        preview,
        None,
        false,
    )
}

#[allow(clippy::too_many_arguments)] // Publication pins and delivery policy are independent boundaries.
fn evaluate_inner(
    project: &Project,
    application: &Path,
    plan: &ProgramPlan,
    saved: &StoredVerificationRun,
    artifacts: &[VerificationArtifact],
    preview: bool,
    done_versions: Option<&BTreeMap<i64, i64>>,
    incremental_delivery: bool,
) -> Result<(Vec<i64>, Vec<VerificationIssue>, Option<AdoptionProposal>)> {
    if saved.run.origin != VerificationRunOrigin::Native {
        return Ok((
            vec![],
            vec![issue(
                "ADOPTION_NATIVE_ORIGIN_REQUIRED",
                "Imported evidence cannot authorize automatic task delivery",
            )],
            None,
        ));
    }
    let run = &saved.run;
    let Some(pin) = &run.provenance.task_pin else {
        return Ok((
            vec![],
            vec![issue(
                "ADOPTION_TASK_PIN_REQUIRED",
                "No task requirements were pinned before actual native execution",
            )],
            None,
        ));
    };
    if (done_versions.is_some() && run.result != VerificationResult::Pass)
        || !run.permits_task_specific_proof()
        || run
            .cases
            .iter()
            .any(|case| case.result != VerificationResult::Pass)
    {
        let mut incomplete = run.provenance.incomplete.clone();
        for id in pin.requirements.keys() {
            if pin.profile_bundles.get(id).is_none_or(String::is_empty) {
                incomplete.push(issue(
                    "TASK_PROFILE_UNPINNED",
                    format!("T{id}: No selected profile bundle is pinned in this proof"),
                ));
            }
        }
        incomplete.push(issue(
            "ADOPTION_COMPLETE_RUN_REQUIRED",
            "Automatic publication requires every mandatory native case PASS; only unrelated non-executable review/suite obligations may remain, and each task still requires complete mapped evidence",
        ));
        return Ok((vec![], incomplete, None));
    }
    let manifest = dctx0_verification::case_manifest(plan);
    let actual_ids = run
        .cases
        .iter()
        .map(|case| case.id.clone())
        .collect::<BTreeSet<_>>();
    if run.required_cases.is_empty()
        || run
            .open_obligations
            .iter()
            .any(|issue| !manifest.open_obligations.contains(issue))
        || actual_ids.len() != run.cases.len()
        || actual_ids != manifest.required_cases
        || run.required_cases != manifest.required_cases
    {
        return Ok((
            vec![],
            vec![issue(
                "ADOPTION_CASE_MANIFEST_MISMATCH",
                "Delivered cases do not exactly match the central mandatory case manifest",
            )],
            None,
        ));
    }
    let session = if done_versions.is_some() {
        project.task_metadata_session(None, None)?
    } else {
        project.task_session(None, None)?
    };
    if preview
        && (session.plan.id != pin.plan_id
            || session.event_seq != pin.event_sequence
            || session.plan.snapshot_id != run.provenance.snapshot
            || session.plan.check_run_id != run.provenance.check_occurrence)
    {
        return Ok((
            vec![],
            vec![issue(
                "ADOPTION_STALE_TASK_PIN",
                "Plan, check occurrence or task events changed since native execution",
            )],
            None,
        ));
    }
    let mut changed_profiles = BTreeSet::new();
    if preview {
        let profiles = match project.profiles(Some(application), Some(&run.provenance.target)) {
            Ok(Some(profiles)) => profiles,
            Ok(None) => {
                return Ok((
                    vec![],
                    vec![issue(
                        "ADOPTION_PROFILE_UNAVAILABLE",
                        "Selected profiles are unavailable",
                    )],
                    None,
                ));
            }
            Err(error) => {
                return Ok((
                    vec![],
                    vec![issue("ADOPTION_PROFILE_CHANGED", error.to_string())],
                    None,
                ));
            }
        };
        let snapshot = project.load(&run.provenance.snapshot)?;
        for id in pin.requirements.keys() {
            let Some(member) = session.plan.members.iter().find(|m| m.task_id == *id) else {
                continue;
            };
            let selection = dctx0_tasks::select_profiles(
                &snapshot,
                &member.candidate,
                &profiles.manifest,
                &profiles.documents,
            );
            if !selection.missing_files.is_empty()
                || pin.profile_bundles.get(id) != Some(&selection.bundle.hash)
            {
                changed_profiles.insert(*id);
            }
        }
    }
    let description = artifacts
        .iter()
        .find(|a| a.reference.media_type == "application/vnd.dctx0.adapter-description+json")
        .map(|a| serde_json::from_slice::<AdapterDescription>(&a.contents))
        .transpose()?;
    let application_proof = artifacts
        .iter()
        .find(|artifact| {
            artifact.reference.media_type
                == dctx0_verification::application_evidence::APPLICATION_EXECUTION_MEDIA_TYPE
        })
        .map(|artifact| {
            serde_json::from_slice::<
                dctx0_verification::application_evidence::ApplicationExecutionProof,
            >(&artifact.contents)
        })
        .transpose()?;
    let incremental_proof = artifacts
        .iter()
        .find(|artifact| {
            artifact.reference.media_type
                == dctx0_verification::incremental::INCREMENTAL_PROOF_MEDIA_TYPE
        })
        .map(|artifact| {
            serde_json::from_slice::<dctx0_verification::incremental::IncrementalProof>(
                &artifact.contents,
            )
        })
        .transpose()?;
    let incremental_application = artifacts.iter().find(|artifact| artifact.reference.media_type == dctx0_verification::application_evidence::INCREMENTAL_APPLICATION_EXECUTION_MEDIA_TYPE)
        .map(|artifact| serde_json::from_slice::<dctx0_verification::application_evidence::IncrementalApplicationExecutionProof>(&artifact.contents)).transpose()?;
    let incremental_hybrid = artifacts
        .iter()
        .find(|artifact| {
            artifact.reference.media_type
                == dctx0_verification::hybrid_evidence::INCREMENTAL_HYBRID_EXECUTION_MEDIA_TYPE
        })
        .map(|artifact| {
            serde_json::from_slice::<
                dctx0_verification::hybrid_evidence::IncrementalHybridExecutionProof,
            >(&artifact.contents)
        })
        .transpose()?;
    let application_proof = application_proof.as_ref().or_else(|| {
        incremental_application
            .as_ref()
            .filter(|proof| {
                incremental_proof
                    .as_ref()
                    .is_some_and(|incremental| proof.validates(run, incremental))
            })
            .map(|proof| &proof.application)
    });
    let hybrid_proof = artifacts
        .iter()
        .find(|artifact| {
            artifact.reference.media_type
                == dctx0_verification::hybrid_evidence::HYBRID_EXECUTION_MEDIA_TYPE
        })
        .map(|artifact| {
            serde_json::from_slice::<dctx0_verification::hybrid_evidence::HybridExecutionProof>(
                &artifact.contents,
            )
        })
        .transpose()?;
    let hybrid_proof = hybrid_proof
        .as_ref()
        .filter(|proof| proof.validates(run))
        .or_else(|| {
            incremental_hybrid
                .as_ref()
                .filter(|proof| {
                    incremental_proof
                        .as_ref()
                        .is_some_and(|incremental| proof.validates(run, incremental))
                })
                .map(|proof| &proof.cohort)
        });
    if done_versions.is_some() && !incremental_delivery && hybrid_proof.is_none() {
        return Ok((
            vec![],
            vec![issue(
                "HYBRID_COMPLETE_PROOF_REQUIRED",
                "Hybrid publication requires the fresh complete cohort proof",
            )],
            None,
        ));
    }
    let inspected_batch = artifacts
        .iter()
        .find(|artifact| artifact.reference.media_type == dctx0_tasks::TASK_INSPECTIONS_MEDIA_TYPE)
        .map(|artifact| {
            serde_json::from_slice::<dctx0_tasks::InspectedDoneBatch>(&artifact.contents)
        })
        .transpose()?;
    if inspected_batch.is_some() && done_versions.is_none() {
        return Ok((
            vec![],
            vec![issue(
                "INSPECTED_HYBRID_REQUIRED",
                "Inspected tasks require a fresh hybrid run",
            )],
            None,
        ));
    }
    let mut native_requirements = BTreeSet::new();
    let mut adapter_requirements = BTreeSet::new();
    if let Some(proof) = hybrid_proof {
        for case in &run.cases {
            if case.result != VerificationResult::Pass {
                continue;
            }
            if let Some(requirements) = manifest.case_requirements.get(&case.id) {
                match proof.application.plan.case_groups.get(&case.id) {
                    Some(group) if group == &proof.application.assessment.group => {
                        native_requirements.extend(requirements.iter().cloned())
                    }
                    Some(group) if group == &proof.adapter_group => {
                        adapter_requirements.extend(requirements.iter().cloned())
                    }
                    _ => (),
                }
            }
        }
    }
    let outcomes = run
        .cases
        .iter()
        .map(|case| (case.id.clone(), case.result.clone()))
        .collect::<BTreeMap<_, _>>();
    let identities = run
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
    let implementation = dctx0_changes::hash(&(&run.provenance, identities));
    let mut proofs = BTreeMap::new();
    for (id, requirements_hash) in &pin.requirements {
        let member = session.member(*id)?;
        let context_requirements = member
            .candidate
            .facts
            .iter()
            .filter(|id| {
                plan.manual_checks
                    .iter()
                    .any(|check| check.subject == **id && check.kind == StatementKind::Intent)
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        let requirements = member
            .candidate
            .subjects
            .iter()
            .chain(&member.candidate.types)
            .chain(&member.candidate.facts)
            .filter(|id| !context_requirements.contains(*id))
            .cloned()
            .collect::<BTreeSet<_>>();
        let coverage = task_case_coverage(&requirements, &manifest.case_requirements, &outcomes);
        let mut incomplete = run
            .provenance
            .incomplete
            .iter()
            .map(|p| p.code.clone())
            .collect::<Vec<_>>();
        if changed_profiles.contains(id) {
            incomplete.push("ADOPTION_PROFILE_CHANGED".into());
        }
        incomplete.extend(
            coverage
                .missing_requirements
                .iter()
                .map(|id| format!("REQUIREMENT_MAPPING:{id}")),
        );
        incomplete.extend(
            coverage
                .unexecuted_cases
                .iter()
                .map(|id| format!("UNEXECUTED_CASE:{id}")),
        );
        if coverage.result != VerificationResult::Pass {
            incomplete.push("TASK_CASES_NOT_PASS".into())
        }
        let mut actual_artifacts = BTreeSet::new();
        let mut representations = description
            .as_ref()
            .map(|d| d.representations.clone())
            .unwrap_or_default();
        if let Some(proof) = &application_proof {
            if proof.validates(run) {
                // The whole captured application, native tests and dependencies
                // jointly implement this finite proof. Do not fabricate an old
                // adapter description or claim a per-file semantic analysis.
                actual_artifacts.extend(proof.source_inputs.keys().cloned());
                representations.insert(
                    "application-test-protocol".into(),
                    dctx0_target_sdk::application_protocol::APPLICATION_PROTOCOL_VERSION
                        .to_string(),
                );
                representations.insert(
                    "assessment".into(),
                    proof.assessment.assessment_version.clone(),
                );
            } else {
                incomplete.push("APPLICATION_NATIVE_PROOF_INVALID".into());
            }
        } else {
            for subject in &member.candidate.subjects {
                let native_covered = native_requirements.contains(subject);
                if native_covered {
                    let proof = hybrid_proof.expect("native requirements require a hybrid proof");
                    actual_artifacts.extend(proof.application.source_inputs.keys().cloned());
                    representations.insert(
                        "application-test-protocol".into(),
                        dctx0_target_sdk::application_protocol::APPLICATION_PROTOCOL_VERSION
                            .to_string(),
                    );
                    representations.insert(
                        "assessment".into(),
                        proof.application.assessment.assessment_version.clone(),
                    );
                }
                if !native_covered || adapter_requirements.contains(subject) {
                    if let Some(entry) = description.as_ref().and_then(|description| {
                        description
                            .subjects
                            .iter()
                            .find(|entry| &entry.subject == subject)
                    }) {
                        if run.provenance.implementation.contains_key(&entry.artifact) {
                            actual_artifacts.insert(entry.artifact.clone());
                        } else {
                            incomplete
                                .push(format!("UNCAPTURED_IMPLEMENTATION:{}", entry.artifact));
                        }
                    } else {
                        incomplete.push(format!("ADAPTER_SUBJECT_UNAVAILABLE:{subject}"));
                    }
                }
            }
        }
        let executed_cases = coverage
            .required_cases
            .iter()
            .filter_map(|id| outcomes.get(id).map(|result| (id.clone(), result.clone())))
            .collect();
        proofs.insert(
            *id,
            AdoptionTaskProof {
                requirements_hash: requirements_hash.clone(),
                context_requirements,
                covered_requirements: requirements
                    .difference(&coverage.missing_requirements)
                    .cloned()
                    .collect(),
                required_cases: coverage.required_cases,
                executed_cases,
                implementation: implementation.clone(),
                artifacts: actual_artifacts.into_iter().collect(),
                representations,
                profile_bundle: pin.profile_bundles.get(id).cloned().unwrap_or_default(),
                incomplete,
            },
        );
    }
    // Preserve historical Done prerequisites while validating current versions.
    // Each eligible Done candidate is checked separately below using the same
    // pure adoption rules; no stored task state changes during the proposal.
    if let Some(done_versions) = done_versions {
        for (id, version) in done_versions {
            let member = session.member(*id)?;
            let state = &session.states[id];
            if pin.requirements.get(id) != Some(&member.candidate.requirements_hash)
                || state.version != *version
                || state.work != WorkState::Done
                || state.check != CheckStatus::Unverified
                || state.result.is_none()
                || state.actor.is_some()
                || state.claim.is_some()
                || state.reservation.is_some()
                || !(matches!(
                    member.candidate.kind,
                    TaskKind::Contract | TaskKind::Operation
                ) || (member.candidate.kind == TaskKind::Adapter
                    && inspected_batch
                        .as_ref()
                        .is_some_and(|batch| batch.tasks.contains_key(id))))
                || session.definitions[id].is_adjustment()
            {
                return Ok((
                    vec![],
                    vec![issue(
                        "ADOPTION_STALE_TASK_PIN",
                        format!("T{id}: Captured completed task changed"),
                    )],
                    None,
                ));
            }
        }
    }
    let mut proposal = match propose_adoption(
        &session,
        &run.provenance.snapshot,
        run.provenance.check_occurrence,
        pin,
        &proofs,
    ) {
        Ok(proposal) => proposal,
        Err(error) => {
            return Ok((
                vec![],
                vec![issue("ADOPTION_STALE_TASK_PIN", error.to_string())],
                None,
            ));
        }
    };
    if let Some(done_versions) = done_versions {
        // Open work must continue to see historical Done prerequisites as
        // delivered even when those prerequisites cannot receive new evidence.
        // Restrict each temporary proposal to one Done candidate for the same
        // reason; all other historical results retain their real state.
        let mut temporary = session.clone();
        for id in done_versions.keys() {
            if inspected_batch
                .as_ref()
                .is_some_and(|batch| batch.tasks.contains_key(id))
            {
                continue;
            }
            temporary.states.get_mut(id).unwrap().work = WorkState::Open;
            let single_pin = VerificationTaskPin {
                plan_id: pin.plan_id,
                event_sequence: pin.event_sequence,
                requirements: [(*id, pin.requirements[id].clone())].into(),
                profile_bundles: pin
                    .profile_bundles
                    .get(id)
                    .map(|hash| [(*id, hash.clone())].into())
                    .unwrap_or_default(),
            };
            let single_proofs = [(*id, proofs[id].clone())].into();
            let result = propose_adoption(
                &temporary,
                &run.provenance.snapshot,
                run.provenance.check_occurrence,
                &single_pin,
                &single_proofs,
            );
            temporary.states.get_mut(id).unwrap().work = WorkState::Done;
            let single = match result {
                Ok(value) => value,
                Err(error) => {
                    return Ok((
                        vec![],
                        vec![issue("ADOPTION_STALE_TASK_PIN", error.to_string())],
                        None,
                    ));
                }
            };
            proposal.blocked.retain(|blocker| blocker.task_id != *id);
            proposal.tasks.extend(single.tasks);
            proposal.blocked.extend(single.blocked);
        }
    }
    if preview {
        for blocker in &mut proposal.blocked {
            if changed_profiles.contains(&blocker.task_id)
                && blocker.code == "INCOMPLETE_REQUIREMENTS"
            {
                blocker.code = "ADOPTION_PROFILE_CHANGED".into();
                blocker.reason = "Selected profile bundle changed or is incomplete".into();
            }
        }
        return Ok((vec![], vec![], Some(proposal)));
    }
    let mut issues = proposal
        .blocked
        .iter()
        .map(|b| issue(&b.code, format!("T{}: {}", b.task_id, b.reason)))
        .collect::<Vec<_>>();
    if proposal.tasks.is_empty()
        && inspected_batch
            .as_ref()
            .is_none_or(|batch| batch.tasks.is_empty())
    {
        return Ok((vec![], issues, Some(proposal)));
    }
    let profiles = match project.profiles(Some(application), Some(&run.provenance.target)) {
        Ok(profiles) => profiles,
        Err(error) => {
            issues.push(issue("ADOPTION_PROFILE_CHANGED", error.to_string()));
            return Ok((vec![], issues, Some(proposal)));
        }
    };
    let Some(profiles) = profiles else {
        return Ok((
            vec![],
            vec![issue(
                "ADOPTION_PROFILE_UNAVAILABLE",
                "Profile bundle unavailable at publication",
            )],
            Some(proposal),
        ));
    };
    let deliveries = proposal
        .tasks
        .iter()
        .filter(|id| done_versions.is_none_or(|versions| !versions.contains_key(id)))
        .map(|id| {
            let proof = &proofs[id];
            TaskDelivery {
                task: *id,
                result: TaskResult {
                    summary: if incremental_proof.is_some() {
                        "Native verification and validated historical cases delivered the pinned requirements".into()
                    } else {
                        "Actual native specification verification delivered the pinned requirements".into()
                    },
                    artifacts: proof.artifacts.clone(),
                    decisions: vec![if incremental_proof.is_some() {
                        "PASS is limited to the finite plan and includes validated historical cases; reused cases are not fresh observations".into()
                    } else { "PASS is limited to the executed finite mandatory plan".into() }],
                    implementation: Some(proof.implementation.clone()),
                },
                evidence: CheckEvidence {
                    requirements_hash: proof.requirements_hash.clone(),
                    implementation: proof.implementation.clone(),
                    verifier: "dctx0-native-verifier".into(),
                    verifier_version: dctx0_verification::VERIFICATION_VERSION.into(),
                    command: if application_proof.is_some() {
                        "dctx0 verify application-tests run"
                    } else {
                        "dctx0 verify run"
                    }
                    .into(),
                    started_at: run.started_at_unix_ns.clone(),
                    finished_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|v| v.as_nanos().to_string())
                        .unwrap_or_else(|_| run.started_at_unix_ns.clone()),
                    report: saved.id.clone(),
                    status: CheckStatus::Pass,
                },
            }
        })
        .collect::<Vec<_>>();
    // The caller already owns the project lock. Store rechecks the exact latest
    // occurrence, plan, event watermark, claims and profile hashes atomically.
    let mut store = project.store(AccessMode::ReadWrite)?;
    let request = TaskDeliveryPublication {
        actor: "dctx0-verifier",
        pin,
        deliveries: &deliveries,
        profiles: (&profiles.manifest, &profiles.documents),
        run_id: Some(&saved.id),
    };
    let mut selected_done = proposal
        .tasks
        .iter()
        .filter_map(|id| {
            done_versions.and_then(|versions| versions.get(id).map(|version| (*id, *version)))
        })
        .collect::<BTreeMap<_, _>>();
    if let Some(batch) = &inspected_batch {
        for id in batch.tasks.keys() {
            let Some(version) = done_versions.and_then(|versions| versions.get(id)) else {
                return Ok((
                    vec![],
                    vec![issue(
                        "INSPECTED_STATE_NOT_CAPTURED",
                        format!("T{id}: state was not captured before execution"),
                    )],
                    Some(proposal),
                ));
            };
            selected_done.insert(*id, *version);
        }
    }
    let publication = if selected_done.is_empty() {
        store.publish_task_deliveries(&project.key(), request)
    } else if inspected_batch.is_some() {
        store.publish_inspected_native_task_batch(&project.key(), request, &selected_done)
    } else {
        store.publish_native_task_batch(&project.key(), request, &selected_done)
    };
    match publication {
        Ok(_) => {
            let mut published = proposal.tasks.clone();
            if let Some(batch) = inspected_batch {
                for id in batch.tasks.keys() {
                    if !published.contains(id) {
                        published.push(*id);
                    }
                    issues.retain(|entry| !entry.message.starts_with(&format!("T{id}:")));
                    proposal.blocked.retain(|blocker| blocker.task_id != *id);
                    if !proposal.tasks.contains(id) {
                        proposal.tasks.push(*id);
                    }
                }
            }
            Ok((published, issues, Some(proposal)))
        }
        Err(error) => {
            issues.push(issue("ADOPTION_PUBLICATION_REJECTED", error.to_string()));
            Ok((vec![], issues, Some(proposal)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_missing_pin_and_incomplete_native_proofs_never_open_task_history() {
        let root =
            std::env::temp_dir().join(format!("dctx0-adoption-rejections-{}", std::process::id()));
        assert!(!root.exists());
        let project = Project {
            root: root.clone(),
            database: root.join("state.sqlite3"),
        };
        let ir = dctx0_language::compile(&[Source {
            file: "w.dctx0".into(),
            text: "workspace W { value A = Int { range 1..2 } }".into(),
        }])
        .ir
        .unwrap();
        let plan = dctx0_verification::project_program(
            &ir,
            "snapshot",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["W.A".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let run: VerificationRun = serde_json::from_value(serde_json::json!({
            "schema_version":1,"origin":"imported","started_at_unix_ns":"1","elapsed_us":0,
            "provenance":{"snapshot":"snapshot","check_occurrence":1,"plan_hash":"plan","implementation":{},"dependencies":{},"adapter_hash":"adapter","configuration_hash":"config","harness_hash":"harness","interpreter_version":"1","generator_version":"1","protocol_version":1,"target":"typescript","runtime_version":"bun","build_configuration":{},"task_pin":null,"incomplete":[]},
            "budgets":VerificationBudgets::default(),"selected_subjects":["W.A"],"required_subjects":["W.A"],"not_selected":[],"required_cases":[],"cases":[],"open_obligations":[],"bounded_obligations":[],"artifacts":[],"result":{"status":"PASS"}
        })).unwrap();
        let mut saved = StoredVerificationRun {
            id: "run".into(),
            created_at: "now".into(),
            run,
        };
        let (adopted, issues, proposal) = publish(&project, &root, &plan, &saved, &[]).unwrap();
        assert!(adopted.is_empty() && proposal.is_none());
        assert_eq!(issues[0].code, "ADOPTION_NATIVE_ORIGIN_REQUIRED");
        saved.run.origin = VerificationRunOrigin::Native;
        let (_, issues, _) = publish(&project, &root, &plan, &saved, &[]).unwrap();
        assert_eq!(issues[0].code, "ADOPTION_TASK_PIN_REQUIRED");
        saved.run.provenance.task_pin = Some(VerificationTaskPin {
            plan_id: 1,
            event_sequence: 0,
            requirements: [(1, "requirements".into())].into(),
            profile_bundles: Default::default(),
        });
        for status in ["FAIL", "UNKNOWN"] {
            saved.run.result = serde_json::from_value(serde_json::json!({"status":status,"issue":{"code":"INCOMPLETE","message":"Not a complete proof","span":null}})).unwrap();
            let (adopted, issues, proposal) = publish(&project, &root, &plan, &saved, &[]).unwrap();
            assert!(adopted.is_empty() && proposal.is_none());
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.code == "TASK_PROFILE_UNPINNED")
            );
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.code == "ADOPTION_COMPLETE_RUN_REQUIRED")
            );
        }
        let unknown = saved.run.result.clone();
        saved.run.result = VerificationResult::Pass;
        saved.run.cases.push(VerificationCaseRecord {
            id: "W.A:value:0".into(),
            subject: "W.A".into(),
            kind: VerificationCaseKind::Contract,
            bindings: Default::default(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: unknown,
            elapsed_us: 0,
        });
        let (adopted, issues, proposal) = publish(&project, &root, &plan, &saved, &[]).unwrap();
        assert!(adopted.is_empty() && proposal.is_none());
        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "ADOPTION_COMPLETE_RUN_REQUIRED")
        );
        saved.run.cases.clear();
        saved.run.open_obligations.push(issue(
            "MANUAL_REQUIREMENT",
            "An explicit review remains open",
        ));
        let (adopted, issues, proposal) = publish(&project, &root, &plan, &saved, &[]).unwrap();
        assert!(adopted.is_empty() && proposal.is_none());
        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "ADOPTION_COMPLETE_RUN_REQUIRED")
        );
        saved.run.open_obligations.clear();
        let manifest = dctx0_verification::case_manifest(&plan);
        saved.run.required_cases = manifest.required_cases.clone();
        for corruption in 0..3 {
            saved.run.cases = manifest
                .required_cases
                .iter()
                .map(|id| VerificationCaseRecord {
                    id: id.clone(),
                    subject: "W.A".into(),
                    kind: VerificationCaseKind::Contract,
                    bindings: Default::default(),
                    setup: vec![],
                    arguments: vec![],
                    observations: vec![],
                    events: None,
                    resources: None,
                    states: vec![],
                    result: VerificationResult::Pass,
                    elapsed_us: 0,
                })
                .collect();
            match corruption {
                0 => {
                    saved.run.cases.pop();
                }
                1 => {
                    saved.run.cases.push(saved.run.cases[0].clone());
                }
                _ => {
                    saved.run.required_cases.clear();
                }
            }
            let (adopted, issues, proposal) = publish(&project, &root, &plan, &saved, &[]).unwrap();
            assert!(adopted.is_empty() && proposal.is_none());
            assert_eq!(issues[0].code, "ADOPTION_CASE_MANIFEST_MISMATCH");
        }
        assert!(
            !root.exists(),
            "Rejected proofs must not create task history or artifacts"
        );
    }
    #[test]
    fn hybrid_done_versions_are_captured_only_from_matching_current_task_state() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("dctx0-done-pin-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("w.dctx0"),
            "workspace W { value A = Int { range 1..2 } value B = Int { range 1..2 } }",
        )
        .unwrap();
        let project = Project::new(&root, None).unwrap();
        assert!(project.check().unwrap().ok);
        project.task_plan("latest").unwrap();
        let session = project.task_session(None, None).unwrap();
        let id = session.next(&Default::default(), None).unwrap().task_id;
        let started = project
            .task_action(dctx0_tasks::Action::Start {
                task: id,
                actor: "author".into(),
                reservation: None,
            })
            .unwrap()
            .state
            .unwrap();
        let completed = project
            .task_action(dctx0_tasks::Action::Complete {
                task: id,
                actor: "author".into(),
                claim: started.claim.unwrap(),
                version: started.version,
                result: TaskResult {
                    summary: "Original".into(),
                    artifacts: vec![],
                    decisions: vec![],
                    implementation: None,
                },
            })
            .unwrap()
            .state
            .unwrap();
        let session = project.task_session(None, None).unwrap();
        let pin = VerificationTaskPin {
            plan_id: session.plan.id,
            event_sequence: session.event_seq,
            requirements: session
                .plan
                .members
                .iter()
                .map(|m| (m.task_id, m.candidate.requirements_hash.clone()))
                .collect(),
            profile_bundles: Default::default(),
        };
        assert_eq!(
            capture_done_versions(&project, Some(&pin)).unwrap(),
            [(id, completed.version)].into()
        );
        assert!(capture_done_versions(&project, None).unwrap().is_empty());
        let mut wrong = pin.clone();
        wrong.requirements.insert(id, "changed".into());
        assert!(capture_done_versions(&project, Some(&wrong)).is_err());
        project
            .task_action(dctx0_tasks::Action::Note {
                task: id,
                note: "later event".into(),
            })
            .unwrap();
        assert!(capture_done_versions(&project, Some(&pin)).is_err());
        assert_eq!(
            project.task_session(None, None).unwrap().states[&id].work,
            WorkState::Done
        );
        fs::remove_dir_all(root).unwrap();
    }
}
