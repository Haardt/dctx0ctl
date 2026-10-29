use dctx0_test_analysis::*;

fn fixture() -> (TestCatalog, CollectionRun, Vec<TestObservation>) {
    let tests: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|name| CatalogTest {
            version: TestVersion {
                key: LogicalTestKey {
                    project: "project".into(),
                    package: "package".into(),
                    target_kind: "lib".into(),
                    target_name: "library".into(),
                    test_name: name.into(),
                },
                snapshot: "base".into(),
                definition_hash: None,
                definition_hash_provenance: "individual definition not resolved".into(),
            },
            ignored: false,
        })
        .collect();
    let observations = tests
        .iter()
        .map(|test| TestObservation {
            id: test.version.key.test_name.clone(),
            collection: "baseline".into(),
            version: test.version.clone(),
            attempt: 1,
            outcome: TestOutcome::Pass,
            started_at: "start".into(),
            ended_at: "end".into(),
            duration_ms: 2.0,
            peak_rss_bytes: Some(4096),
            artifacts: vec![],
            coverage_artifact: None,
            coverage_available: true,
        })
        .collect();
    (
        TestCatalog {
            id: "catalog".into(),
            snapshot: "base".into(),
            scope: RunScope::default(),
            tests,
            targets: vec![],
            enumeration_complete: true,
            diagnostics: vec![],
        },
        CollectionRun {
            id: "baseline".into(),
            catalog: "catalog".into(),
            recipe: vec![],
            environment_fingerprint: "environment".into(),
            collector_version: IMPLEMENTATION_VERSION.into(),
            timeout_secs: 300,
            started_at: "start".into(),
            ended_at: Some("end".into()),
            legacy_execution_ids: vec![],
            collector_peak_rss_bytes: None,
            build_ms: 3.0,
            build_peak_rss_bytes: Some(8192),
            events: vec![],
        },
        observations,
    )
}
#[test]
fn isolated_reference_distinguishes_failures_missing_and_infrastructure() {
    let (mut catalog, collection, mut observations) = fixture();
    assert!(
        completeness(&catalog, &collection, &observations)
            .unwrap()
            .reference_evaluable
    );
    observations[0].outcome = TestOutcome::TestFailure;
    assert!(
        completeness(&catalog, &collection, &observations)
            .unwrap()
            .reference_evaluable
    );
    for outcome in [
        TestOutcome::Timeout,
        TestOutcome::Crash,
        TestOutcome::InfrastructureError,
        TestOutcome::Unsupported,
    ] {
        observations[0].outcome = outcome;
        let c = completeness(&catalog, &collection, &observations).unwrap();
        assert!(c.all_executable_tests_observed);
        assert!(!c.reference_evaluable);
        assert_eq!(c.unknown.len(), 1);
    }
    observations[0].outcome = TestOutcome::NotRun;
    assert!(
        !completeness(&catalog, &collection, &observations)
            .unwrap()
            .all_executable_tests_observed
    );
    observations[0].outcome = TestOutcome::Ignored;
    catalog.tests[0].ignored = true;
    assert!(
        completeness(&catalog, &collection, &observations)
            .unwrap()
            .reference_evaluable
    );
    observations[0].outcome = TestOutcome::Pass;
    assert!(completeness(&catalog, &collection, &observations).is_err());
    observations.remove(0);
    assert!(
        completeness(&catalog, &collection, &observations)
            .unwrap()
            .reference_evaluable
    );
    catalog.enumeration_complete = false;
    assert!(
        !completeness(&catalog, &collection, &observations)
            .unwrap()
            .reference_evaluable
    );
}
#[test]
fn frozen_coverage_selection_and_deliberately_wrong_selection_report_misses() {
    let (base, baseline, observations) = fixture();
    let mut target = base.clone();
    target.id = "target-catalog".into();
    target.snapshot = "target".into();
    for t in &mut target.tests {
        t.version.snapshot = "target".into();
    }
    let mut reference = baseline.clone();
    reference.id = "reference".into();
    reference.catalog = target.id.clone();
    let files = vec![ChangedFile {
        path: "src/lib.rs".into(),
        status: "M".into(),
        binary: false,
        ranges: vec![ChangedRange {
            old_start: 2,
            old_count: 1,
            new_start: 2,
            new_count: 1,
        }],
    }];
    let coverage = vec![CoverageInterval {
        test: base.tests[0].version.key.clone(),
        file: "src/lib.rs".into(),
        start: 1,
        end: 3,
    }];
    let prediction = predict(PredictionInputs {
        base: &base,
        target: &target,
        baseline: &baseline,
        observations: &observations,
        files: &files,
        coverage: &coverage,
    })
    .unwrap();
    assert_eq!(prediction.selected.len(), 1);
    assert!(!prediction.can_skip_other_tests);
    let mut labels = observations.clone();
    for (o, t) in labels.iter_mut().zip(&target.tests) {
        o.collection = reference.id.clone();
        o.version = t.version.clone();
    }
    assert_eq!(
        evaluate(
            &prediction,
            &base,
            &target,
            &reference,
            &labels,
            &observations
        )
        .unwrap()
        .failure_recall,
        None
    );
    labels[0].outcome = TestOutcome::TestFailure;
    let evaluated = evaluate(
        &prediction,
        &base,
        &target,
        &reference,
        &labels,
        &observations,
    )
    .unwrap();
    assert_eq!(evaluated.failure_recall, Some(1.0));
    assert_eq!(evaluated.pass_to_failure.len(), 1);
    assert_eq!(evaluated.shared_build_ms, 3.0);
    assert_eq!(evaluated.hypothetical_selected_test_ms, 2.0);
    assert!(!evaluated.reproducibility_confirmed);
    assert!(!evaluated.causal_regression_confirmed);
    let mut wrong = prediction.clone();
    wrong.selected.clear();
    let evaluated = evaluate(&wrong, &base, &target, &reference, &labels, &observations).unwrap();
    assert_eq!(evaluated.failure_recall, Some(0.0));
    assert_eq!(evaluated.missed_failures.len(), 1);
    labels[1].outcome = TestOutcome::Timeout;
    assert_eq!(
        evaluate(&wrong, &base, &target, &reference, &labels, &observations)
            .unwrap()
            .failure_recall,
        None
    );
    let mut incompatible = baseline.clone();
    incompatible.collector_version = "legacy".into();
    assert!(matches!(
        predict(PredictionInputs {
            base: &base,
            target: &target,
            baseline: &incompatible,
            observations: &observations,
            files: &files,
            coverage: &coverage
        }),
        Err(AnalysisError::BaselineRequired(_))
    ));
}
#[test]
fn unknown_changes_select_every_executable_test_and_new_tests_are_selected() {
    let (base, baseline, observations) = fixture();
    let coverage = vec![CoverageInterval {
        test: base.tests[0].version.key.clone(),
        file: "src/lib.rs".into(),
        start: 1,
        end: 3,
    }];
    for file in [
        ChangedFile {
            path: "src/new.rs".into(),
            status: "A".into(),
            binary: false,
            ranges: vec![],
        },
        ChangedFile {
            path: "Cargo.toml".into(),
            status: "M".into(),
            binary: false,
            ranges: vec![],
        },
        ChangedFile {
            path: "src/lib.rs".into(),
            status: "M".into(),
            binary: true,
            ranges: vec![],
        },
        ChangedFile {
            path: "src/lib.rs".into(),
            status: "M".into(),
            binary: false,
            ranges: vec![ChangedRange {
                old_start: 0,
                old_count: 0,
                new_start: 1,
                new_count: 1,
            }],
        },
    ] {
        let prediction = predict(PredictionInputs {
            base: &base,
            target: &base,
            baseline: &baseline,
            observations: &observations,
            files: &[file],
            coverage: &coverage,
        })
        .unwrap();
        assert_eq!(prediction.selected.len(), 2);
        assert!(!prediction.unknowns.is_empty());
    }
    let mut target = base.clone();
    target.tests[0].version.key.test_name = "new".into();
    let prediction = predict(PredictionInputs {
        base: &base,
        target: &target,
        baseline: &baseline,
        observations: &observations,
        files: &[],
        coverage: &coverage,
    })
    .unwrap();
    assert_eq!(prediction.selected.len(), 1);
    assert_eq!(
        prediction.selected[0].reasons,
        vec![SelectionReason::NewOrUnmappedTest]
    );
    assert_eq!(
        old_interval(&ChangedRange {
            old_start: 2,
            old_count: 3,
            new_start: 2,
            new_count: 0
        }),
        (2, 4, false)
    );
    assert_eq!(content_id(&target).unwrap(), content_id(&target).unwrap());
}

