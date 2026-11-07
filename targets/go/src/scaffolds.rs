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
        .map(|s| serde_json::to_string(s).expect("argument serializes"))
        .collect::<Vec<_>>()
        .join(",");
    vec![
        Artifact {
            path: "contract_testing_test.go".into(),
            contents: include_str!("../harness/testing_test.go").into(),
        },
        Artifact {
            path: "contract_plan_test.go".into(),
            contents: format!(
                "// Editable; published requirements: {provenance}\npackage main\n\nimport (\n\t\"fmt\"\n\t\"testing\"\n)\n\n// Implement setup with actual project fixtures. Register cleanup with t.Cleanup.\nfunc newContractFixture(t *testing.T) (map[string]*string, error) {{\n\tt.Helper()\n\treturn nil, fmt.Errorf(\"production bindings and fixture prerequisites have not been reviewed\")\n}}\n\nfunc TestPublishedContractPlan(t *testing.T) {{\n\tenvironment, e := newContractFixture(t)\n\tif e != nil {{\n\t\tt.Fatal(e)\n\t}}\n\trunContractPlan(t, []string{{{arguments}}}, environment)\n}}\n"
            ),
        },
    ]
}
