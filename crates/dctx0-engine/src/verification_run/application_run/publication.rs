//! Bridge actual managed application execution into the existing native task
//! transaction. Replay and imported/served reports never invoke this bridge.
use super::*;
use dctx0_verification::application_evidence::{
    APPLICATION_EXECUTION_MEDIA_TYPE, ApplicationExecutionProof, ApplicationPhaseProof,
    INCREMENTAL_APPLICATION_EXECUTION_MEDIA_TYPE, IncrementalApplicationExecutionProof,
};

impl Project {
    #[allow(clippy::too_many_arguments)]
    pub async fn verify_application_tests(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        recipe: &str,
        output: &str,
        cancel: &NativeCancellation,
        no_adopt: bool,
    ) -> Result<ApplicationNativeReport> {
        self.verify_application_tests_inner(
            application,
            plan,
            group,
            recipe,
            output,
            cancel,
            no_adopt,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn verify_application_tests_incremental(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        recipe: &str,
        output: &str,
        cancel: &NativeCancellation,
        options: &super::super::reuse_execution::NativeReuseOptions,
    ) -> Result<ApplicationNativeReport> {
        self.verify_application_tests_inner(
            application,
            plan,
            group,
            recipe,
            output,
            cancel,
            true,
            Some(options),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn verify_application_tests_incremental_with_adoption(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        recipe: &str,
        output: &str,
        cancel: &NativeCancellation,
        options: &super::super::reuse_execution::NativeReuseOptions,
        no_adopt: bool,
    ) -> Result<ApplicationNativeReport> {
        self.verify_application_tests_inner(
            application,
            plan,
            group,
            recipe,
            output,
            cancel,
            no_adopt,
            Some(options),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn verify_application_tests_inner(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        recipe: &str,
        output: &str,
        cancel: &NativeCancellation,
        no_adopt: bool,
        options: Option<&super::super::reuse_execution::NativeReuseOptions>,
    ) -> Result<ApplicationNativeReport> {
        let fresh = NativeReuseOptions::fresh();
        let options = options.or(Some(&fresh));
        let (root, bundle, _) = self.read_application_testing_bundle(application, plan)?;
        let (_, config) = self.verification_configuration(Some(&root))?;
        let program = &bundle.plan.program;
        if let Some(options) = options {
            super::super::reuse_preflight::guard(self, &root, program, options, cancel).await?;
        }
        let (_, bundle_path) = crate::verification_setup::safe_file(&root, plan)?;
        let bundle_artifact = VerificationArtifact::new(
            dctx0_verification::application_reuse::APPLICATION_BUNDLE_MEDIA_TYPE,
            serde_json::to_vec(
                &dctx0_verification::application_reuse::ApplicationBundleReceipt {
                    schema_version: 1,
                    path: plan.into(),
                    raw_bundle: fs::read_to_string(bundle_path)?,
                    canonical_bundle: serde_json::to_string(&bundle)?,
                },
            )?,
        );
        let snapshot = self.load(&program.snapshot)?;
        // Capture before running any application code, never from a saved PASS.
        let (pin, mut incomplete) = pins::capture(self, &root, &config.target, program, &snapshot)?;
        let incremental_done_versions = if options.is_some() && !no_adopt {
            Some(adoption::capture_done_versions(self, pin.as_ref())?)
        } else {
            None
        };
        let verifier = crate::native::VerifierIdentity::capture()
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let started_at_unix_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| Error::Invalid(error.to_string()))?
            .as_nanos()
            .to_string();
        let started = Instant::now();
        let mut prepared = if let Some(options) = options {
            Some(
                super::super::reuse_execution::PreparedReuse::capture(
                    self, &root, program, options, cancel,
                )
                .await?,
            )
        } else {
            None
        };
        if let Some(prepared) = &prepared {
            incomplete.extend(prepared.input_issues());
        }
        if let Some(prepared) = &mut prepared
            && let Some(parent) = &prepared.parent
            && prepared.parent_binding.is_some()
        {
            let store = self.store(AccessMode::ReadOnly)?;
            let reference = parent.run.artifacts.iter().find(|artifact| {
                artifact.media_type == "application/vnd.dctx0.application-native-receipt+json"
            });
            let bundle_reference = parent.run.artifacts.iter().find(|artifact| {
                artifact.media_type
                    == dctx0_verification::application_reuse::APPLICATION_BUNDLE_MEDIA_TYPE
            });
            prepared.parent_binding = if let (Some(reference), Some(bundle_reference)) =
                (reference, bundle_reference)
            {
                let receipt = store.verification_artifact(&parent.id, &reference.hash)?;
                let old_bundle = store.verification_artifact(&parent.id, &bundle_reference.hash)?;
                let parent_plan =
                    store.verification_artifact(&parent.id, &parent.run.provenance.plan_hash)?;
                let parent_program: ProgramPlan = serde_json::from_slice(&parent_plan.contents)?;
                dctx0_verification::application_reuse::binding_with_bundle(
                    &parent_program,
                    &parent.run,
                    prepared
                        .parent_binding
                        .as_ref()
                        .expect("parent binding checked")
                        .external_inputs_receipt_hash
                        .clone(),
                    &receipt.contents,
                    &old_bundle.contents,
                )
            } else {
                None
            };
        }
        #[cfg(unix)]
        let mut execution = prepared.as_mut().map(|prepared| ApplicationReuseExecution {
            prepared,
            configuration_hash: digest(
                &serde_json::to_vec(&(&config, &bundle.plan.configuration))
                    .expect("serializable configuration"),
            ),
            task_pin: pin.clone(),
            selected: None,
            bundle_artifact: &bundle_artifact.contents,
        });
        #[cfg(unix)]
        let mut report = self
            .run_application_tests_with_report_policy(
                &root,
                plan,
                group,
                recipe,
                output,
                cancel,
                false,
                None,
                execution.as_mut(),
            )
            .await?;
        #[cfg(not(unix))]
        let mut report = self
            .run_application_tests(&root, plan, group, recipe, output, cancel)
            .await?;
        #[cfg(unix)]
        let selected = execution.and_then(|execution| execution.selected);
        let _lock = self.lock()?;
        if report.source_inputs != Capture::application_inventory(&root, config.capture_budgets())?
            || digest(&serde_json::to_vec(
                &self.read_application_testing_bundle(&root, plan)?.1,
            )?) != report.plan_hash
            || dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash
        {
            incomplete.push(issue(
                "ADOPTION_INPUTS_CHANGED",
                "Source inputs or requirements changed before publication",
            ));
        }
        if verifier
            != crate::native::VerifierIdentity::capture()
                .map_err(|error| Error::Invalid(error.to_string()))?
        {
            incomplete.push(issue(
                "ADOPTION_VERIFIER_CHANGED",
                "Managing verifier identity changed",
            ));
        }
        incomplete.extend(super::super::hybrid_run::application_runtime_freshness(
            &report,
        ));
        incomplete.extend(report.issues.clone());
        if cancel.is_cancelled() {
            incomplete.push(issue(
                "ADOPTION_CANCELLED",
                "Cancelled executions cannot deliver tasks",
            ));
        }
        let manifest = dctx0_verification::case_manifest(program);
        let mut artifacts = vec![
            VerificationArtifact::new(
                "application/vnd.dctx0.plan+json",
                serde_json::to_vec(program)?,
            ),
            VerificationArtifact::new(
                "application/vnd.dctx0.case-manifest+json",
                serde_json::to_vec(&manifest)?,
            ),
            VerificationArtifact::new(
                "application/vnd.dctx0.captured-input-inventory+json",
                serde_json::to_vec(&report.source_inputs)?,
            ),
            VerificationArtifact::new(
                "application/vnd.dctx0.application-native-receipt+json",
                serde_json::to_vec(&report)?,
            ),
            VerificationArtifact::new(
                "application/vnd.dctx0.verifier-identity+json",
                serde_json::to_vec(&verifier)?,
            ),
            VerificationArtifact::new(
                "application/vnd.dctx0.assessment-version+json",
                serde_json::to_vec(
                    &serde_json::json!({"schema_version":1,"version":dctx0_verification::application_session::APPLICATION_ASSESSMENT_VERSION}),
                )?,
            ),
        ];
        if let Some(phase) = report
            .phases
            .iter()
            .find(|phase| phase.phase == "native-tests")
        {
            artifacts.push(VerificationArtifact::new(
                "application/vnd.dctx0.runtime-identity+json",
                serde_json::to_vec(&phase.runtime)?,
            ));
        }
        let mut run = VerificationRun {
            schema_version: 1,
            origin: VerificationRunOrigin::Native,
            started_at_unix_ns,
            elapsed_us: started.elapsed().as_micros().min(u64::MAX as u128) as u64,
            provenance: VerificationProvenance {
                snapshot: program.snapshot.clone(),
                check_occurrence: program.check_occurrence as i64,
                plan_hash: artifacts[0].reference.hash.clone(),
                implementation: report.source_inputs.clone(),
                dependencies: report.materialized_inputs.clone(),
                adapter_hash: report.plan_hash.clone(),
                configuration_hash: digest(&serde_json::to_vec(&(
                    &config,
                    &bundle.plan.configuration,
                ))?),
                harness_hash: artifacts[3].reference.hash.clone(),
                interpreter_version: dctx0_interpreter::INTERPRETER_VERSION.into(),
                generator_version: program.generator_version.clone(),
                protocol_version:
                    dctx0_target_sdk::application_protocol::APPLICATION_PROTOCOL_VERSION,
                target: config.target.clone(),
                runtime_version: String::new(),
                build_configuration: report.build_inputs.clone(),
                task_pin: pin,
                incomplete,
            },
            budgets: program.budgets.clone(),
            selected_subjects: program.scope.subjects.clone(),
            required_subjects: program.closure.clone(),
            not_selected: program.not_selected.clone(),
            required_cases: manifest.required_cases.clone(),
            cases: report
                .assessment
                .as_ref()
                .map(|assessment| assessment.cases.clone())
                .unwrap_or_default(),
            open_obligations: manifest.open_obligations.clone(),
            bounded_obligations: manifest.bounded_obligations.clone(),
            artifacts: vec![],
            result: report.result.clone(),
        };
        #[cfg(unix)]
        let failed_cached_native = selected.is_some()
            && report.phases.iter().any(|phase| {
                phase.phase == "native-tests" && phase.audit.exit_code.is_some_and(|code| code != 0)
            });
        #[cfg(not(unix))]
        let failed_cached_native = false;
        if failed_cached_native {
            prepared
                .as_mut()
                .expect("selected reuse has prepared inputs")
                .discard_cached_records(&mut run);
            run.provenance.incomplete.push(issue(
                "APPLICATION_CACHED_CASES_WITHDRAWN",
                "Historical cases were withdrawn after a native test failure; missing cases were not freshly executed",
            ));
        }
        if failed_cached_native {
            run.provenance.incomplete.push(issue(
                "APPLICATION_COMPLETE_PROOF_REQUIRED",
                "Failed native execution has no complete fresh-suite proof",
            ));
        } else if let Some(assessment) = &report.assessment {
            let proof = ApplicationExecutionProof {
                schema_version: 1,
                plan: bundle.plan.clone(),
                assessment: assessment.clone(),
                source_inputs: report.source_inputs.clone(),
                build_inputs: report.build_inputs.clone(),
                materialized_inputs: report.materialized_inputs.clone(),
                toolchain_inputs: report.toolchain_inputs.clone(),
                phases: report
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
                            .is_some_and(|capabilities| capabilities.process_tree),
                    })
                    .collect(),
                receipt_hash: run.provenance.harness_hash.clone(),
            };
            if !proof.validates(&run) {
                run.provenance.incomplete.push(issue(
                    "APPLICATION_COMPLETE_PROOF_REQUIRED",
                    "Only the complete centrally assigned native suite can deliver tasks",
                ));
            }
            let reused = prepared
                .as_ref()
                .map(|state| state.reused_cases().clone())
                .unwrap_or_default();
            if reused.is_empty() {
                artifacts.push(VerificationArtifact::new(
                    APPLICATION_EXECUTION_MEDIA_TYPE,
                    serde_json::to_vec(&proof)?,
                ));
            } else {
                artifacts.push(VerificationArtifact::new(
                    INCREMENTAL_APPLICATION_EXECUTION_MEDIA_TYPE,
                    serde_json::to_vec(&IncrementalApplicationExecutionProof {
                        schema_version: 1,
                        application: proof,
                        reused_cases: reused,
                    })?,
                ));
            }
        } else {
            run.provenance.incomplete.push(issue(
                "APPLICATION_ASSESSMENT_REQUIRED",
                "No central assessment",
            ));
        }
        let resolved =
            dctx0_verification::resolved_state_preconditions(&snapshot.ir, program, &run.cases);
        if !resolved.is_empty() {
            run.open_obligations
                .retain(|issue| !resolved.contains(issue));
            artifacts.push(VerificationArtifact::new(
                "application/vnd.dctx0.resolved-preconditions+json",
                serde_json::to_vec(&resolved)?,
            ));
        }
        let suites = dctx0_verification::suites::native_suite_policy_assessment(
            &snapshot.ir,
            program,
            &run.cases,
        );
        let resolved_suites = dctx0_verification::suites::resolved_native_suite_policy_obligations(
            &snapshot.ir,
            program,
            &run.cases,
        );
        run.open_obligations
            .retain(|issue| !resolved_suites.contains(issue));
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.suite-policy-assessment+json",
            serde_json::to_vec(&suites)?,
        ));
        if run.result == VerificationResult::Pass {
            if let Some(problem) = run
                .provenance
                .incomplete
                .first()
                .or(run.open_obligations.first())
            {
                run.result = VerificationResult::Unknown {
                    issue: problem.clone(),
                };
            }
        }
        if let Some(prepared) = &mut prepared {
            let current = dctx0_verification::application_reuse::binding_with_bundle(
                program,
                &run,
                prepared.receipt_artifact.reference.hash.clone(),
                &artifacts[3].contents,
                &bundle_artifact.contents,
            )
            .or_else(|| {
                // An incomplete native attempt still records its captured
                // inputs. It cannot acquire a reuse proof or task authority.
                prepared
                    .reused_cases()
                    .is_empty()
                    .then(|| {
                        dctx0_verification::incremental::binding(
                            program,
                            run.provenance.clone(),
                            prepared.receipt_artifact.reference.hash.clone(),
                        )
                        .ok()
                    })
                    .flatten()
            })
            .ok_or_else(|| {
                Error::Invalid("Cannot establish application reuse execution identity".into())
            })?;
            #[cfg(unix)]
            if !failed_cached_native
                && selected
                    .as_ref()
                    .is_some_and(|selected| selected != &current)
            {
                return Err(Error::Invalid(
                    "INCREMENTAL_BINDING_CHANGED: application execution identity changed".into(),
                ));
            }
            prepared
                .finish_binding(&root, program, &mut run, current, &mut artifacts, cancel)
                .await?;
            report.incremental = Some(prepared.summary(program, &run));
        }
        artifacts.push(bundle_artifact);
        run.artifacts = artifacts
            .iter()
            .map(|artifact| artifact.reference.clone())
            .collect();
        preserve_verification_history(&config, &mut run, &mut artifacts);
        let saved = self
            .store(AccessMode::ReadWrite)?
            .record_verification_run_owned_from(&self.key(), run, &artifacts, None)?;
        if !no_adopt {
            let (tasks, issues, proposal) = if let Some(done_versions) = &incremental_done_versions
            {
                adoption::publish_incremental(
                    self,
                    &root,
                    program,
                    &saved,
                    &artifacts,
                    done_versions,
                )?
            } else {
                adoption::publish(self, &root, program, &saved, &artifacts)?
            };
            report.adoption_authority = !tasks.is_empty();
            report.adopted_tasks = tasks;
            report.adoption_issues = issues;
            report.adoption_proposal = proposal;
        }
        if let Some(summary) = &mut report.incremental {
            summary.adoption_authority = !report.adopted_tasks.is_empty();
        }
        report.evidence = Some(saved);
        let (_, path) = crate::verification_setup::safe_file(&root, output)?;
        atomic_write(&path, &serde_json::to_vec_pretty(&report)?)?;
        Ok(report)
    }
}