#[test]
fn prediction_identity_is_independent_of_database_observation_and_interval_order() {
    let (base, collection, mut observations) = fixture();
    let files = vec![ChangedFile {
        path: "src/lib.rs".into(),
        status: "M".into(),
        binary: false,
        ranges: vec![ChangedRange {
            old_start: 2,
            old_count: 1,
            new_start: 2,
            new_count: 1,
        }],
    }];
    let mut coverage = vec![
        CoverageInterval {
            test: base.tests[0].version.key.clone(),
            file: "src/lib.rs".into(),
            start: 1,
            end: 3,
        },
        CoverageInterval {
            test: base.tests[1].version.key.clone(),
            file: "src/lib.rs".into(),
            start: 7,
            end: 8,
        },
    ];
    let first = predict(PredictionInputs {
        base: &base,
        target: &base,
        baseline: &collection,
        observations: &observations,
        files: &files,
        coverage: &coverage,
    })
    .unwrap();
    observations.reverse();
    coverage.reverse();
    coverage.push(coverage[0].clone());
    let second = predict(PredictionInputs {
        base: &base,
        target: &base,
        baseline: &collection,
        observations: &observations,
        files: &files,
        coverage: &coverage,
    })
    .unwrap();
    assert_eq!(first.id, second.id);
}

