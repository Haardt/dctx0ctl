//! Saved inputs against a newly captured implementation; never task adoption.
use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayChange {
    pub component: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationReplayReport {
    pub original_run: String,
    pub original_case: String,
    #[serde(default)]
    pub minimized_input: bool,
    pub replayed_case: Option<String>,
    pub case_result: Option<VerificationResult>,
    pub changes: Vec<ReplayChange>,
    pub evidence: NativeVerificationReport,
}
fn only(
    plan: &mut ProgramPlan,
    case: &VerificationCaseRecord,
) -> Result<VerificationLifecycleFixtures> {
    let mut lifecycles = VerificationLifecycleFixtures::default();
    plan.scope.subjects = BTreeSet::from([case.subject.clone()]);
    plan.scope.tasks.clear();
    plan.issues.clear();
    plan.manual_checks.clear();
    plan.policies.clear();
    plan.suites.clear();
    match case.kind {
        VerificationCaseKind::EntityIdentity => {
            let Some(contract) = plan
                .contracts
                .iter()
                .find(|contract| contract.subject == case.subject)
            else {
                return Err(Error::Invalid(
                    "Saved entity identity is absent from its plan".into(),
                ));
            };
            if contract.identity_requirements.is_empty()
                || case.id != format!("{}:identity", case.subject)
            {
                return Err(Error::Invalid(
                    "Saved entity identity has no declared key requirement".into(),
                ));
            }
            let Some(VerificationValue::List(source_cases)) = case.bindings.get("source_cases")
            else {
                return Err(Error::Invalid(
                    "Saved identity source cases are missing".into(),
                ));
            };
            let sources = source_cases
                .iter()
                .map(|value| {
                    if let VerificationValue::String(id) = value {
                        Some(id.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Option<BTreeSet<_>>>()
                .filter(|sources| !sources.is_empty())
                .ok_or_else(|| Error::Invalid("Saved identity source cases are invalid".into()))?;
            let mut found = BTreeSet::new();
            plan.operations.retain(|operation| {
                let ids = operation
                    .cases
                    .iter()
                    .map(|input| input.id.clone())
                    .chain(
                        (0..operation.inputs.bindings.len())
                            .map(|index| format!("{}:operation:{index}", operation.operation.id)),
                    )
                    .filter(|id| sources.contains(id))
                    .collect::<BTreeSet<_>>();
                let keep = !ids.is_empty();
                found.extend(ids);
                keep
            });
            if found != sources {
                return Err(Error::Invalid(
                    "Saved identity source cases are absent from the pinned plan".into(),
                ));
            }
            // Identity is a derived check: replay the pinned operation inputs
            // that supplied actual persistence observations, without regenerating.
            plan.contracts
                .retain(|contract| contract.subject == case.subject);
            plan.properties.clear();
            plan.examples.clear();
            plan.lifecycles.clear();
        }
        VerificationCaseKind::Contract => {
            plan.contracts.retain(|c| c.subject == case.subject);
            let contract = plan
                .contracts
                .first_mut()
                .ok_or_else(|| Error::Invalid("Saved contract is absent from its plan".into()))?;
            let value = case
                .bindings
                .get("value")
                .ok_or_else(|| Error::Invalid("Saved contract input is missing".into()))?
                .clone();
            let rejection = match case.bindings.get("expect_rejection") {
                Some(VerificationValue::Boolean(rejection)) => *rejection,
                _ => {
                    return Err(Error::Invalid(
                        "Saved contract rejection marker is missing or invalid".into(),
                    ));
                }
            };
            let original = if rejection {
                &contract.rejection_corpus.values
            } else {
                &contract.corpus.values
            };
            let label = if rejection { "rejection" } else { "value" };
            let index = case
                .id
                .strip_prefix(&format!("{}:{label}:", case.subject))
                .and_then(|index| index.parse::<usize>().ok());
            if index.and_then(|index| original.get(index)) != Some(&value) {
                return Err(Error::Invalid(
                    "Saved contract input differs from its pinned plan".into(),
                ));
            }
            contract.corpus.values = if rejection {
                vec![]
            } else {
                vec![value.clone()]
            };
            contract.rejection_corpus.values = if rejection { vec![value] } else { vec![] };
            plan.operations.clear();
            plan.properties.clear();
            plan.examples.clear();
            plan.lifecycles.clear();
        }
        VerificationCaseKind::Assertion => {
            plan.properties.retain(|p| p.subject == case.subject);
            let mut found = false;
            for property in &mut plan.properties {
                property
                    .assertions
                    .retain(|a| a.cases.iter().any(|c| c.id == case.id));
                for assertion in &mut property.assertions {
                    assertion.cases.retain(|c| c.id == case.id);
                    found = true;
                    if assertion.cases[0].variables != case.bindings {
                        return Err(Error::Invalid(
                            "Saved assertion bindings differ from its plan".into(),
                        ));
                    }
                }
            }
            if !found {
                return Err(Error::Invalid(
                    "Saved assertion case is absent from its plan".into(),
                ));
            }
            plan.contracts.clear();
            plan.operations.clear();
            plan.examples.clear();
            plan.lifecycles.clear();
        }
        VerificationCaseKind::Example => {
            plan.examples.retain(|e| e.subject == case.subject);
            let example = plan
                .examples
                .first()
                .ok_or_else(|| Error::Invalid("Saved example is absent from its plan".into()))?;
            if case.id != example.subject {
                return Err(Error::Invalid(
                    "Saved example case ID differs from its plan".into(),
                ));
            }
            if example.bindings != case.bindings {
                return Err(Error::Invalid(
                    "Saved example bindings differ from its plan".into(),
                ));
            }
            plan.contracts.clear();
            plan.operations.clear();
            plan.properties.clear();
            plan.lifecycles.clear();
        }
        VerificationCaseKind::Operation => {
            plan.operations.retain(|o| o.operation.id == case.subject);
            let operation = plan
                .operations
                .first_mut()
                .ok_or_else(|| Error::Invalid("Saved operation is absent from its plan".into()))?;
            if let Some(input) = operation.cases.iter().find(|c| c.id == case.id).cloned() {
                if input.setup != case.setup
                    || case.arguments != vec![input.invocation.arguments.clone()]
                    || !case.bindings.is_empty()
                {
                    return Err(Error::Invalid(
                        "Saved operation input differs from its plan".into(),
                    ));
                }
                operation.cases = vec![input];
                operation.inputs.bindings.clear();
            } else {
                let index = case
                    .id
                    .strip_prefix(&format!("{}:operation:", case.subject))
                    .and_then(|index| index.parse::<usize>().ok());
                let arguments = operation
                    .operation
                    .parameters
                    .iter()
                    .map(|parameter| case.bindings.get(&parameter.name).cloned())
                    .collect::<Option<Vec<_>>>();
                if index.and_then(|index| operation.inputs.bindings.get(index))
                    != Some(&case.bindings)
                    || case.setup != operation.generated_setup(&case.bindings)
                    || arguments
                        .as_ref()
                        .is_none_or(|arguments| case.arguments != vec![arguments.clone()])
                {
                    return Err(Error::Invalid(
                        "Saved generated operation input differs from its plan".into(),
                    ));
                }
                operation.inputs.bindings = vec![case.bindings.clone()];
                operation.cases.clear();
            }
            plan.contracts.clear();
            plan.properties.clear();
            plan.examples.clear();
            plan.lifecycles.clear();
        }
        VerificationCaseKind::Lifecycle => {
            plan.lifecycles.retain(|l| l.subject == case.subject);
            for lifecycle in &mut plan.lifecycles {
                lifecycle.paths.retain(|p| p.id == case.id);
            }
            if !plan
                .lifecycles
                .iter()
                .any(|lifecycle| lifecycle.paths.iter().any(|path| path.id == case.id))
            {
                return Err(Error::Invalid(
                    "Saved lifecycle case is absent from its plan".into(),
                ));
            }
            let Some(VerificationValue::List(keys)) = case.bindings.get("keys") else {
                return Err(Error::Invalid("Saved lifecycle keys are missing".into()));
            };
            lifecycles.paths.insert(
                case.id.clone(),
                LifecycleCaseInput {
                    keys: keys.clone(),
                    setup: case.setup.clone(),
                    arguments: case.arguments.clone(),
                },
            );
            plan.contracts.clear();
            plan.operations.clear();
            plan.properties.clear();
            plan.examples.clear();
        }
    }
    Ok(lifecycles)
}
impl Project {
    pub async fn replay_verification(
        &self,
        application: Option<&Path>,
        run_id: &str,
        case_id: &str,
        generator: &dyn HarnessGenerator,
        cancel: &NativeCancellation,
    ) -> Result<VerificationReplayReport> {
        self.replay_verification_input(application, run_id, case_id, generator, cancel, false)
            .await
    }
    pub async fn replay_minimized_verification(
        &self,
        application: Option<&Path>,
        run_id: &str,
        case_id: &str,
        generator: &dyn HarnessGenerator,
        cancel: &NativeCancellation,
    ) -> Result<VerificationReplayReport> {
        self.replay_verification_input(application, run_id, case_id, generator, cancel, true)
            .await
    }
    async fn replay_verification_input(
        &self,
        application: Option<&Path>,
        run_id: &str,
        case_id: &str,
        generator: &dyn HarnessGenerator,
        cancel: &NativeCancellation,
        minimized: bool,
    ) -> Result<VerificationReplayReport> {
        let store = self.store(AccessMode::ReadOnly)?;
        let original = store.verification_run(&self.key(), run_id)?;
        let case = original
            .run
            .cases
            .iter()
            .find(|c| c.id == case_id)
            .ok_or_else(|| Error::Invalid("Replay needs an actually recorded case ID".into()))?;
        let artifact = store.verification_artifact(run_id, &original.run.provenance.plan_hash)?;
        let mut plan: ProgramPlan = serde_json::from_slice(&artifact.contents)?;
        if plan.snapshot != original.run.provenance.snapshot
            || plan.check_occurrence as i64 != original.run.provenance.check_occurrence
        {
            return Err(Error::Invalid(
                "Saved replay plan provenance does not match its run".into(),
            ));
        }
        if case.kind == VerificationCaseKind::EntityIdentity {
            let snapshot = self.load(&plan.snapshot)?;
            if dctx0_verification::entity_identity_case(
                &snapshot.ir,
                &case.subject,
                &original.run.cases,
                &plan.budgets,
            ) != *case
            {
                return Err(Error::Invalid(
                    "Saved entity identity does not reproduce its recorded source observations"
                        .into(),
                ));
            }
        }
        let mut lifecycles = only(&mut plan, case)?;
        if minimized && case.kind == VerificationCaseKind::Lifecycle {
            let mut selected = None;
            for reference in &original.run.artifacts {
                if reference.media_type != "application/vnd.dctx0.lifecycle-shrink+json" {
                    continue;
                }
                let artifact = store.verification_artifact(run_id, &reference.hash)?;
                let value: serde_json::Value = serde_json::from_slice(&artifact.contents)?;
                if value.get("case_id").and_then(|v| v.as_str()) == Some(case_id) {
                    let report: dctx0_verification::LifecycleShrinkReport =
                        serde_json::from_value(value["report"].clone())?;
                    if selected.replace(report).is_some() {
                        return Err(Error::Invalid(
                            "Ambiguous saved lifecycle shrink artifacts".into(),
                        ));
                    }
                }
            }
            let shrink = selected.ok_or_else(|| {
                Error::Invalid("No saved lifecycle shrink artifact for this case".into())
            })?;
            let input = lifecycles
                .paths
                .get(case_id)
                .ok_or_else(|| Error::Invalid("Missing saved lifecycle input".into()))?;
            let same_failure = |assessment: &dctx0_verification::LifecycleAssessment| {
                matches!((&assessment.result, &case.result), (VerificationResult::Fail { issue: actual }, VerificationResult::Fail { issue: original }) if actual.code == original.code && actual.span == original.span)
                    && assessment.completed_transitions
                        == shrink.original_assessment.completed_transitions
                    && assessment.reached_states == shrink.original_assessment.reached_states
            };
            if !matches!(
                shrink.version.as_str(),
                "dctx0-shrink-1" | dctx0_verification::SHRINK_VERSION
            ) || &shrink.original != input
                || !same_failure(&shrink.original_assessment)
                || shrink.minimized.keys != shrink.original.keys
                || shrink.minimized.arguments.len() != shrink.original.arguments.len()
                || (shrink.minimized != shrink.original
                    && !shrink.attempts.iter().any(|attempt| {
                        attempt.input == shrink.minimized && same_failure(&attempt.assessment)
                    }))
            {
                return Err(Error::Invalid(
                    "Shrink artifact does not match the recorded failing lifecycle".into(),
                ));
            }
            lifecycles.paths.insert(case_id.into(), shrink.minimized);
        } else if minimized {
            if case.kind != VerificationCaseKind::Operation || !case.setup.is_empty() {
                return Err(Error::Invalid(
                    "Minimized replay requires a pure generated operation case".into(),
                ));
            }
            let mut selected = None;
            for reference in &original.run.artifacts {
                if reference.media_type != "application/vnd.dctx0.shrink+json" {
                    continue;
                }
                let artifact = store.verification_artifact(run_id, &reference.hash)?;
                let value: serde_json::Value = serde_json::from_slice(&artifact.contents)?;
                if value.get("case_id").and_then(|v| v.as_str()) == Some(case_id) {
                    let report: dctx0_verification::ShrinkReport =
                        serde_json::from_value(value["report"].clone())?;
                    if selected.replace(report).is_some() {
                        return Err(Error::Invalid("Ambiguous saved shrink artifacts".into()));
                    }
                }
            }
            let shrink = selected
                .ok_or_else(|| Error::Invalid("No saved shrink artifact for this case".into()))?;
            let operation = plan
                .operations
                .first_mut()
                .ok_or_else(|| Error::Invalid("Missing operation for minimized replay".into()))?;
            if !matches!(shrink.version.as_str(), "dctx0-shrink-1" | dctx0_verification::SHRINK_VERSION)
                || !matches!(case.result, VerificationResult::Fail { .. })
                || !dctx0_verification::supports_generated_operation(operation)
                || !operation.operation.pure
                || !operation.operation.failures.is_empty()
                || shrink.original.subject != case.subject
                || case.arguments != vec![shrink.original.arguments.clone()]
                || shrink.minimized.subject != case.subject
                || shrink.minimized.arguments.len() != operation.operation.parameters.len()
                || (shrink.minimized != shrink.original
                    && !shrink.attempts.iter().any(|a| {
                        a.invocation == shrink.minimized
                            && matches!((&a.result, &case.result), (VerificationResult::Fail { issue: actual }, VerificationResult::Fail { issue: original }) if actual.code == original.code && actual.span == original.span)
                    }))
            {
                return Err(Error::Invalid(
                    "Shrink artifact does not match the recorded failing operation".into(),
                ));
            }
            operation.inputs.bindings = vec![
                operation
                    .operation
                    .parameters
                    .iter()
                    .zip(shrink.minimized.arguments)
                    .map(|(parameter, value)| (parameter.name.clone(), value))
                    .collect(),
            ];
            operation.cases.clear();
        }
        // Replay evaluates exactly the selected saved input, without a new search.
        plan.budgets.shrink_steps = 0;
        drop(store);
        let evidence = self
            .run_verification_plan(
                application,
                Default::default(),
                generator,
                cancel,
                true,
                Some((plan, lifecycles)),
            )
            .await?;
        let replayed = if case.kind == VerificationCaseKind::EntityIdentity {
            evidence
                .evidence
                .run
                .cases
                .iter()
                .find(|candidate| candidate.id == case.id)
        } else {
            evidence.evidence.run.cases.first()
        };
        let a = &original.run.provenance;
        let b = &evidence.evidence.run.provenance;
        let mut changes = vec![];
        for (component, before, after) in [
            ("adapter", &a.adapter_hash, &b.adapter_hash),
            (
                "configuration",
                &a.configuration_hash,
                &b.configuration_hash,
            ),
            ("harness", &a.harness_hash, &b.harness_hash),
            ("runtime", &a.runtime_version, &b.runtime_version),
            (
                "interpreter",
                &a.interpreter_version,
                &b.interpreter_version,
            ),
            ("generator", &a.generator_version, &b.generator_version),
        ] {
            if before != after {
                changes.push(ReplayChange {
                    component: component.into(),
                    before: Some(before.clone()),
                    after: Some(after.clone()),
                });
            }
        }
        let identity = |run: &VerificationRun| {
            run.artifacts
                .iter()
                .find(|artifact| {
                    artifact.media_type == "application/vnd.dctx0.runtime-identity+json"
                })
                .map(|artifact| artifact.hash.clone())
        };
        let before = identity(&original.run);
        let after = identity(&evidence.evidence.run);
        if before != after {
            changes.push(ReplayChange {
                component: "runtime-identity".into(),
                before,
                after,
            });
        }
        let assessment_version = |run: &VerificationRun| {
            run.artifacts
                .iter()
                .find(|artifact| {
                    artifact.media_type == "application/vnd.dctx0.assessment-version+json"
                })
                .map(|artifact| artifact.hash.clone())
        };
        let native_limits = |run: &VerificationRun| {
            run.artifacts
                .iter()
                .find(|artifact| artifact.media_type == "application/vnd.dctx0.native-limits+json")
                .map(|artifact| artifact.hash.clone())
        };
        let before = native_limits(&original.run);
        let after = native_limits(&evidence.evidence.run);
        if before != after {
            changes.push(ReplayChange {
                component: "native-limits".into(),
                before,
                after,
            });
        }
        let dependency_limits = |run: &VerificationRun| {
            run.artifacts
                .iter()
                .find(|artifact| {
                    artifact.media_type == "application/vnd.dctx0.dependency-resolution-limits+json"
                })
                .map(|artifact| artifact.hash.clone())
        };
        let before = dependency_limits(&original.run);
        let after = dependency_limits(&evidence.evidence.run);
        if before != after {
            changes.push(ReplayChange {
                component: "dependency-resolution-limits".into(),
                before,
                after,
            });
        }
        let before = assessment_version(&original.run);
        let after = assessment_version(&evidence.evidence.run);
        if before != after {
            changes.push(ReplayChange {
                component: "assessment-version".into(),
                before,
                after,
            });
        }
        let verifier_identity = |run: &VerificationRun| {
            run.artifacts
                .iter()
                .find(|artifact| {
                    artifact.media_type == "application/vnd.dctx0.verifier-identity+json"
                })
                .map(|artifact| artifact.hash.clone())
        };
        let before = verifier_identity(&original.run);
        let after = verifier_identity(&evidence.evidence.run);
        if before != after {
            changes.push(ReplayChange {
                component: "verifier-identity".into(),
                before,
                after,
            });
        }
        for (kind, before, after) in [
            ("implementation", &a.implementation, &b.implementation),
            ("dependency", &a.dependencies, &b.dependencies),
        ] {
            for key in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
                if before.get(key) != after.get(key) {
                    changes.push(ReplayChange {
                        component: format!("{kind}:{key}"),
                        before: before.get(key).cloned(),
                        after: after.get(key).cloned(),
                    });
                }
            }
        }
        Ok(VerificationReplayReport {
            original_run: run_id.into(),
            original_case: case_id.into(),
            minimized_input: minimized,
            replayed_case: replayed.map(|c| c.id.clone()),
            case_result: replayed.map(|c| c.result.clone()),
            changes,
            evidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cinema_assertion_and_example_replay_bindings_are_pinned_and_isolate_one_subject() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema/dctx0");
        let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|file| Source {
            file: file.into(),
            text: fs::read_to_string(root.join(file)).unwrap(),
        });
        let compiled = dctx0_language::compile(&sources);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        let ir = compiled.ir.unwrap();
        let plan = dctx0_verification::project_program(
            &ir,
            "snapshot",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["HomeCinema.MovieNight".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let property = &plan.properties[0];
        let input = &property.assertions[0].cases[0];
        for (kind, subject, id, bindings) in [
            (
                VerificationCaseKind::Assertion,
                property.subject.clone(),
                input.id.clone(),
                input.variables.clone(),
            ),
            (
                VerificationCaseKind::Example,
                plan.examples[0].subject.clone(),
                plan.examples[0].subject.clone(),
                plan.examples[0].bindings.clone(),
            ),
        ] {
            let mut case = VerificationCaseRecord {
                id,
                subject: subject.clone(),
                kind,
                bindings,
                setup: vec![],
                arguments: vec![],
                observations: vec![],
                events: None,
                resources: None,
                states: vec![],
                result: VerificationResult::Pass,
                elapsed_us: 0,
            };
            let mut selected = plan.clone();
            assert!(only(&mut selected, &case).unwrap().paths.is_empty());
            assert_eq!(selected.scope.subjects, [subject].into());
            assert!(
                selected.contracts.is_empty()
                    && selected.operations.is_empty()
                    && selected.lifecycles.is_empty()
            );
            if case.kind == VerificationCaseKind::Assertion {
                assert!(selected.examples.is_empty());
                assert_eq!(selected.properties[0].assertions[0].cases.len(), 1);
            } else {
                assert!(selected.properties.is_empty());
                assert_eq!(selected.examples.len(), 1);
            }
            if case.kind == VerificationCaseKind::Example {
                let mut renamed = case.clone();
                renamed.id.push_str(":other");
                assert!(
                    matches!(only(&mut plan.clone(), &renamed), Err(Error::Invalid(message)) if message.contains("case ID differs"))
                );
            }
            case.bindings
                .insert("__corrupt_binding__".into(), VerificationValue::Unit);
            assert!(
                matches!(only(&mut plan.clone(), &case), Err(Error::Invalid(message)) if message.contains("bindings differ"))
            );
        }
        let fixtures: VerificationLifecycleFixtures = serde_json::from_slice(
            &fs::read(root.join("../.dctx0/testing/typescript/lifecycles.json")).unwrap(),
        )
        .unwrap();
        for path in &plan.lifecycles[0].paths {
            let input = &fixtures.paths[&path.id];
            let mut case = VerificationCaseRecord {
                id: path.id.clone(),
                subject: plan.lifecycles[0].subject.clone(),
                kind: VerificationCaseKind::Lifecycle,
                bindings: [("keys".into(), VerificationValue::List(input.keys.clone()))].into(),
                setup: input.setup.clone(),
                arguments: input.arguments.clone(),
                observations: vec![],
                events: None,
                resources: None,
                states: vec![],
                result: VerificationResult::Pass,
                elapsed_us: 0,
            };
            let mut selected = plan.clone();
            let replayed = only(&mut selected, &case).unwrap();
            assert_eq!(replayed.paths[&path.id], *input);
            assert_eq!(selected.lifecycles.len(), 1);
            assert_eq!(selected.lifecycles[0].paths.len(), 1);
            assert!(
                selected.contracts.is_empty()
                    && selected.operations.is_empty()
                    && selected.examples.is_empty()
                    && selected.properties.is_empty()
            );
            for keys in [
                None,
                Some(VerificationValue::Unit),
                Some(VerificationValue::String("wrong".into())),
            ] {
                case.bindings.remove("keys");
                if let Some(keys) = keys {
                    case.bindings.insert("keys".into(), keys);
                }
                assert!(
                    matches!(only(&mut plan.clone(), &case), Err(Error::Invalid(message)) if message.contains("lifecycle keys are missing"))
                );
            }
        }
    }
    #[test]
    fn generated_operation_replay_rejects_corrupt_bindings_arguments_setup_and_indices() {
        let compiled = dctx0_language::compile(&[Source { file: "replay-input.dctx0".into(), text: "workspace W { value A = Int { range 1..2 } surface Api { operation calculate(input:A) -> A { pure } } }".into() }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        let ir = compiled.ir.unwrap();
        let plan = dctx0_verification::project_program(
            &ir,
            "snapshot",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["W.Api.calculate".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let operation = &plan.operations[0];
        let bindings = operation.inputs.bindings[0].clone();
        let arguments = vec![vec![bindings["input"].clone()]];
        let case = VerificationCaseRecord {
            id: "W.Api.calculate:operation:0".into(),
            subject: "W.Api.calculate".into(),
            kind: VerificationCaseKind::Operation,
            bindings,
            setup: vec![],
            arguments,
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        };
        let mut selected = plan.clone();
        assert!(only(&mut selected, &case).unwrap().paths.is_empty());
        assert_eq!(
            selected.operations[0].inputs.bindings,
            vec![case.bindings.clone()]
        );
        assert!(selected.contracts.is_empty());
        for corruption in 0..5 {
            let mut corrupt = case.clone();
            match corruption {
                0 => corrupt.id = "W.Api.calculate:operation:999".into(),
                1 => {
                    corrupt.bindings.clear();
                }
                2 => corrupt.arguments = vec![vec![VerificationValue::Integer("999".into())]],
                3 => corrupt.setup.push(Invocation {
                    subject: "W.Api.calculate".into(),
                    arguments: vec![VerificationValue::Integer("1".into())],
                }),
                _ => corrupt.id = "W.Api.calculate:operation:invalid".into(),
            }
            assert!(
                matches!(only(&mut plan.clone(), &corrupt), Err(Error::Invalid(message)) if message.contains("differs from its plan")),
                "Corruption {corruption} must never reach native execution"
            );
        }
    }
    #[test]
    fn prepared_generated_replay_preserves_setup_and_rejects_changes() {
        let compiled = dctx0_language::compile(&[Source {
            file: "prepared-replay.dctx0".into(),
            text: "workspace W { surface Api { operation calculate(input:Int) -> Int } }".into(),
        }]);
        let ir = compiled.ir.unwrap();
        let mut plan = dctx0_verification::project_program(
            &ir,
            "snapshot",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["W.Api.calculate".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let bindings = plan.operations[0].inputs.bindings[0].clone();
        let setup = vec![Invocation {
            subject: "W.Api.calculate".into(),
            arguments: vec![VerificationValue::Integer("1".into())],
        }];
        plan.operations[0].generated_preparations =
            vec![dctx0_verification::GeneratedPreparation {
                bindings: bindings.clone(),
                setup: setup.clone(),
            }];
        let mut case = VerificationCaseRecord {
            id: "W.Api.calculate:operation:0".into(),
            subject: "W.Api.calculate".into(),
            kind: VerificationCaseKind::Operation,
            bindings,
            setup,
            arguments: vec![vec![plan.operations[0].inputs.bindings[0]["input"].clone()]],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        };
        let mut selected = plan.clone();
        only(&mut selected, &case).unwrap();
        assert_eq!(
            selected.operations[0].generated_setup(&case.bindings),
            case.setup
        );
        case.setup.clear();
        assert!(only(&mut plan.clone(), &case).is_err());
    }
    #[test]
    fn reviewed_operation_replay_keeps_setup_and_rejects_extra_bindings() {
        let compiled = dctx0_language::compile(&[Source {
            file: "reviewed-replay.dctx0".into(),
            text: "workspace W { surface Api { operation calculate(input:Int) -> Int { pure } } }"
                .into(),
        }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        let ir = compiled.ir.unwrap();
        let mut plan = dctx0_verification::project_program(
            &ir,
            "snapshot",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["W.Api.calculate".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let invocation = Invocation {
            subject: "W.Api.calculate".into(),
            arguments: vec![VerificationValue::Integer("2".into())],
        };
        let setup = vec![Invocation {
            subject: invocation.subject.clone(),
            arguments: vec![VerificationValue::Integer("1".into())],
        }];
        let case = VerificationCaseRecord {
            id: "W.Api.calculate:contract:reviewed".into(),
            subject: invocation.subject.clone(),
            kind: VerificationCaseKind::Operation,
            bindings: Default::default(),
            setup: setup.clone(),
            arguments: vec![invocation.arguments.clone()],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        };
        plan.operations[0]
            .cases
            .push(dctx0_verification::OperationCaseInput {
                id: case.id.clone(),
                invocation,
                setup,
                mocks: vec![],
            });
        let mut selected = plan.clone();
        assert!(only(&mut selected, &case).unwrap().paths.is_empty());
        assert_eq!(selected.operations[0].cases.len(), 1);
        assert_eq!(selected.operations[0].cases[0].setup, case.setup);
        assert!(selected.operations[0].inputs.bindings.is_empty());
        for corruption in 0..3 {
            let mut corrupt = case.clone();
            match corruption {
                0 => corrupt.setup.clear(),
                1 => corrupt.arguments[0][0] = VerificationValue::Integer("3".into()),
                _ => {
                    corrupt
                        .bindings
                        .insert("input".into(), VerificationValue::Integer("2".into()));
                }
            }
            assert!(
                matches!(only(&mut plan.clone(), &corrupt), Err(Error::Invalid(message)) if message.contains("Saved operation input differs"))
            );
        }
    }
    #[test]
    fn lifecycle_replay_rejects_unknown_paths_instead_of_executing_an_empty_plan() {
        let compiled = dctx0_language::compile(&[Source {
            file: "w.dctx0".into(),
            text: "workspace W { value A = Int }".into(),
        }]);
        let ir = compiled.ir.unwrap();
        let mut plan = dctx0_verification::project_program(
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
        let case = VerificationCaseRecord {
            id: "absent-path".into(),
            subject: "W.Flow".into(),
            kind: VerificationCaseKind::Lifecycle,
            bindings: [("keys".into(), VerificationValue::List(vec![]))].into(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: VerificationResult::Pass,
            elapsed_us: 0,
        };
        assert!(
            matches!(only(&mut plan, &case), Err(Error::Invalid(message)) if message.contains("absent from its plan"))
        );
    }
    #[test]
    fn replay_rejects_missing_pinned_cases_for_every_case_kind() {
        let compiled = dctx0_language::compile(&[Source {
            file: "w.dctx0".into(),
            text: "workspace W { value A = Int { range 1..2 } }".into(),
        }]);
        let ir = compiled.ir.unwrap();
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
        for kind in [
            VerificationCaseKind::Contract,
            VerificationCaseKind::Assertion,
            VerificationCaseKind::Example,
            VerificationCaseKind::Operation,
            VerificationCaseKind::Lifecycle,
        ] {
            let case = VerificationCaseRecord {
                id: "absent".into(),
                subject: "missing".into(),
                kind,
                bindings: Default::default(),
                setup: vec![],
                arguments: vec![],
                observations: vec![],
                events: None,
                resources: None,
                states: vec![],
                result: VerificationResult::Pass,
                elapsed_us: 0,
            };
            assert!(matches!(
                only(&mut plan.clone(), &case),
                Err(Error::Invalid(_))
            ));
        }
        let mut case = VerificationCaseRecord {
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
            result: VerificationResult::Pass,
            elapsed_us: 0,
        };
        assert!(
            matches!(only(&mut plan.clone(), &case), Err(Error::Invalid(message)) if message.contains("input is missing"))
        );
        case.bindings
            .insert("value".into(), plan.contracts[0].corpus.values[0].clone());
        for marker in [
            None,
            Some(VerificationValue::Unit),
            Some(VerificationValue::String("false".into())),
        ] {
            case.bindings.remove("expect_rejection");
            if let Some(marker) = marker {
                case.bindings.insert("expect_rejection".into(), marker);
            }
            assert!(
                matches!(only(&mut plan.clone(), &case), Err(Error::Invalid(message)) if message.contains("rejection marker"))
            );
        }
        case.bindings
            .insert("expect_rejection".into(), VerificationValue::Boolean(false));
        case.bindings
            .insert("value".into(), VerificationValue::Integer("999".into()));
        assert!(
            matches!(only(&mut plan.clone(), &case), Err(Error::Invalid(message)) if message.contains("differs from its pinned plan"))
        );
        case.bindings
            .insert("value".into(), plan.contracts[0].corpus.values[0].clone());
        for changed_id in [
            "W.A:value:1",
            "W.A:value:999",
            "W.A:rejection:0",
            "W.A:value:invalid",
        ] {
            let mut corrupt = case.clone();
            corrupt.id = changed_id.into();
            assert!(
                matches!(only(&mut plan.clone(), &corrupt), Err(Error::Invalid(message)) if message.contains("differs from its pinned plan"))
            );
        }
        let mut selected = plan.clone();
        assert!(only(&mut selected, &case).unwrap().paths.is_empty());
        assert_eq!(
            selected.contracts[0].corpus.values,
            vec![case.bindings["value"].clone()]
        );
        assert!(selected.contracts[0].rejection_corpus.values.is_empty());
        case.id = "W.A:rejection:0".into();
        case.bindings
            .insert("expect_rejection".into(), VerificationValue::Boolean(true));
        case.bindings.insert(
            "value".into(),
            plan.contracts[0].rejection_corpus.values[0].clone(),
        );
        let mut selected = plan.clone();
        assert!(only(&mut selected, &case).unwrap().paths.is_empty());
        assert!(selected.contracts[0].corpus.values.is_empty());
        assert_eq!(
            selected.contracts[0].rejection_corpus.values,
            vec![case.bindings["value"].clone()]
        );
    }
}
