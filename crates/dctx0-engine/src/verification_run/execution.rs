//! Execute mandatory cases; unsupported/unexecuted work never becomes PASS.
use super::*;
use dctx0_interpreter::{ValueContract, check_value};
use dctx0_verification::{
    VerificationPlan, VerificationScope, supports_generated_operation as supported_operation,
};
struct Pending {
    id: String,
    subject: String,
    kind: VerificationCaseKind,
    bindings: BTreeMap<String, VerificationValue>,
    setup: Vec<Invocation>,
    arguments: Vec<Vec<VerificationValue>>,
}
impl Pending {
    fn new(id: String, subject: &str, kind: VerificationCaseKind) -> Self {
        Self {
            id,
            subject: subject.into(),
            kind,
            bindings: BTreeMap::new(),
            setup: vec![],
            arguments: vec![],
        }
    }
}
fn failure(error: &NativeFailure) -> VerificationResult {
    unknown("NATIVE_EXECUTION_INCOMPLETE", error.to_string())
}
fn record(
    run: &mut VerificationRun,
    pending: Pending,
    worker: &NativeWorker,
    result: VerificationResult,
    elapsed: Instant,
    events_supported: bool,
) {
    let mut observations = Vec::new();
    let mut events = Vec::new();
    let mut resources: Option<Vec<ResourceObservation>> = None;
    let mut resources_complete = true;
    let mut states = Vec::new();
    for entry in worker.session_evidence() {
        match entry {
            NativeSessionEvidence::Invoked {
                invocation,
                observation,
                events: emitted,
                resources: accessed,
                ..
            } => {
                observations.push(InvocationObservation {
                    invocation: invocation.clone(),
                    observation: observation.clone(),
                });
                events.extend(emitted.clone());
                if let Some(accessed) = accessed {
                    resources
                        .get_or_insert_with(Vec::new)
                        .extend(accessed.clone());
                } else {
                    resources_complete = false;
                }
            }
            NativeSessionEvidence::Observed {
                phase,
                states: entities,
            } => {
                if let Some(phase) = phase {
                    states.push(VerificationStateEvidence {
                        phase: *phase,
                        phase_recorded: Some(true),
                        entities: entities.clone(),
                    });
                } else {
                    run.open_obligations.push(issue(
                        "STATE_EVIDENCE_PHASE_REQUIRED",
                        "State observation has no recorded before/after phase",
                    ));
                }
            }
            _ => {}
        }
    }
    run.cases.push(VerificationCaseRecord {
        id: pending.id,
        subject: pending.subject,
        kind: pending.kind,
        bindings: pending.bindings,
        setup: pending.setup,
        arguments: pending.arguments,
        observations,
        events: events_supported.then_some(events),
        resources: if resources_complete { resources } else { None },
        states,
        result,
        elapsed_us: elapsed.elapsed().as_micros().min(u64::MAX as u128) as u64,
    });
}
fn preserve_prefix(
    artifacts: &mut Vec<VerificationArtifact>,
    id: &str,
    worker: &NativeWorker,
) -> Result<()> {
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.session+json",
        serde_json::to_vec(
            &serde_json::json!({"case_id":id,"evidence":worker.session_evidence()}),
        )?,
    ));
    Ok(())
}
pub(super) struct Input<'a> {
    pub ir: &'a WorkspaceIr,
    pub root: &'a Path,
    pub adapter: &'a str,
    pub generated: &'a BTreeSet<String>,
    pub plan: &'a ProgramPlan,
    pub lifecycles: &'a VerificationLifecycleFixtures,
    pub cancel: &'a NativeCancellation,
    pub limits: NativeLimits,
}
async fn aborted(run: &mut VerificationRun, worker: &mut NativeWorker, id: &str) {
    let cleanup = worker.abort().await;
    run.open_obligations.push(issue(
        "NATIVE_RUN_ABORTED",
        format!("{id}; cleanup: {cleanup:?}"),
    ));
}
pub(super) async fn execute(
    input: Input<'_>,
    recipe: &ExecutionRecipe,
    directory: &Path,
    run: &mut VerificationRun,
    artifacts: &mut Vec<VerificationArtifact>,
) -> Result<()> {
    let Input {
        ir,
        root,
        adapter,
        generated,
        plan,
        lifecycles,
        cancel,
        limits,
    } = input;
    let manifest = dctx0_verification::case_manifest(plan);
    run.required_cases = manifest.required_cases.clone();
    run.open_obligations
        .extend(manifest.open_obligations.clone());
    run.bounded_obligations
        .extend(manifest.bounded_obligations.clone());
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.case-manifest+json",
        serde_json::to_vec(&manifest)?,
    ));
    if manifest
        .open_obligations
        .iter()
        .any(|issue| issue.code == "DUPLICATE_CASE_ID")
    {
        return Ok(());
    }

    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.native-limits+json",
        serde_json::to_vec(&serde_json::json!({"schema_version":1,"limits":limits}))?,
    ));
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.native-supervision+json",
        serde_json::to_vec(&limits.supervision_capabilities())?,
    ));
    let mut worker = match NativeWorker::spawn(recipe, directory, limits).await {
        Ok(worker) => worker,
        Err(error) => {
            run.open_obligations
                .push(issue("NATIVE_INITIALIZATION", error.to_string()));
            return Ok(());
        }
    };
    let described = worker
        .request(WorkerRequest::Describe, cancel, &mut reject)
        .await;
    let description = match described {
        Ok(WorkerReply::Described { description })
            if description.protocol_version == SDK_VERSION =>
        {
            description
        }
        other => {
            run.open_obligations.push(issue(
                "ADAPTER_DESCRIPTION",
                format!("No valid adapter description: {other:?}"),
            ));
            let _ = worker.abort().await;
            return Ok(());
        }
    };
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.adapter-description+json",
        serde_json::to_vec(&description)?,
    ));
    for artifact in &description.artifacts {
        if Path::new(artifact).is_absolute()
            || Path::new(artifact)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            run.provenance
                .incomplete
                .push(issue("IMPLEMENTATION_ARTIFACT_OUTSIDE_CAPTURE", artifact));
        }
    }
    let events_supported = description.capabilities.iter().any(|c| c == "events");
    for capability in &plan.capabilities {
        if let Some(required) = capability.adapter_capability()
            && !description
                .capabilities
                .iter()
                .any(|offered| offered == required)
        {
            run.open_obligations.push(issue(
                "ADAPTER_CAPABILITY_UNAVAILABLE",
                format!("The selected plan requires adapter capability {required}"),
            ));
        }
    }
    if !events_supported {
        run.open_obligations.push(issue(
            "EVENT_OBSERVER_UNAVAILABLE",
            "No actual event observer is declared",
        ));
    }
    match worker
        .request(WorkerRequest::Metrics, cancel, &mut reject)
        .await
    {
        Ok(WorkerReply::Measured {
            runtime_version, ..
        }) => run.provenance.runtime_version = runtime_version,
        other => {
            run.open_obligations.push(issue(
                "RUNTIME_PROVENANCE",
                format!("Runtime evidence unavailable: {other:?}"),
            ));
            let _ = worker.abort().await;
            return Ok(());
        }
    }
    match worker.runtime_identity() {
        Ok(identity) => artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.runtime-identity+json",
            serde_json::to_vec(&identity)?,
        )),
        Err(error) => run
            .provenance
            .incomplete
            .push(issue("RUNTIME_IDENTITY_UNAVAILABLE", error.to_string())),
    }
    super::dependencies::resolve_native(
        &mut worker,
        root,
        adapter,
        generated,
        run,
        artifacts,
        cancel,
    )
    .await?;
    for contract in &plan.contracts {
        for (rejection, index, value) in contract
            .corpus
            .values
            .iter()
            .enumerate()
            .map(|(i, v)| (false, i, v))
            .chain(
                contract
                    .rejection_corpus
                    .values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (true, i, v)),
            )
        {
            let id = format!(
                "{}:{}:{index}",
                contract.subject,
                if rejection { "rejection" } else { "value" }
            );
            let mut pending = Pending::new(
                id.clone(),
                &contract.subject,
                VerificationCaseKind::Contract,
            );
            pending.bindings.insert("value".into(), value.clone());
            pending.bindings.insert(
                "expect_rejection".into(),
                VerificationValue::Boolean(rejection),
            );
            let started = Instant::now();
            let result = worker
                .execute_value_case(&id, &contract.subject, value, cancel)
                .await;
            let fatal = result.is_err();
            let result = match result {
                Ok(ValueCheck::Unavailable { reason }) => {
                    unknown("NATIVE_VALUE_UNAVAILABLE", reason)
                }
                Ok(ValueCheck::Rejected { .. }) if rejection => VerificationResult::Pass,
                Ok(ValueCheck::Rejected { reason }) => VerificationResult::Fail {
                    issue: issue("NATIVE_VALUE_REJECTED", reason),
                },
                Ok(ValueCheck::Accepted { .. }) if rejection => VerificationResult::Fail {
                    issue: issue(
                        "NATIVE_INVALID_VALUE_ACCEPTED",
                        "Production accepted a centrally rejected contract input",
                    ),
                },
                Ok(ValueCheck::Accepted { value: actual }) => {
                    pending
                        .bindings
                        .insert("native_value".into(), actual.clone());
                    match check_value(ir, &contract.ty, &actual, &plan.budgets) {
                        ValueContract::Accepted if &actual == value => VerificationResult::Pass,
                        ValueContract::Accepted => VerificationResult::Fail {
                            issue: issue(
                                "NATIVE_VALUE_CHANGED",
                                "Adapter changed a canonical input",
                            ),
                        },
                        ValueContract::Rejected { reason } => VerificationResult::Fail {
                            issue: issue("NATIVE_VALUE_CONTRACT", reason),
                        },
                        ValueContract::Unknown { issue } => VerificationResult::Unknown { issue },
                    }
                }
                Err(error) => failure(&error),
            };
            preserve_prefix(artifacts, &id, &worker)?;
            record(run, pending, &worker, result, started, events_supported);
            if fatal {
                aborted(run, &mut worker, &id).await;
                return Ok(());
            }
        }
    }
    let pure = VerificationPlan {
        schema_version: 1,
        snapshot: plan.snapshot.clone(),
        check_occurrence: plan.check_occurrence,
        scope: VerificationScope {
            subjects: plan.scope.subjects.clone(),
            tasks: plan.scope.tasks.clone(),
        },
        budgets: plan.budgets.clone(),
        obligations: vec![],
    };
    let observers = dctx0_verification::OperationObservers {
        states: description.capabilities.iter().any(|c| c == "state"),
        calls: description
            .capabilities
            .iter()
            .any(|c| c == "call-observation"),
        events: events_supported,
    };
    for operation in &plan.operations {
        for input in &operation.cases {
            let mut pending = Pending::new(
                input.id.clone(),
                &operation.operation.id,
                VerificationCaseKind::Operation,
            );
            pending.setup = input.setup.clone();
            pending.arguments.push(input.invocation.arguments.clone());
            let started = Instant::now();
            let result = worker
                .execute_operation_contract(
                    ir,
                    operation,
                    input,
                    (&plan.budgets, observers, cancel),
                )
                .await;
            let fatal = result.is_err();
            if let Ok(assessed) = &result {
                run.open_obligations
                    .extend(assessed.assessment.open.clone());
            }
            artifacts.push(VerificationArtifact::new(
                "application/vnd.dctx0.operation-assessment+json",
                serde_json::to_vec(&serde_json::json!({"case_id":input.id,"assessment":result}))?,
            ));
            let result = result
                .map(|r| r.assessment.result)
                .unwrap_or_else(|e| failure(&e));
            preserve_prefix(artifacts, &input.id, &worker)?;
            record(run, pending, &worker, result, started, events_supported);
            if fatal {
                aborted(run, &mut worker, &input.id).await;
                return Ok(());
            }
        }
    }
    for operation in plan.operations.iter().filter(|o| supported_operation(o)) {
        for (index, bindings) in operation.inputs.bindings.iter().enumerate() {
            let id = format!("{}:operation:{index}", operation.operation.id);
            let arguments = operation
                .operation
                .parameters
                .iter()
                .map(|p| bindings[&p.name].clone())
                .collect::<Vec<_>>();
            let invocation = Invocation {
                subject: operation.operation.id.clone(),
                arguments: arguments.clone(),
            };
            let mut pending = Pending::new(
                id.clone(),
                &operation.operation.id,
                VerificationCaseKind::Operation,
            );
            pending.bindings = bindings.clone();
            pending.setup = operation.generated_setup(bindings);
            pending.arguments.push(arguments);
            let started = Instant::now();
            let pure = operation.operation.pure && operation.operation.failures.is_empty();
            let result = if pure {
                worker
                    .execute_pure_operation(
                        ir,
                        &operation.operation,
                        invocation.clone(),
                        &plan.budgets,
                        cancel,
                    )
                    .await
            } else {
                let input = dctx0_verification::OperationCaseInput {
                    id: id.clone(),
                    invocation: invocation.clone(),
                    setup: pending.setup.clone(),
                    mocks: vec![],
                };
                let assessed = worker
                    .execute_operation_contract(
                        ir,
                        operation,
                        &input,
                        (&plan.budgets, observers, cancel),
                    )
                    .await;
                if let Ok(report) = &assessed {
                    run.open_obligations.extend(report.assessment.open.clone());
                }
                artifacts.push(VerificationArtifact::new(
                    "application/vnd.dctx0.operation-assessment+json",
                    serde_json::to_vec(&serde_json::json!({"case_id":id,"assessment":assessed}))?,
                ));
                assessed.map(|report| report.assessment.result)
            };
            let fatal = result.is_err();
            let result = result.unwrap_or_else(|e| failure(&e));
            preserve_prefix(artifacts, &id, &worker)?;
            record(
                run,
                pending,
                &worker,
                result.clone(),
                started,
                events_supported,
            );
            if pure && matches!(result, VerificationResult::Fail { .. }) && !fatal {
                let shrink_started = Instant::now();
                let (shrink, evidence) = super::shrink::pure(
                    &mut worker,
                    ir,
                    operation,
                    invocation,
                    &result,
                    &plan.budgets,
                    cancel,
                )
                .await;
                artifacts.push(VerificationArtifact::new(
                    "application/vnd.dctx0.shrink+json",
                    serde_json::to_vec(&serde_json::json!({"case_id":id,"report":shrink,"native_attempts":evidence,"elapsed_us":shrink_started.elapsed().as_micros()}))?,
                ));
            }
            if fatal {
                aborted(run, &mut worker, &id).await;
                return Ok(());
            }
        }
    }
    for property in &plan.properties {
        for assertion in &property.assertions {
            for case in &assertion.cases {
                let mut pending = Pending::new(
                    case.id.clone(),
                    &property.subject,
                    VerificationCaseKind::Assertion,
                );
                pending.bindings = case.variables.clone();
                let started = Instant::now();
                let result = worker.execute_pure_case(ir, &pure, case, cancel).await;
                let fatal = result.is_err();
                let result = result
                    .map(|r| r.assessment.result)
                    .unwrap_or_else(|e| failure(&e));
                preserve_prefix(artifacts, &case.id, &worker)?;
                record(run, pending, &worker, result, started, events_supported);
                if fatal {
                    aborted(run, &mut worker, &case.id).await;
                    return Ok(());
                }
            }
        }
    }
    for example in &plan.examples {
        if !events_supported
            && example
                .expectations
                .iter()
                .any(|e| matches!(e, dctx0_verification::Expectation::Emitted { .. }))
        {
            run.open_obligations
                .push(issue("EXAMPLE_EVENT_OBSERVER", &example.subject));
            continue;
        }
        let mut pending = Pending::new(
            example.subject.clone(),
            &example.subject,
            VerificationCaseKind::Example,
        );
        pending.bindings = example.bindings.clone();
        let started = Instant::now();
        let result = worker
            .execute_example(ir, example, &plan.budgets, cancel)
            .await;
        let fatal = result.is_err();
        artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.example-assessment+json",
            serde_json::to_vec(&result)?,
        ));
        let result = result.map(|r| r.result).unwrap_or_else(|e| failure(&e));
        preserve_prefix(artifacts, &example.subject, &worker)?;
        record(run, pending, &worker, result, started, events_supported);
        if fatal {
            aborted(run, &mut worker, &example.subject).await;
            return Ok(());
        }
    }
    for lifecycle in &plan.lifecycles {
        for path in &lifecycle.paths {
            let Some(input) = lifecycles.paths.get(&path.id) else {
                run.open_obligations
                    .push(issue("LIFECYCLE_FIXTURE_REQUIRED", &path.id));
                continue;
            };
            if !description.capabilities.iter().any(|c| c == "state") {
                run.open_obligations
                    .push(issue("LIFECYCLE_OBSERVER_REQUIRED", &path.id));
                continue;
            }
            let mut pending = Pending::new(
                path.id.clone(),
                &lifecycle.subject,
                VerificationCaseKind::Lifecycle,
            );
            pending.setup = input.setup.clone();
            pending.arguments = input.arguments.clone();
            pending
                .bindings
                .insert("keys".into(), VerificationValue::List(input.keys.clone()));
            let started = Instant::now();
            let result = worker
                .execute_lifecycle(ir, lifecycle, path, input, &plan.budgets, cancel)
                .await;
            let fatal = result.is_err();
            artifacts.push(VerificationArtifact::new(
                "application/vnd.dctx0.lifecycle-assessment+json",
                serde_json::to_vec(&serde_json::json!({"case_id":path.id,"assessment":result}))?,
            ));
            let failing_assessment = result
                .as_ref()
                .ok()
                .filter(|result| {
                    matches!(result.assessment.result, VerificationResult::Fail { .. })
                })
                .map(|result| result.assessment.clone());
            let result = result
                .map(|r| r.assessment.result)
                .unwrap_or_else(|e| failure(&e));
            preserve_prefix(artifacts, &path.id, &worker)?;
            record(run, pending, &worker, result, started, events_supported);
            if let Some(assessment) = failing_assessment {
                let shrink_started = Instant::now();
                let (report, evidence) = super::shrink::lifecycle(
                    &mut worker,
                    ir,
                    lifecycle,
                    path,
                    input,
                    &assessment,
                    (&plan.budgets, cancel),
                )
                .await;
                artifacts.push(VerificationArtifact::new("application/vnd.dctx0.lifecycle-shrink+json",
                    serde_json::to_vec(&serde_json::json!({"case_id":path.id,"report":report,"native_attempts":evidence,"elapsed_us":shrink_started.elapsed().as_micros()}))?));
            }
            if fatal {
                aborted(run, &mut worker, &path.id).await;
                return Ok(());
            }
        }
    }
    for contract in &plan.contracts {
        if !contract.identity_requirements.is_empty() {
            run.cases.push(dctx0_verification::entity_identity_case(
                ir,
                &contract.subject,
                &run.cases,
                &plan.budgets,
            ));
        }
    }
    let suite_policy =
        dctx0_verification::suites::native_suite_policy_assessment(ir, plan, &run.cases);
    artifacts.push(VerificationArtifact::new(
        "application/vnd.dctx0.suite-policy-assessment+json",
        serde_json::to_vec(&suite_policy)?,
    ));
    if generated
        .iter()
        .any(|path| path.ends_with("/module-observer.json") || path == "module-observer.json")
    {
        match worker
            .request(WorkerRequest::ObserveDependencies, cancel, &mut reject)
            .await
        {
            Ok(WorkerReply::DependenciesObserved { evidence }) => {
                super::dependencies::reconcile_observer(run, artifacts, &evidence)?;
            }
            other => run
                .provenance
                .incomplete
                .push(issue("MODULE_OBSERVER_UNAVAILABLE", format!("{other:?}"))),
        }
    }
    match worker
        .request(WorkerRequest::Metrics, cancel, &mut reject)
        .await
    {
        Ok(metrics @ WorkerReply::Measured { .. }) => artifacts.push(VerificationArtifact::new(
            "application/vnd.dctx0.runtime-metrics+json",
            serde_json::to_vec(&metrics)?,
        )),
        other => run
            .open_obligations
            .push(issue("RUNTIME_FINAL_METRICS", format!("{other:?}"))),
    }
    match worker
        .request(WorkerRequest::Shutdown, cancel, &mut reject)
        .await
    {
        Ok(WorkerReply::Bye) => match worker.finish().await {
            Ok(audit) => artifacts.push(VerificationArtifact::new(
                "application/vnd.dctx0.native-audit+json",
                serde_json::to_vec(&audit)?,
            )),
            Err(error) => run
                .open_obligations
                .push(issue("NATIVE_CLEANUP", error.to_string())),
        },
        other => {
            run.open_obligations
                .push(issue("NATIVE_SHUTDOWN", format!("{other:?}")));
            let _ = worker.abort().await;
        }
    }
    Ok(())
}
fn reject(_invocation: &Invocation) -> std::result::Result<Observation, NativeFailure> {
    Ok(Observation::Unavailable {
        reason: "No collaborator is configured for this request".into(),
    })
}
