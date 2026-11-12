//! Real Bun legacy-worker regression for Float constructor equality.
use dctx0_core::{VerificationResult, VerificationValue};
use dctx0_engine::{ConfiguredVerificationScope, NativeCancellation, Project};
use dctx0_target_typescript::BunHarness;
use std::fs;

#[tokio::test]
async fn legacy_bun_float_constructor_compares_modeled_values_and_keeps_actual_wire_text() {
    let root =
        std::env::temp_dir().join(format!("dctx0-constructor-equality-{}", std::process::id()));
    let spec = root.join("spec");
    let app = root.join("app");
    fs::create_dir_all(&spec).unwrap();
    fs::create_dir_all(&app).unwrap();
    fs::write(spec.join("w.dctx0"), "workspace W { value X = Float }").unwrap();
    let project = Project::new(&spec, None).unwrap();
    assert!(project.check().unwrap().ok);
    project
        .initialize_verification(
            &app,
            &BunHarness,
            ConfiguredVerificationScope {
                subjects: ["W.X".into()].into(),
                tasks: Default::default(),
            },
        )
        .unwrap();
    fs::write(app.join("app.ts"), "export function factory(v:number){if(!Number.isFinite(v))throw Error('invalid');return v;}").unwrap();
    fs::write(app.join(".dctx0/testing/typescript/adapter.ts"), r#"import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.X',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Float:'finite IEEE-754'}}},async begin(){return {async invoke(){throw Error('unsupported')},async validate(_subject,value){try {if(value.kind!=='float')throw Error('invalid');const result=factory(Number(value.value));return {kind:'accepted',value:{kind:'float',value:String(result)}}}catch(error){return {kind:'rejected',reason:String(error)}}},async close(){}}}};"#).unwrap();
    let cancellation = NativeCancellation::default();
    let run = || {
        project.run_verification(
            Some(&app),
            Default::default(),
            &BunHarness,
            &cancellation,
            true,
        )
    };
    let first = run().await.unwrap();
    let relevant: Vec<_> = first
        .evidence
        .run
        .cases
        .iter()
        .filter(|case| {
            case.subject == "W.X"
                && case.bindings.get("expect_rejection") == Some(&VerificationValue::Boolean(false))
        })
        .collect();
    assert!(!relevant.is_empty());
    assert!(
        relevant
            .iter()
            .all(|case| case.result == VerificationResult::Pass),
        "{:?}",
        first.evidence.run.result
    );
    assert!(
        relevant
            .iter()
            .any(|case| case.bindings.get("value") != case.bindings.get("native_value")),
        "fixture must cover distinct wire spellings"
    );
    fs::write(app.join("app.ts"), "export function factory(v:number){if(!Number.isFinite(v))throw Error('invalid');return 0;}").unwrap();
    let changed = run().await.unwrap();
    assert!(changed.evidence.run.cases.iter().any(|case| matches!(&case.result, VerificationResult::Fail { issue } if issue.code == "NATIVE_VALUE_CHANGED")));
}
