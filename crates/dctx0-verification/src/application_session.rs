//! Stateful application-test assessment. Process/transport/runner evidence lives in the engine.
//! Each case is selected from the original manifest; a native test cannot reset an
//! observed failure by catching an error, retrying or reporting its own PASS.
mod assertion;
mod decoder;
mod decoder_batch;
mod lifecycle;
mod model;
mod operation;
mod scenario;
use crate::application_plan::*;
use dctx0_core::*;
pub use model::*;
use std::collections::{BTreeMap, BTreeSet};

pub const APPLICATION_ASSESSMENT_VERSION: &str = "dctx0-application-assessment-2";

pub struct ApplicationSession<'a> {
    ir: &'a WorkspaceIr,
    boundary: ApplicationValueBoundary,
    plan: &'a ApplicationTestPlan,
    group: &'a ExecutionGroup,
    registrations: BTreeMap<String, &'a TestRegistration>,
    registered: BTreeSet<String>,
    closed_tests: BTreeSet<String>,
    case_cursor: BTreeMap<String, String>,
    active: BTreeMap<u64, ActiveCase<'a>>,
    completed: BTreeMap<String, VerificationCaseRecord>,
    faults: Vec<VerificationIssue>,
    case_seconds: u64,
    next_context: u64,
    finished: bool,
    lifecycle_inputs: BTreeMap<String, crate::LifecycleCaseInput>,
}

struct ActiveCase<'a> {
    test: String,
    record: VerificationCaseRecord,
    decoder: Option<decoder::DecoderCase>,
    assertion: Option<assertion::AssertionCase>,
    operation: Option<operation::OperationCase>,
    scenario: Option<scenario::ScenarioCase<'a>>,
    lifecycle: Option<lifecycle::LifecycleCase>,
    verdict: Option<VerificationResult>,
    batched: bool,
}

pub(super) fn issue(code: &str, message: impl Into<String>) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: None,
    }
}
pub(super) fn unknown(code: &str, message: impl Into<String>) -> VerificationResult {
    VerificationResult::Unknown {
        issue: issue(code, message),
    }
}
pub(super) fn failed(code: &str, message: impl Into<String>) -> VerificationResult {
    VerificationResult::Fail {
        issue: issue(code, message),
    }
}

impl<'a> ApplicationSession<'a> {
    pub fn new(
        ir: &'a WorkspaceIr,
        plan: &'a ApplicationTestPlan,
        group: &str,
    ) -> Result<Self, Vec<VerificationIssue>> {
        Self::new_with_boundary(ir, plan, group, ApplicationValueBoundary::JsonDecoder)
    }

    pub fn new_with_boundary(
        ir: &'a WorkspaceIr,
        plan: &'a ApplicationTestPlan,
        group: &str,
        boundary: ApplicationValueBoundary,
    ) -> Result<Self, Vec<VerificationIssue>> {
        plan.validate()?;
        let group = plan
            .configuration
            .groups
            .iter()
            .find(|g| g.id == group && g.backend == TestBackend::ApplicationTests)
            .ok_or_else(|| vec![issue("APPLICATION_GROUP_REQUIRED", group)])?;
        // No opaque labels or client metadata can introduce an operation/type.
        if plan
            .program
            .closure
            .iter()
            .any(|id| !ir.symbols.contains_key(id) && !ir.nodes.iter().any(|node| &node.id == id))
            || plan.program.contracts.iter().any(|contract| {
                !ir.data.contains_key(&contract.subject)
                    || contract.ty.symbol_id.as_ref() != Some(&contract.subject)
            })
            || plan.program.operations.iter().any(|operation| {
                ir.operations.get(&operation.operation.id) != Some(&operation.operation)
            })
        {
            return Err(vec![issue(
                "APPLICATION_IR_SCOPE",
                "Plan closure does not belong to this IR",
            )]);
        }
        Ok(Self {
            ir,
            boundary,
            plan,
            group,
            registrations: plan
                .registrations
                .iter()
                .filter(|r| r.group == group.id)
                .map(|r| (r.test.clone(), r))
                .collect(),
            registered: BTreeSet::new(),
            closed_tests: BTreeSet::new(),
            case_cursor: BTreeMap::new(),
            active: BTreeMap::new(),
            completed: BTreeMap::new(),
            faults: vec![],
            case_seconds: 5,
            next_context: 1,
            finished: false,
            lifecycle_inputs: BTreeMap::new(),
        })
    }
    pub fn with_case_seconds(mut self, seconds: u64) -> Self {
        self.case_seconds = seconds;
        self
    }

