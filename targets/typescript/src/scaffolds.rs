//! Native test entrypoints over the shared central executor.
use dctx0_core::BindingRequirements;
use dctx0_target_sdk::Artifact;
pub fn generate_test_scaffold(
    requirements: &BindingRequirements,
    arguments: &[String],
) -> Vec<Artifact> {
    let provenance = serde_json::to_string(requirements).expect("requirements serialize");
    let arguments = serde_json::to_string(arguments).expect("arguments serialize");
    vec![
        Artifact {
            path: "contract-testing.ts".into(),
            contents: include_str!("../harness/testing.ts").into(),
        },
        Artifact {
            path: "contract-plan.test.ts".into(),
            contents: format!(
                "// Editable; published requirements: {provenance}\nimport {{test}} from 'bun:test';\nimport {{runPlan,type Fixture}} from './contract-testing.ts';\nasync function newContractFixture():Promise<Fixture>{{throw new Error('Production bindings and fixture prerequisites have not been reviewed');}}\ntest('published contract plan',async()=>{{const fixture=await newContractFixture();try{{await runPlan({arguments},fixture);}}finally{{await fixture.close();}}}});\n"
            ),
        },
    ]
}
