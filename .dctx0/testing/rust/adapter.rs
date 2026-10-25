//! Version 1 testing-only adapter. Product behavior is called, never reconstructed.
mod cli;
pub mod contract;
mod native;
mod sessions;
mod site;
mod translation;
mod validation;
use contract::*;
use dctx0_core::{Source, VerificationIssue, VerificationResult};
use dctx0_engine::Project;
use dctx0_tasks::{
    Action, CheckStatus, Filters, ProfileUpdateSelection, TaskResult, WorkState, task_case_coverage,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use translation::*;
static NEXT: AtomicU64 = AtomicU64::new(0);
const SPEC: &str = "workspace Sample { value Id = Int { range 1..10 } surface Api { operation next(input: Id) -> Id } }";
fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
pub fn describe() -> Result<Value, TechnicalError> {
    let model = model()?;
    let subjects=model["operations"].as_object().ok_or("Missing operations")?.keys().map(|subject|{
        let artifact=if subject.contains("Site."){ "crates/dctx0-engine/src/site.rs" }else if subject.contains("Compiler."){"crates/dctx0-language/src/compiler.rs"}else if subject.contains("Evidence."){"crates/dctx0-tasks/src/adoption.rs"}else if subject.contains("Tasks."){"crates/dctx0-engine/src/tasks.rs"}else if subject.contains("History."){"crates/dctx0-engine/src/lib.rs"}else if subject.contains("Profiles."){"crates/dctx0-engine/src/profiles.rs"}else if subject.contains("Editors."){"crates/dctx0-lsp/src/lib.rs"}else{"crates/dctx0-cli/src/verification.rs"};
        json!({"subject":subject,"role":"operation","artifact":artifact,"entry_point":subject})
    }).chain(model["data"].as_object().ok_or("Missing data")?.keys().map(|subject|json!({"subject":subject,"role":"value","artifact":".dctx0/testing/rust/validation.rs","entry_point":"serde projection decoding"}))).collect::<Vec<_>>();
    Ok(
        json!({"protocol_version":1,"subjects":subjects,"artifacts":["crates/dctx0-engine/src/site.rs","crates/dctx0-language/src/compiler.rs","crates/dctx0-tasks/src/adoption.rs","crates/dctx0-engine/src/tasks.rs","crates/dctx0-engine/src/lib.rs","crates/dctx0-engine/src/profiles.rs","crates/dctx0-cli/src/verification.rs","crates/dctx0-lsp/src/lib.rs",".dctx0/testing/rust/validation.rs"],"capabilities":["sessions","invocation","events","values"],"representations":{"Int":"checked i64 Rust API conversion","List":"ordered Vec; coverage sets/maps normalized by BTreeSet/BTreeMap","Option":"Option"}}),
    )
}
pub fn begin(_: &str, _: Context) -> Result<Box<dyn Session>, TechnicalError> {
    let root = std::env::current_dir()
        .map_err(error)?
        .join(".bin/sessions")
        .join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(root.join("spec")).map_err(error)?;
    fs::write(root.join("spec/main.dctx0"), SPEC).map_err(error)?;
    Ok(Box::new(Actual {
        root,
        model: model()?,
        task: None,
        lsp: None,
    }))
}
struct Actual {
    root: PathBuf,
    model: Value,
    task: Option<i64>,
    lsp: Option<cli::Lsp>,
}
impl Actual {
    fn project(&self) -> Result<Project, String> {
        Project::new(&self.root.join("spec"), None).map_err(error)
    }
    fn dispatch(&mut self, subject: &str, args: &[Value]) -> Result<Value, String> {
        match subject {
            "Dctx0.Compiler.Frontend.compile" => {
                self.compile(vec![args[0].as_str().ok_or("Missing source")?.into()])
            }
            "Dctx0.Compiler.Frontend.compose" => self.compile(strings(&args[0])?),
            "Dctx0.Evidence.TaskEvidence.coverage" => {
                let requirements = strings(&args[0])?.into_iter().collect();
                let mut mappings = std::collections::BTreeMap::new();
                for row in args[1].as_array().ok_or("Expected mappings")? {
                    mappings
                        .entry(text(row, "caseId")?.to_owned())
                        .or_insert_with(std::collections::BTreeSet::new)
                        .extend(strings(&row["requirements"])?);
                }
                let mut outcomes = std::collections::BTreeMap::new();
                for row in args[2].as_array().ok_or("Expected outcomes")? {
                    let status = match text(row, "status")? {
                        "Passed" => VerificationResult::Pass,
                        "Failed" => VerificationResult::Fail {
                            issue: VerificationIssue {
                                code: "ACTUAL_CASE_FAILURE".into(),
                                message: "Observed failure".into(),
                                span: None,
                            },
                        },
                        "Unknown" => VerificationResult::Unknown {
                            issue: VerificationIssue {
                                code: "ACTUAL_CASE_UNKNOWN".into(),
                                message: "Unavailable observation".into(),
                                span: None,
                            },
                        },
                        _ => return Err("Unknown status".into()),
                    };
                    outcomes.insert(text(row, "caseId")?.to_owned(), status);
                }
                let actual = task_case_coverage(&requirements, &mappings, &outcomes);
                Ok(
                    json!({"status":status(&actual.result),"requiredCases":actual.required_cases,"missingRequirements":actual.missing_requirements,"unexecutedCases":actual.unexecuted_cases}),
                )
            }
            s if s.starts_with("Dctx0.Tasks.Work.") => self.tasks(s, args),
            s if s.starts_with("Dctx0.History.Publication.") => self.history(s, args),
            s if s.starts_with("Dctx0.Profiles.Guidance.") => self.profiles(s, args),
            s if s.starts_with("Dctx0.Site.Companion.") => self.site(s),
            s if s.starts_with("Dctx0.Distribution.CommandLine.") => self.command(s, args),
            s if s.starts_with("Dctx0.Editors.LanguageService.") => self.editors(s, args),
            s if s.starts_with("Dctx0.Native.Verification.") => self.native(s, args),
            _ => Err(format!("Unbound actual API: {subject}")),
        }
    }
    fn compile(&self, sources: Vec<String>) -> Result<Value, String> {
        let sources = sources
            .into_iter()
            .enumerate()
            .map(|(i, text)| Source {
                file: format!("source-{i}.dctx0"),
                text,
            })
            .collect::<Vec<_>>();
        let compilation = dctx0_language::compile(&sources);
        Ok(
            json!({"accepted":compilation.ir.is_some(),"diagnostics":compilation.diagnostics.len(),"symbols":compilation.ir.as_ref().map_or(0,|ir|ir.symbols.len()),"operations":compilation.ir.as_ref().map_or(0,|ir|ir.operations.len()),"files":compilation.diagnostics.iter().map(|d|&d.span.file).collect::<Vec<_>>(),"lines":compilation.diagnostics.iter().map(|d|d.span.start.line).collect::<Vec<_>>(),"columns":compilation.diagnostics.iter().map(|d|d.span.start.column).collect::<Vec<_>>()}),
        )
    }
}
impl Session for Actual {
    fn invoke(&mut self, invocation: Value) -> Result<Value, TechnicalError> {
        let subject = invocation["subject"].as_str().ok_or("Missing subject")?;
        let args = invocation["arguments"]
            .as_array()
            .ok_or("Missing arguments")?
            .iter()
            .map(decode)
            .collect::<Result<Vec<_>, _>>()
            .map_err(TechnicalError::adapter)?;
        let ty = self.model["operations"][subject]["result"].clone();
        let result = call_application(|| self.dispatch(subject, &args));
        match result {
            Ok(value) => Ok(json!({"kind":"returned","value":encode(&self.model,&value,&ty)?})),
            Err(message)
                if subject.starts_with("Dctx0.Tasks.")
                    || subject.starts_with("Dctx0.Profiles.")
                    || subject.starts_with("Dctx0.Native.")
                    || subject.starts_with("Dctx0.Editors.")
                    || subject.ends_with("Publication.head") =>
            {
                let declaration = if subject.starts_with("Dctx0.Tasks.") {
                    "Dctx0.Tasks.TransitionConflict"
                } else if subject.starts_with("Dctx0.Profiles.") {
                    "Dctx0.Profiles.InvalidRequest"
                } else if subject.starts_with("Dctx0.History.") {
                    "Dctx0.History.MissingSnapshot"
                } else if subject.starts_with("Dctx0.Editors.") {
                    "Dctx0.Editors.InvalidRequest"
                } else {
                    "Dctx0.Native.InvalidRequest"
                };
                let payload = if subject.starts_with("Dctx0.Tasks.") {
                    json!({"taskId":args.first().and_then(Value::as_i64).unwrap_or(-1)})
                } else {
                    json!({"reason":message})
                };
                let payload = encode(&self.model, &payload, &json!({"symbol_id":declaration}))?;
                Ok(
                    json!({"kind":"returned","value":{"kind":"failure","value":{"declaration":declaration,"payload":payload}}}),
                )
            }
            Err(message) => Ok(json!({"kind":"threw","message":message})),
        }
    }
    fn validate(&mut self, subject: &str, value: Value) -> Result<Value, TechnicalError> {
        let native = decode(&value).map_err(TechnicalError::adapter)?;
        Ok(match validation::validate(subject, native) {
            Ok(()) => json!({"kind":"accepted","value":value}),
            Err(reason) => json!({"kind":"rejected","reason":reason}),
        })
    }
    fn close(&mut self) -> Result<(), TechnicalError> {
        if let Some(mut lsp) = self.lsp.take() {
            lsp.close()?;
        }
        fs::remove_dir_all(&self.root).map_err(TechnicalError::adapter)
    }
}
impl Drop for Actual {
    fn drop(&mut self) {
        if let Some(mut lsp) = self.lsp.take() {
            let _ = lsp.close();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value[name]
        .as_str()
        .ok_or_else(|| format!("Missing text {name}"))
}
fn strings(value: &Value) -> Result<Vec<String>, String> {
    value
        .as_array()
        .ok_or("Expected list")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "Expected text".into())
        })
        .collect()
}
fn int(value: &Value) -> Result<i64, String> {
    value.as_i64().ok_or_else(|| "Expected Rust i64".into())
}
fn status(value: &VerificationResult) -> &'static str {
    match value {
        VerificationResult::Pass => "Passed",
        VerificationResult::Fail { .. } => "Failed",
        VerificationResult::Unknown { .. } => "Unknown",
    }
}