    pub fn with_lifecycle_inputs(
        mut self,
        inputs: BTreeMap<String, crate::LifecycleCaseInput>,
    ) -> Self {
        self.lifecycle_inputs = inputs;
        self
    }

    /// Seed centrally validated historical PASS cases. Registrations still have
    /// to run and close, including any native assertions in their test bodies.
    pub fn with_reused_cases(
        mut self,
        cases: Vec<VerificationCaseRecord>,
    ) -> Result<Self, Vec<VerificationIssue>> {
        let allowed: BTreeSet<_> = self
            .registrations
            .values()
            .flat_map(|registration| registration.cases.iter().cloned())
            .collect();
        for case in cases {
            if case.result != VerificationResult::Pass
                || !allowed.contains(&case.id)
                || self.completed.contains_key(&case.id)
            {
                return Err(vec![issue("APPLICATION_REUSED_CASE_INVALID", case.id)]);
            }
            self.completed.insert(case.id.clone(), case);
        }
        Ok(self)
    }

    fn fault(&mut self, code: &str, message: impl Into<String>) -> ApplicationReply {
        let issue = issue(code, message);
        // Bound protocol faults independently of the number of planned cases.
        if self.faults.len() < 1024 {
            self.faults.push(issue.clone());
        }
        ApplicationReply::Rejected { issue }
    }

    /// Engine transport failures are retained even after a producer sent Finish.
    pub fn record_transport_fault(&mut self, message: impl Into<String>) {
        self.fault("APPLICATION_TRANSPORT", message);
    }

    pub fn handle(&mut self, request: ApplicationRequest) -> ApplicationReply {
        if self.finished {
            return self.fault(
                "APPLICATION_SESSION_CLOSED",
                "Assessment session is finalized",
            );
        }
        match request {
            ApplicationRequest::Collaborator {
                context,
                invocation,
            } => {
                let observation = self
                    .active
                    .get_mut(&context)
                    .filter(|active| active.verdict.is_none())
                    .and_then(|active| {
                        if let Some(scenario) = &mut active.scenario {
                            scenario.collaborator(&invocation)
                        } else {
                            active
                                .operation
                                .as_mut()
                                .and_then(|operation| operation.collaborator(&invocation))
                        }
                    });
                match observation {
                    Some(observation) => ApplicationReply::CollaboratorResult {
                        context,
                        observation,
                    },
                    None => self.fault("APPLICATION_COLLABORATOR_CONTEXT", context.to_string()),
                }
            }
            ApplicationRequest::Register { test } => self.register(test),
            ApplicationRequest::RegisterDecoder { test } => {
                if !self
                    .registrations
                    .get(&test)
                    .is_some_and(|r| r.form == TestForm::Decoder)
                {
                    return self.fault("APPLICATION_DECODER_REGISTRATION", test);
                }
                let batch = self.batch_allowed(&test);
                match self.register(test) {
                    ApplicationReply::Registered => ApplicationReply::DecoderRegistered {
                        batch,
                        max_message_bytes: self.group.resources.max_message_bytes as usize,
                        case_timeout_seconds: self.case_seconds,
                    },
                    rejected => rejected,
                }
            }
            ApplicationRequest::NextDecoderBatch {
                test,
                max_message_bytes,
            } => self.next_decoder_batch(&test, max_message_bytes),
            ApplicationRequest::CompleteDecoderBatch { test, cases } => {
                self.complete_decoder_batch(&test, cases)
            }
            ApplicationRequest::Next { test } => self.next(&test),
            ApplicationRequest::Observe {
                context,
                observation,
            } if self.single_decoder_request_allowed(context) => self.observe(context, observation),
            ApplicationRequest::CloseCase {
                context,
                cleanup_error,
            } if self.single_decoder_request_allowed(context) => {
                self.close_case(context, cleanup_error)
            }
            ApplicationRequest::Observe { context, .. }
            | ApplicationRequest::CloseCase { context, .. } => {
                self.fault("APPLICATION_DECODER_BATCH_CONTEXT", context.to_string())
            }
            ApplicationRequest::CloseTest { test } => {
                if !self.registered.contains(&test)
                    || self.active.values().any(|c| c.test == test)
                    || self.registrations[&test]
                        .cases
                        .iter()
                        .any(|id| !self.completed.contains_key(id))
                    || !self.closed_tests.insert(test.clone())
                {
                    return self.fault("APPLICATION_TEST_INCOMPLETE", test);
                }
                ApplicationReply::TestClosed
            }
            ApplicationRequest::Finish => {
                self.finished = true;
                ApplicationReply::Finished {
                    summary: self.summary(),
                }
            }
        }
    }

