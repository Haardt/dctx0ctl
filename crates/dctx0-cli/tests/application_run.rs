#![cfg(all(target_os = "linux", feature = "go"))]
use dctx0_core::VerificationBudgets;
use dctx0_engine::Project;
use dctx0_tasks::{Action, CheckStatus, WorkState};
use serde_json::{Value, json};
use std::{fs, process::Command};

#[test]
fn native_run_preserves_diagnostic_and_claimed_work_then_delivers_the_actual_contract() {
    let root = std::env::temp_dir().join(format!("dctx0-native-run-cli-{}", std::process::id()));
    fs::create_dir_all(root.join(".dctx0/testing")).unwrap();
    fs::write(
        root.join("spec.dctx0"),
        "workspace W { value Year = Int { range 3..4 } }",
    )
    .unwrap();
    fs::write(root.join("fixtures.json"), r#"{"examples":{}}"#).unwrap();
    fs::write(root.join(".dctx0/testing/verification.json"), json!({
        "schema_version":1,"target":"go","runtime":"go","adapter":"adapter.go","fixtures":"fixtures.json","history_retention":"preserve-all","application_tests":{"plan_bytes":1048576},
        "scope":{"subjects":["W.Year"],"tasks":[]},"budgets":VerificationBudgets::default()
    }).to_string()).unwrap();
    fs::create_dir_all(root.join(".dctx0ctl")).unwrap();
    fs::write(
        root.join(".dctx0ctl/verification.json"),
        json!({"schema_version":1,"project":root}).to_string(),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_dctx0");
    let invoke = |args: &[&str]| {
        let output = Command::new(binary).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{:?}: {}\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let path = root.to_str().unwrap();
    invoke(&[
        "verify",
        "application-tests",
        "library",
        "--project",
        path,
        "--target",
        "go",
    ]);
    invoke(&["profile", "go", "create", "--project", path, "--default"]);
    fs::write(root.join("go.mod"), "module year\n\ngo 1.25.0\n\nrequire github.com/Haardt/dctx0ctl/applicationtests/go v0.3.0\n\nreplace github.com/Haardt/dctx0ctl/applicationtests/go => ./.dctx0/application-tests/library\n").unwrap();
    fs::write(
        root.join("year.go"),
        r#"package year
import "encoding/json"
type InvalidYear struct{}
func (*InvalidYear) Error()string{return "Invalid year"}
func DecodeYear(raw []byte)(int,error){
 var year int
 if err:=json.Unmarshal(raw,&year);err!=nil{return 0,&InvalidYear{}}
 if year<3||year>4{return 0,&InvalidYear{}}
 return year,nil
}
"#,
    )
    .unwrap();
    fs::write(root.join("year_test.go"), r#"package year
import("encoding/json";"errors";"os";"strconv";"testing";"time";app "github.com/Haardt/dctx0ctl/applicationtests/go")
func TestYear(t *testing.T){
 client,err:=app.Dial(os.Getenv("DCTX0_APPLICATION_SOCKET"),app.DefaultFrameBytes,5*time.Second)
 if err!=nil{t.Fatal(err)}
 t.Cleanup(func(){defer client.Close();if _,err:=client.Finish();err!=nil{t.Error(err)}})
 client.StatelessDecoderTest(t,func(raw []byte,_ app.TypeRef)(any,error){
  value,err:=DecodeYear(raw);var rejection *InvalidYear
  if errors.As(err,&rejection){return nil,app.Reject(err.Error())}
  return value,err
 },func(value any,_ app.TypeRef)(app.Value,error){
  raw,err:=json.Marshal(strconv.Itoa(value.(int)));return app.Value{Kind:"integer",Value:raw},err
 })
}
"#).unwrap();
    fs::write(root.join("recipe.json"),json!({
        "schema_version":1,"runtime":"go","root_test":"TestYear","test_directory":"root","prepare":[],
        "build":[{"program":"go","arguments":["test","-c","-mod=readonly","-buildvcs=false","-o","{artifacts}/year.test","."]}],
        "catalog":{"program":"{artifacts}/year.test","arguments":["-test.list=."]},
        "run":{"program":"go","arguments":["tool","test2json","-t","{artifacts}/year.test","-test.v","-test.run=^TestYear$"]},"junit":null
    }).to_string()).unwrap();
    // Also cover the human-readable prepare result and explicit form/boundary.
    invoke(&[
        "verify",
        "application-tests",
        "prepare",
        path,
        "--project",
        path,
        "--plan",
        "plan.json",
        "--group",
        "native",
        "--test-name",
        "TestYear",
        "--form",
        "decoder",
        "--boundary",
        "json-decoder",
        "--stateless-decoder",
        "TestYear",
    ]);
    let project = Project::new(&root, None).unwrap();
    assert!(
        project
            .read_application_testing_bundle(&root, "plan.json")
            .is_ok()
    );
    let source = root.join("year.go");
    let original = fs::read_to_string(&source).unwrap();
    fs::write(&source, format!("{original}\n// changed since review\n")).unwrap();
    assert!(
        project
            .read_application_testing_bundle(&root, "plan.json")
            .unwrap_err()
            .to_string()
            .contains("DECODER_REVIEW_STALE")
    );
    fs::write(source, original).unwrap();
    let module = root.join("go.mod");
    let original = fs::read_to_string(&module).unwrap();
    let absolute = root.join(".dctx0/application-tests/library");
    fs::write(
        &module,
        original.replace(
            "./.dctx0/application-tests/library",
            absolute.to_str().unwrap(),
        ),
    )
    .unwrap();
    assert!(
        project
            .read_application_testing_bundle(&root, "plan.json")
            .unwrap_err()
            .to_string()
            .contains("DECODER_REVIEW_EXTERNAL_SOURCE")
    );
    fs::write(module, original).unwrap();
    let bundle: Value = serde_json::from_slice(&fs::read(root.join("plan.json")).unwrap()).unwrap();
    project
        .task_plan(bundle["plan"]["program"]["snapshot"].as_str().unwrap())
        .unwrap();
    let session = project.task_session(None, None).unwrap();
    assert_eq!(session.plan.members.len(), 1);
    let task = session.plan.members[0].task_id;
    let run = |diagnostic: bool, machine: bool| {
        let mut args = vec![
            "verify",
            "application-tests",
            "run",
            "--reuse",
            "off",
            path,
            "--project",
            path,
            "--plan",
            "plan.json",
            "--group",
            "native",
            "--recipe",
            "recipe.json",
            "--report",
            ".dctx0ctl/application-tests/report.json",
        ];
        if diagnostic {
            args.push("--no-adopt");
        }
        if machine {
            args.push("--json");
        }
        let output = invoke(&args);
        if machine {
            let json: Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(json["backend"], "application-tests");
        } else {
            assert!(String::from_utf8_lossy(&output).contains("Managed finite-case assessment"));
        }
        serde_json::from_slice::<Value>(
            &fs::read(root.join(".dctx0ctl/application-tests/report.json")).unwrap(),
        )
        .unwrap()
    };
    assert!(
        project
            .verification_gc(false, false, None)
            .unwrap_err()
            .to_string()
            .contains("History preservation hold"),
        "Explicit configuration protects before the first saved native run"
    );
    let diagnostic = run(true, false);
    assert_eq!(diagnostic["result"]["status"], "PASS");
    assert_eq!(diagnostic["limits"]["plan_bytes"], 1048576);
    assert_eq!(diagnostic["adopted_tasks"], json!([]));
    assert_eq!(diagnostic["adoption_authority"], false);
    assert_eq!(
        project.task_session(None, None).unwrap().states[&task].work,
        WorkState::Open
    );
    let claimed = project
        .task_action_selected(
            Action::Start {
                task,
                actor: "another-worker".into(),
                reservation: None,
            },
            Some(&root),
            Some("go"),
        )
        .unwrap()
        .state
        .unwrap();
    let blocked = run(false, true);
    assert_eq!(blocked["adopted_tasks"], json!([]));
    assert_eq!(
        project.task_session(None, None).unwrap().states[&task]
            .actor
            .as_deref(),
        Some("another-worker")
    );
    project
        .task_action_selected(
            Action::Release {
                task,
                actor: "another-worker".into(),
                claim: claimed.claim.unwrap(),
                version: claimed.version,
                reason: "Return the claimed contract for verified delivery".into(),
            },
            Some(&root),
            Some("go"),
        )
        .unwrap();
    let delivered = run(false, true);
    assert_eq!(delivered["adopted_tasks"], json!([task]));
    assert_eq!(delivered["adoption_authority"], true);
    let state = &project.task_session(None, None).unwrap().states[&task];
    assert_eq!(state.work, WorkState::Done);
    assert_eq!(state.check, CheckStatus::Pass);
    // The retention hold never upgrades diagnostic or claimed work. It retains
    // all three actual runs and independently blocks both GC entry points.
    for report in [&diagnostic, &blocked, &delivered] {
        let id = report["evidence"]["id"].as_str().unwrap();
        assert!(project.verification_summary(id).unwrap().details_available);
    }
    for compact in [false, true] {
        assert!(
            project
                .verification_gc(false, compact, None)
                .unwrap_err()
                .to_string()
                .contains("History preservation hold")
        );
    }
    // Retain this migration fixture rather than deleting its evidence.
}
