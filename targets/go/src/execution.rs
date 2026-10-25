//! Generated Go worker transport; application code and semantic decisions stay external.
use dctx0_target_sdk::*;

const SUBJECT_SLOT: &str = "__SUBJECTS__";

pub struct GoHarness;

impl HarnessGenerator for GoHarness {
    fn dependency_languages(&self) -> &'static [InspectionLanguage] {
        &[InspectionLanguage::Go]
    }
    fn runtime(&self) -> &str {
        "go"
    }
    fn inspect_dependencies(
        &self,
        sources: &[InspectionSource],
    ) -> Result<DependencyInspection, Error> {
        crate::GoInspector.dependencies(sources)
    }
    fn generate(&self, request: &HarnessRequest) -> Result<HarnessBundle, Error> {
        sibling_adapter(&request.adapter_import)?;
        let mut subjects = String::new();
        for (index, subject) in request.subjects.iter().enumerate() {
            if index > 0 {
                subjects.push(',');
            }
            subjects.push_str(&serde_json::to_string(subject).expect("strings serialize"));
            subjects.push_str(": true");
        }
        let worker = include_str!("../harness/worker.go").replace(SUBJECT_SLOT, &subjects);
        Ok(HarnessBundle {
            sdk_version: SDK_VERSION,
            generator_version: "dctx0-go-harness-3".into(),
            target: "go".into(),
            build: Some(ExecutionRecipe {
                program: "go".into(),
                arguments: vec!["run".into(), "build.go".into()],
            }),
            artifacts: vec![
                Artifact {
                    path: "contract.go".into(),
                    contents: include_str!("../harness/contract.go").into(),
                },
                Artifact {
                    path: "worker.go".into(),
                    contents: worker,
                },
                Artifact {
                    path: "build.go".into(),
                    contents: include_str!("../harness/build.go").into(),
                },
            ],
            recipe: ExecutionRecipe {
                program: format!("./.bin/worker{}", std::env::consts::EXE_SUFFIX),
                arguments: vec![],
            },
        })
    }
    fn scaffold(&self) -> Artifact {
        Artifact {
            path: "adapter.go".into(),
            contents: include_str!("../harness/adapter.go").into(),
        }
    }
}

/// The harness compiles one directory as a package; adapters must be siblings.
fn sibling_adapter(import: &str) -> Result<&str, Error> {
    let invalid = || Error::Invalid("Adapter must be a relative sibling .go file".into());
    let name = import
        .strip_prefix("./")
        .filter(|rest| !rest.is_empty())
        .ok_or_else(invalid)?;
    let base = name
        .strip_suffix(".go")
        .filter(|base| !base.is_empty())
        .ok_or_else(invalid)?;
    if base.contains('/') || base.contains('\\') || base.chars().any(char::is_control) {
        return Err(invalid());
    }
    if base == "contract" || base == "worker" {
        return Err(Error::Invalid(
            "Adapter must not replace generated harness files".into(),
        ));
    }
    Ok(name)
}