#[test]
fn malformed_references_and_selections_are_rejected_and_unknown_costs_remain_unknown() {
    let (catalog, collection, observations) = fixture();
    let mut malformed = catalog.clone();
    malformed.tests.push(malformed.tests[0].clone());
    assert!(completeness(&malformed, &collection, &observations).is_err());
    let mut wrong = collection.clone();
    wrong.catalog = "other".into();
    assert!(completeness(&catalog, &wrong, &observations).is_err());
    assert_eq!(
        completeness(&catalog, &collection, &observations[..1])
            .unwrap()
            .missing
            .len(),
        1
    );
    for (attempt, duration) in [(0, 1.0), (1, f64::NAN)] {
        let mut invalid = observations.clone();
        invalid[0].attempt = attempt;
        invalid[0].duration_ms = duration;
        assert!(completeness(&catalog, &collection, &invalid).is_err());
    }
    let mut invalid = observations.clone();
    invalid[0].version.snapshot = "wrong".into();
    assert!(completeness(&catalog, &collection, &invalid).is_err());
    let files = vec![ChangedFile {
        path: "src/lib.rs".into(),
        status: "D".into(),
        binary: false,
        ranges: vec![ChangedRange {
            old_start: 1,
            old_count: 2,
            new_start: 0,
            new_count: 0,
        }],
    }];
    let invalid_coverage = vec![CoverageInterval {
        test: catalog.tests[0].version.key.clone(),
        file: "src/lib.rs".into(),
        start: 0,
        end: 1,
    }];
    assert!(
        predict(PredictionInputs {
            base: &catalog,
            target: &catalog,
            baseline: &collection,
            observations: &observations,
            files: &files,
            coverage: &invalid_coverage
        })
        .is_err()
    );
    let mut unavailable = observations.clone();
    unavailable[0].coverage_available = false;
    let prediction = predict(PredictionInputs {
        base: &catalog,
        target: &catalog,
        baseline: &collection,
        observations: &unavailable,
        files: &files,
        coverage: &[],
    })
    .unwrap();
    assert!(
        prediction
            .unknowns
            .contains("baseline_coverage_unavailable")
    );
    let mut reference = collection.clone();
    reference.id = "reference".into();
    let mut labels = observations.clone();
    for label in &mut labels {
        label.collection = reference.id.clone();
        label.outcome = TestOutcome::TestFailure;
        label.artifacts = vec![Artifact {
            sha256: "shared".into(),
            bytes: 7,
            media_type: "text/plain".into(),
        }];
    }
    labels[1].peak_rss_bytes = None;
    let mut prior = observations.clone();
    prior[0].outcome = TestOutcome::TestFailure;
    let result = evaluate(
        &prediction,
        &catalog,
        &catalog,
        &reference,
        &labels,
        &prior[..1],
    )
    .unwrap();
    assert_eq!(result.existing_failures.len(), 1);
    assert_eq!(result.new_or_unmapped_failures.len(), 1);
    assert_eq!(result.artifact_bytes, 7);
    assert_eq!(result.peak_test_rss_bytes, None);
    let mut bad = prediction.clone();
    bad.target_snapshot = "wrong".into();
    assert!(evaluate(&bad, &catalog, &catalog, &reference, &labels, &prior).is_err());
    let mut bad = prediction.clone();
    bad.selected.push(bad.selected[0].clone());
    assert!(evaluate(&bad, &catalog, &catalog, &reference, &labels, &prior).is_err());
    prior[0].collection = "future".into();
    assert!(evaluate(&prediction, &catalog, &catalog, &reference, &labels, &prior).is_err());
}

