//! Current task delivery from independent native units, without a synthetic run.
use super::*;
use dctx0_store::{
    ComposedTaskReport, CompositionTaskPublication, TaskDelivery, TaskDeliveryPublication,
    TaskDeliveryPublicationReceipt, composition_implementation,
};
use dctx0_tasks::{CheckEvidence, CheckStatus, TaskResult};
use dctx0_verification::unit_planner;

/// The author explicitly pins the profiles inspected for current publication.
/// Capturing the latest profile bundle on the author's behalf would silently
/// rebind an old review when profiles change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionTaskAttestation {
    pub task: i64,
    pub inspection: VerificationAttestation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<dctx0_tasks::ReviewReport>,
    pub profile_bundle_hash: String,
}

fn publication_inputs(
    root: &Path,
    config: &VerificationConfiguration,
    configuration: &str,
) -> Result<BTreeMap<String, String>> {
    [
        ".dctx0/testing/verification.json",
        config.fixtures.as_str(),
        configuration,
    ]
    .into_iter()
    .map(|name| {
        let path = crate::verification_setup::safe_file(root, name)?.1;
        Ok((name.into(), digest(&fs::read(path)?)))
    })
    .collect()
}

pub(super) struct ComposedBinding {
    pub program: ProgramPlan,
    pub implementation: BTreeMap<String, String>,
    pub snapshot: dctx0_changes::Snapshot,
    pub profiles: LoadedProfiles,
    pub current: bool,
}

pub(super) fn current_task_binding(
    project: &Project,
    store: &dctx0_store::Store,
    root: &Path,
    report: &ComposedTaskReport,
) -> Result<ComposedBinding> {
    let (root, config, _) = units::configuration(project, root, &report.unit_configuration)?;
    let input_hashes = publication_inputs(&root, &config, &report.unit_configuration)?;
    let fixtures: ProjectionFixtures = crate::read_review_input(
        &crate::verification_setup::safe_file(&root, &config.fixtures)?.1,
        crate::DEFAULT_REVIEW_INPUT_BYTES,
        "Decoder projection fixtures",
    )?;
    let program = project.verification_plan(
        VerificationSelection {
            subjects: config.scope.subjects.clone(),
            tasks: config.scope.tasks.clone(),
            historical_check: None,
        },
        &fixtures,
        config.budgets.clone(),
    )?;
    let snapshot = project.load(&program.snapshot)?;
    let profiles = project
        .profiles(Some(&root), Some("go"))?
        .ok_or_else(|| Error::Invalid("Composition delivery needs complete profiles".into()))?;
    let composition = store.evidence_composition(&project.key(), &report.composition)?;
    let mut current = input_hashes == report.input_hashes
        && program.snapshot == report.inspection.snapshot
        && i64::try_from(program.check_occurrence).ok() == Some(report.check_occurrence)
        && unit_planner::evidence_digest(&program)? == report.inspection.plan_hash
        && dctx0_changes::source_hash(&project.sources()?) == snapshot.source_hash;
    let mut bindings = Vec::new();
    if current {
        for entry in &composition.units {
            let binding = units::binding::refresh_captured(&root, &config, &entry.binding)?;
            current &= binding == entry.binding;
            bindings.push(binding);
        }
        if current {
            store.validate_composition_publication(
                &project.key(),
                &CompositionTaskPublication {
                    composition: &report.composition,
                    program: &program,
                    current_bindings: &bindings,
                    configuration: &report.unit_configuration,
                    input_hashes: &input_hashes,
                },
            )?;
        }
    }
    let original: Vec<_> = composition
        .units
        .into_iter()
        .map(|unit| unit.binding)
        .collect();
    Ok(ComposedBinding {
        program,
        implementation: composition_implementation(&original),
        snapshot,
        profiles,
        current,
    })
}

impl Project {
    /// Reconstruct current decoder input bindings without building, acquiring
    /// fixtures, querying providers or starting workers. This is deliberately
    /// limited to the target whose complete process closure is implemented.
    fn current_decoder_composition(
        &self,
        application: &Path,
        configuration: &str,
        generator: &dyn HarnessGenerator,
        id: &str,
    ) -> Result<(ProgramPlan, Vec<UnitEvidenceBindingV1>)> {
        let (root, config, groups) = units::configuration(self, application, configuration)?;
        let fixtures: ProjectionFixtures = crate::read_review_input(
            &crate::verification_setup::safe_file(&root, &config.fixtures)?.1,
            crate::DEFAULT_REVIEW_INPUT_BYTES,
            "Decoder projection fixtures",
        )?;
        let project = |subjects, tasks| {
            self.verification_plan(
                VerificationSelection {
                    subjects,
                    tasks,
                    historical_check: None,
                },
                &fixtures,
                config.budgets.clone(),
            )
        };
        let program = project(config.scope.subjects.clone(), config.scope.tasks.clone())?;
        let composition = self
            .store(AccessMode::ReadOnly)?
            .evidence_composition(&self.key(), id)?;
        if composition.units.len() != groups.groups.len() {
            return Err(Error::Invalid(
                "Composition unit configuration changed".into(),
            ));
        }
        let mut seen = BTreeSet::new();
        let mut bindings = Vec::new();
        for group in groups.groups {
            if !seen.insert(group.id.clone()) {
                return Err(Error::Invalid("Duplicate decoder unit group".into()));
            }
            let original = composition
                .units
                .iter()
                .find(|unit| unit.binding.unit.id == group.id)
                .ok_or_else(|| Error::Invalid("Composition unit configuration changed".into()))?;
            let unit_program = project(group.subjects.clone(), BTreeSet::new())?;
            let recipe = units::materialize::recipe(&root, &config, &group, generator)?;
            bindings.push(units::binding::current(
                self,
                &unit_program,
                &group,
                &recipe,
                Some(&original.binding),
            )?);
        }
        unit_planner::validate_unit_partition(
            &dctx0_verification::case_manifest(&program).required_cases,
            bindings.iter().map(|binding| &binding.unit),
        )
        .map_err(Error::Invalid)?;
        Ok((program, bindings))
    }

