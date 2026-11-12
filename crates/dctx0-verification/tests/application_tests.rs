use dctx0_core::*;
use dctx0_verification::{application_plan::*, *};
use std::collections::BTreeSet;

fn plan() -> ProgramPlan {
    let compiled = dctx0_language::compile(&[Source {
        file: "application-tests.dctx0".into(),
        text: "workspace W { type Year = Int\n port API { operation echo(value:Year) -> Year } }"
            .into(),
    }]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    project_program(
        &compiled.ir.unwrap(),
        "snapshot",
        7,
        VerificationScope {
            subjects: ["W".into()].into(),
            tasks: BTreeSet::new(),
        },
        &Default::default(),
        VerificationBudgets {
            generated_cases: 3,
            ..Default::default()
        },
    )
}
fn group(id: &str, cases: BTreeSet<String>) -> ExecutionGroup {
    ExecutionGroup {
        id: id.into(),
        backend: TestBackend::Adapter,
        target: "go".into(),
        selection: ["legacy".into()].into(),
        area: TestArea {
            cases,
            ..Default::default()
        },
        resources: GroupResources {
            namespace: id.into(),
            timeout_seconds: 30,
            max_message_bytes: 1_048_576,
        },
    }
}
fn configuration(plan: &ProgramPlan) -> ApplicationTestConfiguration {
    ApplicationTestConfiguration {
        schema_version: APPLICATION_TEST_CONFIGURATION_VERSION,
        groups: vec![group("legacy", case_manifest(plan).required_cases)],
    }
}
fn codes(
    program: &ProgramPlan,
    config: &ApplicationTestConfiguration,
    registrations: &[TestRegistration],
) -> BTreeSet<String> {
    application_test_plan(program, config, registrations)
        .unwrap_err()
        .into_iter()
        .map(|issue| issue.code)
        .collect()
}
fn has(
    program: &ProgramPlan,
    config: &ApplicationTestConfiguration,
    registrations: &[TestRegistration],
    expected: &str,
) {
    let actual = codes(program, config, registrations);
    assert!(actual.contains(expected), "Expected {expected}: {actual:?}");
}

#[test]
fn mixed_backends_keep_identical_cases_inputs_requirements_and_versions() {
    let program = plan();
    let config = configuration(&program);
    let original = application_test_plan(&program, &config, &[]).unwrap();
    let mut mixed = config.clone();
    let cases = original
        .manifest
        .case_requirements
        .iter()
        .filter(|(_, r)| r.contains("W.Year"))
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    mixed.groups[0].area.cases = original
        .manifest
        .required_cases
        .difference(&cases)
        .cloned()
        .collect();
    let mut direct = group("direct", cases.clone());
    direct.backend = TestBackend::ApplicationTests;
    direct.selection = ["TestYear".into()].into();
    mixed.groups.push(direct);
    let registration = TestRegistration {
        group: "direct".into(),
        test: "TestYear".into(),
        form: TestForm::Decoder,
        cases,
    };
    let export = application_test_plan(&program, &mixed, &[registration]).unwrap();
    assert_eq!(original.program, export.program);
    assert_eq!(original.manifest, export.manifest);
    assert_eq!(
        export.case_groups.len(),
        export.manifest.required_cases.len()
    );
    let encoded = serde_json::to_string(&export).unwrap();
    let restored: ApplicationTestPlan = serde_json::from_str(&encoded).unwrap();
    assert_eq!(export, restored);
    restored.validate().unwrap();
    for target in ["typescript", "rust", "java"] {
        let mut config = config.clone();
        config.groups[0].target = target.into();
        let other = application_test_plan(&program, &config, &[]).unwrap();
        assert_eq!(export.program, other.program);
        assert_eq!(export.manifest, other.manifest);
    }
}

#[test]
fn requirements_select_whole_cases_and_detect_stale_or_conflicting_ownership() {
    let program = plan();
    let mut config = configuration(&program);
    let cases = config.groups[0].area.cases.clone();
    config.groups[0].area.cases.clear();
    config.groups[0].area.requirements = case_manifest(&program)
        .case_requirements
        .values()
        .flatten()
        .cloned()
        .collect();
    assert_eq!(
        application_test_plan(&program, &config, &[])
            .unwrap()
            .case_groups
            .len(),
        cases.len()
    );
    config.groups[0].area.cases.insert("unknown".into());
    config.groups[0].area.requirements.insert("unknown".into());
    has(&program, &config, &[], "UNKNOWN_TEST_CASE");
    has(&program, &config, &[], "UNKNOWN_TEST_REQUIREMENT");
    config.groups.push(group("other", cases));
    has(&program, &config, &[], "CONFLICTING_CASE_GROUPS");
    let config = ApplicationTestConfiguration {
        schema_version: 2,
        groups: vec![],
    };
    has(&program, &config, &[], "UNASSIGNED_TEST_CASE");
    has(&program, &config, &[], "EMPTY_EXECUTION_GROUPS");
}

#[test]
fn invalid_group_identifiers_selection_resources_and_versions_are_rejected() {
    let program = plan();
    let config = configuration(&program);
    for mutation in 0..11 {
        let mut config = config.clone();
        match mutation {
            0 => config.schema_version = 3,
            1 => config.groups[0].id = "../escape".into(),
            2 => config.groups.push(config.groups[0].clone()),
            3 => config.groups[0].target = "future".into(),
            4 => config.groups[0].selection = ["\n".into()].into(),
            5 => {
                config.groups[0].selection.insert("second-adapter".into());
            }
            6 => config.groups[0].resources.timeout_seconds = 0,
            7 => config.groups[0].resources.max_message_bytes = 67_108_865,
            8 => config.groups[0].resources.namespace = "../shared".into(),
            9 => config.groups[0].area = TestArea::default(),
            _ => config.groups[0].selection.clear(),
        }
        assert!(
            application_test_plan(&program, &config, &[]).is_err(),
            "mutation {mutation}"
        );
    }
    let mut bad = program.clone();
    bad.schema_version = 2;
    bad.snapshot.clear();
    has(&bad, &config, &[], "APPLICATION_PROGRAM_VERSION");
    has(&bad, &config, &[], "APPLICATION_PLAN_UNPINNED");
    let mut encoded = serde_json::to_value(config).unwrap();
    encoded["unexpected"] = true.into();
    assert!(serde_json::from_value::<ApplicationTestConfiguration>(encoded).is_err());
}

#[test]
fn missing_duplicate_or_wrongly_owned_registrations_never_establish_completeness() {
    let mut program = plan();
    program.operations.clear();
    let mut config = configuration(&program);
    config.groups[0].backend = TestBackend::ApplicationTests;
    config.groups[0].selection = ["TestAll".into()].into();
    has(&program, &config, &[], "MISSING_TEST_REGISTRATION");
    has(&program, &config, &[], "UNREGISTERED_TEST_CASE");
    let valid = TestRegistration {
        group: "legacy".into(),
        test: "TestAll".into(),
        form: TestForm::Decoder,
        cases: config.groups[0].area.cases.clone(),
    };
    application_test_plan(&program, &config, std::slice::from_ref(&valid)).unwrap();
    has(
        &program,
        &config,
        &[valid.clone(), valid.clone()],
        "INVALID_TEST_REGISTRATION",
    );
    has(
        &program,
        &config,
        &[valid.clone(), valid.clone()],
        "DUPLICATE_CASE_REGISTRATION",
    );
    let mut wrong = valid.clone();
    wrong.group = "absent".into();
    has(&program, &config, &[wrong], "REGISTRATION_CASE_GROUP");
    let mut wrong = valid.clone();
    wrong.test = "NotSelected".into();
    has(&program, &config, &[wrong], "INVALID_TEST_REGISTRATION");
    let mut empty = valid;
    empty.cases.clear();
    has(&program, &config, &[empty], "INVALID_TEST_REGISTRATION");
}

#[test]
fn imported_manifest_and_ownership_are_recomputed_and_open_points_survive_export() {
    let program = plan();
    let config = configuration(&program);
    let export = application_test_plan(&program, &config, &[]).unwrap();
    assert_eq!(
        export.manifest.open_obligations,
        case_manifest(&program).open_obligations
    );
    for mutation in 0..4 {
        let mut bad = export.clone();
        match mutation {
            0 => bad.schema_version = 2,
            1 => bad.manifest.required_cases.clear(),
            2 => bad.case_groups.clear(),
            _ => bad.configuration.groups.clear(),
        }
        assert!(bad.validate().is_err());
    }
}

#[test]
fn lifecycle_paths_must_share_the_same_execution_group() {
    let mut program = plan();
    program.lifecycles.push(LifecyclePlan {
        subject: "W.Lifecycle".into(),
        entity: "W.Entity".into(),
        field: "status".into(),
        states: vec![],
        transitions: vec![],
        issues: vec![],
        paths: ["first", "second"]
            .map(|id| LifecyclePath {
                id: id.into(),
                initial: "Open".into(),
                transitions: vec![],
                final_state: "Closed".into(),
            })
            .into(),
    });
    let mut config = configuration(&program);
    application_test_plan(&program, &config, &[]).unwrap();
    config.groups[0].area.cases.remove("second");
    config.groups.push(group("other", ["second".into()].into()));
    has(&program, &config, &[], "SPLIT_LIFECYCLE_GROUP");
}

#[test]
fn cinema_pilot_assigns_direct_contract_policy_and_persistence_cases_with_legacy_remainder() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema-go");
    let sources = ["workspace.dctx0", "movie-night.dctx0"].map(|file| Source {
        file: file.into(),
        text: std::fs::read_to_string(root.join("dctx0").join(file)).unwrap(),
    });
    let compiled = dctx0_language::compile(&sources);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let fixtures = serde_json::from_slice(
        &std::fs::read(root.join(".dctx0/testing/go/examples.json")).unwrap(),
    )
    .unwrap();
    let program = project_program(
        &compiled.ir.unwrap(),
        "cinema-snapshot",
        3,
        VerificationScope {
            subjects: ["HomeCinema.MovieNight".into()].into(),
            tasks: BTreeSet::new(),
        },
        &fixtures,
        Default::default(),
    );
    let reference = application_test_plan(&program, &configuration(&program), &[]).unwrap();
    let mut config = configuration(&program);
    let mut registrations = vec![];
    for (subject, test, form) in [
        (
            "HomeCinema.MovieNight.ReleaseYear",
            "TestReleaseYear",
            TestForm::Decoder,
        ),
        (
            "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner",
            "TestWinner",
            TestForm::Operation,
        ),
        (
            "HomeCinema.MovieNight.Catalog.FilmCatalogStore.save",
            "TestSave",
            TestForm::StateChange,
        ),
    ] {
        let cases = reference
            .manifest
            .case_requirements
            .iter()
            .filter(|(_, r)| r.contains(subject))
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        assert!(!cases.is_empty(), "{subject}");
        config.groups[0].area.cases.retain(|id| !cases.contains(id));
        let mut direct = group(test, cases.clone());
        direct.backend = TestBackend::ApplicationTests;
        direct.selection = [test.into()].into();
        config.groups.push(direct);
        registrations.push(TestRegistration {
            group: test.into(),
            test: test.into(),
            form,
            cases,
        });
    }
    let mixed = application_test_plan(&program, &config, &registrations).unwrap();
    assert_eq!(mixed.manifest, reference.manifest);
    assert_eq!(mixed.program, reference.program);
    assert!(mixed.case_groups.values().any(|id| id == "legacy"));
    let forms = mixed.case_forms.values().copied().collect::<BTreeSet<_>>();
    for form in [
        TestForm::Decoder,
        TestForm::Operation,
        TestForm::StateChange,
        TestForm::Scenario,
        TestForm::Lifecycle,
        TestForm::Assertion,
        TestForm::EntityIdentity,
    ] {
        assert!(forms.contains(&form), "Missing {form:?}");
    }
    let mut wrong = registrations.clone();
    wrong[2].form = TestForm::Operation;
    has(&program, &config, &wrong, "REGISTRATION_TEST_FORM");
}

