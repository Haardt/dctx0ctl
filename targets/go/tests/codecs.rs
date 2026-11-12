use dctx0_core::{CodecOptions, DataIr};
use dctx0_target_go::{GoHarness, generate_codecs};
use dctx0_target_sdk::HarnessGenerator;
use std::{collections::BTreeMap, fs, process::Command};
#[test]
fn generated_go_codecs_execute_independent_values_and_failures() {
    let declarations: BTreeMap<String, DataIr> = serde_json::from_str(include_str!(
        "../../../fixtures/conformance/codecs/declarations.json"
    ))
    .unwrap();
    let artifact = generate_codecs(&declarations, &CodecOptions::default(), "pin").unwrap();
    assert_eq!(
        artifact,
        generate_codecs(&declarations, &CodecOptions::default(), "pin").unwrap()
    );
    let root = std::env::temp_dir().join(format!("dctx0-go-codecs-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("go.mod"), "module codecs\n\ngo 1.25.0\n").unwrap();
    let bundle = GoHarness
        .generate(&dctx0_target_sdk::HarnessRequest {
            adapter_import: "./adapter.go".into(),
            subjects: Default::default(),
        })
        .unwrap();
    fs::write(
        root.join("contract.go"),
        &bundle
            .artifacts
            .iter()
            .find(|a| a.path == "contract.go")
            .unwrap()
            .contents,
    )
    .unwrap();
    fs::write(root.join(artifact.path), artifact.contents).unwrap();
    fs::write(
        root.join("testing_test.go"),
        include_str!("../harness/testing_test.go"),
    )
    .unwrap();
    fs::write(
        root.join("codecs_test.go"),
        include_str!("fixtures/codecs_test.go"),
    )
    .unwrap();
    let output = Command::new("go")
        .args([
            "test",
            "-count=1",
            "-timeout=30s",
            "-coverprofile=coverage.out",
            ".",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let unsupported = CodecOptions {
        option: dctx0_core::CodecRepresentation::Undefined,
        ..Default::default()
    };
    assert!(generate_codecs(&declarations, &unsupported, "pin").is_err());
}

#[test]
fn go_type_overrides_accept_native_numbers_and_reject_undefined_options() {
    use dctx0_core::CodecRepresentation;
    let declarations: BTreeMap<String, DataIr> = serde_json::from_str(include_str!(
        "../../../fixtures/conformance/codecs/declarations.json"
    ))
    .unwrap();
    for (name, representation, supported) in [
        ("W.Id", CodecRepresentation::Number, true),
        ("W.NestedOption", CodecRepresentation::Undefined, false),
    ] {
        let options = CodecOptions {
            types: BTreeMap::from([(name.into(), representation)]),
            ..Default::default()
        };
        assert!(options.validate(&declarations).is_ok());
        let generated = generate_codecs(&declarations, &options, "pin");
        assert_eq!(generated.is_ok(), supported, "{name}: {generated:?}");
        if let Err(error) = generated {
            assert!(
                error
                    .to_string()
                    .contains("undefined has no Go representation")
            );
        }
    }
}
