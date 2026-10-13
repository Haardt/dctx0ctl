//! Isolated real project/store transitions. Observations come from persisted state.
use super::*;
use dctx0_engine::{AccessMode, create_profiles};
use dctx0_store::HistoryReader;
impl Actual {
    fn task_state(&self, id: i64) -> Result<Value, String> {
        let session = self
            .project()?
            .task_show_session(id, None, None)
            .map_err(error)?;
        let state = session.states.get(&id).ok_or("Task state missing")?;
        Ok(
            json!({"taskId":id,"work":match state.work{WorkState::Open=>"Open",WorkState::Progress=>"Progress",WorkState::Done=>"Done"},"check":match state.check{CheckStatus::Unverified=>"Unverified",CheckStatus::Pass=>"Passed",CheckStatus::Fail=>"Failed"},"version":state.version,"claim":state.claim}),
        )
    }
    fn setup_tasks(&mut self) -> Result<i64, String> {
        let project = self.project()?;
        let report = project.check().map_err(error)?;
        if !report.ok {
            return Err("Fixture compilation rejected".into());
        }
        project.task_plan("latest").map_err(error)?;
        let session = project.task_session(None, None).map_err(error)?;
        let id = session
            .next(&Filters::default(), None)
            .ok_or("Fixture has no ready task")?
            .task_id;
        self.task = Some(id);
        Ok(id)
    }
    pub(super) fn tasks(&mut self, subject: &str, args: &[Value]) -> Result<Value, String> {
        let project = self.project()?;
        let action = match subject.rsplit('.').next().ok_or("Missing operation")? {
            "setup" => {
                let id = self.setup_tasks()?;
                return self.task_state(id);
            }
            "state" => return self.task_state(int(&args[0])?),
            "start" => Action::Start {
                task: int(&args[0])?,
                actor: args[1].as_str().ok_or("Missing actor")?.into(),
                reservation: None,
            },
            "next" => Action::ClaimNext {
                actor: args[0].as_str().ok_or("Missing actor")?.into(),
                filters: Filters::default(),
            },
            "release" => Action::Release {
                task: int(&args[0])?,
                actor: args[1].as_str().ok_or("Missing actor")?.into(),
                claim: int(&args[2])?,
                version: int(&args[3])?,
                reason: "Self-verification release".into(),
            },
            "complete" => Action::Complete {
                task: int(&args[0])?,
                actor: args[1].as_str().ok_or("Missing actor")?.into(),
                claim: int(&args[2])?,
                version: int(&args[3])?,
                result: TaskResult {
                    summary: args[4].as_str().ok_or("Missing summary")?.into(),
                    artifacts: vec![],
                    decisions: vec![],
                    implementation: None,
                },
            },
            "blocked" => {
                self.setup_tasks()?;
                let session = project.task_session(None, None).map_err(error)?;
                let id = session
                    .overviews()
                    .iter()
                    .find(|row| !row.ready && !row.blockers.is_empty())
                    .ok_or("Fixture has no blocked task")?
                    .task_id;
                return self.task_state(id);
            }
            "race" => {
                self.setup_tasks()?;
                let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
                let handles = (0..2)
                    .map(|i| {
                        let barrier = barrier.clone();
                        let project = project.clone();
                        std::thread::spawn(move || {
                            barrier.wait();
                            project.task_action(Action::ClaimNext {
                                actor: format!("worker-{i}"),
                                filters: Filters::default(),
                            })
                        })
                    })
                    .collect::<Vec<_>>();
                barrier.wait();
                let mut ids = Vec::new();
                let mut rejected = 0;
                for handle in handles {
                    match handle.join().map_err(|_| "Claim worker panicked")? {
                        Ok(receipt) => match receipt.task_id {
                            Some(id) => ids.push(id),
                            None => rejected += 1,
                        },
                        Err(_) => rejected += 1,
                    }
                }
                return Ok(
                    json!({"claims":ids.len(),"distinct":ids.iter().collect::<std::collections::BTreeSet<_>>().len(),"rejected":rejected}),
                );
            }
            _ => return Err("Unknown task operation".into()),
        };
        let receipt = project.task_action(action).map_err(error)?;
        self.task_state(receipt.task_id.ok_or("Transition did not identify task")?)
    }
    fn snapshot(&self) -> Result<Value, String> {
        let project = self.project()?;
        let store = project.store(AccessMode::ReadOnly).map_err(error)?;
        let (occurrence, id) = store.task_check(&project.key()).map_err(error)?;
        let snapshot = project.load(&id).map_err(error)?;
        Ok(json!({"id":id,"sourceHash":snapshot.source_hash,"checkOccurrence":occurrence}))
    }
    pub(super) fn history(&mut self, subject: &str, args: &[Value]) -> Result<Value, String> {
        match subject.rsplit('.').next().ok_or("Missing operation")? {
            "write" => {
                fs::write(
                    self.root.join("spec/main.dctx0"),
                    args[0].as_str().ok_or("Missing source")?,
                )
                .map_err(error)?;
                Ok(Value::Null)
            }
            "check" => {
                let actual = self.project()?.check().map_err(error)?;
                Ok(
                    json!({"accepted":actual.ok,"diagnostics":actual.diagnostics.len(),"publishedSnapshot":if actual.ok{self.snapshot()?}else{Value::Null}}),
                )
            }
            "latest" => {
                let project = self.project()?;
                if !project.database.exists() {
                    return Ok(Value::Null);
                }
                let store = project.store(AccessMode::ReadOnly).map_err(error)?;
                if store.latest(&project.key()).map_err(error)?.is_none() {
                    Ok(Value::Null)
                } else {
                    self.snapshot()
                }
            }
            "head" => self.snapshot(),
            "readerUnchanged" => {
                let project = self.project()?;
                if !project.database.exists() {
                    let history_missing = project.history().is_err();
                    let snapshot_missing = project.load("latest").is_err();
                    return Ok(json!(
                        history_missing && snapshot_missing && !project.database.exists()
                    ));
                }
                let before = fs::read(&project.database).map_err(error)?;
                let _ = project.history().map_err(error)?;
                let _ = project.load("latest").map_err(error)?;
                let _ = self.snapshot()?;
                let after = fs::read(&project.database).map_err(error)?;
                Ok(json!(before == after))
            }
            _ => Err("Unknown history operation".into()),
        }
    }
    fn profile_path(&self, name: &str) -> Result<PathBuf, String> {
        if name.is_empty()
            || name.contains('\\')
            || name.split('/').any(|s| matches!(s, "" | "." | ".."))
            || !name.ends_with(".md")
        {
            return Err("Invalid local profile document path".into());
        }
        let path = self.root.join(".dctx0/profiles/rust").join(name);
        if !path.is_file() {
            return Err("Unknown profile document".into());
        }
        Ok(path)
    }
    pub(super) fn profiles(&mut self, subject: &str, args: &[Value]) -> Result<Value, String> {
        match subject.rsplit('.').next().ok_or("Missing operation")? {
            "adaptation" => {
                create_profiles(&self.root, &dctx0_target_rust::Provider, true, false, false)
                    .map_err(error)?;
                let id = self.setup_tasks()?;
                let project = self.project()?;
                let profiles = project
                    .profiles(Some(&self.root), Some("rust"))
                    .map_err(error)?
                    .ok_or("Missing profiles")?;
                let receipt = project
                    .task_action_with_profiles(
                        Action::Start {
                            task: id,
                            actor: "profile-worker".into(),
                            reservation: None,
                        },
                        Some(&profiles),
                    )
                    .map_err(error)?;
                let state = receipt.state.ok_or("Missing claimed state")?;
                let frozen = state
                    .execution_bundle
                    .as_ref()
                    .ok_or("Claim did not freeze profile bundle")?
                    .hash
                    .clone();
                fs::write(self.profile_path("common.md")?, "Revised common guidance")
                    .map_err(error)?;
                let after = project.task_session(None, None).map_err(error)?;
                let kept = after.states[&id]
                    .execution_bundle
                    .as_ref()
                    .ok_or("Frozen bundle lost")?
                    .hash
                    .clone();
                let unchanged = after.states[&id].version == state.version;
                project
                    .task_action(Action::Complete {
                        task: id,
                        actor: "profile-worker".into(),
                        claim: state.claim.ok_or("Missing claim")?,
                        version: state.version,
                        result: TaskResult {
                            summary: "Implemented before adaptation".into(),
                            artifacts: vec![],
                            decisions: vec![],
                            implementation: None,
                        },
                    })
                    .map_err(error)?;
                let updated = project
                    .profiles(Some(&self.root), Some("rust"))
                    .map_err(error)?
                    .ok_or("Missing profiles")?;
                let preview = project
                    .task_profile_changes(
                        &updated,
                        ProfileUpdateSelection {
                            tasks: vec![id],
                            ..Default::default()
                        },
                    )
                    .map_err(error)?;
                let plan = project
                    .task_profile_update(
                        &preview,
                        &updated,
                        "Explicit self-verification adaptation",
                    )
                    .map_err(error)?;
                let session = project.task_session(None, None).map_err(error)?;
                let original = project.task_show_session(id, None, None).map_err(error)?;
                let old = original
                    .definitions
                    .get(&id)
                    .ok_or("Missing original definition")?;
                let replacement = plan
                    .members
                    .iter()
                    .find(|m| m.task_id != id && m.candidate.subjects == old.candidate.subjects)
                    .ok_or("Adaptation did not create replacement")?;
                let new = &session.states[&replacement.task_id];
                Ok(
                    json!({"oldTask":id,"newTask":replacement.task_id,"frozenHash":frozen,"retainedHash":kept,"editUnchanged":unchanged,"oldDone":original.states[&id].work==WorkState::Done,"newOpen":new.work==WorkState::Open,"newUnverified":new.check==CheckStatus::Unverified}),
                )
            }
            "initialize" => {
                if args[0] != "rust" {
                    return Err("Self fixture supports the Rust profile provider".into());
                }
                let before = self.root.join(".dctx0/profiles/rust");
                let preserved = if before.exists() {
                    self.project()?
                        .profiles(Some(&self.root), Some("rust"))
                        .map_err(error)?
                        .ok_or("Profiles missing")?
                        .documents
                        .iter()
                        .map(|d| d.path.clone())
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                };
                let result =
                    create_profiles(&self.root, &dctx0_target_rust::Provider, true, false, false)
                        .map_err(error)?;
                Ok(json!({"created":result.files,"preserved":preserved}))
            }
            "read" => {
                let name = args[0].as_str().ok_or("Missing path")?;
                let path = self.profile_path(name)?;
                Ok(json!({"path":name,"content":fs::read_to_string(path).map_err(error)?}))
            }
            "write" => {
                let path = self.profile_path(args[0].as_str().ok_or("Missing path")?)?;
                fs::write(path, args[1].as_str().ok_or("Missing content")?).map_err(error)?;
                Ok(Value::Null)
            }
            _ => Err("Unknown profile operation".into()),
        }
    }
}