#[test]
fn duplicate_manifest_occurrences_and_invalid_reviewed_cases_are_rejected_before_execution() {
    let mut program = plan();
    let operation = &mut program.operations[0];
    let input = OperationCaseInput {
        id: "same-obligation".into(),
        invocation: Invocation {
            subject: operation.operation.id.clone(),
            arguments: vec![VerificationValue::Integer("1".into())],
        },
        setup: vec![],
        argument_bindings: vec![],
        mocks: vec![],
    };
    let mut different = input.clone();
    different.invocation.arguments = vec![VerificationValue::Integer("2".into())];
    operation.cases = vec![input, different];
    let config = configuration(&program);
    has(&program, &config, &[], "DUPLICATE_CASE_ID");
    program.operations[0].cases.pop();
    program.issues.push(VerificationIssue {
        code: "INVALID_OPERATION_CASE".into(),
        message: "reviewed fixture has a wrong signature".into(),
        span: None,
    });
    let config = configuration(&program);
    has(&program, &config, &[], "INVALID_OPERATION_CASE");
}

fn hybrid_fixture() -> (
    ApplicationTestPlan,
    VerificationProvenance,
    Vec<dctx0_verification::hybrid_assessment::HybridGroupRun>,
) {
    let program = plan();
    let mut config = configuration(&program);
    let classified = application_test_plan(&program, &config, &[]).unwrap();
    let native = classified
        .case_forms
        .iter()
        .filter(|(_, form)| **form == TestForm::Operation)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    assert!(!native.is_empty());
    config.groups[0].area.cases = classified
        .manifest
        .required_cases
        .difference(&native)
        .cloned()
        .collect();
    let mut direct = group("native", native.clone());
    direct.backend = TestBackend::ApplicationTests;
    direct.selection = ["TestEcho".into()].into();
    config.groups.push(direct);
    let plan = application_test_plan(
        &program,
        &config,
        &[TestRegistration {
            group: "native".into(),
            test: "TestEcho".into(),
            form: TestForm::Operation,
            cases: native,
        }],
    )
    .unwrap();
    let expected: VerificationProvenance = serde_json::from_value(serde_json::json!({
        "snapshot":"snapshot","check_occurrence":7,"plan_hash":"current-plan","implementation":{"main.go":"current-source"},"dependencies":{},"adapter_hash":"adapter","configuration_hash":"configuration","harness_hash":"harness","interpreter_version":"interpreter","generator_version":"generator","protocol_version":1,"target":"go","runtime_version":"go-current","build_configuration":{},"task_pin":null,"incomplete":[]
    })).unwrap();
    let groups = plan
        .configuration
        .groups
        .iter()
        .map(|group| {
            let required_cases = plan
                .case_groups
                .iter()
                .filter(|(_, owner)| *owner == &group.id)
                .map(|(id, _)| id.clone())
                .collect::<BTreeSet<_>>();
            let cases = required_cases
                .iter()
                .map(|id| VerificationCaseRecord {
                    id: id.clone(),
                    subject: if plan.case_forms[id] == TestForm::Operation {
                        "W.API.echo"
                    } else {
                        "W.Year"
                    }
                    .into(),
                    kind: if plan.case_forms[id] == TestForm::Operation {
                        VerificationCaseKind::Operation
                    } else {
                        VerificationCaseKind::Contract
                    },
                    bindings: Default::default(),
                    setup: vec![],
                    arguments: vec![],
                    observations: vec![],
                    events: None,
                    resources: None,
                    states: vec![],
                    result: VerificationResult::Pass,
                    elapsed_us: 1,
                })
                .collect();
            let mut provenance = expected.clone();
            provenance.protocol_version = if group.backend == TestBackend::Adapter {
                1
            } else {
                2
            };
            dctx0_verification::hybrid_assessment::HybridGroupRun {
                group: group.id.clone(),
                run: VerificationRun {
                    schema_version: 1,
                    origin: VerificationRunOrigin::Native,
                    started_at_unix_ns: "2000000".into(),
                    elapsed_us: 1,
                    provenance,
                    budgets: program.budgets.clone(),
                    selected_subjects: program.scope.subjects.clone(),
                    required_subjects: program.closure.clone(),
                    not_selected: program.not_selected.clone(),
                    required_cases,
                    cases,
                    open_obligations: vec![],
                    bounded_obligations: vec![],
                    artifacts: vec![],
                    result: VerificationResult::Pass,
                },
            }
        })
        .collect();
    (plan, expected, groups)
}

