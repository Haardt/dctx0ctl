//! Testing-only Bun library distributed with the CLI, independently versioned
//! from the existing worker SDK. Publishing never changes production imports.
use dctx0_target_sdk::{ApplicationTestLibrary, Artifact};

pub fn application_test_library() -> ApplicationTestLibrary {
    let sources = [
        (
            "package.json",
            include_str!("../applicationtests/package.json"),
        ),
        (
            "protocol.ts",
            include_str!("../applicationtests/protocol.ts"),
        ),
        ("client.ts", include_str!("../applicationtests/client.ts")),
        ("testing.ts", include_str!("../applicationtests/testing.ts")),
        ("values.ts", include_str!("../applicationtests/values.ts")),
        (
            "operation.ts",
            include_str!("../applicationtests/operation.ts"),
        ),
        ("context.ts", include_str!("../applicationtests/context.ts")),
        ("catalog.ts", include_str!("../applicationtests/catalog.ts")),
        ("index.ts", include_str!("../applicationtests/index.ts")),
        ("README.md", include_str!("../applicationtests/README.md")),
    ];
    ApplicationTestLibrary {
        target: "typescript".into(),
        version: "0.2.0".into(),
        protocol_version: dctx0_target_sdk::application_protocol::APPLICATION_PROTOCOL_VERSION,
        artifacts: sources
            .into_iter()
            .map(|(path, contents)| Artifact {
                path: path.into(),
                contents: contents.into(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn shipped_package_retains_its_entrypoint_and_all_local_module_dependencies() {
        let library = application_test_library();
        assert_eq!(library.target, "typescript");
        let files = library
            .artifacts
            .iter()
            .map(|artifact| (artifact.path.as_str(), artifact.contents.as_str()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            files.len(),
            library.artifacts.len(),
            "duplicate distribution artifact"
        );
        let package: serde_json::Value = serde_json::from_str(files["package.json"]).unwrap();
        assert_eq!(package["version"].as_str(), Some(library.version.as_str()));
        assert_eq!(package["private"], true);
        let entry = package["exports"]
            .as_str()
            .unwrap()
            .strip_prefix("./")
            .unwrap();
        assert!(files.contains_key(entry));
        assert!(files.contains_key("catalog.ts"));
        assert!(files.contains_key("README.md"));
        for (name, source) in &files {
            assert!(!name.contains('/') && !name.ends_with(".test.ts"));
            if name.ends_with(".ts") {
                for import in source.split("from '").skip(1) {
                    let path = import.split('\'').next().unwrap();
                    if let Some(path) = path.strip_prefix("./") {
                        assert!(files.contains_key(path), "missing shipped import {path}");
                    }
                }
            }
        }
    }
}
