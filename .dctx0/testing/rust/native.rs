//! One inner verification only: independent Rust fixture, actual run/show/replay/adoption.
use super::*;
use dctx0_engine::{
    NativeCancellation, NativeVerificationReport, VerificationSelection, create_profiles,
};
use dctx0_target_rust::RustHarness;
impl Actual {
    fn native_project(&self) -> Result<(PathBuf, Project), String> {
        let root = self.root.join("native");
        if !root.exists() {
            fn copy(source: &std::path::Path, destination: &std::path::Path) -> Result<(), String> {
                fs::create_dir_all(destination).map_err(error)?;
                for entry in fs::read_dir(source).map_err(error)? {
                    let entry = entry.map_err(error)?;
                    if matches!(
                        entry.file_name().to_str(),
                        Some("target" | ".bin" | ".dctx0ctl")
                    ) {
                        continue;
                    }
                    if entry.file_type().map_err(error)?.is_dir() {
                        copy(&entry.path(), &destination.join(entry.file_name()))?;
                    } else {
                        fs::copy(entry.path(), destination.join(entry.file_name()))
                            .map_err(error)?;
                    }
                }
                Ok(())
            }
            copy(
                &std::env::current_dir()
                    .map_err(error)?
                    .join("fixtures/native"),
                &root,
            )?;
            create_profiles(&root, &dctx0_target_rust::Provider, true, false, false)
                .map_err(error)?;
            let project = Project::new(&root.join("spec"), None).map_err(error)?;
            if !project.check().map_err(error)?.ok {
                return Err("Native setup spec rejected".into());
            }
            project.task_plan("latest").map_err(error)?;
        }
        Ok((
            root.clone(),
            Project::new(&root.join("spec"), None).map_err(error)?,
        ))
    }
    pub(super) fn native(&mut self, subject: &str, args: &[Value]) -> Result<Value, String> {
        let (root, project) = self.native_project()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(error)?;
        match subject
            .rsplit('.')
            .next()
            .ok_or("Missing native operation")?
        {
            "run" => {
                let subjects = strings(&args[0])?;
                if subjects
                    .iter()
                    .any(|s| s != "Fixture" && s != "Fixture.Api.increment")
                {
                    return Err("Explicit bounded fixture scope required".into());
                }
                let configuration = root.join(".dctx0/testing/verification.json");
                let mut config: Value =
                    serde_json::from_slice(&fs::read(&configuration).map_err(error)?)
                        .map_err(error)?;
                config["scope"]["subjects"] = json!(subjects);
                fs::write(
                    configuration,
                    serde_json::to_vec_pretty(&config).map_err(error)?,
                )
                .map_err(error)?;
                let check = project.check().map_err(error)?;
                if !check.ok {
                    return Err("Native fixture spec invalid".into());
                }
                project.task_plan("latest").map_err(error)?;
                let report = runtime
                    .block_on(project.run_verification(
                        Some(&root),
                        VerificationSelection {
                            subjects: subjects.into_iter().collect(),
                            ..Default::default()
                        },
                        &RustHarness,
                        &NativeCancellation::default(),
                        args[1].as_bool().ok_or("Missing noAdopt")?,
                    ))
                    .map_err(error)?;
                summary(&report)
            }
            "show" => {
                let saved = project
                    .saved_verification_run(args[0].as_str().ok_or("Missing run ID")?)
                    .map_err(error)?;
                let report = NativeVerificationReport {
                    evidence: saved,
                    supervision: None,
                    adopted_tasks: vec![],
                    adoption_issues: vec![],
                    adoption_proposal: None,
                };
                summary(&report)
            }
            "firstCase" => {
                let saved = project
                    .saved_verification_run(args[0].as_str().ok_or("Missing run ID")?)
                    .map_err(error)?;
                Ok(json!(saved.run.cases.first().ok_or("Run has no cases")?.id))
            }
            "replay" => {
                let report = runtime
                    .block_on(project.replay_verification(
                        Some(&root),
                        args[0].as_str().ok_or("Missing run ID")?,
                        args[1].as_str().ok_or("Missing case ID")?,
                        &RustHarness,
                        &NativeCancellation::default(),
                    ))
                    .map_err(error)?;
                Ok(
                    json!({"originalRun":report.original_run,"originalCase":report.original_case,"runId":report.evidence.evidence.id,"status":report.case_result.as_ref().map(status).unwrap_or("Unknown"),"provenanceChanges":report.changes.len()}),
                )
            }
            "capability" => {
                let enabled = args[0].as_bool().ok_or("Missing capability flag")?;
                let adapter = "fixtures/native/.dctx0/testing/rust/adapter.rs";
                let actual =
                    fs::read_to_string(std::env::current_dir().map_err(error)?.join(adapter))
                        .map_err(error)?;
                fs::write(
                    root.join(".dctx0/testing/rust/adapter.rs"),
                    if enabled {
                        actual
                    } else {
                        actual.replace("\"invocation\",", "")
                    },
                )
                .map_err(error)?;
                Ok(Value::Null)
            }
            "mutate" => {
                let broken = args[0].as_bool().ok_or("Missing mutation flag")?;
                let actual = fs::read_to_string(
                    std::env::current_dir()
                        .map_err(error)?
                        .join("fixtures/native/src/lib.rs"),
                )
                .map_err(error)?;
                fs::write(
                    root.join("src/lib.rs"),
                    if broken {
                        actual.replace("input + 1", "input + 2")
                    } else {
                        actual
                    },
                )
                .map_err(error)?;
                Ok(Value::Null)
            }
            "taskCounts" => {
                let session = project.task_session(None, None).map_err(error)?;
                Ok(
                    json!({"done":session.states.values().filter(|s|s.work==WorkState::Done).count(),"passed":session.states.values().filter(|s|s.check==CheckStatus::Pass).count()}),
                )
            }
            _ => Err("Unknown native operation".into()),
        }
    }
}
fn summary(report: &NativeVerificationReport) -> Result<Value, String> {
    let run = &report.evidence.run;
    Ok(
        json!({"runId":report.evidence.id,"mandatoryCases":run.required_cases.len(),"recordedCases":run.cases.len(),"openObligations":run.open_obligations.len(),"provenanceGaps":run.provenance.incomplete.len(),"status":status(&run.result),"adoptedTasks":report.adopted_tasks.len()}),
    )
}
