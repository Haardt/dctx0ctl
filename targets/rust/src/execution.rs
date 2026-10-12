//! Testing-only Rust artifacts; compiler/process access belongs to the engine.
use dctx0_target_sdk::*;
pub struct RustHarness;
impl HarnessGenerator for RustHarness {
    fn runtime(&self) -> &str {
        "cargo"
    }
    fn inspect_dependencies(
        &self,
        sources: &[InspectionSource],
    ) -> Result<DependencyInspection, Error> {
        crate::RustInspector.dependencies(sources)
    }
    fn generate(&self, request: &HarnessRequest) -> Result<HarnessBundle, Error> {
        if request.adapter_import != "./adapter.rs" {
            return Err(Error::Invalid(
                "Rust harness requires the reviewed sibling adapter.rs".into(),
            ));
        }
        let subjects =
            serde_json::to_string(&request.subjects).map_err(|e| Error::Invalid(e.to_string()))?;
        Ok(HarnessBundle {
            sdk_version: SDK_VERSION,
            generator_version: "dctx0-rust-harness-1".into(),
            target: "rust".into(),
            artifacts: vec![
                Artifact {
                    path: "build-version.txt".into(),
                    contents: "Rust build not initialized".into(),
                },
                Artifact {
                    path: "contract.rs".into(),
                    contents: include_str!("../harness/contract.rs").into(),
                },
                Artifact {
                    path: "worker.rs".into(),
                    contents: include_str!("../harness/worker.rs")
                        .replace("__SUBJECTS__", &format!("{subjects:?}")),
                },
            ],
            build: Some(ExecutionRecipe {
                program: "cargo".into(),
                arguments: vec![
                    "build".into(),
                    "--release".into(),
                    "--offline".into(),
                    "--locked".into(),
                    "--bin".into(),
                    "dctx0-worker".into(),
                    "--target-dir".into(),
                    ".bin/target".into(),
                ],
            }),
            recipe: ExecutionRecipe {
                program: "./.bin/worker".into(),
                arguments: vec![],
            },
        })
    }
    fn scaffold(&self) -> Artifact {
        Artifact {
            path: "adapter.rs".into(),
            contents: include_str!("../harness/adapter.rs").into(),
        }
    }
    fn setup_artifacts(&self) -> Vec<Artifact> {
        vec![Artifact {
            path: "Cargo.toml".into(),
            contents: include_str!("../harness/Cargo.toml").into(),
        }]
    }
}
