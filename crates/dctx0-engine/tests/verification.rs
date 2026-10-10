use dctx0_engine::{Project, VerificationSelection};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-verification-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn project(&self) -> Project {
        Project::new(&self.0, None).unwrap()
    }
    fn write(&self, spec: &str) {
        fs::write(self.0.join("spec.dctx0"), spec).unwrap();
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn selected() -> VerificationSelection {
    VerificationSelection {
        subjects: BTreeSet::from(["W.A".into()]),
        ..Default::default()
    }
}
#[test]
fn planning_pins_published_occurrences_rejects_stale_sources_and_never_writes_history() {
    let temp = Temp::new();
    temp.write("workspace W { value A = Int { range 1..2 } }");
    let p = temp.project();
    assert!(
        p.verification_plan(Default::default(), &Default::default(), Default::default())
            .is_err()
    );
    assert!(!p.database.exists());
    assert!(
        p.verification_plan(selected(), &Default::default(), Default::default())
            .is_err()
    );
    p.check().unwrap();
    let before = fs::read(&p.database).unwrap();
    let history = p.history().unwrap();
    let plan = p
        .verification_plan(selected(), &Default::default(), Default::default())
        .unwrap();
    assert_eq!(plan.check_occurrence, history[0].run_id as u64);
    assert_eq!(plan.contracts.len(), 1);
    assert_eq!(fs::read(&p.database).unwrap(), before);
    assert_eq!(p.history().unwrap().len(), 1);
    temp.write("workspace W { value A = Int { range 1..3 } }");
    assert!(
        p.verification_plan(selected(), &Default::default(), Default::default())
            .unwrap_err()
            .to_string()
            .contains("run check first")
    );
    let mut historical = selected();
    historical.historical_check = Some(history[0].run_id);
    let old = p
        .verification_plan(historical.clone(), &Default::default(), Default::default())
        .unwrap();
    assert_eq!(old.contracts[0].corpus.values.len(), 2);
    historical.historical_check = Some(9999);
    assert!(
        p.verification_plan(historical, &Default::default(), Default::default())
            .is_err()
    );
}
#[test]
fn task_selection_is_bound_to_the_exact_plan_and_check_occurrence() {
    let temp = Temp::new();
    temp.write("workspace W { value A = Int { range 1..2 } }");
    let p = temp.project();
    p.check().unwrap();
    let tasks = p.task_plan("latest").unwrap();
    let id = tasks.members[0].task_id;
    let selection = VerificationSelection {
        tasks: BTreeSet::from([id]),
        ..Default::default()
    };
    let plan = p
        .verification_plan(selection.clone(), &Default::default(), Default::default())
        .unwrap();
    assert_eq!(plan.scope.tasks, BTreeSet::from([format!("T{id}")]));
    assert!(plan.scope.subjects.contains("W.A"));
    let mut missing = selection.clone();
    missing.tasks = BTreeSet::from([9999]);
    assert!(
        p.verification_plan(missing, &Default::default(), Default::default())
            .is_err()
    );
    temp.write("workspace W { value A = Int { range 1..3 } }");
    p.check().unwrap();
    assert!(
        p.verification_plan(selection, &Default::default(), Default::default())
            .unwrap_err()
            .to_string()
            .contains("different check occurrence")
    );
}
#[test]
fn corrupted_occurrences_do_not_become_pinned_verification_requirements() {
    let temp = Temp::new();
    temp.write("workspace W { type A = Int }");
    let p = temp.project();
    p.check().unwrap();
    let connection = rusqlite::Connection::open(&p.database).unwrap();
    connection.execute("INSERT INTO checks(id,project,snapshot_id,status,source_hash,diagnostics) VALUES(-1,?1,NULL,'pass','corrupt','[]')",[p.key()]).unwrap();
    let mut selection = selected();
    selection.historical_check = Some(-1);
    assert!(
        p.verification_plan(selection.clone(), &Default::default(), Default::default())
            .unwrap_err()
            .to_string()
            .contains("no snapshot")
    );
    connection.execute("UPDATE checks SET snapshot_id=(SELECT latest FROM projects WHERE project=?1) WHERE id=-1",[p.key()]).unwrap();
    assert!(
        p.verification_plan(selection, &Default::default(), Default::default())
            .unwrap_err()
            .to_string()
            .contains("positive")
    );
}
