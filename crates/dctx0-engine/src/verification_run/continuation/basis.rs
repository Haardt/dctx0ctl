//! Bounded immutable continuation ancestry. Newer observations win, including
//! FAIL and UNKNOWN; a later interruption never restores an older green row.
use super::*;

pub(super) struct Basis {
    pub root: StoredVerificationRun,
    pub program: ProgramPlan,
    pub cases: Vec<VerificationCaseRecord>,
    pub observations: BTreeMap<String, String>,
    pub additional_sources: BTreeSet<String>,
}

pub(super) fn load(project: &Project, store: &dctx0_store::Store, id: &str) -> Result<Basis> {
    let mut cursor = id.to_owned();
    let mut visited = BTreeSet::new();
    let mut records = BTreeMap::new();
    let mut observations = BTreeMap::new();
    let mut shadowed = BTreeSet::new();
    let mut additional_sources = BTreeSet::new();
    let mut plan_hash: Option<String> = None;
    let mut inventory_hash: Option<String> = None;
    for _ in 0..64 {
        if !visited.insert(cursor.clone()) {
            return Err(Error::Invalid("Continuation ancestry cycle".into()));
        }
        let mut saved = store.verification_run(&project.key(), &cursor)?;
        if saved.run.origin != VerificationRunOrigin::Native {
            return Err(Error::Invalid(
                "Continuation requires original native observations".into(),
            ));
        }
        if plan_hash
            .as_ref()
            .is_some_and(|hash| hash != &saved.run.provenance.plan_hash)
        {
            return Err(Error::Invalid(
                "Continuation ancestors have different concrete plans".into(),
            ));
        }
        plan_hash = Some(saved.run.provenance.plan_hash.clone());
        let inventories = saved
            .run
            .artifacts
            .iter()
            .filter(|entry| {
                entry.media_type == "application/vnd.dctx0.captured-input-inventory+json"
            })
            .collect::<Vec<_>>();
        let [inventory] = inventories.as_slice() else {
            return Err(Error::Invalid(
                "Continuation ancestor lacks one captured input inventory".into(),
            ));
        };
        // Retain one source identity across the chain. This is no assertion
        // that differing live environments or external states are compatible.
        if inventory_hash
            .as_ref()
            .is_some_and(|hash| hash != &inventory.hash)
        {
            return Err(Error::Invalid(
                "Continuation ancestors have different source inputs".into(),
            ));
        }
        inventory_hash = Some(inventory.hash.clone());
        let links = saved
            .run
            .artifacts
            .iter()
            .filter(|entry| entry.media_type == "application/vnd.dctx0.continuation+json")
            .collect::<Vec<_>>();
        if links.len() > 1 {
            return Err(Error::Invalid("Ambiguous continuation ancestry".into()));
        }
        let link = if let Some(reference) = links.first() {
            let artifact = store.verification_artifact(&cursor, &reference.hash)?;
            let link: VerificationContinuationPlan = serde_json::from_slice(&artifact.contents)?;
            if link.schema_version != 1
                || link.adoption_authority
                || link.current_bindings_validated
                || link.plan_hash != saved.run.provenance.plan_hash
                || link.selected_cases != saved.run.required_cases
                || link.parent_run.is_empty()
            {
                return Err(Error::Invalid(
                    "Invalid continuation ancestry binding".into(),
                ));
            }
            Some(link)
        } else {
            None
        };
        for case in std::mem::take(&mut saved.run.cases) {
            if !shadowed.contains(&case.id) {
                observations.insert(case.id.clone(), saved.id.clone());
                records.entry(case.id.clone()).or_insert(case);
            }
        }
        shadowed.extend(saved.run.required_cases.iter().cloned());
        if let Some(link) = link {
            additional_sources.extend(link.additional_sources);
            cursor = link.parent_run;
            continue;
        }
        let artifact = store.verification_artifact(&cursor, &saved.run.provenance.plan_hash)?;
        let program: ProgramPlan = serde_json::from_slice(&artifact.contents)?;
        let manifest = dctx0_verification::case_manifest(&program);
        if saved.run.required_cases != manifest.required_cases
            || program.snapshot != saved.run.provenance.snapshot
            || program.check_occurrence as i64 != saved.run.provenance.check_occurrence
            || program.budgets != saved.run.budgets
            || program.scope.subjects != saved.run.selected_subjects
            || program.closure != saved.run.required_subjects
            || records
                .keys()
                .any(|id| !manifest.required_cases.contains(id))
        {
            return Err(Error::Invalid(
                "Continuation needs an intact complete root plan".into(),
            ));
        }
        return Ok(Basis {
            root: saved,
            program,
            cases: records.into_values().collect(),
            observations,
            additional_sources,
        });
    }
    Err(Error::Invalid(
        "Continuation ancestry exceeds 64 validated links".into(),
    ))
}
