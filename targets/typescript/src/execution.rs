//! Bun transport artifacts only. Application code and semantic decisions stay external.
use dctx0_target_sdk::*;
pub struct BunHarness;
impl HarnessGenerator for BunHarness {
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
            generator_version: "dctx0-bun-harness-1".into(),
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