#[test]
fn malformed_offline_costs_cannot_overflow_or_publish_non_finite_totals() {
    let (catalog, collection, observations) = fixture();
    let prediction = predict(PredictionInputs {
        base: &catalog,
        target: &catalog,
        baseline: &collection,
        observations: &observations,
        files: &[],
        coverage: &[],
    })
    .unwrap();
    let mut invalid = collection.clone();
    invalid.build_ms = f64::INFINITY;
    assert!(completeness(&catalog, &invalid, &observations).is_err());
    let mut labels = observations.clone();
    for label in &mut labels {
        label.duration_ms = f64::MAX;
    }
    assert!(
        evaluate(
            &prediction,
            &catalog,
            &catalog,
            &collection,
            &labels,
            &observations
        )
        .is_err()
    );
    let mut labels = observations.clone();
    for (index, label) in labels.iter_mut().enumerate() {
        label.artifacts = vec![Artifact {
            sha256: format!("artifact-{index}"),
            bytes: u64::MAX,
            media_type: "application/octet-stream".into(),
        }];
    }
    assert!(
        evaluate(
            &prediction,
            &catalog,
            &catalog,
            &collection,
            &labels,
            &observations
        )
        .is_err()
    );
    labels[1].artifacts[0].sha256 = labels[0].artifacts[0].sha256.clone();
    labels[1].artifacts[0].bytes = 1;
    assert!(
        evaluate(
            &prediction,
            &catalog,
            &catalog,
            &collection,
            &labels,
            &observations
        )
        .is_err()
    );
}

#[test]
fn failure_labels_separate_new_tests_known_definition_changes_and_unknown_identity() {
    fn assess(
        base: &TestCatalog,
        collection: &CollectionRun,
        prior: &[TestObservation],
        target: &TestCatalog,
    ) -> SelectionEvaluation {
        let prediction = predict(PredictionInputs {
            base,
            target,
            baseline: collection,
            observations: prior,
            files: &[],
            coverage: &[],
        })
        .unwrap();
        let mut reference = collection.clone();
        reference.id = "reference".into();
        reference.catalog = target.id.clone();
        let labels: Vec<_> = target
            .tests
            .iter()
            .enumerate()
            .map(|(index, test)| {
                let mut observation = prior[0].clone();
                observation.id = format!("reference-{index}");
                observation.collection = reference.id.clone();
                observation.version = test.version.clone();
                observation.outcome = TestOutcome::TestFailure;
                observation
            })
            .collect();
        evaluate(&prediction, base, target, &reference, &labels, prior).unwrap()
    }
    let (mut base, collection, mut prior) = fixture();
    base.tests[0].version.definition_hash = Some(content_id(&"old test definition").unwrap());
    base.tests[0].version.definition_hash_provenance = "fixture definition sha256".into();
    prior[0].version = base.tests[0].version.clone();
    let mut target = base.clone();
    target.id = "target-catalog".into();
    target.snapshot = "target".into();
    for test in &mut target.tests {
        test.version.snapshot = target.snapshot.clone();
    }
    target.tests[0].version.definition_hash = Some(content_id(&"changed test definition").unwrap());
    let mut new = target.tests[1].clone();
    new.version.key.test_name = "new".into();
    target.tests.push(new);
    let result = assess(&base, &collection, &prior, &target);
    let labels = result.failure_classifications.as_ref().unwrap();
    assert_eq!(labels[0].identity, TestIdentityRelation::Known);
    assert_eq!(labels[0].definition, TestDefinitionChange::Changed);
    assert_eq!(labels[1].definition, TestDefinitionChange::Unknown);
    assert_eq!(labels[2].identity, TestIdentityRelation::New);
    assert!(!result.causal_regression_confirmed);
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["failure_classifications"][0]["definition"], "Changed");
    let mut historical = json;
    historical
        .as_object_mut()
        .unwrap()
        .remove("failure_classifications");
    assert!(
        serde_json::from_value::<SelectionEvaluation>(historical)
            .unwrap()
            .failure_classifications
            .is_none()
    );
    target.tests[0].version.definition_hash = base.tests[0].version.definition_hash.clone();
    assert_eq!(
        assess(&base, &collection, &prior, &target)
            .failure_classifications
            .unwrap()[0]
            .definition,
        TestDefinitionChange::Unchanged
    );
    target.tests[0].version.definition_hash_provenance = "another hash scheme".into();
    assert_eq!(
        assess(&base, &collection, &prior, &target)
            .failure_classifications
            .unwrap()[0]
            .definition,
        TestDefinitionChange::Unknown
    );
    base.tests[0].ignored = true;
    prior[0].outcome = TestOutcome::Ignored;
    assert!(
        assess(&base, &collection, &prior, &target)
            .failure_classifications
            .unwrap()[0]
            .previously_ignored
    );
    target.tests[2].version.key.target_kind = "test".into();
    target.tests[2].version.key.target_name = "custom".into();
    base.targets.push(TargetSupport {
        package: "package".into(),
        kind: "test".into(),
        name: "custom".into(),
        supported: false,
        reason: Some("custom harness".into()),
    });
    assert_eq!(
        assess(&base, &collection, &prior, &target)
            .failure_classifications
            .unwrap()[2]
            .identity,
        TestIdentityRelation::Unmapped
    );
}
