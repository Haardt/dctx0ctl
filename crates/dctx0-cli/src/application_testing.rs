use clap::{Args, Subcommand, ValueEnum};
use dctx0_engine::Project;
use dctx0_verification::application_session::ApplicationValueBoundary;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(crate) struct ApplicationTestingArgs {
    #[command(subcommand)]
    action: Action,
}
#[derive(Clone, Copy, ValueEnum)]
enum Boundary {
    JsonDecoder,
    ValueConstructor,
}
#[derive(Clone, Copy, ValueEnum)]
enum Form {
    All,
    Assertion,
    Decoder,
    Operation,
    StateChange,
    Scenario,
    Lifecycle,
    EntityIdentity,
}

#[derive(Subcommand)]
enum Action {
    /// Diagnose one centrally assigned adapter group; never deliver tasks.
    AdapterGroup {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        group: String,
    },
    /// Publish the shipped testing-only library; preserve authored tests.
    Library {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        target: String,
    },
    /// Rerun saved inputs in their native test; historical PASS grants no authority.
    Replay {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        group: String,
        #[arg(long)]
        recipe: String,
        #[arg(long)]
        evidence: String,
        #[arg(long)]
        case: String,
        #[arg(long)]
        report: String,
    },
    /// Execute native tests and attempt guarded task delivery unless --no-adopt.
    Run {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        group: String,
        #[arg(long)]
        recipe: String,
        #[arg(long)]
        report: String,
        /// Store diagnostic evidence without delivering tasks.
        #[arg(long)]
        no_adopt: bool,
        #[command(flatten)]
        reuse: crate::reuse_args::ReuseArgs,
    },
    /// Execute assigned adapter and application tests, optionally reusing eligible cases.
    Hybrid {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        group: String,
        #[arg(long)]
        recipe: String,
        /// New output path, relative to the application; existing files are preserved.
        #[arg(long)]
        report: String,
        #[arg(long)]
        no_adopt: bool,
        #[command(flatten)]
        reuse: crate::reuse_args::ReuseArgs,
    },
    /// Complete fresh hybrid evidence and source-bound authored reviews.
    HybridComplete {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        group: String,
        #[arg(long)]
        recipe: String,
        #[arg(long)]
        report: String,
        #[arg(long)]
        notes: PathBuf,
        /// Require the exact plan file bytes captured during explicit preparation.
        #[arg(long)]
        expected_plan_sha256: Option<String>,
        #[command(flatten)]
        reuse: crate::reuse_args::ReuseArgs,
    },
    /// Complete native scope and source-bound authored reviews with guarded delivery.
    Complete {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        group: String,
        #[arg(long)]
        recipe: String,
        #[arg(long)]
        report: String,
        #[arg(long)]
        notes: Option<PathBuf>,
        #[command(flatten)]
        reuse: crate::reuse_args::ReuseArgs,
    },
    /// Explicitly check/publish specs and write a diagnostic native test plan.
    Prepare {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        group: String,
        #[arg(long)]
        test_name: String,
        #[arg(long, value_enum, default_value = "decoder")]
        form: Form,
        /// Whole native group budget; does not alter corpus or interpreter budgets.
        #[arg(long,value_parser=clap::value_parser!(u64).range(1..=86_400))]
        group_seconds: Option<u64>,
        /// Exact Go registration whose decoder and projector were reviewed as stateless.
        #[arg(long)]
        stateless_decoder: Vec<String>,
        #[arg(long)]
        subject: Vec<String>,
        #[arg(long, value_enum, default_value = "json-decoder")]
        boundary: Boundary,
    },
    /// Serve one native group over a separate Unix channel; no task adoption.
    Serve {
        input: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        group: String,
        #[arg(long)]
        socket: String,
        /// Output relative to the application; diagnostic assessment only.
        #[arg(long)]
        report: String,
    },
}
pub(crate) fn run(
    args: ApplicationTestingArgs,
    database: Option<&Path>,
    json: bool,
    output: &mut dyn Write,
) -> Result<bool, String> {
    match args.action {
        Action::AdapterGroup {
            input,
            project,
            plan,
            group,
        } => {
            let source = Project::new(&input, database).map_err(crate::error_message)?;
            let (_, bundle, _) = source
                .read_application_testing_bundle(&project, &plan)
                .map_err(crate::error_message)?;
            let (_, configuration) = source
                .verification_configuration(Some(&project))
                .map_err(crate::error_message)?;
            let (_, generator) = crate::verification::execution_target(&configuration.target)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::error_message)?;
            let cancellation = dctx0_engine::NativeCancellation::default();
            let report = runtime
                .block_on(async {
                    let execution = source.run_adapter_execution_group(
                        &project,
                        &bundle.plan,
                        &group,
                        generator,
                        &cancellation,
                    );
                    tokio::pin!(execution);
                    tokio::select! {
                        result = &mut execution => result,
                        _ = tokio::signal::ctrl_c() => { cancellation.cancel(); execution.await },
                    }
                })
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &report)?;
            } else {
                writeln!(output, "Diagnostic adapter group {group}: {:?}; {} assigned cases; {} recorded. No task delivery; complete cohort required.",
                    report.evidence.run.result, report.evidence.run.required_cases.len(), report.evidence.run.cases.len()).map_err(crate::error_message)?;
            }
            Ok(report.evidence.run.result == dctx0_core::VerificationResult::Pass)
        }
        Action::Library { project, target } => {
            let library = application_library_for_target(&target)?;
            let publication = dctx0_engine::publish_application_test_library(&project, &library)
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &publication)?;
            } else {
                writeln!(output,"Published {} application-test library {} (protocol {}); {} pinned files. Production dependencies and authored tests are configured separately.",publication.target,publication.version,publication.protocol_version,publication.files.len()).map_err(crate::error_message)?;
            }
            Ok(true)
        }
        Action::Replay {
            input,
            project,
            plan,
            group,
            recipe,
            evidence,
            case,
            report,
        } => {
            let source = Project::new(&input, database).map_err(crate::error_message)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::error_message)?;
            let cancellation = dctx0_engine::NativeCancellation::default();
            let result = runtime
                .block_on(async {
                    let execution = source.replay_application_test(
                        &project,
                        &plan,
                        &group,
                        &recipe,
                        &evidence,
                        &case,
                        &report,
                        &cancellation,
                    );
                    tokio::pin!(execution);
                    tokio::select! {
                        result=&mut execution=>result,
                        _=tokio::signal::ctrl_c()=>{cancellation.cancel();execution.await},
                    }
                })
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &result)?;
            } else {
                writeln!(
                    output,
                    "Native finite-case replay: {:?}. Evidence: {report}. No task adoption.",
                    result.result
                )
                .map_err(crate::error_message)?;
            }
            Ok(matches!(
                result.result,
                dctx0_core::VerificationResult::Pass
            ))
        }
        Action::Run {
            input,
            project,
            plan,
            group,
            recipe,
            report,
            no_adopt,
            reuse,
        } => {
            let reuse = reuse.options()?;
            let source = Project::new(&input, database).map_err(crate::error_message)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::error_message)?;
            let cancellation = dctx0_engine::NativeCancellation::default();
            let result = runtime
                .block_on(async {
                    let options = Some(reuse);
                    let execution = async {
                        if let Some(options) = &options {
                            source
                                .verify_application_tests_incremental_with_adoption(
                                    &project,
                                    &plan,
                                    &group,
                                    &recipe,
                                    &report,
                                    &cancellation,
                                    options,
                                    no_adopt,
                                )
                                .await
                        } else {
                            source
                                .verify_application_tests(
                                    &project,
                                    &plan,
                                    &group,
                                    &recipe,
                                    &report,
                                    &cancellation,
                                    no_adopt,
                                )
                                .await
                        }
                    };
                    tokio::pin!(execution);
                    tokio::select! {
                        result=&mut execution=>result,
                        _=tokio::signal::ctrl_c()=>{cancellation.cancel();execution.await},
                    }
                })
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &result)?;
            } else {
                writeln!(
                    output,
                    "Managed finite-case assessment: {:?}. Evidence: {report}. Adopted tasks: {:?}; diagnostic: {}.",
                    result.result, result.adopted_tasks, no_adopt
                )
                .map_err(crate::error_message)?;
            }
            Ok(matches!(
                result.result,
                dctx0_core::VerificationResult::Pass
            ))
        }
        Action::Hybrid {
            input,
            project,
            plan,
            group,
            recipe,
            report,
            no_adopt,
            reuse,
        } => {
            let reuse = reuse.options()?;
            let source = Project::new(&input, database).map_err(crate::error_message)?;
            let (_, configuration) = source
                .verification_configuration(Some(&project))
                .map_err(crate::error_message)?;
            let (_, generator) = crate::verification::execution_target(&configuration.target)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::error_message)?;
            let cancellation = dctx0_engine::NativeCancellation::default();
            let options = dctx0_engine::HybridVerificationOptions {
                plan: &plan,
                application_group: &group,
                recipe: &recipe,
                native_report: &report,
            };
            let result = runtime
                .block_on(async {
                    let reuse = Some(reuse);
                    let execution = async {
                        if let Some(reuse) = &reuse {
                            source
                                .run_hybrid_verification_incremental_with_adoption(
                                    &project,
                                    &options,
                                    generator,
                                    &cancellation,
                                    reuse,
                                    no_adopt,
                                )
                                .await
                        } else {
                            source
                                .run_hybrid_verification(
                                    &project,
                                    &options,
                                    generator,
                                    &cancellation,
                                    no_adopt,
                                )
                                .await
                        }
                    };
                    tokio::pin!(execution);
                    tokio::select! {
                        result = &mut execution => result,
                        _ = tokio::signal::ctrl_c() => { cancellation.cancel(); execution.await },
                    }
                })
                .map_err(crate::error_message)?;
            let passed = result.evidence.run.result == dctx0_core::VerificationResult::Pass;
            if json {
                crate::render::json(output, &result)?;
            } else {
                let origin = if result
                    .incremental
                    .as_ref()
                    .is_some_and(|summary| summary.reused_cases > 0)
                {
                    "Incremental"
                } else {
                    "Fresh"
                };
                writeln!(output, "{origin} hybrid assessment: {:?}; {} mandatory cases; adopted tasks: {:?}; native report: {report}.",
                    result.evidence.run.result, result.evidence.run.required_cases.len(), result.adopted_tasks)
                    .map_err(crate::error_message)?;
                if let Some(summary) = &result.incremental {
                    writeln!(
                        output,
                        "Historical reused cases: {}; fresh cases: {}",
                        summary.reused_cases, summary.executed_cases
                    )
                    .map_err(crate::error_message)?;
                }
                for issue in &result.adoption_issues {
                    writeln!(output, "{}: {}", issue.code, issue.message)
                        .map_err(crate::error_message)?;
                }
            }
            Ok(passed)
        }
        Action::HybridComplete {
            input,
            project,
            plan,
            group,
            recipe,
            report,
            notes,
            expected_plan_sha256,
            reuse,
        } => {
            let reuse = reuse.options()?;
            let source = Project::new(&input, database).map_err(crate::error_message)?;
            let (_, configuration) = source
                .verification_configuration(Some(&project))
                .map_err(crate::error_message)?;
            let (provider, generator) =
                crate::verification::execution_target(&configuration.target)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::error_message)?;
            let cancellation = dctx0_engine::NativeCancellation::default();
            let options = dctx0_engine::HybridVerificationOptions {
                plan: &plan,
                application_group: &group,
                recipe: &recipe,
                native_report: &report,
            };
            let result = runtime
                .block_on(async {
                    let execution = async {
                        source
                            .complete_hybrid_verification_with_reuse(
                                &project,
                                &options,
                                &notes,
                                provider,
                                generator,
                                &cancellation,
                                expected_plan_sha256.as_deref(),
                                &reuse,
                            )
                            .await
                    };
                    tokio::pin!(execution);
                    tokio::select! {
                        result = &mut execution => result,
                        _ = tokio::signal::ctrl_c() => { cancellation.cancel(); execution.await },
                    }
                })
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &result)?;
            } else {
                writeln!(output, "Complete hybrid acceptance: {:?}; {} mandatory cases; {} tasks, {} unfinished; adopted: {:?}.",
                    result.result, result.mandatory_cases, result.tasks, result.open_tasks, result.adopted_tasks)
                    .map_err(crate::error_message)?;
            }
            Ok(result.result == dctx0_core::VerificationResult::Pass)
        }
        Action::Complete {
            input,
            project,
            plan,
            group,
            recipe,
            report,
            notes,
            reuse,
        } => {
            let reuse = reuse.options()?;
            let source = Project::new(&input, database).map_err(crate::error_message)?;
            let (_, config) = source
                .verification_configuration(Some(&project))
                .map_err(crate::error_message)?;
            let (provider, _) = crate::verification::execution_target(&config.target)?;
            let notes = notes.unwrap_or_else(|| {
                project.join(format!(
                    ".dctx0/testing/{}/inspection-notes.json",
                    config.target
                ))
            });
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::error_message)?;
            let cancellation = dctx0_engine::NativeCancellation::default();
            let result = runtime
                .block_on(async {
                    let execution = source.complete_application_tests_with_reuse(
                        &project,
                        &plan,
                        &group,
                        &recipe,
                        &report,
                        &notes,
                        provider,
                        &cancellation,
                        &reuse,
                    );
                    tokio::pin!(execution);
                    tokio::select! {
                        result=&mut execution=>result,
                        _=tokio::signal::ctrl_c()=>{cancellation.cancel();execution.await},
                    }
                })
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &result)?;
            } else {
                writeln!(output, "Complete application-test acceptance: {:?}; {} mandatory cases; {} tasks, {} unfinished. Adopted: {:?}; inspected: {:?}; source-bound reviews: {:?}.", result.result, result.mandatory_cases, result.tasks, result.open_tasks, result.adopted_tasks, result.inspected_tasks, result.reviewed_tasks).map_err(crate::error_message)?;
            }
            Ok(matches!(
                result.result,
                dctx0_core::VerificationResult::Pass
            ))
        }
        Action::Prepare {
            input,
            project,
            plan,
            group,
            test_name,
            form,
            group_seconds,
            stateless_decoder,
            subject,
            boundary,
        } => {
            let source = Project::new(&input, database).map_err(crate::error_message)?;
            let prepare = if matches!(form, Form::All) {
                Project::prepare_application_test_suite
            } else {
                Project::prepare_application_tests
            };
            let bundle = prepare(
                &source,
                &project,
                &dctx0_engine::ApplicationTestingPreparation {
                    plan: &plan,
                    group: &group,
                    test: &test_name,
                    boundary: match boundary {
                        Boundary::JsonDecoder => ApplicationValueBoundary::JsonDecoder,
                        Boundary::ValueConstructor => ApplicationValueBoundary::ValueConstructor,
                    },
                    form: match form {
                        Form::All => dctx0_verification::application_plan::TestForm::Decoder,
                        Form::Scenario => dctx0_verification::application_plan::TestForm::Scenario,
                        Form::Lifecycle => {
                            dctx0_verification::application_plan::TestForm::Lifecycle
                        }
                        Form::EntityIdentity => {
                            dctx0_verification::application_plan::TestForm::EntityIdentity
                        }
                        Form::Assertion => {
                            dctx0_verification::application_plan::TestForm::Assertion
                        }
                        Form::Decoder => dctx0_verification::application_plan::TestForm::Decoder,
                        Form::Operation => {
                            dctx0_verification::application_plan::TestForm::Operation
                        }
                        Form::StateChange => {
                            dctx0_verification::application_plan::TestForm::StateChange
                        }
                    },
                    subjects: subject.into_iter().collect(),
                    group_seconds,
                    stateless_decoders: stateless_decoder.into_iter().collect(),
                },
            )
            .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &bundle)?;
            } else {
                writeln!(output, "Prepared {} central cases; {} native cases. Plan: {plan}. No execution or task adoption.", bundle.plan.manifest.required_cases.len(), bundle.plan.registrations.iter().map(|r| r.cases.len()).sum::<usize>()).map_err(crate::error_message)?;
            }
            Ok(true)
        }
        Action::Serve {
            input,
            project,
            plan,
            group,
            socket,
            report,
        } => {
            let source = Project::new(&input, database).map_err(crate::error_message)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::error_message)?;
            let assessment = runtime
                .block_on(source.serve_application_tests(&project, &plan, &group, &socket, &report))
                .map_err(crate::error_message)?;
            if json {
                crate::render::json(output, &assessment)?;
            } else {
                writeln!(output, "Diagnostic assessment: {:?}; {} records, {} missing. Report: {report}. Native runner/build evidence and task adoption are separate.", assessment.result, assessment.cases.len(), assessment.missing_cases.len()).map_err(crate::error_message)?;
            }
            Ok(matches!(
                assessment.result,
                dctx0_core::VerificationResult::Pass
            ))
        }
    }
}

fn application_library_for_target(
    target: &str,
) -> Result<dctx0_target_sdk::ApplicationTestLibrary, String> {
    match target {
        #[cfg(feature = "go")]
        "go" => Ok(dctx0_target_go::application_test_library()),
        #[cfg(feature = "typescript")]
        "typescript" => Ok(dctx0_target_typescript::application_test_library()),
        _ => Err(format!(
            "Application testing library unavailable for target {target}"
        )),
    }
}
