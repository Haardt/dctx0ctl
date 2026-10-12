#![cfg(feature = "go")]
//! Go CLI registration, source discovery and native execution without Bun selection.
use std::{fs, process::Command};
fn cli(args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dctx0"))
        .arg("--json")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn go_cli_selects_and_executes_the_real_go_target() {
    let root = std::env::temp_dir().join(format!("dctx0-go-cli-{}", std::process::id()));
    fs::create_dir_all(root.join("dctx0")).unwrap();
    fs::create_dir_all(root.join("production")).unwrap();
    fs::write(root.join("dctx0/w.dctx0"),"workspace W { surface Api { operation next(input:Int) -> Int {\n pure\n ensures result == input + 1\n } } }").unwrap();
    fs::write(root.join("go.mod"), "module fixture\n\ngo 1.25.0\n").unwrap();
    fs::write(root.join("production/next.go"),"package production\nimport \"math/big\"\nfunc Next(n *big.Int)*big.Int{return new(big.Int).Add(n,big.NewInt(1))}\n").unwrap();
    let spec = root.join("dctx0");
    cli(&["check", spec.to_str().unwrap()]);
    let setup = cli(&[
        "verify",
        "init",
        spec.to_str().unwrap(),
        "--project",
        root.to_str().unwrap(),
        "--target",
        "go",
        "--subject",
        "W.Api.next",
    ]);
    assert_eq!(setup["configuration"]["target"], "go");
    let discovery = cli(&[
        "verify",
        "discover",
        spec.to_str().unwrap(),
        "--project",
        root.to_str().unwrap(),
        "--target",
        "go",
    ]);
    assert_eq!(discovery["inspection"]["target"], "go");
    assert!(
        !discovery["inspection"]["sources"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    fs::write(root.join(".dctx0/testing/go/adapter.go"),r#"package main
import("encoding/json";"math/big";"fixture/production")
type actualAdapter struct{}
type actualSession struct{}
var adapter ProjectAdapter = actualAdapter{}
func(actualAdapter)Describe()(AdapterDescription,error){return AdapterDescription{ProtocolVersion:1,Subjects:[]SubjectDescription{{Subject:"W.Api.next",Role:"operation",Artifact:"production/next.go",EntryPoint:"production.Next"}},Artifacts:[]string{"production/next.go"},Capabilities:[]string{"sessions","invocation","events"},Representations:map[string]string{"Int":"math/big.Int"}},nil}
func(actualAdapter)Begin(string,TestContext)(TestSession,error){return actualSession{},nil}
func(actualSession)Close()error{return nil}
func(actualSession)Invoke(invocation Invocation)(Observation,error){var digits string;if err:=json.Unmarshal(invocation.Arguments[0].Value,&digits);err!=nil{return Observation{},err};n,_:=new(big.Int).SetString(digits,10);result:=production.Next(n);raw,_:=json.Marshal(result.String());v:=Value{Kind:"integer",Value:raw};return Observation{Kind:"returned",Value:&v},nil}
"#).unwrap();
    let result = cli(&[
        "verify",
        "run",
        spec.to_str().unwrap(),
        "--project",
        root.to_str().unwrap(),
        "--no-adopt",
    ]);
    assert_eq!(result["evidence"]["run"]["result"]["status"], "PASS");
    assert_eq!(result["evidence"]["run"]["provenance"]["target"], "go");
    assert!(
        result["evidence"]["run"]["provenance"]["incomplete"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    fs::remove_dir_all(root).unwrap();
}
