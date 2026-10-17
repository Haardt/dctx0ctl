//! Explicit complete acceptance using authored reviews, never generated approvals.
use crate::*;
use dctx0_core::{
    StatementKind, TargetProfileProvider, VerificationAttestation, VerificationResult,
};
use dctx0_target_sdk::HarnessGenerator;
use dctx0_tasks::{CheckStatus, Finding, ReviewReport, Session, TaskKind, WorkState};
use dctx0_verification::program_inspection::{InspectedReview, ProgramInspection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredInspectionNote {
    pub report: String,
    pub limitations: Vec<String>,
    pub review: Option<InspectedReview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredInspections {
    pub schema_version: u32,
    pub actor: String,
    pub reviewed_hashes: BTreeMap<String, String>,
    /// Spec-root-relative hashes when specs live outside the application root.
    #[serde(default)]
    pub reviewed_spec_hashes: BTreeMap<String, String>,
    pub subjects: BTreeMap<String, AuthoredInspectionNote>,
    pub scope: String,
}

impl AuthoredInspections {
    fn validate(&self, application: &Path) -> Result<()> {
        if self.schema_version != 1
            || self.actor.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.reviewed_hashes.is_empty()
            || self.subjects.is_empty()
            || self
                .subjects
                .values()
                .any(|note| note.report.trim().is_empty())
        {
            return Err(Error::Invalid(
                "Incomplete authored inspection notes".into(),
            ));
        }
        for (name, expected) in &self.reviewed_hashes {
            let file = crate::verification_setup::safe_file(application, name)?.1;
            let hash =
                dctx0_store::VerificationArtifact::new("application/octet-stream", fs::read(file)?)
                    .reference
                    .hash;
            if &hash != expected {
                return Err(Error::Invalid(format!(
                    "New source inspection required: {name}"
                )));
            }
        }
        Ok(())
    }

    fn validate_spec(&self, project: &Project, application: &Path) -> Result<()> {
        for source in project.sources()? {
            let path = project.root.join(&source.file);
            let expected = if let Ok(relative) = path.strip_prefix(application) {
                self.reviewed_hashes
                    .get(&relative.to_string_lossy().replace('\\', "/"))
            } else {
                self.reviewed_spec_hashes.get(&source.file)
            };
            let actual = dctx0_store::VerificationArtifact::new(
                "application/octet-stream",
                source.text.into_bytes(),
            )
            .reference
            .hash;
            if expected != Some(&actual) {
                return Err(Error::Invalid(format!(
                    "New specification inspection required: {}",
                    source.file
                )));
            }
        }
        Ok(())
    }

    fn attestation(
        &self,
        run: &dctx0_core::VerificationRun,
        subject: String,
        requirements: BTreeSet<String>,
        report: String,
        limitations: Vec<String>,
    ) -> VerificationAttestation {
        VerificationAttestation {
            schema_version: 1,
            actor: self.actor.clone(),
            subject,
            snapshot: run.provenance.snapshot.clone(),
            plan_hash: run.provenance.plan_hash.clone(),
            requirements,
            implementation_artifacts: run.provenance.implementation.clone(),
            report,
            limitations,
            result: VerificationResult::Pass,
        }
    }

    fn task_reports(
        &self,
        session: &Session,
        run: &dctx0_core::VerificationRun,
    ) -> Result<Vec<TaskAttestation>> {
        let mut reports = Vec::new();
        for member in &session.plan.members {
            let id = member.task_id;
            let state = session
                .states
                .get(&id)
                .ok_or(Error::Invalid(format!("Missing state for T{id}")))?;
            if state.work == WorkState::Done {
                if state.check != CheckStatus::Pass && member.candidate.kind != TaskKind::Review {
                    return Err(Error::Invalid(format!(
                        "T{id}: historical Done without PASS requires separate work"
                    )));
                }
                continue;
            }
            if state.work != WorkState::Open
                || state.actor.is_some()
                || state.claim.is_some()
                || state.reservation.is_some()
            {
                return Err(Error::Invalid(format!(
                    "T{id}: active work or reservation is preserved"
                )));
            }
            let candidate = &member.candidate;
            let requirements: BTreeSet<String> = candidate
                .subjects
                .iter()
                .chain(&candidate.types)
                .chain(&candidate.facts)
                .cloned()
                .collect();
            let applicable: Vec<_> = self
                .subjects
                .iter()
                .filter(|(subject, _)| {
                    requirements.iter().any(|requirement| {
                        requirement == *subject || requirement.starts_with(&format!("{subject}#"))
                    })
                })
                .collect();
            let mut descriptions: BTreeSet<String> = self
                .subjects
                .values()
                .filter(|note| note.review.is_none())
                .map(|note| note.report.clone())
                .collect();
            descriptions.extend(applicable.iter().map(|(_, note)| note.report.clone()));
            let review = if candidate.kind == TaskKind::Review {
                let note = candidate
                    .subjects
                    .iter()
                    .filter_map(|subject| self.subjects.get(subject))
                    .find(|note| note.review.is_some());
                let fallback = InspectedReview {
                    approved: true,
                    findings: vec![],
                };
                if candidate
                    .review_obligations
                    .iter()
                    .any(|subject| !self.subjects.contains_key(subject))
                {
                    return Err(Error::Invalid(format!(
                        "T{id}: every assigned obligation requires a concrete authored review"
                    )));
                }
                let authored = note
                    .and_then(|note| note.review.as_ref())
                    .unwrap_or(&fallback);
                Some(ReviewReport {
                    conformance: None,
                    reviewer: self.actor.clone(),
                    judgement: if authored.approved {
                        "approved"
                    } else {
                        "rejected"
                    }
                    .into(),
                    summary: applicable
                        .iter()
                        .map(|(_, note)| note.report.clone())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    findings: authored
                        .findings
                        .iter()
                        .map(|finding| Finding {
                            id: finding.id.clone(),
                            severity: match finding.severity {
                                dctx0_verification::program_inspection::FindingSeverity::Low => {
                                    "low"
                                }
                                dctx0_verification::program_inspection::FindingSeverity::Medium => {
                                    "medium"
                                }
                                dctx0_verification::program_inspection::FindingSeverity::High => {
                                    "high"
                                }
                            }
                            .into(),
                            description: finding.description.clone(),
                            task_ids: vec![],
                            action_required: false,
                        })
                        .collect(),
                })
            } else {
                None
            };
            reports.push(TaskAttestation {
                task: id,
                inspection: self.attestation(run, format!("T{id}"), requirements, format!("Reviewed task subjects: {}. {}", candidate.subjects.join(", "), descriptions.into_iter().collect::<Vec<_>>().join(" ")), vec!["Explicit agent inspection supplements the complete finite native corpus; it is not an automatic proof.".into()]),
                review,
            });
        }
        Ok(reports)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteVerificationReport {
    pub schema_version: u32,
    pub result: VerificationResult,
    pub native_result: VerificationResult,
    pub native_run: String,
    pub inspected_run: String,
    pub mandatory_cases: usize,
    pub tasks: usize,
    pub open_tasks: usize,
    pub non_pass_tasks: usize,
    #[serde(default)]
    pub reviewed_tasks: Vec<i64>,
    pub adopted_tasks: Vec<i64>,
    pub inspected_tasks: Vec<i64>,
    pub basis: String,
}

impl Project {
    /// Check, publish requirements, execute configured scope and bind explicit reviews.
    /// Each phase retains its normal transaction boundaries and historical evidence.
    pub async fn complete_verification(
        &self,
        application: &Path,
        notes_file: &Path,
        provider: &dyn TargetProfileProvider,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
    ) -> Result<CompleteVerificationReport> {
        self.complete_verification_with_input_limit(
            application,
            notes_file,
            provider,
            generator,
            cancellation,
            crate::DEFAULT_REVIEW_INPUT_BYTES,
        )
        .await
    }

    /// Complete acceptance with an explicit bound for authored inspection notes.
    pub async fn complete_verification_with_input_limit(
        &self,
        application: &Path,
        notes_file: &Path,
        provider: &dyn TargetProfileProvider,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        max_input_bytes: u64,
    ) -> Result<CompleteVerificationReport> {
        let application = fs::canonicalize(application)?;
        let imported = match fs::metadata(notes_file) {
            Ok(_) => {
                let notes: AuthoredInspections =
                    crate::read_review_input(notes_file, max_input_bytes, "Inspection notes")?;
                notes.validate(&application)?;
                notes.validate_spec(self, &application)?;
                Some(notes)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let (_, config) = self.verification_configuration(Some(&application))?;
        if config.target != provider.manifest().target {
            return Err(Error::Invalid(
                "Configured verification target and profile provider differ".into(),
            ));
        }
        if config.scope.subjects.is_empty() && config.scope.tasks.is_empty() {
            return Err(Error::Invalid(
                "Complete verification requires an explicitly configured scope".into(),
            ));
        }
        let check = self.check()?;
        if !check.ok {
            return Err(Error::Invalid(format!(
                "Spec check failed: {:?}",
                check.diagnostics
            )));
        }
        create_profiles(&application, provider, true, false, false)?;
        self.task_plan(
            check
                .snapshot_id
                .as_deref()
                .ok_or(Error::Invalid("Successful check has no snapshot".into()))?,
        )?;
        let mut notes = self.stored_conformance_notes(&application)?;
        if let Some(imported) = imported {
            let session = self.task_session(None, None)?;
            let snapshot = self.load(&session.plan.snapshot_id)?;
            let authoritative = session
                .plan
                .members
                .iter()
                .filter(|member| {
                    session.states[&member.task_id]
                        .reviews
                        .last()
                        .is_some_and(|review| {
                            review.conformance.is_some() || review.judgement != "approved"
                        })
                })
                .flat_map(|member| {
                    dctx0_tasks::conformance_subjects(&snapshot.ir, &member.candidate)
                })
                .collect::<BTreeSet<_>>();
            for (subject, note) in imported.subjects {
                if !authoritative.contains(&subject) {
                    notes.subjects.entry(subject).or_insert(note);
                }
            }
        }
        let native = self
            .run_verification(
                Some(&application),
                Default::default(),
                generator,
                cancellation,
                false,
            )
            .await?;
        let run = &native.evidence.run;
        if !run.permits_task_specific_proof()
            || !run.provenance.incomplete.is_empty()
            || run.required_cases.is_empty()
            || run.cases.len() != run.required_cases.len()
            || run
                .cases
                .iter()
                .map(|case| case.id.clone())
                .collect::<BTreeSet<_>>()
                != run.required_cases
            || run
                .cases
                .iter()
                .any(|case| case.result != VerificationResult::Pass)
        {
            return Err(Error::Invalid(format!(
                "Complete native evidence required; inspect run {}",
                native.evidence.id
            )));
        }
        // Empty/missing reviews remain UNKNOWN in the effective assessment.
        if !notes.subjects.is_empty() {
            notes.validate(&application)?;
        }
        notes.validate_spec(self, &application)?;
        // Read the exact plan retained by this run rather than projecting a new scope.
        let artifact = run
            .artifacts
            .iter()
            .find(|artifact| artifact.media_type == "application/vnd.dctx0.plan+json");
        let plan = if let Some(artifact) = artifact {
            let store = self.store(AccessMode::ReadOnly)?;
            serde_json::from_slice::<dctx0_verification::ProgramPlan>(
                &store
                    .verification_artifact(&native.evidence.id, &artifact.hash)?
                    .contents,
            )?
        } else {
            return Err(Error::Invalid("Native plan artifact unavailable".into()));
        };
        let mut inspections = Vec::new();
        let subjects = plan
            .suites
            .iter()
            .map(|suite| (&suite.declaration.id, &suite.nodes, false))
            .chain(
                plan.manual_checks
                    .iter()
                    .filter(|manual| manual.kind != StatementKind::Intent)
                    .map(|manual| {
                        (
                            &manual.subject,
                            &manual.nodes,
                            manual.kind == StatementKind::Review,
                        )
                    }),
            );
        for (subject, nodes, is_review) in subjects {
            // Automatically proved suites need no authored approval. Missing manual
            // notes remain UNKNOWN in the central effective assessment.
            if let Some(note) = notes.subjects.get(subject) {
                inspections.push(ProgramInspection {
                    inspection: notes.attestation(
                        run,
                        subject.clone(),
                        std::iter::once(subject.clone())
                            .chain(nodes.iter().map(|node| node.id.clone()))
                            .collect(),
                        note.report.clone(),
                        note.limitations.clone(),
                    ),
                    review: if is_review { note.review.clone() } else { None },
                });
            }
        }
        let inspected = self.publish_program_inspections(
            Some(&application),
            &native.evidence.id,
            &inspections,
        )?;
        if inspected.assessment.result != VerificationResult::Pass
            || !inspected.assessment.open_obligations.is_empty()
        {
            return Err(Error::Invalid(format!(
                "Effective assessment incomplete; inspect run {}",
                inspected.evidence.id
            )));
        }
        // Executable release policies are evaluated centrally, not approved by an
        // agent. Their actual assessment also supplies the policy task's evidence.
        for policy in &inspected.assessment.policies {
            if policy.result == VerificationResult::Pass {
                notes.subjects.entry(policy.subject.clone()).or_insert(AuthoredInspectionNote {
                    report: format!("Rust evaluated the declared policy {} against recorded execution and explicit reviews: PASS.", policy.subject),
                    limitations: vec!["Only the modeled policy expression was evaluated.".into()],
                    review: None,
                });
            }
        }
        let session = self.task_session(None, None)?;
        let reports = notes.task_reports(&session, run)?;
        let inspected_tasks = reports.iter().map(|report| report.task).collect();
        if !reports.is_empty() {
            notes.validate(&application)?;
            notes.validate_spec(self, &application)?;
            self.publish_task_attestations(Some(&application), &native.evidence.id, &reports)?;
        }
        let session = self.task_session(None, None)?;
        let open_tasks = session
            .plan
            .members
            .iter()
            .filter(|member| {
                session
                    .states
                    .get(&member.task_id)
                    .is_none_or(|state| state.work != WorkState::Done)
            })
            .count();
        let non_pass_tasks = session
            .plan
            .members
            .iter()
            .filter(|member| {
                session
                    .states
                    .get(&member.task_id)
                    .is_none_or(|state| state.check != CheckStatus::Pass)
            })
            .count();
        let reviewed_tasks = session
            .plan
            .members
            .iter()
            .filter(|member| {
                member.candidate.kind == TaskKind::Review
                    && !member.candidate.review_obligations.is_empty()
                    && member
                        .candidate
                        .review_obligations
                        .iter()
                        .all(|subject| notes.subjects.contains_key(subject))
            })
            .map(|member| member.task_id)
            .collect::<Vec<_>>();
        let missing_evidence = session.plan.members.iter().any(|member| {
            session.states.get(&member.task_id).is_none_or(|state| {
                state.check != CheckStatus::Pass && !reviewed_tasks.contains(&member.task_id)
            })
        });
        if session.plan.members.is_empty() || open_tasks != 0 || missing_evidence {
            return Err(Error::Invalid(format!(
                "Complete acceptance requires delivered tasks and tests or source-bound reviews: {open_tasks} unfinished, {non_pass_tasks} without PASS"
            )));
        }
        let report = CompleteVerificationReport {
            schema_version: 1,
            result: VerificationResult::Pass,
            native_result: run.result.clone(),
            native_run: native.evidence.id,
            inspected_run: inspected.evidence.id,
            mandatory_cases: run.cases.len(),
            tasks: session.plan.members.len(),
            open_tasks,
            non_pass_tasks,
            reviewed_tasks,
            adopted_tasks: native.adopted_tasks,
            inspected_tasks,
            basis: "deterministic native cases plus explicitly source-bound agent inspection"
                .into(),
        };
        let directory = self.root.join(".dctx0ctl/verification-complete");
        fs::create_dir_all(&directory)?;
        atomic_write(
            &directory.join(format!("{}.json", report.native_run)),
            &serde_json::to_vec_pretty(&report)?,
        )?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_notes_bind_all_spec_sources_and_reject_invalid_or_escaping_inputs() {
        let root =
            std::env::temp_dir().join(format!("dctx0-complete-notes-{}", std::process::id()));
        let app = root.join("app");
        let spec = app.join("spec");
        fs::create_dir_all(&spec).unwrap();
        fs::write(app.join("app.ts"), "export const value = 1;").unwrap();
        fs::write(spec.join("w.dctx0"), "workspace W { value A = Int }").unwrap();
        let project = Project::new(&spec, None).unwrap();
        let hashes = ["app.ts", "spec/w.dctx0"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    dctx0_store::VerificationArtifact::new(
                        "application/octet-stream",
                        fs::read(app.join(name)).unwrap(),
                    )
                    .reference
                    .hash,
                )
            })
            .collect();
        let notes = AuthoredInspections {
            schema_version: 1,
            actor: "reviewer".into(),
            scope: "W".into(),
            reviewed_hashes: hashes,
            reviewed_spec_hashes: Default::default(),
            subjects: BTreeMap::from([(
                "W.A".into(),
                AuthoredInspectionNote {
                    report: "Explicit reviewed source".into(),
                    limitations: vec![],
                    review: None,
                },
            )]),
        };
        notes.validate(&app).unwrap();
        notes.validate_spec(&project, &app).unwrap();
        let mut invalid = notes.clone();
        invalid.schema_version = 2;
        assert!(invalid.validate(&app).is_err());
        invalid = notes.clone();
        invalid.actor.clear();
        assert!(invalid.validate(&app).is_err());
        invalid = notes.clone();
        invalid.subjects.get_mut("W.A").unwrap().report.clear();
        assert!(invalid.validate(&app).is_err());
        invalid = notes.clone();
        invalid
            .reviewed_hashes
            .insert("../outside.ts".into(), "fake".into());
        assert!(invalid.validate(&app).is_err());
        invalid = notes.clone();
        invalid.reviewed_hashes.remove("spec/w.dctx0");
        assert!(invalid.validate_spec(&project, &app).is_err());
        fs::write(spec.join("new.dctx0"), "workspace W { value B = Int }").unwrap();
        assert!(notes.validate_spec(&project, &app).is_err());
        fs::remove_file(spec.join("new.dctx0")).unwrap();
        fs::write(app.join("app.ts"), "export const value = 2;").unwrap();
        assert!(notes.validate(&app).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
