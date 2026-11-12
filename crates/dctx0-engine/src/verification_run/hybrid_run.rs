//! One managed occurrence owns both backends. Saved reports are not inputs.
use super::*;
use dctx0_verification::{
    application_evidence::{ApplicationExecutionProof, ApplicationPhaseProof},
    application_plan::{ApplicationTestPlan, TestBackend},
    hybrid_evidence::{
        HYBRID_EXECUTION_MEDIA_TYPE, HybridExecutionProof, INCREMENTAL_HYBRID_EXECUTION_MEDIA_TYPE,
        IncrementalHybridExecutionProof,
    },
};

pub struct HybridVerificationOptions<'a> {
    pub plan: &'a str,
    pub application_group: &'a str,
    pub recipe: &'a str,
    pub native_report: &'a str,
}

struct HybridRunPolicy<'a> {
    no_adopt: bool,
    inspections: Option<&'a crate::AuthoredInspections>,
    expected_plan_sha256: Option<&'a str>,
    reuse: Option<&'a NativeReuseOptions>,
}

pub(super) struct HybridExecution {
    pub assignment: ApplicationTestPlan,
    pub adapter_group: String,
    pub report: ApplicationNativeReport,
    pub task_pin: Option<VerificationTaskPin>,
    pub done_versions: BTreeMap<i64, i64>,
    pub inspections: Option<crate::AuthoredInspections>,
    pub pin_issues: Vec<VerificationIssue>,
    pub source_inputs: BTreeMap<String, String>,
    pub started_at_unix_ns: String,
    pub started: Instant,
    pub application_started_at_unix_ns: String,
    pub application_elapsed_us: u64,
    pub verifier: crate::native::VerifierIdentity,
}

fn now() -> Result<String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::Invalid(error.to_string()))?
        .as_nanos()
        .to_string())
}

// The application capture may already be gone at publication. Its executable
// hashes are retained in the materialized inventory; external runtimes must
// still match their executable and launch environment at the end of the cohort.
pub(super) fn application_runtime_freshness(
    report: &ApplicationNativeReport,
) -> Vec<VerificationIssue> {
    report
        .phases
        .iter()
        .filter_map(|phase| {
            let current = match Path::new(&phase.runtime.executable_path)
                .strip_prefix(&report.captured_root)
            {
                Ok(relative) => {
                    report
                        .materialized_inputs
                        .get(&relative.to_string_lossy().replace('\\', "/"))
                        == Some(&phase.runtime.executable_hash)
                }
                Err(_) => phase.runtime.still_current().unwrap_or(false),
            };
            (!current).then(|| {
                issue(
                    "ADOPTION_RUNTIME_CHANGED",
                    format!("{} runtime changed", phase.phase),
                )
            })
        })
        .collect()
}

