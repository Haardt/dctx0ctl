use dctx0_core::{CodecOptions, DataIr};
use std::{collections::BTreeMap, fs, process::Command};
#[test]
fn generated_rust_codecs_execute_with_exact_numbers_and_failures() {
    let data: BTreeMap<String, DataIr> = serde_json::from_str(include_str!(
        "../../../fixtures/conformance/codecs/declarations.json"
    ))
    .unwrap();
    let a = dctx0_target_rust::generate_codecs(&data, &CodecOptions::default(), "pin").unwrap();
    assert_eq!(
        a,
        dctx0_target_rust::generate_codecs(&data, &CodecOptions::default(), "pin").unwrap()
    );
    let root = std::env::temp_dir().join(format!("dctx0-rust-codecs-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname='codec-conformance'\nversion='0.0.0'\nedition='2024'\n[workspace]\n[lib]\npath='lib.rs'\n[dependencies]\nserde_json={version='=1.0.151',features=['arbitrary_precision']}\n").unwrap();
    fs::write(
        root.join("contract.rs"),
        include_str!("../harness/contract.rs"),
    )
    .unwrap();
    fs::write(
        root.join("support.rs"),
        include_str!("../harness/testing.rs"),
    )
    .unwrap();
    fs::write(
        root.join("lib.rs"),
        "pub mod contract;mod support;mod generated;",
    )
    .unwrap();
    fs::write(
        root.join("generated.rs"),
        format!(
            "{}\n{}",
            a.contents,
            include_str!("fixtures/codecs_tests.rs")
        ),
    )
    .unwrap();
    let output = Command::new("cargo")
        .args(["test", "--offline", "--quiet"])
        .env_remove("CARGO_LLVM_COV")
        .env_remove("RUSTFLAGS")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let options = CodecOptions {
        integer: dctx0_core::CodecRepresentation::Bigint,
        ..Default::default()
    };
    assert!(dctx0_target_rust::generate_codecs(&data, &options, "pin").is_err());
}

#[test]
fn rust_type_overrides_require_authored_mappings_for_bigint_and_undefined() {
    use dctx0_core::CodecRepresentation;
    let declarations: BTreeMap<String, DataIr> = serde_json::from_str(include_str!(
        "../../../fixtures/conformance/codecs/declarations.json"
    ))
    .unwrap();
    for (name, representation, supported) in [
        ("W.Id", CodecRepresentation::Number, true),
        ("W.Id", CodecRepresentation::Bigint, false),
        ("W.NestedOption", CodecRepresentation::Undefined, false),
    ] {
        let options = CodecOptions {
            types: BTreeMap::from([(name.into(), representation)]),
            ..Default::default()
        };
        assert!(options.validate(&declarations).is_ok());
        let generated = dctx0_target_rust::generate_codecs(&declarations, &options, "pin");
        assert_eq!(generated.is_ok(), supported, "{name}: {generated:?}");
        if let Err(error) = generated {
            assert!(
                error
                    .to_string()
                    .contains("bigint/undefined need authored mappings")
            );
        }
    }
}
