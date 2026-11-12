//! Released testing library; production never imports generated test artifacts.
use dctx0_target_sdk::{ApplicationTestLibrary, Artifact, application_protocol::*};

pub fn application_test_library() -> ApplicationTestLibrary {
    ApplicationTestLibrary {
        target: "go".into(),
        version: GO_APPLICATION_LIBRARY_VERSION.into(),
        protocol_version: APPLICATION_PROTOCOL_VERSION,
        artifacts: [
            ("go.mod", include_str!("../applicationtests/go.mod")),
            ("client.go", include_str!("../applicationtests/client.go")),
            ("decoder.go", include_str!("../applicationtests/decoder.go")),
            (
                "operation.go",
                include_str!("../applicationtests/operation.go"),
            ),
            (
                "protocol.go",
                include_str!("../applicationtests/protocol.go"),
            ),
        ]
        .into_iter()
        .map(|(path, contents)| Artifact {
            path: path.into(),
            contents: contents.into(),
        })
        .collect(),
    }
}
