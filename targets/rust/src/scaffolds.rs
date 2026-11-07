//! Native test entrypoints over the shared central executor.
use dctx0_core::BindingRequirements;
use dctx0_target_sdk::Artifact;
pub fn generate_test_scaffold(
    requirements: &BindingRequirements,
    arguments: &[String],
) -> Vec<Artifact> {
    let provenance = serde_json::to_string(requirements).expect("requirements serialize");
    let arguments = arguments
        .iter()
        .map(|s| format!("{s:?}"))
        .collect::<Vec<_>>()
        .join(",");
    vec![
        Artifact {
            path: "contract_testing.rs".into(),
            contents: include_str!("../harness/testing.rs").into(),
        },
        Artifact {
            path: "contract_plan_tests.rs".into(),
            contents: format!(
                "// Editable; published requirements: {provenance}\n// Include from adapter.rs with #[cfg(test)] mod contract_plan_tests;\n#[path=\"contract_testing.rs\"] mod support;\nfn new_contract_fixture()->Result<support::Fixture,String>{{Err(\"production bindings and fixture prerequisites have not been reviewed\".into())}}\n#[test] fn published_contract_plan(){{let fixture=new_contract_fixture().expect(\"authored fixture\");support::run_plan(&[{arguments}],&fixture).expect(\"central assessment\");}}\n"
            ),
        },
    ]
}
