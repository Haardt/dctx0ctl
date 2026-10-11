//! Bun transport artifacts only. Application code and semantic decisions stay external.
use dctx0_target_sdk::*;
pub struct BunHarness;
impl HarnessGenerator for BunHarness {
    fn inspect_dependencies(
        &self,
        sources: &[InspectionSource],
    ) -> Result<DependencyInspection, Error> {
        crate::TypeScriptDependencies.dependencies(sources)
    }
    fn module_observer_artifacts(
        &self,
        sources: &[InspectionSource],
        project_root_relative: &str,
    ) -> Result<Vec<Artifact>, Error> {
        let manifest = crate::dependencies::observer_manifest(sources, project_root_relative)?;
        if manifest.sources.is_empty() {
            return Ok(vec![]);
        }
        Ok(vec![Artifact {
            path: "module-observer.json".into(),
            contents: serde_json::to_string(&manifest)
                .map_err(|error| Error::Invalid(error.to_string()))?,
        }])
    }
    fn generate(&self, request: &HarnessRequest) -> Result<HarnessBundle, Error> {
        let path = &request.adapter_import;
        if !path.starts_with("./")
            || path.contains('\\')
            || path.chars().any(char::is_control)
            || path.split('/').any(|part| part == ".." || part.is_empty())
            || !path.ends_with(".ts")
        {
            return Err(Error::Invalid(
                "Adapter import must be a relative .ts path without escaping segments".into(),
            ));
        }
        let (prefix, rest) = include_str!("../harness/worker.ts")
            .split_once("__ADAPTER_IMPORT__")
            .expect("worker template has adapter slot");
        let (middle, suffix) = rest
            .split_once("__SUBJECTS__")
            .expect("worker template has subjects slot");
        let adapter = serde_json::to_string(path).expect("strings serialize");
        let subjects = serde_json::to_string(&request.subjects).expect("subject strings serialize");
        let worker = format!("{prefix}{adapter}{middle}{subjects}{suffix}");
        Ok(HarnessBundle {
            sdk_version: SDK_VERSION,
            generator_version: "dctx0-bun-harness-3".into(),
            target: "typescript".into(),
            artifacts: vec![
                Artifact {
                    path: "contract.ts".into(),
                    contents: include_str!("../harness/contract.ts").into(),
                },
                Artifact {
                    path: "worker.ts".into(),
                    contents: worker,
                },
                Artifact {
                    path: "module-observer.ts".into(),
                    contents: include_str!("../harness/module-observer.ts").into(),
                },
            ],
            recipe: ExecutionRecipe {
                program: "bun".into(),
                arguments: vec!["run".into(), "worker.ts".into()],
            },
        })
    }
    fn scaffold(&self) -> Artifact {
        Artifact {
            path: "adapter.ts".into(),
            contents: include_str!("../harness/adapter.ts").into(),
        }
    }
}
