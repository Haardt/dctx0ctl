//! Source-bound task reviews supplement execution without rewriting native results.
use super::*;
use dctx0_tasks::{ConformanceReview, ReviewReport, ReviewedRequirement, TaskKind};

impl Project {
    pub(crate) fn validate_conformance_action(
        &self,
        action: &dctx0_tasks::Action,
        application: Option<&Path>,
    ) -> Result<()> {
        if let dctx0_tasks::Action::RecordReview { task, report, .. } = action {
            if let Some(binding) = &report.conformance {
                let template = self.conformance_review_template(application, *task)?;
                let expected = template
                    .conformance
                    .ok_or_else(|| Error::Invalid("Review bindings unavailable".into()))?;
                if binding.snapshot != expected.snapshot
                    || binding.requirements_hash != expected.requirements_hash
                    || binding.spec_source_hash != expected.spec_source_hash
                    || binding.implementation_artifacts != expected.implementation_artifacts
                    || binding.subjects.keys().collect::<BTreeSet<_>>()
                        != expected.subjects.keys().collect::<BTreeSet<_>>()
                {
                    return Err(Error::Invalid("Conformance review sources or task requirements changed; renew the substantive review".into()));
                }
            }
        }
        Ok(())
    }
    /// Read-only template. Empty reports deliberately cannot authorize acceptance.
    pub fn conformance_review_template(
        &self,
        application: Option<&Path>,
        task: i64,
    ) -> Result<ReviewReport> {
        let (application, _) = self.verification_configuration(application)?;
        let session = self.task_session(None, None)?;
        let member = session.member(task)?;
        let snapshot = self.load(&session.plan.snapshot_id)?;
        let subjects = dctx0_tasks::conformance_subjects(&snapshot.ir, &member.candidate);
        if subjects.is_empty() {
            return Err(Error::Invalid(
                "Task has no assigned conformance obligations".into(),
            ));
        }
        if dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash {
            return Err(Error::Invalid(
                "Check the changed specification before reviewing".into(),
            ));
        }
        Ok(ReviewReport {
            reviewer: String::new(),
            judgement: "request-changes".into(),
            summary: String::new(),
            findings: vec![],
            conformance: Some(ConformanceReview {
                snapshot: snapshot.id,
                requirements_hash: member.candidate.requirements_hash.clone(),
                spec_source_hash: snapshot.source_hash,
                implementation_artifacts: Capture::inventory(&application)?,
                subjects: subjects
                    .into_iter()
                    .map(|subject| {
                        (
                            subject,
                            ReviewedRequirement {
                                report: String::new(),
                                limitations: vec![],
                            },
                        )
                    })
                    .collect(),
            }),
        })
    }

