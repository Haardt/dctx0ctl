//! Read-only prerequisites, sharing the published planner and native task pins.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationIntent {
    Diagnostic,
    Delivery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPreflightPin {
    pub snapshot: String,
    pub check_occurrence: u64,
    pub plan_hash: String,
    pub inputs_hash: String,
    pub task_pin: Option<VerificationTaskPin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationPreflight {
    pub schema_version: u32,
    pub intent: VerificationIntent,
    pub pin: VerificationPreflightPin,
    pub required_cases: usize,
    pub required_capabilities: BTreeSet<String>,
    /// Potential native delivery only. Actual observations, source reviews and
    /// transactional guards must still pass; this list never authorizes delivery.
    pub potential_tasks: Vec<i64>,
    pub blockers: Vec<VerificationIssue>,
    pub warnings: Vec<VerificationIssue>,
    pub bindings_checked: bool,
    pub fixture_readiness: String,
}
impl VerificationPreflight {
    pub fn permits_execution(&self) -> bool {
        self.intent == VerificationIntent::Diagnostic || self.blockers.is_empty()
    }
}

impl Project {
    /// No adapter/runtime execution, capture directory, locks or database writes.
    pub fn verification_preflight(
        &self,
        application: Option<&Path>,
        mut selection: VerificationSelection,
        intent: VerificationIntent,
        bindings: Option<&str>,
    ) -> Result<VerificationPreflight> {
        let (application, config) = self.verification_configuration(application)?;
        if selection.subjects.is_empty() && selection.tasks.is_empty() {
            selection.subjects = config.scope.subjects.clone();
            selection.tasks = config.scope.tasks.clone();
        }
        let historical = selection.historical_check.is_some();
        let fixtures = crate::read_review_input(
            &crate::verification_setup::safe_file(&application, &config.fixtures)?.1,
            crate::DEFAULT_REVIEW_INPUT_BYTES,
            "Preflight fixtures",
        )?;
        let plan = self.verification_plan(selection, &fixtures, config.budgets.clone())?;
        let snapshot = self.load(&plan.snapshot)?;
        let (task_pin, pin_issues) =
            pins::capture(self, &application, &config.target, &plan, &snapshot)?;
        let requirements = dctx0_verification::binding_requirements(&snapshot.ir, &plan);
        let manifest = dctx0_verification::case_manifest(&plan);
        let mut blockers = pin_issues;
        let mut warnings = vec![];
        if historical {
            blockers.push(issue("PREFLIGHT_HISTORICAL_SCOPE", "Historical checks support diagnostic execution only; select the current published task plan for delivery"));
        }
        if task_pin.is_none() {
            blockers.push(issue("ADOPTION_TASK_PIN_REQUIRED", "No task requirements would be pinned. Select matching --task IDs or publish a matching task plan; use --no-adopt for an explicit diagnostic run"));
        }
        // Non-executable authored reviews do not prohibit gathering native
        // evidence. They remain mandatory for their own task-specific proofs.
        // Input planning has no before-state observer. The native executor can
        // discharge these exact hints only after checking every generated case
        // against actual recorded before-state; absence is not yet established.
        let runtime_state_requirements = plan
            .operations
            .iter()
            .flat_map(|operation| &operation.inputs.issues)
            .filter(|issue| issue.code == "STATE_OBSERVER_REQUIRED")
            .collect::<Vec<_>>();
        for problem in &manifest.open_obligations {
            if matches!(
                problem.code.as_str(),
                "MANUAL_REQUIREMENT" | "SUITE_INSPECTION_REQUIRED"
            ) || runtime_state_requirements.contains(&problem)
            {
                warnings.push(problem.clone());
            } else {
                blockers.push(problem.clone());
            }
        }
        blockers.extend(self.verification_lifecycle_input_issues(
            &application,
            &config.adapter,
            &plan,
        )?);
        let mut bindings_checked = false;
        if let Some(path) = bindings {
            let description = crate::read_verification_bindings(&application, path)?;
            if description.target != config.target {
                blockers.push(issue(
                    "PREFLIGHT_BINDING_TARGET",
                    "Reviewed bindings target does not match verification configuration",
                ));
            }
            blockers.extend(description.missing(&requirements));
            for (capability, bound) in [
                ("events", description.effects.events.is_some()),
                ("call-observation", description.effects.calls.is_some()),
                (
                    "resource-observation",
                    description.effects.resources.is_some(),
                ),
            ] {
                if requirements.capabilities.contains(capability) && !bound {
                    blockers.push(issue(
                        "PREFLIGHT_OBSERVER_REQUIRED",
                        format!("{capability}: explicit observer binding is missing"),
                    ));
                }
            }
            bindings_checked = true;
        } else {
            warnings.push(issue("PREFLIGHT_BINDINGS_UNKNOWN", "Handwritten adapter bindings and observer capabilities cannot be proved statically. Supply --bindings with an authored BindingDescription; runtime Describe and actual execution remain required"));
        }
        warnings.push(issue("PREFLIGHT_FIXTURE_UNKNOWN", "Private fixture availability, provisioning and ownership are authored runtime prerequisites; static planning does not establish them"));
        let mut potential_tasks = vec![];
        if let Some(pin) = &task_pin {
            let session =
                self.task_metadata_session(Some(pin.plan_id), Some(pin.event_sequence))?;
            let covered = manifest
                .case_requirements
                .values()
                .flatten()
                .collect::<BTreeSet<_>>();
            for id in pin.requirements.keys() {
                let state = &session.states[id];
                let member = session.member(*id)?;
                let requirements_mapped = member
                    .candidate
                    .subjects
                    .iter()
                    .chain(&member.candidate.types)
                    .chain(&member.candidate.facts)
                    .filter(|requirement| {
                        !plan.manual_checks.iter().any(|check| {
                            check.subject.as_str() == requirement.as_str()
                                && check.kind == StatementKind::Intent
                        })
                    })
                    .all(|requirement| covered.contains(requirement));
                if !pin.profile_bundles.contains_key(id) {
                    blockers.push(issue(
                        "PREFLIGHT_PROFILE_REQUIRED",
                        format!("T{id}: no complete selected profile bundle is pinned"),
                    ));
                }
                if !requirements_mapped {
                    warnings.push(issue("PREFLIGHT_TASK_REQUIREMENTS_UNMAPPED",format!("T{id}: some task requirements have no mandatory native case mapping; dedicated source-bound inspection or implementation work remains required")));
                }
                if state.work == dctx0_tasks::WorkState::Open
                    && state.actor.is_none()
                    && state.claim.is_none()
                    && state.reservation.is_none()
                    && matches!(
                        member.candidate.kind,
                        dctx0_tasks::TaskKind::Contract | dctx0_tasks::TaskKind::Operation
                    )
                    && pin.profile_bundles.contains_key(id)
                    && requirements_mapped
                {
                    potential_tasks.push(*id);
                }
            }
        }
        if potential_tasks.is_empty() {
            blockers.push(issue("PREFLIGHT_NO_DELIVERABLE_TASKS", "No unclaimed Open contract/operation tasks have matching task and profile pins; use the dedicated review/claim workflow or an explicit diagnostic run"));
        }
        let inputs = Capture::inventory_with_budget(&application, config.capture_budgets())?;
        if config
            .adapter_scaffold_hash
            .as_ref()
            .is_some_and(|hash| inputs.get(&config.adapter) == Some(hash))
        {
            blockers.push(issue("PREFLIGHT_BINDINGS_UNAVAILABLE", "The generated unavailable adapter is unchanged; implement actual production bindings before a delivery run"));
        }
        let report = VerificationPreflight {
            schema_version: 1,
            intent,
            pin: VerificationPreflightPin {
                snapshot: plan.snapshot.clone(),
                check_occurrence: plan.check_occurrence,
                plan_hash: digest(&serde_json::to_vec(&plan)?),
                inputs_hash: digest(&serde_json::to_vec(&inputs)?),
                task_pin,
            },
            required_cases: manifest.required_cases.len(),
            required_capabilities: requirements.capabilities,
            potential_tasks: if blockers.is_empty() {
                potential_tasks
            } else {
                vec![]
            },
            blockers,
            warnings,
            bindings_checked,
            fixture_readiness: "unknown-until-authored-runtime-setup".into(),
        };
        Ok(report)
    }
}