    /// Explicit source-bound inspection plus validated native composition. Current
    /// publication pins are captured now; old native task IDs are not reused.
    pub fn publish_composition_attestations(
        &self,
        application: &Path,
        composition: &str,
        configuration: &str,
        generator: &dyn HarnessGenerator,
        attestations: &[CompositionTaskAttestation],
    ) -> Result<TaskDeliveryPublicationReceipt> {
        if attestations.is_empty() {
            return Err(Error::Invalid(
                "Nonempty inspected task reports are required".into(),
            ));
        }
        let _lock = self.lock()?;
        let (root, config) = self.verification_configuration(Some(application))?;
        let input_hashes = publication_inputs(&root, &config, configuration)?;
        let (program, bindings) =
            self.current_decoder_composition(application, configuration, generator, composition)?;
        if publication_inputs(&root, &config, configuration)? != input_hashes {
            return Err(Error::Invalid(
                "Composition publication inputs changed during capture".into(),
            ));
        }
        let store = self.store(AccessMode::ReadOnly)?;
        let session = self.task_session(None, None)?;
        let (check, snapshot_id) = store.task_check(&self.key())?;
        let snapshot = store.load(&self.key(), &snapshot_id)?;
        if program.snapshot != snapshot_id
            || i64::try_from(program.check_occurrence).ok() != Some(check)
            || session.plan.check_run_id != check
            || session.plan.snapshot_id != snapshot_id
            || dctx0_changes::source_hash(&self.sources()?) != snapshot.source_hash
        {
            return Err(Error::Invalid(
                "Composition publication requires current published specification and task plan"
                    .into(),
            ));
        }
        let profiles = self
            .profiles(Some(application), Some("go"))?
            .ok_or_else(|| Error::Invalid("Composition delivery needs complete profiles".into()))?;
        let implementation = composition_implementation(&bindings);
        let plan_hash = unit_planner::evidence_digest(&program)?;
        let evidence_identity = dctx0_changes::hash(&(composition, &plan_hash, &implementation));
        let actor = &attestations[0].inspection.actor;
        let mut pin = VerificationTaskPin {
            plan_id: session.plan.id,
            event_sequence: session.event_seq,
            requirements: BTreeMap::new(),
            profile_bundles: BTreeMap::new(),
        };
        let mut deliveries = Vec::new();
        let mut reviews = BTreeMap::new();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| Error::Invalid(error.to_string()))?
            .as_nanos()
            .to_string();
        for attestation in attestations {
            let member = session.member(attestation.task)?;
            let selection = dctx0_tasks::select_profiles(
                &snapshot,
                &member.candidate,
                &profiles.manifest,
                &profiles.documents,
            );
            selection.require_complete()?;
            if attestation.profile_bundle_hash != selection.bundle.hash {
                return Err(Error::Invalid("Composition inspection profile pin changed; a current authored inspection is required".into()));
            }
            if pin
                .requirements
                .insert(attestation.task, member.candidate.requirements_hash.clone())
                .is_some()
            {
                return Err(Error::Invalid(
                    "Duplicate task attestations are not permitted".into(),
                ));
            }
            pin.profile_bundles
                .insert(attestation.task, selection.bundle.hash.clone());
            if let Some(review) = &attestation.review {
                reviews.insert(attestation.task, review.clone());
            }
            deliveries.push(TaskDelivery {
                task: attestation.task,
                result: TaskResult {
                    summary: attestation.inspection.report.clone(),
                    artifacts: implementation.keys().cloned().collect(),
                    decisions: vec!["Explicit authored inspection backed by a current native evidence composition; original native runs remain unchanged".into()],
                    implementation: Some(evidence_identity.clone()),
                },
                evidence: CheckEvidence {
                    requirements_hash: member.candidate.requirements_hash.clone(),
                    implementation: evidence_identity.clone(),
                    verifier: actor.clone(),
                    verifier_version: "dctx0-composition-inspection-1".into(),
                    command: "dctx0 verify attest --composition-unit-groups".into(),
                    started_at: now.clone(), finished_at: now.clone(),
                    report: serde_json::to_string(&ComposedTaskReport {
                        schema_version: 1, basis: "native-evidence-composition+authored-inspection".into(),
                        composition: composition.into(), inspection: attestation.inspection.clone(),
                        check_occurrence: check,
                        unit_configuration: configuration.into(),
                        input_hashes: input_hashes.clone(),
                        profile_bundle_hash: selection.bundle.hash,
                    })?,
                    status: CheckStatus::Pass,
                },
            });
        }
        deliveries.sort_by_key(|delivery| {
            session
                .plan
                .members
                .iter()
                .find(|member| member.task_id == delivery.task)
                .map(|member| {
                    (
                        member.candidate.wave,
                        member.candidate.key.clone(),
                        delivery.task,
                    )
                })
                .unwrap_or((usize::MAX, String::new(), delivery.task))
        });
        Ok(self
            .store(AccessMode::ReadWrite)?
            .publish_composed_task_deliveries(
                &self.key(),
                TaskDeliveryPublication {
                    actor,
                    pin: &pin,
                    deliveries: &deliveries,
                    profiles: (&profiles.manifest, &profiles.documents),
                    run_id: None,
                },
                &reviews,
                &CompositionTaskPublication {
                    composition,
                    program: &program,
                    current_bindings: &bindings,
                    configuration,
                    input_hashes: &input_hashes,
                },
            )?)
    }
}