    pub(crate) fn stored_conformance_notes(
        &self,
        application: &Path,
    ) -> Result<AuthoredInspections> {
        let session = self.task_session(None, None)?;
        let snapshot = self.load(&session.plan.snapshot_id)?;
        let inventory = Capture::inventory(application)?;
        let mut notes = AuthoredInspections {
            schema_version: 1,
            actor: "dctx0-task-review".into(),
            scope: "Stored source-bound task reviews".into(),
            reviewed_hashes: inventory.clone(),
            reviewed_spec_hashes: BTreeMap::new(),
            subjects: BTreeMap::new(),
        };
        for source in self.sources()? {
            notes
                .reviewed_spec_hashes
                .insert(source.file, digest(source.text.as_bytes()));
        }
        for member in &session.plan.members {
            if member.candidate.kind != TaskKind::Review {
                continue;
            }
            let state = &session.states[&member.task_id];
            // The latest judgement supersedes earlier approvals; never search backwards
            // for a convenient PASS. Historical/nonmember tasks are not consulted.
            let Some(report) = state.reviews.last() else {
                continue;
            };
            let Some(binding) = &report.conformance else {
                continue;
            };
            let expected: BTreeSet<_> =
                dctx0_tasks::conformance_subjects(&snapshot.ir, &member.candidate)
                    .into_iter()
                    .collect();
            if binding.snapshot != snapshot.id
                || binding.spec_source_hash != snapshot.source_hash
                || binding.requirements_hash != member.candidate.requirements_hash
                || binding.implementation_artifacts != inventory
                || binding.subjects.keys().cloned().collect::<BTreeSet<_>>() != expected
                || binding
                    .subjects
                    .values()
                    .any(|note| note.report.trim().is_empty())
                || report.judgement != "approved"
                || report
                    .findings
                    .iter()
                    .any(|finding| finding.action_required)
            {
                continue;
            }
            for (subject, note) in &binding.subjects {
                let review = snapshot.ir.symbols.get(subject).filter(|s| s.kind == StatementKind::Review)
                    .map(|_| dctx0_verification::program_inspection::InspectedReview {
                        approved: true,
                        findings: report.findings.iter().map(|f| dctx0_verification::program_inspection::InspectionFinding {
                            id: f.id.clone(), description: f.description.clone(),
                            severity: match f.severity.as_str() {
                                "high" => dctx0_verification::program_inspection::FindingSeverity::High,
                                "medium" => dctx0_verification::program_inspection::FindingSeverity::Medium,
                                _ => dctx0_verification::program_inspection::FindingSeverity::Low,
                            },
                        }).collect(),
                    });
                if notes
                    .subjects
                    .insert(
                        subject.clone(),
                        AuthoredInspectionNote {
                            report: format!(
                                "T{}; reviewer {}: {}",
                                member.task_id, report.reviewer, note.report
                            ),
                            limitations: note.limitations.clone(),
                            review,
                        },
                    )
                    .is_some()
                {
                    return Err(Error::Invalid(format!(
                        "Multiple active review owners for {subject}"
                    )));
                }
            }
        }
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dctx0_tasks::{Action, CheckStatus, Finding, TaskResult, WorkState};

    #[test]
    fn source_bound_reviews_validate_current_requirements_and_keep_latest_judgements() {
        let root =
            std::env::temp_dir().join(format!("dctx0-review-bindings-{}", std::process::id()));
        let spec = root.join("spec");
        let app = root.join("app");
        fs::create_dir_all(&spec).unwrap();
        fs::create_dir(&app).unwrap();
        let text = "workspace W { value A = Int\nconstraint \"Use ordinary application code\"\nreview Boundary { inspect { implementation }\ncriteria { \"Review the real factory\" } } }";
        fs::write(spec.join("w.dctx0"), text).unwrap();
        fs::write(
            app.join("app.ts"),
            "export function factory(value:number){return value;}",
        )
        .unwrap();
        let project = Project::new(&spec, None).unwrap();
        project
            .initialize_verification(
                &app,
                &dctx0_target_typescript::BunHarness,
                ConfiguredVerificationScope {
                    subjects: ["W".into()].into(),
                    tasks: Default::default(),
                },
            )
            .unwrap();
        let check = project.check().unwrap();
        let plan = project
            .task_plan(check.snapshot_id.as_deref().unwrap())
            .unwrap();
        let contract = plan
            .members
            .iter()
            .find(|member| member.candidate.kind == TaskKind::Contract)
            .unwrap()
            .task_id;
        let review = plan
            .members
            .iter()
            .find(|member| member.candidate.kind == TaskKind::Review)
            .unwrap()
            .task_id;
        assert!(
            project
                .conformance_review_template(Some(&app), contract)
                .unwrap_err()
                .to_string()
                .contains("no assigned")
        );
        assert!(
            project
                .task_renew_review(-1, Some(&app), "Inspect")
                .unwrap_err()
                .to_string()
                .contains("no longer active")
        );
        assert!(
            project
                .stored_conformance_notes(&app)
                .unwrap()
                .subjects
                .is_empty()
        );
        let started = project
            .task_action(Action::Start {
                task: contract,
                actor: "agent".into(),
                reservation: None,
            })
            .unwrap()
            .state
            .unwrap();
        project
            .task_action(Action::Complete {
                task: contract,
                actor: "agent".into(),
                claim: started.claim.unwrap(),
                version: started.version,
                result: TaskResult {
                    summary: "Fixture factory source supplied; no native execution claimed.".into(),
                    artifacts: vec!["app.ts".into()],
                    decisions: vec![],
                    implementation: None,
                },
            })
            .unwrap();
        let started = project
            .task_action(Action::Start {
                task: review,
                actor: "agent".into(),
                reservation: None,
            })
            .unwrap()
            .state
            .unwrap();
        assert!(
            project
                .task_renew_review(review, Some(&app), "Inspect")
                .unwrap_err()
                .to_string()
                .contains("delivered")
        );
        let mut report = project
            .conformance_review_template(Some(&app), review)
            .unwrap();
        report.reviewer = "agent".into();
        report.summary =
            "Fixture source inspection for binding tests, not application acceptance.".into();
        report.judgement = "approved".into();
        for note in report.conformance.as_mut().unwrap().subjects.values_mut() {
            note.report = "app.ts exports an ordinary identity factory; this fixture verifies report binding only.".into();
            note.limitations = vec!["No native proof supplied by this unit test.".into()];
        }
        report.findings = ["low", "medium", "high"]
            .into_iter()
            .map(|severity| Finding {
                id: severity.into(),
                severity: severity.into(),
                description: "Fixture severity retained in the source-bound review.".into(),
                task_ids: vec![],
                action_required: false,
            })
            .collect();
        let action = |report| Action::RecordReview {
            task: review,
            actor: "agent".into(),
            claim: started.claim.unwrap(),
            version: started.version,
            report,
        };
        fs::write(app.join("app.ts"), "changed source").unwrap();
        assert!(
            project
                .task_action(action(report.clone()))
                .unwrap_err()
                .to_string()
                .contains("sources or task requirements changed")
        );
        fs::write(
            app.join("app.ts"),
            "export function factory(value:number){return value;}",
        )
        .unwrap();
        let recorded = project
            .task_action(action(report.clone()))
            .unwrap()
            .state
            .unwrap();
        let notes = project.stored_conformance_notes(&app).unwrap();
        assert_eq!(notes.subjects.len(), 2);
        assert_eq!(
            notes.subjects["W.Boundary"]
                .review
                .as_ref()
                .unwrap()
                .findings
                .len(),
            3
        );
        fs::write(
            spec.join("w.dctx0"),
            format!("{text}\n// changed reviewed specification\n"),
        )
        .unwrap();
        assert!(
            project
                .conformance_review_template(Some(&app), review)
                .unwrap_err()
                .to_string()
                .contains("changed specification")
        );
        fs::write(spec.join("w.dctx0"), text).unwrap();
        project
            .task_action(Action::Complete {
                task: review,
                actor: "agent".into(),
                claim: recorded.claim.unwrap(),
                version: recorded.version,
                result: TaskResult {
                    summary: "Fixture inspection activity completed.".into(),
                    artifacts: vec![],
                    decisions: vec![],
                    implementation: None,
                },
            })
            .unwrap();
        let session = project.task_session(None, None).unwrap();
        assert_eq!(session.states[&review].work, WorkState::Done);
        assert_eq!(session.states[&review].check, CheckStatus::Unverified);
        fs::write(app.join("app.ts"), "changed source").unwrap();
        assert!(
            project
                .stored_conformance_notes(&app)
                .unwrap()
                .subjects
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
