//! Diagnostic application-test transport. Full native collection/build provenance
//! remains a separate requirement; these reports never authorize task adoption.
use crate::*;
use dctx0_core::{DataIr, OperationIr};
use dctx0_verification::{
    ProjectionFixtures,
    application_plan::*,
    application_session::{ApplicationAssessmentReport, ApplicationValueBoundary},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationTestingModel {
    pub data: BTreeMap<String, DataIr>,
    pub operations: BTreeMap<String, OperationIr>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationTestingBundle {
    pub schema_version: u32,
    pub plan: ApplicationTestPlan,
    pub model: ApplicationTestingModel,
    pub spec_source_hash: String,
    pub boundary: ApplicationValueBoundary,
    #[serde(default)]
    pub case_seconds: u64,
    #[serde(default)]
    pub evidence_bytes: usize,
    #[serde(default)]
    pub lifecycle_inputs: BTreeMap<String, dctx0_verification::LifecycleCaseInput>,
}

fn lifecycle_inputs(
    root: &Path,
    adapter: &str,
) -> Result<BTreeMap<String, dctx0_verification::LifecycleCaseInput>> {
    let relative = Path::new(adapter)
        .parent()
        .unwrap_or(Path::new(""))
        .join("lifecycles.json");
    let (_, path) = verification_setup::safe_file(root, &relative.to_string_lossy())?;
    if !path.try_exists()? {
        return Ok(BTreeMap::new());
    }
    if fs::metadata(&path)?.len() > 1_048_576 {
        return Err(Error::Invalid("Lifecycle inputs exceed 1 MiB".into()));
    }
    let fixtures: verification_run::VerificationLifecycleFixtures =
        serde_json::from_slice(&fs::read(path)?)?;
    Ok(fixtures.paths)
}

fn invalid(issues: Vec<dctx0_core::VerificationIssue>) -> Error {
    Error::Invalid(
        issues
            .into_iter()
            .map(|i| format!("{}: {}", i.code, i.message))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
pub struct ApplicationTestingPreparation<'a> {
    pub plan: &'a str,
    pub group: &'a str,
    pub test: &'a str,
    pub boundary: ApplicationValueBoundary,
    pub form: TestForm,
    pub group_seconds: Option<u64>,
    pub subjects: BTreeSet<String>,
}

impl Project {
    /// Explicit publication; the serve path below is read-only with respect to history.
    pub fn prepare_application_tests(
        &self,
        application: &Path,
        options: &ApplicationTestingPreparation<'_>,
    ) -> Result<ApplicationTestingBundle> {
        self.prepare_application_test_selection(application, options, false)
    }
    /// One session owns all forms so derived identity uses the actual source
    /// cases from this execution, rather than imported or historical results.
    pub fn prepare_application_test_suite(
        &self,
        application: &Path,
        options: &ApplicationTestingPreparation<'_>,
    ) -> Result<ApplicationTestingBundle> {
        self.prepare_application_test_selection(application, options, true)
    }
    fn prepare_application_test_selection(
        &self,
        application: &Path,
        options: &ApplicationTestingPreparation<'_>,
        suite: bool,
    ) -> Result<ApplicationTestingBundle> {
        let ApplicationTestingPreparation {
            plan: relative_plan,
            group,
            test,
            boundary,
            form,
            group_seconds,
            subjects,
        } = options;
        let (group, test, boundary, form) = (*group, *test, *boundary, *form);
        let (root, config) = self.verification_configuration(Some(application))?;
        let (directory, output) = verification_setup::safe_file(&root, relative_plan)?;
        let check = self.check()?;
        if !check.ok {
            return Err(Error::Invalid(
                "Application-test preparation needs a successful check".into(),
            ));
        }
        let fixtures: ProjectionFixtures =
            serde_json::from_slice(&fs::read(root.join(&config.fixtures))?)?;
        let program = self.verification_plan(
            VerificationSelection {
                subjects: config.scope.subjects.clone(),
                tasks: config.scope.tasks.clone(),
                historical_check: None,
            },
            &fixtures,
            config.budgets.clone(),
        )?;
        let manifest = dctx0_verification::case_manifest(&program);
        let resources = |namespace: String| GroupResources {
            namespace,
            timeout_seconds: config.execution.as_ref().map_or(120, |v| v.run_seconds),
            max_message_bytes: config
                .execution
                .as_ref()
                .map_or(1_048_576, |execution| execution.message_bytes as u64),
        };
        let legacy = ExecutionGroup {
            id: "legacy".into(),
            backend: TestBackend::Adapter,
            target: config.target.clone(),
            selection: BTreeSet::from([config.adapter.clone()]),
            area: TestArea {
                cases: manifest.required_cases.clone(),
                ..Default::default()
            },
            resources: resources("legacy".into()),
        };
        let classified = application_test_plan(
            &program,
            &ApplicationTestConfiguration {
                schema_version: 2,
                groups: vec![legacy.clone()],
            },
            &[],
        )
        .map_err(invalid)?;
        let cases = classified
            .case_forms
            .iter()
            .filter(|(id, case_form)| {
                (suite || **case_form == form)
                    && (subjects.is_empty()
                        || !classified.manifest.case_requirements[*id].is_disjoint(subjects))
            })
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        if cases.is_empty() || group == "legacy" {
            return Err(Error::Invalid(
                "Expected a distinct nonempty application group".into(),
            ));
        }
        let registrations = if suite {
            let mut forms: BTreeMap<TestForm, BTreeSet<String>> = BTreeMap::new();
            for id in &cases {
                forms
                    .entry(classified.case_forms[id])
                    .or_default()
                    .insert(id.clone());
            }
            forms
                .into_iter()
                .map(|(form, cases)| {
                    let label = serde_json::to_value(form).expect("test form serializes");
                    TestRegistration {
                        group: group.into(),
                        test: format!("{test}/{}", label.as_str().expect("test form is a string")),
                        form,
                        cases,
                    }
                })
                .collect::<Vec<_>>()
        } else {
            vec![TestRegistration {
                group: group.into(),
                test: test.into(),
                form,
                cases: cases.clone(),
            }]
        };
        let mut groups = vec![ExecutionGroup {
            id: group.into(),
            backend: TestBackend::ApplicationTests,
            target: config.target.clone(),
            selection: registrations
                .iter()
                .map(|registration| registration.test.clone())
                .collect(),
            area: TestArea {
                cases: cases.clone(),
                ..Default::default()
            },
            resources: GroupResources {
                timeout_seconds: group_seconds.unwrap_or_else(|| {
                    config
                        .execution
                        .as_ref()
                        .map_or(120, |limits| limits.run_seconds)
                }),
                ..resources(group.into())
            },
        }];
        let remainder = manifest
            .required_cases
            .difference(&cases)
            .cloned()
            .collect::<BTreeSet<_>>();
        if !remainder.is_empty() {
            groups.push(ExecutionGroup {
                area: TestArea {
                    cases: remainder,
                    ..Default::default()
                },
                ..legacy
            });
        }
        let plan = application_test_plan(
            &program,
            &ApplicationTestConfiguration {
                schema_version: 2,
                groups,
            },
            &registrations,
        )
        .map_err(invalid)?;
        let snapshot = self.load(&program.snapshot)?;
        let bundle = ApplicationTestingBundle {
            schema_version: 1,
            plan,
            model: ApplicationTestingModel {
                data: snapshot.ir.data,
                operations: snapshot.ir.operations,
            },
            spec_source_hash: snapshot.source_hash,
            boundary,
            lifecycle_inputs: lifecycle_inputs(&root, &config.adapter)?,
            evidence_bytes: config
                .execution
                .as_ref()
                .map_or(4_194_304, |limits| limits.evidence_bytes),
            case_seconds: config
                .execution
                .as_ref()
                .map_or(5, |limits| limits.case_seconds),
        };
        let encoded = serde_json::to_vec(&bundle)?;
        if encoded.len() as u64 > config.application_plan_bytes() {
            return Err(plan_budget_error(config.application_plan_bytes()));
        }
        fs::create_dir_all(directory)?;
        atomic_write(&output, &encoded)?;
        Ok(bundle)
    }

    pub fn read_application_testing_bundle(
        &self,
        application: &Path,
        relative_plan: &str,
    ) -> Result<(PathBuf, ApplicationTestingBundle, dctx0_core::WorkspaceIr)> {
        self.read_application_testing_bundle_checked(application, relative_plan, None)
    }

    /// Read one exact plan byte sequence and reject it before parsing or execution
    /// if its SHA-256 differs from an explicitly captured caller pin.
    pub fn read_application_testing_bundle_with_hash(
        &self,
        application: &Path,
        relative_plan: &str,
        expected_sha256: &str,
    ) -> Result<(PathBuf, ApplicationTestingBundle, dctx0_core::WorkspaceIr)> {
        self.read_application_testing_bundle_checked(
            application,
            relative_plan,
            Some(expected_sha256),
        )
    }

    pub(crate) fn read_application_testing_bundle_checked(
        &self,
        application: &Path,
        relative_plan: &str,
        expected_sha256: Option<&str>,
    ) -> Result<(PathBuf, ApplicationTestingBundle, dctx0_core::WorkspaceIr)> {
        let (root, config) = self.verification_configuration(Some(application))?;
        let (_, path) = verification_setup::safe_file(&root, relative_plan)?;
        if fs::metadata(&path)?.len() > config.application_plan_bytes() {
            return Err(plan_budget_error(config.application_plan_bytes()));
        }
        let bytes = fs::read(path)?;
        if bytes.len() as u64 > config.application_plan_bytes() {
            return Err(plan_budget_error(config.application_plan_bytes()));
        }
        if let Some(expected) = expected_sha256 {
            if expected.len() != 64
                || !expected
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || format!("{:x}", Sha256::digest(&bytes)) != expected
            {
                return Err(Error::Invalid(
                    "Application-test plan SHA-256 changed since preparation".into(),
                ));
            }
        }
        let bundle: ApplicationTestingBundle = serde_json::from_slice(&bytes)?;
        if bundle.schema_version != 1 {
            return Err(Error::Invalid(
                "Unsupported application-test bundle version".into(),
            ));
        }
        bundle.plan.validate().map_err(invalid)?;
        let fixtures = serde_json::from_slice(&fs::read(root.join(&config.fixtures))?)?;
        let current = self.verification_plan(
            VerificationSelection {
                subjects: config.scope.subjects,
                tasks: config.scope.tasks,
                historical_check: None,
            },
            &fixtures,
            config.budgets,
        )?;
        let snapshot = self.load(&current.snapshot)?;
        if bundle.plan.program != current
            || bundle.spec_source_hash != snapshot.source_hash
            || bundle.model.data != snapshot.ir.data
            || bundle.model.operations != snapshot.ir.operations
            || bundle.lifecycle_inputs != lifecycle_inputs(&root, &config.adapter)?
            || bundle.evidence_bytes
                != config
                    .execution
                    .as_ref()
                    .map_or(4_194_304, |limits| limits.evidence_bytes)
            || bundle.case_seconds
                != config
                    .execution
                    .as_ref()
                    .map_or(5, |limits| limits.case_seconds)
        {
            return Err(Error::Invalid("Application-test bundle no longer matches published requirements, fixtures, inputs or budgets; prepare it again".into()));
        }
        Ok((root, bundle, snapshot.ir))
    }

    pub async fn serve_application_tests(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        endpoint: &str,
        report: &str,
    ) -> Result<ApplicationAssessmentReport> {
        let (root, bundle, ir) = self.read_application_testing_bundle(application, plan)?;
        let (directory, output) = verification_setup::safe_file(&root, report)?;
        let result =
            crate::application_testing_service::serve(&ir, &bundle, group, endpoint).await?;
        fs::create_dir_all(directory)?;
        atomic_write(&output, &serde_json::to_vec_pretty(&result)?)?;
        Ok(result)
    }
}

fn plan_budget_error(bytes: u64) -> Error {
    if bytes == 33_554_432 {
        Error::Invalid("Application-test bundle exceeds 32 MiB".into())
    } else {
        Error::Invalid(format!(
            "Application-test bundle exceeds configured plan budget of {bytes} bytes"
        ))
    }
}
