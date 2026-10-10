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
            let rejection =
                case.bindings.get("expect_rejection") == Some(&VerificationValue::Boolean(true));
            let original = if rejection {
                &contract.rejection_corpus.values
            } else {
                &contract.corpus.values
            };
            if !original.contains(&value) {
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
            for property in &mut plan.properties {
                property
                    .assertions
                    .retain(|a| a.cases.iter().any(|c| c.id == case.id));
                for assertion in &mut property.assertions {
                    assertion.cases.retain(|c| c.id == case.id);
                    if assertion.cases[0].variables != case.bindings {
                        return Err(Error::Invalid(
                            "Saved assertion bindings differ from its plan".into(),
                        ));
                    }
                }
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
                {
                    return Err(Error::Invalid(
                        "Saved operation input differs from its plan".into(),
                    ));
                }
                operation.cases = vec![input];
                operation.inputs.bindings.clear();
            } else {
                if !operation.inputs.bindings.contains(&case.bindings) {
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
        let lifecycles = only(&mut plan, case)?;
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
        let replayed = evidence.evidence.run.cases.first();
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
            replayed_case: replayed.map(|c| c.id.clone()),
            case_result: replayed.map(|c| c.result.clone()),
            changes,
            evidence,
        })
    }
}
