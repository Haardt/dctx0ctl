//! Published generation and native unavailable scaffolds; these tests never adopt tasks.
use serde_json::Value;
use std::{fs, path::Path, process::Command};
fn command(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}
fn ok(root: &Path, args: &[&str]) -> Value {
    let out = command(root, args);
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn generators_preserve_edits_propose_deterministically_and_compile_unavailable_tests() {
    let targets: [&str; 3] = ["go", "typescript", "rust"];
    for target in targets {
        if !match target {
            "go" => cfg!(feature = "go"),
            "typescript" => cfg!(feature = "typescript"),
            _ => cfg!(feature = "rust"),
        } {
            continue;
        }
        let root = std::env::temp_dir().join(format!(
            "dctx0-cli-generators-{target}-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("spec")).unwrap();
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("spec/w.dctx0"),
            "workspace W { value Id = Int { range 1..3 } }",
        )
        .unwrap();
        ok(&root, &["check", "spec", "--json"]);
        ok(
            &root,
            &[
                "verify",
                "init",
                "spec",
                "--project",
                "app",
                "--target",
                target,
                "--subject",
                "W.Id",
                "--json",
            ],
        );
        let before = fs::read(root.join("spec/.dctx0ctl/state.sqlite3")).unwrap();
        let scaffold = [
            "verify",
            "bindings",
            "scaffold",
            "spec",
            "--project",
            "app",
            "--target",
            target,
            "--subject",
            "W.Id",
            "--dry-run",
            "--json",
        ];
        let proposal = ok(&root, &scaffold);
        assert_eq!(proposal, ok(&root, &scaffold));
        for report in proposal.as_array().unwrap() {
            assert_eq!(report["status"], "proposed");
            assert!(
                !root
                    .join("app")
                    .join(report["path"].as_str().unwrap())
                    .exists()
            );
        }
        let generated = ok(
            &root,
            &scaffold[..scaffold.len() - 2]
                .iter()
                .copied()
                .chain(["--json"])
                .collect::<Vec<_>>(),
        );
        assert!(
            generated
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["status"] == "created")
        );
        let directory = root.join("app/.dctx0/testing").join(target);
        let extension = match target {
            "go" => "go",
            "rust" => "rs",
            _ => "ts",
        };
        let output = format!("candidate-codecs.{extension}");
        ok(
            &root,
            &[
                "verify",
                "scaffold-codecs",
                "spec",
                "--project",
                "app",
                "--target",
                target,
                "--subject",
                "W.Id",
                "--output",
                &output,
                "--candidate",
                "upgrade",
                "--json",
            ],
        );
        assert!(directory.join(format!("upgrade.{output}.txt")).exists());
        assert!(!directory.join(&output).exists());
        let path = directory.join(match target {
            "go" => "wire-codecs.go",
            "rust" => "wire_codecs.rs",
            _ => "wire-codecs.ts",
        });
        let original = fs::read(&path).unwrap();
        fs::write(&path, "// authored edits").unwrap();
        let preserved = ok(
            &root,
            &scaffold[..scaffold.len() - 2]
                .iter()
                .copied()
                .chain(["--json"])
                .collect::<Vec<_>>(),
        );
        assert!(
            preserved
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["status"] == "preserved")
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "// authored edits");
        fs::write(&path, original).unwrap();
        assert_eq!(
            before,
            fs::read(root.join("spec/.dctx0ctl/state.sqlite3")).unwrap()
        );
        // Compile and execute the default hook: no binding can silently become PASS.
        let native = match target {
            "go" => {
                fs::write(root.join("app/go.mod"), "module fixture\n\ngo 1.25.0\n").unwrap();
                Command::new("go")
                    .args([
                        "test",
                        "-count=1",
                        "-timeout=30s",
                        "-run=^TestPublishedContractPlan$",
                        ".",
                    ])
                    .current_dir(&directory)
                    .output()
                    .unwrap()
            }
            "typescript" => Command::new("bun")
                .args(["test", "contract-plan.test.ts"])
                .current_dir(&directory)
                .output()
                .unwrap(),
            _ => {
                let adapter = directory.join("adapter.rs");
                fs::write(
                    &adapter,
                    format!(
                        "{}\n#[cfg(test)] mod contract_plan_tests;\n",
                        fs::read_to_string(&adapter).unwrap()
                    ),
                )
                .unwrap();
                Command::new("cargo")
                    .args(["test", "--offline", "--lib", "published_contract_plan"])
                    .env_remove("RUSTFLAGS")
                    .current_dir(&directory)
                    .output()
                    .unwrap()
            }
        };
        assert!(!native.status.success());
        let diagnostics = format!(
            "{}{}",
            String::from_utf8_lossy(&native.stdout),
            String::from_utf8_lossy(&native.stderr)
        );
        assert!(
            diagnostics.contains("prerequisites have not been reviewed"),
            "{target}: {diagnostics}"
        );
        // Direct central execution also remains UNKNOWN with an unimplemented adapter.
        let project = dctx0_engine::Project::new(&root.join("spec"), None).unwrap();
        let (_, configuration) = project
            .verification_configuration(Some(&root.join("app")))
            .unwrap();
        let fixtures = serde_json::from_slice(
            &fs::read(root.join("app").join(&configuration.fixtures)).unwrap(),
        )
        .unwrap();
        let plan = project
            .verification_plan(
                dctx0_engine::VerificationSelection {
                    subjects: ["W.Id".into()].into(),
                    ..Default::default()
                },
                &fixtures,
                configuration.budgets,
            )
            .unwrap();
        use sha2::{Digest, Sha256};
        let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&plan).unwrap()));
        let check = plan.check_occurrence.to_string();
        let result = command(
            &root,
            &[
                "verify",
                "bindings",
                "test",
                "spec",
                "--project",
                "app",
                "--target",
                target,
                "--subject",
                "W.Id",
                "--expected-snapshot",
                &plan.snapshot,
                "--expected-check",
                &check,
                "--expected-plan",
                &hash,
                "--json",
            ],
        );
        assert!(!result.status.success());
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["published"], false);
        assert_eq!(report["run"]["result"]["status"], "UNKNOWN");
        assert_eq!(
            before,
            fs::read(root.join("spec/.dctx0ctl/state.sqlite3")).unwrap()
        );
        fs::write(
            root.join("spec/w.dctx0"),
            "workspace W { value Id = Int { range 1..4 } }",
        )
        .unwrap();
        assert!(!command(&root, &scaffold).status.success());
        fs::remove_dir_all(root).unwrap();
    }
}
