mod args;
mod benchmark;
mod commands;
mod harness_benchmark;
mod metrics;
mod profile_benchmark;
mod profiles;
mod render;
mod sources;
mod tasks;
mod verification;
mod verification_benchmark;
#[cfg(feature = "typescript")]
mod verification_complete;
mod verification_discovery_benchmark;
mod verification_history_benchmark;
mod verification_setup_benchmark;
use commands::run;
use std::{io, process::ExitCode};

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn main() -> ExitCode {
    match run(
        &std::env::args().skip(1).collect::<Vec<_>>(),
        &mut io::stdout().lock(),
    ) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        args::Phase,
        benchmark::benchmark,
        metrics::peak_rss,
        render::{display_changes, json},
    };
    use dctx0_engine::Project;
    use std::{fs, io::Write, path::Path};
    struct FailingWriter {
        budget: usize,
    }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.budget == 0 {
                return Err(io::Error::other("output unavailable"));
            }
            let count = self.budget.min(bytes.len());
            self.budget -= count;
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush unavailable"))
        }
    }
    #[test]
    fn output_failures_and_unavailable_memory_are_explicit() {
        assert_eq!(peak_rss("VmHWM: 42 kB"), Some(43008));
        for status in [
            "",
            "VmHWM:",
            "VmHWM: wrong",
            "VmHWM: 18446744073709551615 kB",
        ] {
            assert_eq!(peak_rss(status), None);
        }
        let value = serde_json::json!({"result":"pass"});
        for budget in [0, value.to_string().len()] {
            assert!(json(&mut FailingWriter { budget }, &value).is_err());
        }
        let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema/dctx0");
        for args in [
            vec!["parse".into(), input.display().to_string()],
            vec!["bench-parse".into(), input.display().to_string()],
            vec!["bench-compile".into(), input.display().to_string()],
        ] {
            for budget in [0, usize::MAX] {
                assert!(run(&args, &mut FailingWriter { budget }).is_err());
            }
        }
        let changes = dctx0_changes::Changes {
            before: None,
            after: "test".into(),
            source_changed: false,
            changes: vec![],
        };
        assert!(display_changes(&mut FailingWriter { budget: 0 }, &changes).is_err());
    }
    #[test]
    fn invalid_compilation_output_failures_are_reported() {
        let root =
            std::env::temp_dir().join(format!("dctx0-invalid-output-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("spec.dctx0"), "workspace W { type T = Missing }").unwrap();
        let project = Project::new(&root, None).unwrap();
        assert!(
            benchmark(
                &project,
                Phase::Compile,
                1,
                &mut FailingWriter { budget: 0 }
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn task_markdown_json_and_benchmark_output_errors_are_propagated() {
        let root = std::env::temp_dir().join(format!("dctx0-task-output-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("spec.dctx0"), "workspace W { type A = Int }").unwrap();
        let project = Project::new(&root, None).unwrap();
        project.check().unwrap();
        project.task_plan("latest").unwrap();
        for json in [false, true] {
            let mut args = vec!["tasks".into(), "list".into(), root.display().to_string()];
            if json {
                args.push("--json".into());
            }
            assert!(
                run(&args, &mut FailingWriter { budget: 0 })
                    .unwrap_err()
                    .contains("output unavailable")
            );
        }
        let bench_project = Project::new(&root, Some(&root.join("bench.sqlite3"))).unwrap();
        assert!(
            benchmark(
                &bench_project,
                Phase::TasksPlan,
                1,
                &mut FailingWriter { budget: 0 }
            )
            .unwrap_err()
            .contains("output unavailable")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verification_planning_retains_unknowns_and_propagates_every_output_failure() {
        let root =
            std::env::temp_dir().join(format!("dctx0-verification-output-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("spec.dctx0"),"workspace W { type A = Int; invariant P { assert true }; example E { expect Success } }").unwrap();
        let project = Project::new(&root, None).unwrap();
        assert!(project.check().unwrap().ok);
        let plan = project.task_plan("latest").unwrap();
        let args = vec![
            "verify".into(),
            "plan".into(),
            root.display().to_string(),
            "--subject".into(),
            "W".into(),
        ];
        let mut output = Vec::new();
        assert!(run(&args, &mut output).unwrap());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("MISSING_INVOCATION"));
        assert!(text.contains("no execution"));
        // Break at each Markdown boundary to exercise failed output independently.
        let mut offset = 0;
        for line in text.lines() {
            assert!(run(&args, &mut FailingWriter { budget: offset }).is_err());
            offset += line.len() + 1;
        }
        let mut json_args = args.clone();
        json_args.push("--json".into());
        assert!(run(&json_args, &mut FailingWriter { budget: 0 }).is_err());
        let mut out = Vec::new();
        assert!(run(&json_args, &mut out).unwrap());
        let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["examples"].as_array().unwrap().len(), 1);
        let fixture = root.join("inputs.json");
        fs::write(&fixture, "{\"examples\":{}}").unwrap();
        let task = plan
            .members
            .iter()
            .find(|m| m.candidate.subjects.contains(&"W.A".into()))
            .unwrap()
            .task_id;
        let fixture_args = vec![
            "verify".into(),
            "plan".into(),
            root.display().to_string(),
            "--task".into(),
            format!("T{task}"),
            "--fixtures".into(),
            fixture.display().to_string(),
            "--json".into(),
        ];
        assert!(run(&fixture_args, &mut Vec::new()).unwrap());
        fs::write(&fixture, "invalid JSON").unwrap();
        assert!(run(&fixture_args, &mut Vec::new()).is_err());
        fs::remove_file(&fixture).unwrap();
        assert!(run(&fixture_args, &mut Vec::new()).is_err());
        for id in ["bad", "T0", "-1"] {
            let bad = vec![
                "verify".into(),
                "plan".into(),
                root.display().to_string(),
                "--task".into(),
                id.into(),
            ];
            assert!(run(&bad, &mut Vec::new()).is_err());
        }
        let numeric = vec![
            "verify".into(),
            "plan".into(),
            root.display().to_string(),
            "--task".into(),
            task.to_string(),
        ];
        assert!(run(&numeric, &mut Vec::new()).unwrap());
        let mut invalid = args;
        *invalid.last_mut().unwrap() = "missing".into();
        assert!(!run(&invalid, &mut Vec::new()).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
