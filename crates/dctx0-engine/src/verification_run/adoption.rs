//! Private native-execution publication path. Imported reports never invoke it.
use super::*;
use dctx0_store::{TaskDelivery, TaskDeliveryPublication};
use dctx0_target_sdk::AdapterDescription;
use dctx0_tasks::{
    AdoptionProposal, AdoptionTaskProof, CheckEvidence, CheckStatus, TaskResult, propose_adoption,
    task_case_coverage,
};
pub(super) fn publish(
    project: &Project,
    application: &Path,
    plan: &ProgramPlan,
    saved: &StoredVerificationRun,
    artifacts: &[VerificationArtifact],
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
    if !run.permits_task_specific_proof()
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
    let session = project.task_session(None, None)?;
    let description = artifacts
        .iter()
        .find(|a| a.reference.media_type == "application/vnd.dctx0.adapter-description+json")
        .map(|a| serde_json::from_slice::<AdapterDescription>(&a.contents))
        .transpose()?;
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
        let representations = description
            .as_ref()
            .map(|d| d.representations.clone())
            .unwrap_or_default();
        if let Some(description) = &description {
            for subject in &member.candidate.subjects {
                if let Some(entry) = description
                    .subjects
                    .iter()
                    .find(|entry| &entry.subject == subject)
                {
                    if run.provenance.implementation.contains_key(&entry.artifact) {
                        actual_artifacts.insert(entry.artifact.clone());
                    } else {
                        incomplete.push(format!("UNCAPTURED_IMPLEMENTATION:{}", entry.artifact));
                    }
                } else {
                    incomplete.push(format!("ADAPTER_SUBJECT_UNAVAILABLE:{subject}"));
                }
            }
        } else {
            incomplete.push("ADAPTER_DESCRIPTION_UNAVAILABLE".into())
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
    let proposal = match propose_adoption(
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
    let mut issues = proposal
        .blocked
        .iter()
        .map(|b| issue(&b.code, format!("T{}: {}", b.task_id, b.reason)))
        .collect::<Vec<_>>();
    if proposal.tasks.is_empty() {
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
        .map(|id| {
            let proof = &proofs[id];
            TaskDelivery {
                task: *id,
                result: TaskResult {
                    summary:
                        "Actual native specification verification delivered the pinned requirements"
                            .into(),
                    artifacts: proof.artifacts.clone(),
                    decisions: vec!["PASS is limited to the executed finite mandatory plan".into()],
                    implementation: Some(proof.implementation.clone()),
                },
                evidence: CheckEvidence {
                    requirements_hash: proof.requirements_hash.clone(),
                    implementation: proof.implementation.clone(),
                    verifier: "dctx0-native-verifier".into(),
                    verifier_version: dctx0_verification::VERIFICATION_VERSION.into(),
                    command: "dctx0 verify run".into(),
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
    match project
        .store(AccessMode::ReadWrite)?
        .publish_task_deliveries(
            &project.key(),
            TaskDeliveryPublication {
                actor: "dctx0-verifier",
                pin,
                deliveries: &deliveries,
                profiles: (&profiles.manifest, &profiles.documents),
                run_id: Some(&saved.id),
            },
        ) {
        Ok(_) => Ok((proposal.tasks.clone(), issues, Some(proposal))),
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
}