#[test]
fn hybrid_assembly_requires_each_current_assigned_group_without_changing_original_inventory() {
    use dctx0_verification::hybrid_assessment::assemble_hybrid_cases;
    let (plan, pin, groups) = hybrid_fixture();
    let cases = assemble_hybrid_cases(&plan, &pin, "1000000", "5000000", &groups).unwrap();
    assert_eq!(
        cases
            .iter()
            .map(|case| case.id.clone())
            .collect::<BTreeSet<_>>(),
        plan.manifest.required_cases
    );
    let mut reverse = groups.clone();
    reverse.reverse();
    assert_eq!(
        cases,
        assemble_hybrid_cases(&plan, &pin, "1000000", "5000000", &reverse).unwrap()
    );
    // This pure domain result deliberately supplies no runtime/build receipt or
    // transactional task authority; production integration must prove both.
    assert!(
        groups
            .iter()
            .all(|group| group.run.provenance.task_pin.is_none())
    );
    let missing =
        assemble_hybrid_cases(&plan, &pin, "1000000", "5000000", &groups[..1]).unwrap_err();
    assert!(
        missing
            .iter()
            .any(|issue| issue.code == "HYBRID_INCOMPLETE_COHORT")
    );
    let mut duplicate = groups.clone();
    duplicate.push(groups[0].clone());
    let errors = assemble_hybrid_cases(&plan, &pin, "1000000", "5000000", &duplicate).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|issue| issue.code == "HYBRID_DUPLICATE_GROUP")
    );
    assert!(
        errors
            .iter()
            .any(|issue| issue.code == "HYBRID_DUPLICATE_CASE")
    );
}