    fn register(&mut self, test: String) -> ApplicationReply {
        if !self.registrations.contains_key(&test) || !self.registered.insert(test.clone()) {
            return self.fault("APPLICATION_REGISTRATION", test);
        }
        ApplicationReply::Registered
    }

    fn can_start_case(&self, test: &str) -> bool {
        self.registered.contains(test)
            && !self.closed_tests.contains(test)
            && !self.active.values().any(|case| case.test == test)
    }

    fn next(&mut self, test: &str) -> ApplicationReply {
        if !self.can_start_case(test) {
            return self.fault("APPLICATION_TEST_ORDER", test);
        }
        self.issue_case(test)
    }

    fn issue_case(&mut self, test: &str) -> ApplicationReply {
        let registration = self.registrations[test];
        use std::ops::Bound::{Excluded, Unbounded};
        let cases = &registration.cases;
        let lower = self
            .case_cursor
            .get(test)
            .map_or(Unbounded, |id| Excluded(id.as_str()));
        let Some(id) = cases
            .range::<str, _>((lower, Unbounded))
            .find(|id| !self.completed.contains_key(*id))
            .cloned()
        else {
            return ApplicationReply::NoMoreCases;
        };
        self.case_cursor.insert(test.into(), id.clone());
        let context = self.next_context;
        self.next_context += 1;
        let kind = match registration.form {
            TestForm::Decoder => VerificationCaseKind::Contract,
            TestForm::Operation | TestForm::StateChange => VerificationCaseKind::Operation,
            TestForm::Scenario => VerificationCaseKind::Example,
            TestForm::Lifecycle => VerificationCaseKind::Lifecycle,
            TestForm::Assertion => VerificationCaseKind::Assertion,
            TestForm::EntityIdentity => VerificationCaseKind::EntityIdentity,
        };
        let subject = self.plan.manifest.case_requirements[&id]
            .iter()
            .find(|subject| self.plan.program.closure.contains(*subject))
            .cloned()
            .unwrap_or_else(|| id.strip_suffix(":identity").unwrap_or(&id).to_string());
        let mut record = VerificationCaseRecord {
            id: id.clone(),
            subject,
            kind,
            bindings: BTreeMap::new(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: unknown("APPLICATION_CASE_INCOMPLETE", &id),
            elapsed_us: 0,
        };
        let form = registration.form;
        let decoder = (form == TestForm::Decoder)
            .then(|| decoder::case(&self.plan.program, &id))
            .flatten();
        let mut operation = matches!(form, TestForm::Operation | TestForm::StateChange)
            .then(|| operation::OperationCase::find(&self.plan.program, &id))
            .flatten();
        let mut assertion = (form == TestForm::Assertion)
            .then(|| assertion::AssertionCase::find(&self.plan.program, &id))
            .flatten();
        let mut scenario = (form == TestForm::Scenario)
            .then(|| scenario::ScenarioCase::find(self.ir, &self.plan.program, &id))
            .flatten();
        let mut lifecycle = (form == TestForm::Lifecycle)
            .then(|| {
                lifecycle::LifecycleCase::find(&self.plan.program, &self.lifecycle_inputs, &id)
            })
            .flatten();
        let (directive, verdict) = if let Some(case) = &decoder {
            record.subject = case.subject.clone();
            record.bindings.insert("value".into(), case.input.clone());
            record.bindings.insert(
                "expect_rejection".into(),
                VerificationValue::Boolean(case.rejection),
            );
            if self.boundary == ApplicationValueBoundary::ValueConstructor {
                (
                    ApplicationDirective::Construct {
                        subject: case.subject.clone(),
                        ty: case.ty.clone(),
                        value: case.input.clone(),
                    },
                    None,
                )
            } else {
                match decoder::raw_json(&case.input) {
                    Ok(raw) => (
                        ApplicationDirective::Decode {
                            subject: case.subject.clone(),
                            ty: case.ty.clone(),
                            raw,
                        },
                        None,
                    ),
                    Err(issue) => {
                        let result = VerificationResult::Unknown { issue };
                        (
                            ApplicationDirective::Verdict {
                                result: result.clone(),
                            },
                            Some(result),
                        )
                    }
                }
            }
        } else if let Some(case) = &mut lifecycle {
            let directive = case.start(self.ir, &self.plan.program, &mut record);
            let verdict = match &directive {
                ApplicationDirective::Verdict { result } => Some(result.clone()),
                _ => None,
            };
            (directive, verdict)
        } else if let Some(case) = &mut scenario {
            record.subject = id.clone();
            let directive = case.advance(&mut record);
            let verdict = match &directive {
                ApplicationDirective::Verdict { result } => Some(result.clone()),
                _ => None,
            };
            (directive, verdict)
        } else if let Some(case) = &mut assertion {
            let directive = case.advance(self.ir, &self.plan.program, &mut record);
            let verdict = match &directive {
                ApplicationDirective::Verdict { result } => Some(result.clone()),
                _ => None,
            };
            (directive, verdict)
        } else if let Some(case) = &mut operation {
            let directive = case.start(self.ir, &self.plan.program.budgets, &mut record);
            let verdict = match &directive {
                ApplicationDirective::Verdict { result } => Some(result.clone()),
                _ => None,
            };
            (directive, verdict)
        } else if registration.form == TestForm::EntityIdentity {
            let subject = id.strip_suffix(":identity").unwrap_or(&id);
            record = crate::entity_identity_case(
                self.ir,
                subject,
                &self.completed.values().cloned().collect::<Vec<_>>(),
                &self.plan.program.budgets,
            );
            let result = record.result.clone();
            (
                ApplicationDirective::Verdict {
                    result: result.clone(),
                },
                Some(result),
            )
        } else {
            let result = unknown("APPLICATION_CASE_CAPABILITY", &id);
            (
                ApplicationDirective::Verdict {
                    result: result.clone(),
                },
                Some(result),
            )
        };
        let mocks = scenario
            .as_ref()
            .map(|case| case.mocks())
            .or_else(|| operation.as_ref().map(|case| case.mock_subjects()))
            .unwrap_or_default();
        self.active.insert(
            context,
            ActiveCase {
                test: test.into(),
                record,
                decoder,
                assertion,
                operation,
                scenario,
                lifecycle,
                verdict,
                batched: false,
            },
        );
        ApplicationReply::Case {
            context,
            id,
            form: registration.form,
            directive,
            mocks,
        }
    }

    fn observe(&mut self, context: u64, observation: ApplicationObservation) -> ApplicationReply {
        let Some(mut active) = self.active.remove(&context) else {
            return self.fault("APPLICATION_CASE_CONTEXT", context.to_string());
        };
        if active.verdict.is_some() {
            self.active.insert(context, active);
            return self.fault("APPLICATION_CASE_ALREADY_ASSESSED", context.to_string());
        }
        if let Some(lifecycle) = &mut active.lifecycle {
            let directive =
                lifecycle.observe(self.ir, &self.plan.program, observation, &mut active.record);
            if let ApplicationDirective::Verdict { result } = &directive {
                active.verdict = Some(result.clone());
            }
            self.active.insert(context, active);
            return ApplicationReply::Action {
                context,
                directive,
                mocks: vec![],
            };
        }
        if let Some(scenario) = &mut active.scenario {
            let directive = scenario.observe(observation, &mut active.record);
            let mocks = scenario.mocks();
            if let ApplicationDirective::Verdict { result } = &directive {
                active.verdict = Some(result.clone());
            }
            self.active.insert(context, active);
            return ApplicationReply::Action {
                context,
                directive,
                mocks,
            };
        }
        if let Some(assertion) = &mut active.assertion {
            let directive =
                assertion.observe(self.ir, &self.plan.program, observation, &mut active.record);
            if let ApplicationDirective::Verdict { result } = &directive {
                active.verdict = Some(result.clone());
            }
            self.active.insert(context, active);
            return ApplicationReply::Action {
                context,
                directive,
                mocks: vec![],
            };
        }
        if let Some(operation) = &mut active.operation {
            let mocks = operation.mock_subjects();
            let directive = operation.observe(
                self.ir,
                &self.plan.program.budgets,
                observation,
                &mut active.record,
            );
            if let ApplicationDirective::Verdict { result } = &directive {
                active.verdict = Some(result.clone());
            }
            self.active.insert(context, active);
            return ApplicationReply::Action {
                context,
                directive,
                mocks,
            };
        }
        let result = match (&active.decoder, observation) {
            (Some(case), ApplicationObservation::Decoded { raw, outcome })
                if self.boundary == ApplicationValueBoundary::JsonDecoder =>
            {
                decoder::assess(
                    self.ir,
                    &self.plan.program,
                    case,
                    &raw,
                    outcome,
                    &mut active.record,
                )
            }
            (Some(case), ApplicationObservation::Constructed { input, outcome })
                if self.boundary == ApplicationValueBoundary::ValueConstructor =>
            {
                if input != case.input {
                    failed(
                        "APPLICATION_INPUT_CHANGED",
                        "Production constructor input differs from its centrally selected case",
                    )
                } else {
                    decoder::assess_outcome(
                        self.ir,
                        &self.plan.program,
                        case,
                        outcome,
                        &mut active.record,
                    )
                }
            }
            (_, ApplicationObservation::BindingError { message }) => {
                unknown("APPLICATION_BINDING_ERROR", message)
            }
            _ => failed(
                "APPLICATION_OBSERVATION_ORDER",
                "Observation does not match the pending directive",
            ),
        };
        active.verdict = Some(result.clone());
        self.active.insert(context, active);
        ApplicationReply::Action {
            context,
            mocks: vec![],
            directive: ApplicationDirective::Verdict { result },
        }
    }

    fn close_case(&mut self, context: u64, cleanup_error: Option<String>) -> ApplicationReply {
        let Some(mut active) = self.active.remove(&context) else {
            return self.fault("APPLICATION_CASE_CONTEXT", context.to_string());
        };
        let mut result = active
            .verdict
            .unwrap_or_else(|| unknown("APPLICATION_CASE_INCOMPLETE", &active.record.id));
        if let Some(error) = cleanup_error {
            self.faults
                .push(issue("APPLICATION_CLEANUP_FAILED", &error));
            if !matches!(result, VerificationResult::Fail { .. }) {
                result = unknown("APPLICATION_CLEANUP_FAILED", error);
            }
        }
        active.record.result = result.clone();
        self.completed
            .insert(active.record.id.clone(), active.record);
        ApplicationReply::CaseClosed { context, result }
    }

    /// A preview never finalizes execution. Native runner collection/source guards
    /// must independently succeed before an engine may accept this assessment.
    pub fn report(&self) -> ApplicationAssessmentReport {
        let required = self
            .plan
            .case_groups
            .iter()
            .filter(|(_, group)| *group == &self.group.id)
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        let missing_cases = required
            .iter()
            .filter(|id| !self.completed.contains_key(*id))
            .cloned()
            .collect::<BTreeSet<_>>();
        let missing_tests = self
            .registrations
            .keys()
            .filter(|test| !self.closed_tests.contains(*test))
            .cloned()
            .collect();
        let mut cases = self.completed.clone();
        for active in self.active.values() {
            let mut record = active.record.clone();
            record.result = active
                .verdict
                .clone()
                .unwrap_or_else(|| unknown("APPLICATION_CASE_INCOMPLETE", &record.id));
            cases.insert(record.id.clone(), record);
        }
        let mut result = cases
            .values()
            .find_map(|case| match &case.result {
                result @ VerificationResult::Fail { .. } => Some(result.clone()),
                _ => None,
            })
            .unwrap_or(VerificationResult::Pass);
        if result == VerificationResult::Pass {
            result = cases
                .values()
                .find_map(|case| match &case.result {
                    result @ VerificationResult::Unknown { .. } => Some(result.clone()),
                    _ => None,
                })
                .unwrap_or(VerificationResult::Pass);
        }
        if result == VerificationResult::Pass
            && (!self.finished
                || !missing_cases.is_empty()
                || !self.faults.is_empty()
                || self.closed_tests.len() != self.registrations.len())
        {
            result = unknown(
                "APPLICATION_GROUP_INCOMPLETE",
                "Registration, cases, cleanup or finalization are incomplete",
            );
        }
        ApplicationAssessmentReport {
            assessment_version: APPLICATION_ASSESSMENT_VERSION.into(),
            group: self.group.id.clone(),
            finalized: self.finished,
            required_cases: required,
            missing_cases,
            missing_tests,
            cases: cases.into_values().collect(),
            issues: self.faults.clone(),
            result,
        }
    }

    pub fn summary(&self) -> ApplicationAssessmentSummary {
        let report = self.report();
        let (status, issue_code) = match &report.result {
            VerificationResult::Pass => (ApplicationAssessmentStatus::Pass, None),
            VerificationResult::Fail { issue } => {
                (ApplicationAssessmentStatus::Fail, Some(issue.code.clone()))
            }
            VerificationResult::Unknown { issue } => (
                ApplicationAssessmentStatus::Unknown,
                Some(issue.code.clone()),
            ),
        };
        ApplicationAssessmentSummary {
            assessment_version: report.assessment_version,
            group: report.group,
            finalized: report.finalized,
            required_cases: report.required_cases.len(),
            completed_cases: self.completed.len(),
            missing_cases: report.missing_cases.len(),
            missing_tests: report.missing_tests.len(),
            issues: report.issues.len(),
            status,
            issue_code,
        }
    }
}