fn require_new_native_report(root: &Path, output: &str) -> Result<()> {
    if !output.starts_with(".dctx0ctl/application-tests/") {
        return Err(Error::Invalid(
            "Managed application evidence must be under .dctx0ctl/application-tests/".into(),
        ));
    }
    let (_, path) = crate::verification_setup::safe_file(root, output)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => Err(Error::Invalid(format!(
            "Hybrid native report already exists: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

impl Project {
    /// Execute both assigned backends freshly. A diagnostic call still stores
    /// incomplete evidence, but neither imports a group nor delivers a subset.
    pub async fn run_hybrid_verification(
        &self,
        application: &Path,
        options: &HybridVerificationOptions<'_>,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        no_adopt: bool,
    ) -> Result<NativeVerificationReport> {
        self.run_hybrid_verification_inner(
            application,
            options,
            generator,
            cancellation,
            HybridRunPolicy {
                no_adopt,
                inspections: None,
                expected_plan_sha256: None,
                reuse: None,
            },
        )
        .await
    }

    /// Reuse declared adapter contract units while always running the native group.
    /// Historical cases retain explicit lineage and grant no task authority.
    pub async fn run_hybrid_verification_incremental(
        &self,
        application: &Path,
        options: &HybridVerificationOptions<'_>,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        reuse: &NativeReuseOptions,
    ) -> Result<NativeVerificationReport> {
        self.run_hybrid_verification_inner(
            application,
            options,
            generator,
            cancellation,
            HybridRunPolicy {
                no_adopt: true,
                inspections: None,
                expected_plan_sha256: None,
                reuse: Some(reuse),
            },
        )
        .await
    }

    pub async fn run_hybrid_verification_incremental_with_adoption(
        &self,
        application: &Path,
        options: &HybridVerificationOptions<'_>,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        reuse: &NativeReuseOptions,
        no_adopt: bool,
    ) -> Result<NativeVerificationReport> {
        self.run_hybrid_verification_inner(
            application,
            options,
            generator,
            cancellation,
            HybridRunPolicy {
                no_adopt,
                inspections: None,
                expected_plan_sha256: None,
                reuse: Some(reuse),
            },
        )
        .await
    }

    pub(crate) async fn run_hybrid_verification_with_inspections(
        &self,
        application: &Path,
        options: &HybridVerificationOptions<'_>,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        inspections: Option<&crate::AuthoredInspections>,
        expected_plan_sha256: Option<&str>,
    ) -> Result<NativeVerificationReport> {
        self.run_hybrid_verification_inner(
            application,
            options,
            generator,
            cancellation,
            HybridRunPolicy {
                no_adopt: false,
                inspections,
                expected_plan_sha256,
                reuse: None,
            },
        )
        .await
    }

    async fn run_hybrid_verification_inner(
        &self,
        application: &Path,
        options: &HybridVerificationOptions<'_>,
        generator: &dyn HarnessGenerator,
        cancellation: &NativeCancellation,
        policy: HybridRunPolicy<'_>,
    ) -> Result<NativeVerificationReport> {
        let HybridRunPolicy {
            no_adopt,
            inspections,
            expected_plan_sha256,
            reuse,
        } = policy;
        let (root, bundle, _) = self.read_application_testing_bundle_checked(
            application,
            options.plan,
            expected_plan_sha256,
        )?;
        require_new_native_report(&root, options.native_report)?;
        let (_, config) = self.verification_configuration(Some(&root))?;
        let groups = &bundle.plan.configuration.groups;
        let adapter = groups
            .iter()
            .find(|group| group.backend == TestBackend::Adapter);
        let native = groups.iter().find(|group| {
            group.backend == TestBackend::ApplicationTests && group.id == options.application_group
        });
        if groups.len() != 2
            || adapter.is_none()
            || native.is_none()
            || groups.iter().any(|group| group.target != config.target)
        {
            return Err(Error::Invalid("Hybrid execution requires one assigned adapter and one application-test group of the configured target".into()));
        }
        let adapter_group = adapter.unwrap().id.clone();
        let snapshot = self.load(&bundle.plan.program.snapshot)?;
        let (task_pin, pin_issues) =
            pins::capture(self, &root, &config.target, &bundle.plan.program, &snapshot)?;
        let mut done_versions = adoption::capture_done_versions(self, task_pin.as_ref())?;
        if let Some(batch) = inspections.and_then(|notes| notes.done_inspections.as_ref()) {
            let pin = task_pin
                .as_ref()
                .ok_or_else(|| Error::Invalid("Inspected tasks require a pinned plan".into()))?;
            let session = self.task_metadata_session(None, None)?;
            if &batch.task_pin != pin
                || batch.snapshot != session.plan.snapshot_id
                || batch.check_occurrence != session.plan.check_run_id
                || batch.tasks.is_empty()
            {
                return Err(Error::Invalid("Inspected task pin is stale".into()));
            }
            for (id, inspection) in &batch.tasks {
                let member = session.member(*id)?;
                let state = &session.states[id];
                if state.work != dctx0_tasks::WorkState::Done
                    || state.check != dctx0_tasks::CheckStatus::Unverified
                    || state.result.is_none()
                    || state.actor.is_some()
                    || state.claim.is_some()
                    || state.reservation.is_some()
                    || session.definitions[id].is_adjustment()
                    || inspection.state_version != state.version
                    || pin.requirements.get(id) != Some(&member.candidate.requirements_hash)
                {
                    return Err(Error::Invalid(format!(
                        "T{id}: inspected historical state changed"
                    )));
                }
                done_versions.insert(*id, state.version);
            }
        }
        let source_inputs = Capture::inventory_with_budget(&root, config.capture_budgets())?;
        let verifier = crate::native::VerifierIdentity::capture()
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let started_at_unix_ns = now()?;
        let started = Instant::now();
        let application_started_at_unix_ns = now()?;
        let native_started = Instant::now();
        let report = self
            .run_application_tests_no_clobber(
                &root,
                options.plan,
                options.application_group,
                options.recipe,
                options.native_report,
                (cancellation, expected_plan_sha256),
            )
            .await?;
        let application_elapsed_us =
            native_started.elapsed().as_micros().min(u64::MAX as u128) as u64;
        let context = HybridExecution {
            assignment: bundle.plan,
            adapter_group,
            report,
            task_pin,
            done_versions,
            inspections: inspections.cloned(),
            pin_issues,
            source_inputs,
            started_at_unix_ns,
            started,
            application_started_at_unix_ns,
            application_elapsed_us,
            verifier,
        };
        let limits = config
            .execution
            .as_ref()
            .map(VerificationExecutionBudgets::native_limits)
            .unwrap_or_default();
        match self
            .execute_verification(
                Some(&root),
                VerificationSelection::default(),
                generator,
                cancellation,
                no_adopt,
                None,
                limits,
                None,
                None,
                Some((&context.assignment, &context.adapter_group)),
                Some(&context),
                reuse,
            )
            .await?
        {
            ExecutionReport::Published(report) => Ok(report),
            ExecutionReport::Transient(_) => Err(Error::Invalid(
                "Unexpected transient hybrid execution".into(),
            )),
        }
    }
}

impl HybridExecution {
    pub fn cases(&self) -> Vec<VerificationCaseRecord> {
        self.report
            .assessment
            .as_ref()
            .map(|assessment| assessment.cases.clone())
            .unwrap_or_default()
    }

    /// Called before publication, while the outer project lock and all standard
    /// adapter source/runtime checks still apply. Preserve exact native records.
    pub fn attach(
        &self,
        run: &mut VerificationRun,
        artifacts: &mut Vec<VerificationArtifact>,
        adapter_started_at_unix_ns: String,
        adapter_elapsed_us: u64,
        reused_cases: &BTreeSet<String>,
    ) -> Result<()> {
        run.provenance
            .incomplete
            .extend(application_runtime_freshness(&self.report));
        let receipt = VerificationArtifact::new(
            "application/vnd.dctx0.application-native-receipt+json",
            serde_json::to_vec(&self.report)?,
        );
        let receipt_hash = receipt.reference.hash.clone();
        artifacts.push(receipt);
        let Some(assessment) = &self.report.assessment else {
            run.provenance.incomplete.push(issue(
                "HYBRID_APPLICATION_ASSESSMENT_REQUIRED",
                "Application group did not provide a complete assessment",
            ));
            return Ok(());
        };
        let mut application_run = run.clone();
        application_run.started_at_unix_ns = self.application_started_at_unix_ns.clone();
        application_run.elapsed_us = self.application_elapsed_us;
        application_run.required_cases = assessment.required_cases.clone();
        application_run.cases = assessment.cases.clone();
        application_run.artifacts.clear();
        application_run.result = self.report.result.clone();
        application_run.open_obligations.clear();
        application_run.provenance.protocol_version = 2;
        application_run.provenance.adapter_hash = self.report.plan_hash.clone();
        application_run.provenance.harness_hash = receipt_hash.clone();
        application_run.provenance.implementation = self.report.source_inputs.clone();
        application_run.provenance.dependencies = self.report.materialized_inputs.clone();
        application_run.provenance.build_configuration = self.report.build_inputs.clone();
        application_run.provenance.incomplete = self.report.issues.clone();
        application_run.provenance.runtime_version.clear();
        let proof = HybridExecutionProof {
            schema_version: 1,
            application: ApplicationExecutionProof {
                schema_version: 1,
                plan: self.assignment.clone(),
                assessment: assessment.clone(),
                source_inputs: self.report.source_inputs.clone(),
                build_inputs: self.report.build_inputs.clone(),
                materialized_inputs: self.report.materialized_inputs.clone(),
                toolchain_inputs: self.report.toolchain_inputs.clone(),
                receipt_hash,
                phases: self
                    .report
                    .phases
                    .iter()
                    .map(|phase| ApplicationPhaseProof {
                        phase: phase.phase.clone(),
                        exit_code: phase.audit.exit_code,
                        runtime_hash: phase.runtime.executable_hash.clone(),
                        supervised: phase
                            .audit
                            .supervision
                            .as_ref()
                            .is_some_and(|supervision| supervision.process_tree),
                    })
                    .collect(),
            },
            application_run,
            adapter_group: self.adapter_group.clone(),
            adapter_started_at_unix_ns,
            adapter_elapsed_us,
        };
        if reused_cases.is_empty() && !proof.validates(run) {
            run.provenance.incomplete.push(issue(
                "HYBRID_COMPLETE_PROOF_REQUIRED",
                "Every original case needs fresh source-matched native backend evidence",
            ));
        }
        if reused_cases.is_empty() {
            artifacts.push(VerificationArtifact::new(
                HYBRID_EXECUTION_MEDIA_TYPE,
                serde_json::to_vec(&proof)?,
            ));
        } else {
            artifacts.push(VerificationArtifact::new(
                INCREMENTAL_HYBRID_EXECUTION_MEDIA_TYPE,
                serde_json::to_vec(&IncrementalHybridExecutionProof {
                    schema_version: 1,
                    cohort: proof,
                    reused_cases: reused_cases.clone(),
                })?,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_native_report_rejects_existing_evidence_before_execution() {
        let dir = std::env::temp_dir().join(format!(
            "dctx0-hybrid-report-test-{}-{}",
            std::process::id(),
            now().unwrap()
        ));
        let evidence = dir.join(".dctx0ctl/application-tests");
        fs::create_dir_all(&evidence).unwrap();
        let path = evidence.join("old.json");
        fs::write(&path, b"historical evidence").unwrap();
        assert!(require_new_native_report(&dir, ".dctx0ctl/application-tests/old.json").is_err());
        assert_eq!(fs::read(path).unwrap(), b"historical evidence");
        assert!(require_new_native_report(&dir, ".dctx0ctl/application-tests/new.json").is_ok());
        assert!(require_new_native_report(&dir, "elsewhere/report.json").is_err());
    }

    #[test]
    fn hybrid_runtime_freshness_rejects_changed_captured_and_missing_external_runtimes() {
        let mut report: ApplicationNativeReport = serde_json::from_value(serde_json::json!({
            "schema_version":1,"group":"native","adoption_authority":false,
            "plan_hash":"plan","source_inputs":{},"captured_root":"/isolated/capture",
            "build_inputs":{},"materialized_inputs":{"artifact/test":"original"},
            "phases":[{"phase":"native-tests","recipe":{"program":"test","arguments":[]},
                "runtime":{"schema_version":1,"executable_hash":"original","executable_path":"/isolated/capture/artifact/test","executable_bytes":1,"initial_environment_hash":"env"},
                "audit":{"pid":1,"stderr":"","received_messages":0,"exit_code":0},"elapsed_us":1}],
            "assessment":null,"issues":[],"result":{"status":"PASS"}
        })).unwrap();
        assert!(application_runtime_freshness(&report).is_empty());
        report.phases[0].runtime.executable_hash = "changed".into();
        let issues = application_runtime_freshness(&report);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "ADOPTION_RUNTIME_CHANGED");
        report.phases[0].runtime.executable_path = "/nonexistent/dctx0-test-runtime".into();
        assert_eq!(
            application_runtime_freshness(&report)[0].code,
            "ADOPTION_RUNTIME_CHANGED"
        );
    }
}
