//! Diagnostic application-test planning. See docs/application-test-implementation.md.
//! This export grants no execution or adoption authority. In particular, backend
//! selection never generates, removes or renumbers a mandatory case.
use crate::{CaseManifest, ProgramPlan, case_manifest};
use dctx0_core::VerificationIssue;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const APPLICATION_TEST_PLAN_VERSION: u32 = 1;
pub const APPLICATION_TEST_CONFIGURATION_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestBackend {
    Adapter,
    ApplicationTests,
}

/// Exact case IDs and/or exact requirement IDs, selected from the central manifest.
/// A requirement selects whole cases; it cannot select a scenario prefix.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestArea {
    #[serde(default)]
    pub cases: BTreeSet<String>,
    #[serde(default)]
    pub requirements: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupResources {
    /// Groups have distinct fixture namespaces. Actual resource isolation still
    /// requires collector evidence; a name alone is not an isolation proof.
    pub namespace: String,
    pub timeout_seconds: u64,
    pub max_message_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionGroup {
    pub id: String,
    pub backend: TestBackend,
    pub target: String,
    /// Native test identities for application tests, adapter identity otherwise.
    /// These are identities, never shell commands.
    pub selection: BTreeSet<String>,
    pub area: TestArea,
    pub resources: GroupResources,
}

/// Separate from the historical schema-1 adapter configuration. No implicit
/// upgrade or fallback is permitted, and resource limits remain per group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationTestConfiguration {
    pub schema_version: u32,
    pub groups: Vec<ExecutionGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestForm {
    Decoder,
    Operation,
    StateChange,
    Scenario,
    Lifecycle,
    Assertion,
    EntityIdentity,
}

/// Expected catalog entry. Native enumeration and finalization must later
/// establish actual execution; registration is not a successful test result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestRegistration {
    pub group: String,
    pub test: String,
    pub form: TestForm,
    pub cases: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationTestPlan {
    pub schema_version: u32,
    /// Retain the original plan verbatim, including generated preparations,
    /// argument bindings, budgets and all issues. Do not flatten setup inputs:
    /// placeholders become actual inputs only after native setup results arrive.
    pub program: ProgramPlan,
    pub manifest: CaseManifest,
    pub configuration: ApplicationTestConfiguration,
    pub case_groups: BTreeMap<String, String>,
    pub case_forms: BTreeMap<String, TestForm>,
    pub registrations: Vec<TestRegistration>,
}

fn issue(code: &str, message: impl Into<String>) -> VerificationIssue {
    VerificationIssue {
        code: code.into(),
        message: message.into(),
        span: None,
    }
}

fn name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 256
        && text
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

impl ApplicationTestConfiguration {
    pub fn validate(&self) -> Vec<VerificationIssue> {
        let mut issues = vec![];
        if self.schema_version != APPLICATION_TEST_CONFIGURATION_VERSION {
            issues.push(issue(
                "APPLICATION_CONFIGURATION_VERSION",
                "Expected schema 2",
            ));
        }
        if self.groups.is_empty() {
            issues.push(issue(
                "EMPTY_EXECUTION_GROUPS",
                "At least one group is required",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut namespaces = BTreeSet::new();
        for group in &self.groups {
            if !name(&group.id) || !ids.insert(&group.id) {
                issues.push(issue("INVALID_EXECUTION_GROUP", &group.id));
            }
            if !matches!(group.target.as_str(), "go" | "typescript" | "rust" | "java")
                || group.selection.is_empty()
                || group
                    .selection
                    .iter()
                    .any(|s| s.trim().is_empty() || s.chars().any(char::is_control))
                || (group.backend == TestBackend::Adapter && group.selection.len() != 1)
            {
                issues.push(issue("INVALID_GROUP_SELECTION", &group.id));
            }
            if !name(&group.resources.namespace)
                || !namespaces.insert(&group.resources.namespace)
                || !(1..=86_400).contains(&group.resources.timeout_seconds)
                || !(1..=67_108_864).contains(&group.resources.max_message_bytes)
            {
                issues.push(issue("INVALID_GROUP_RESOURCES", &group.id));
            }
            if group.area.cases.is_empty() && group.area.requirements.is_empty() {
                issues.push(issue("EMPTY_TEST_AREA", &group.id));
            }
        }
        issues
    }
}

/// Resolve complete ownership before any process starts. Open obligations stay
/// in the exported manifest: complete assignment does not resolve them.
pub fn application_test_plan(
    program: &ProgramPlan,
    configuration: &ApplicationTestConfiguration,
    registrations: &[TestRegistration],
) -> Result<ApplicationTestPlan, Vec<VerificationIssue>> {
    let mut issues = configuration.validate();
    if program.schema_version != 1 {
        issues.push(issue(
            "APPLICATION_PROGRAM_VERSION",
            "Expected ProgramPlan schema 1",
        ));
    }
    if program.snapshot.is_empty() || program.check_occurrence == 0 {
        issues.push(issue(
            "APPLICATION_PLAN_UNPINNED",
            "Snapshot and check occurrence are required",
        ));
    }
    let manifest = case_manifest(program);
    // A set cannot give two different mandatory occurrences the same identity.
    // Keep unsupported/manual obligations inspectable, but reject structurally
    // invalid case inventories before any backend selects an input.
    issues.extend(
        manifest
            .open_obligations
            .iter()
            .filter(|issue| issue.code == "DUPLICATE_CASE_ID")
            .cloned(),
    );
    issues.extend(
        program
            .issues
            .iter()
            .filter(|issue| issue.code == "INVALID_OPERATION_CASE")
            .cloned(),
    );
    let requirements = manifest
        .case_requirements
        .values()
        .flatten()
        .collect::<BTreeSet<_>>();
    let mut owners = BTreeMap::new();
    for group in &configuration.groups {
        for requirement in &group.area.requirements {
            if !requirements.contains(requirement) {
                issues.push(issue(
                    "UNKNOWN_TEST_REQUIREMENT",
                    format!("{}: {requirement}", group.id),
                ));
            }
        }
        let mut cases = group.area.cases.clone();
        for (id, requirements) in &manifest.case_requirements {
            if !requirements.is_disjoint(&group.area.requirements) {
                cases.insert(id.clone());
            }
        }
        for id in cases {
            if !manifest.required_cases.contains(&id) {
                issues.push(issue("UNKNOWN_TEST_CASE", format!("{}: {id}", group.id)));
            } else if let Some(previous) = owners.insert(id.clone(), group.id.clone()) {
                issues.push(issue(
                    "CONFLICTING_CASE_GROUPS",
                    format!("{id}: {previous}, {}", group.id),
                ));
            }
        }
    }
    for id in &manifest.required_cases {
        if !owners.contains_key(id) {
            issues.push(issue("UNASSIGNED_TEST_CASE", id));
        }
    }
    // Paths of a connected lifecycle share a session/fixture contract and must
    // not be divided between execution groups, even when backends are equal.
    for lifecycle in &program.lifecycles {
        let groups = lifecycle
            .paths
            .iter()
            .filter_map(|p| owners.get(&p.id))
            .collect::<BTreeSet<_>>();
        if groups.len() > 1 {
            issues.push(issue("SPLIT_LIFECYCLE_GROUP", &lifecycle.subject));
        }
    }
    let forms = case_forms(program, &manifest, &mut issues);
    validate_registrations(configuration, registrations, &owners, &forms, &mut issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    Ok(ApplicationTestPlan {
        schema_version: APPLICATION_TEST_PLAN_VERSION,
        program: program.clone(),
        manifest,
        configuration: configuration.clone(),
        case_groups: owners,
        case_forms: forms,
        registrations: registrations.to_vec(),
    })
}

// Classify only IDs already selected by case_manifest. This is not another
// generator: actual inputs, ordering, setup and counts stay in ProgramPlan.
fn case_forms(
    program: &ProgramPlan,
    manifest: &CaseManifest,
    issues: &mut Vec<VerificationIssue>,
) -> BTreeMap<String, TestForm> {
    use dctx0_core::StatementKind;
    let mut result = BTreeMap::new();
    for id in &manifest.required_cases {
        let mut forms = BTreeSet::new();
        for contract in &program.contracts {
            if id == &format!("{}:identity", contract.subject) {
                forms.insert(TestForm::EntityIdentity);
            }
            if id.starts_with(&format!("{}:value:", contract.subject))
                || id.starts_with(&format!("{}:rejection:", contract.subject))
            {
                forms.insert(TestForm::Decoder);
            }
        }
        for operation in &program.operations {
            if operation.cases.iter().any(|case| &case.id == id)
                || id == &format!("{}:operation", operation.operation.id)
                || id.starts_with(&format!("{}:operation:", operation.operation.id))
            {
                forms.insert(
                    if operation.clauses.iter().any(|clause| {
                        matches!(
                            clause.kind,
                            StatementKind::State | StatementKind::Read | StatementKind::Write
                        )
                    }) {
                        TestForm::StateChange
                    } else {
                        TestForm::Operation
                    },
                );
            }
        }
        if program
            .properties
            .iter()
            .flat_map(|property| &property.assertions)
            .flat_map(|assertion| &assertion.cases)
            .any(|case| &case.id == id)
        {
            forms.insert(TestForm::Assertion);
        }
        if program
            .examples
            .iter()
            .any(|example| &example.subject == id)
        {
            forms.insert(TestForm::Scenario);
        }
        if program
            .lifecycles
            .iter()
            .flat_map(|lifecycle| &lifecycle.paths)
            .any(|path| &path.id == id)
        {
            forms.insert(TestForm::Lifecycle);
        }
        if forms.len() == 1 {
            if let Some(form) = forms.into_iter().next() {
                result.insert(id.clone(), form);
            }
        } else {
            issues.push(issue("AMBIGUOUS_TEST_FORM", id));
        }
    }
    result
}

fn validate_registrations(
    configuration: &ApplicationTestConfiguration,
    registrations: &[TestRegistration],
    owners: &BTreeMap<String, String>,
    forms: &BTreeMap<String, TestForm>,
    issues: &mut Vec<VerificationIssue>,
) {
    let mut tests = BTreeSet::new();
    let mut registered = BTreeSet::new();
    for registration in registrations {
        let group = configuration
            .groups
            .iter()
            .find(|g| g.id == registration.group);
        if !group.is_some_and(|g| {
            g.backend == TestBackend::ApplicationTests && g.selection.contains(&registration.test)
        }) || !tests.insert((&registration.group, &registration.test))
            || registration.cases.is_empty()
        {
            issues.push(issue(
                "INVALID_TEST_REGISTRATION",
                format!("{}: {}", registration.group, registration.test),
            ));
        }
        for id in &registration.cases {
            if forms.get(id) != Some(&registration.form) {
                issues.push(issue("REGISTRATION_TEST_FORM", id));
            }
            if owners.get(id) != Some(&registration.group) {
                issues.push(issue("REGISTRATION_CASE_GROUP", id));
            }
            if !registered.insert(id) {
                issues.push(issue("DUPLICATE_CASE_REGISTRATION", id));
            }
        }
    }
    for group in &configuration.groups {
        if group.backend != TestBackend::ApplicationTests {
            continue;
        }
        for test in &group.selection {
            if !tests.contains(&(&group.id, test)) {
                issues.push(issue(
                    "MISSING_TEST_REGISTRATION",
                    format!("{}: {test}", group.id),
                ));
            }
        }
        for (id, owner) in owners {
            if owner == &group.id && !registered.contains(id) {
                issues.push(issue("UNREGISTERED_TEST_CASE", id));
            }
        }
    }
}

impl ApplicationTestPlan {
    /// Recompute derived fields after reading an export. Caller-supplied case
    /// assignments and manifests cannot override the pinned program.
    pub fn validate(&self) -> Result<(), Vec<VerificationIssue>> {
        if self.schema_version != APPLICATION_TEST_PLAN_VERSION {
            return Err(vec![issue("APPLICATION_PLAN_VERSION", "Expected schema 1")]);
        }
        let rebuilt =
            application_test_plan(&self.program, &self.configuration, &self.registrations)?;
        if self.manifest != rebuilt.manifest
            || self.case_groups != rebuilt.case_groups
            || self.case_forms != rebuilt.case_forms
        {
            return Err(vec![issue(
                "APPLICATION_PLAN_DERIVATION",
                "Manifest or ownership differs from the program",
            )]);
        }
        Ok(())
    }
}
