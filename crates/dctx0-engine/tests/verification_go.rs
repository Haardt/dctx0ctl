//! Actual Go build/execution through the shared engine, including failure evidence.
use dctx0_core::VerificationResult;
use dctx0_engine::*;
use dctx0_target_go::GoHarness;
use std::fs;

#[tokio::test]
async fn go_builds_real_application_records_exact_results_and_preserves_failed_build_cases() {
    let root = std::env::temp_dir().join(format!("dctx0-engine-go-{}", std::process::id()));
    let app = root.join("app");
    let spec = root.join("spec");
    fs::create_dir_all(app.join("production")).unwrap();
    fs::create_dir_all(&spec).unwrap();
    fs::write(spec.join("w.dctx0"), "workspace W { surface Api { operation next(input:Int) -> Int {\n pure\n ensures result == input + 1\n } } }").unwrap();
    fs::write(app.join("go.mod"), "module fixture\n\ngo 1.25.0\n").unwrap();
    fs::write(app.join("production/next.go"), "package production\nimport \"math/big\"\nfunc Next(n *big.Int) *big.Int { return new(big.Int).Add(n,big.NewInt(1)) }\n").unwrap();
    let project = Project::new(&spec, None).unwrap();
    assert!(project.check().unwrap().ok);
    project
        .initialize_verification(
            &app,
            &GoHarness,
            ConfiguredVerificationScope {
                subjects: ["W.Api.next".into()].into(),
                tasks: Default::default(),
            },
        )
        .unwrap();
    let adapter = r#"package main
import("encoding/json";"math/big";"fixture/production")
type actualAdapter struct{}
type actualSession struct{}
var adapter ProjectAdapter = actualAdapter{}
func(actualAdapter)Describe()(AdapterDescription,error){return AdapterDescription{ProtocolVersion:1,Subjects:[]SubjectDescription{{Subject:"W.Api.next",Role:"operation",Artifact:"production/next.go",EntryPoint:"production.Next"}},Artifacts:[]string{"production/next.go"},Capabilities:[]string{"sessions","invocation","events"},Representations:map[string]string{"Int":"math/big.Int"}},nil}
func(actualAdapter)Begin(string,TestContext)(TestSession,error){return actualSession{},nil}
func(actualSession)Close()error{return nil}
func(actualSession)Invoke(invocation Invocation)(Observation,error){var digits string;if err:=json.Unmarshal(invocation.Arguments[0].Value,&digits);err!=nil{return Observation{},err};n,_:=new(big.Int).SetString(digits,10);result:=production.Next(n);raw,_:=json.Marshal(result.String());v:=Value{Kind:"integer",Value:raw};return Observation{Kind:"returned",Value:&v},nil}
"#;
    fs::write(app.join(".dctx0/testing/go/adapter.go"), adapter).unwrap();
    let report = project
        .run_verification(
            Some(&app),
            Default::default(),
            &GoHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        report.evidence.run.result,
        VerificationResult::Pass,
        "{:?}",
        report.evidence.run.provenance.incomplete
    );
    assert!(!report.evidence.run.cases.is_empty());
    assert!(report.evidence.run.provenance.incomplete.is_empty());
    assert_eq!(report.evidence.run.provenance.target, "go");
    assert!(
        report
            .evidence
            .run
            .cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
    );
    fs::write(
        app.join("production/next.go"),
        "package production\nfunc Broken(\n",
    )
    .unwrap();
    let failed = project
        .run_verification(
            Some(&app),
            Default::default(),
            &GoHarness,
            &NativeCancellation::default(),
            true,
        )
        .await
        .unwrap();
    assert!(matches!(
        failed.evidence.run.result,
        VerificationResult::Unknown { .. }
    ));
    assert!(!failed.evidence.run.required_cases.is_empty());
    assert!(failed.evidence.run.cases.is_empty());
    assert!(
        failed
            .evidence
            .run
            .provenance
            .incomplete
            .iter()
            .any(|issue| issue.code == "NATIVE_BUILD")
    );
    assert!(failed.adopted_tasks.is_empty());
    fs::remove_dir_all(root).unwrap();
}
