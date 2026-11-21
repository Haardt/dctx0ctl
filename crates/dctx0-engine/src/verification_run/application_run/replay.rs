//! Diagnostic native replay keeps the original centrally generated program and
//! changes only ownership. Historical results never supply current observations.
use super::*;
use dctx0_verification::application_plan::*;

impl Project {
    #[allow(clippy::too_many_arguments)]
    pub async fn replay_application_test(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        recipe: &str,
        evidence: &str,
        case: &str,
        output: &str,
        cancel: &NativeCancellation,
    ) -> Result<ApplicationNativeReport> {
        let (root, mut bundle, _) = self.read_application_testing_bundle(application, plan)?;
        let (_, config) = self.verification_configuration(Some(&root))?;
        let (_, evidence_path) = crate::verification_setup::safe_file(&root, evidence)?;
        if fs::metadata(&evidence_path)?.len() > 67_108_864 {
            return Err(Error::Invalid(
                "Native replay evidence exceeds 64 MiB".into(),
            ));
        }
        let bytes = fs::read(evidence_path)?;
        let parent: ApplicationNativeReport = serde_json::from_slice(&bytes)?;
        if parent.schema_version != 1
            || parent.group != group
            || parent.plan_hash != digest(&serde_json::to_vec(&bundle)?)
        {
            return Err(Error::Invalid(
                "Native replay plan does not match its saved execution".into(),
            ));
        }
        let assessment = parent
            .assessment
            .as_ref()
            .ok_or_else(|| Error::Invalid("Native replay needs saved case observations".into()))?;
        let record = assessment
            .cases
            .iter()
            .find(|record| record.id == case)
            .ok_or_else(|| {
                Error::Invalid("Requested native replay case is absent from saved evidence".into())
            })?;
        let mut selected = BTreeSet::from([case.to_string()]);
        if record.kind == VerificationCaseKind::EntityIdentity {
            let Some(VerificationValue::List(sources)) = record.bindings.get("source_cases") else {
                return Err(Error::Invalid(
                    "Identity replay requires its actual source case IDs".into(),
                ));
            };
            for source in sources {
                let VerificationValue::String(id) = source else {
                    return Err(Error::Invalid("Invalid identity source case ID".into()));
                };
                if !assessment.cases.iter().any(|case| {
                    case.id == id.as_ref() && case.kind == VerificationCaseKind::Operation
                }) {
                    return Err(Error::Invalid(
                        "Identity replay source case is missing".into(),
                    ));
                }
                selected.insert(id.to_string());
            }
        }
        for lifecycle in &bundle.plan.program.lifecycles {
            if lifecycle
                .paths
                .iter()
                .any(|path| selected.contains(&path.id))
            {
                selected.extend(lifecycle.paths.iter().map(|path| path.id.clone()));
            }
        }
        let mut native = bundle
            .plan
            .configuration
            .groups
            .iter()
            .find(|candidate| {
                candidate.id == group && candidate.backend == TestBackend::ApplicationTests
            })
            .cloned()
            .ok_or_else(|| Error::Invalid("Replay group is unavailable".into()))?;
        let mut registrations = bundle.plan.registrations.clone();
        for registration in &mut registrations {
            registration.cases = registration
                .cases
                .intersection(&selected)
                .cloned()
                .collect();
        }
        registrations
            .retain(|registration| registration.group == group && !registration.cases.is_empty());
        native.selection = registrations
            .iter()
            .map(|registration| registration.test.clone())
            .collect();
        native
            .stateless_decoders
            .retain(|test| native.selection.contains(test));
        native.area = TestArea {
            cases: selected.clone(),
            ..Default::default()
        };
        let remainder = bundle
            .plan
            .manifest
            .required_cases
            .difference(&selected)
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut groups = vec![native];
        if !remainder.is_empty() {
            groups.push(ExecutionGroup {
                id: "replay-remainder".into(),
                backend: TestBackend::Adapter,
                target: config.target,
                selection: [config.adapter].into(),
                area: TestArea {
                    cases: remainder,
                    ..Default::default()
                },
                stateless_decoders: Default::default(),
                resources: GroupResources {
                    namespace: "replay-remainder".into(),
                    timeout_seconds: 120,
                    max_message_bytes: 1_048_576,
                },
            });
        }
        bundle.plan = application_test_plan(
            &bundle.plan.program,
            &ApplicationTestConfiguration {
                schema_version: 2,
                groups,
            },
            &registrations,
        )
        .map_err(|issues| Error::Invalid(format!("Invalid replay ownership: {issues:?}")))?;
        let replay_plan = format!(
            ".dctx0ctl/application-tests/replay-{}.json",
            digest(format!("{}:{case}", digest(&bytes)).as_bytes())
        );
        let (directory, path) = crate::verification_setup::safe_file(&root, &replay_plan)?;
        fs::create_dir_all(directory)?;
        atomic_write(&path, &serde_json::to_vec(&bundle)?)?;
        let mut report = self
            .run_application_tests(&root, &replay_plan, group, recipe, output, cancel)
            .await?;
        report.replay_of = Some(digest(&bytes));
        report.requested_case = Some(case.into());
        let (_, path) = crate::verification_setup::safe_file(&root, output)?;
        atomic_write(&path, &serde_json::to_vec_pretty(&report)?)?;
        Ok(report)
    }
}