#[test]
fn hybrid_assembly_rejects_historical_imported_changed_or_incomplete_group_evidence() {
    use dctx0_verification::hybrid_assessment::assemble_hybrid_cases;
    let (plan, pin, groups) = hybrid_fixture();
    type GroupMutation = fn(&mut VerificationRun);
    let probes: [(&str, GroupMutation); 14] = [
        ("HYBRID_FRESH_EXECUTION_REQUIRED", |run| {
            run.origin = VerificationRunOrigin::Imported
        }),
        ("HYBRID_FRESH_EXECUTION_REQUIRED", |run| {
            run.started_at_unix_ns = "999999".into()
        }),
        ("HYBRID_FRESH_EXECUTION_REQUIRED", |run| {
            run.elapsed_us = 4000
        }),
        ("HYBRID_FRESH_EXECUTION_REQUIRED", |run| {
            run.started_at_unix_ns = "bad-time".into()
        }),
        ("HYBRID_GROUP_PIN_MISMATCH", |run| {
            run.provenance.implementation.clear()
        }),
        ("HYBRID_GROUP_PIN_MISMATCH", |run| {
            run.provenance.check_occurrence += 1
        }),
        ("HYBRID_GROUP_PIN_MISMATCH", |run| {
            run.provenance.configuration_hash = "changed".into()
        }),
        ("HYBRID_GROUP_PIN_MISMATCH", |run| run.budgets.seed += 1),
        ("HYBRID_GROUP_PIN_MISMATCH", |run| {
            run.provenance.protocol_version = 999
        }),
        ("HYBRID_GROUP_CASE_MISMATCH", |run| {
            run.cases.pop();
        }),
        ("HYBRID_GROUP_CASE_MISMATCH", |run| {
            run.required_cases.clear()
        }),
        ("HYBRID_GROUP_CASE_MISMATCH", |run| {
            run.cases.push(run.cases[0].clone())
        }),
        ("HYBRID_CASE_KIND_MISMATCH", |run| {
            run.cases[0].kind = VerificationCaseKind::Lifecycle
        }),
        ("HYBRID_GROUP_NOT_PASS", |run| {
            run.cases[0].result = VerificationResult::Fail {
                issue: VerificationIssue {
                    code: "ACTUAL_CONTRACT_FAILURE".into(),
                    message: "wrong return".into(),
                    span: None,
                },
            }
        }),
    ];
    for (expected, mutate) in probes {
        let mut changed = groups.clone();
        mutate(&mut changed[0].run);
        let issues =
            assemble_hybrid_cases(&plan, &pin, "1000000", "5000000", &changed).unwrap_err();
        assert!(
            issues.iter().any(|issue| issue.code == expected),
            "{expected}: {issues:?}"
        );
    }
    for (start, end) in [("bad", "5000000"), ("0", "5000000"), ("5000001", "5000000")] {
        assert_eq!(
            assemble_hybrid_cases(&plan, &pin, start, end, &groups).unwrap_err()[0].code,
            "HYBRID_COHORT_PIN"
        );
    }
    let mut foreign = groups.clone();
    foreign[0].group = "foreign".into();
    assert!(
        assemble_hybrid_cases(&plan, &pin, "1000000", "5000000", &foreign)
            .unwrap_err()
            .iter()
            .any(|issue| issue.code == "HYBRID_UNKNOWN_GROUP")
    );
}
