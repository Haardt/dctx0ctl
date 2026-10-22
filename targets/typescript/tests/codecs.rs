use dctx0_core::*;
use dctx0_target_sdk::HarnessGenerator;
use dctx0_target_typescript::{BunHarness, generate_codecs};
use std::{collections::BTreeMap, fs, path::PathBuf, process::Command};

fn declarations() -> BTreeMap<String, DataIr> {
    let builtin = |name: &str| TypeRef::builtin(name);
    let named = |name: &str| TypeRef {
        name: name.into(),
        symbol_id: Some(name.into()),
        arguments: vec![],
    };
    let generic = |name: &str, argument: TypeRef| TypeRef {
        name: name.into(),
        symbol_id: None,
        arguments: vec![argument],
    };
    let mut data = BTreeMap::new();
    for (id, ty) in [
        ("W.Id", builtin("Int")),
        ("W.Sequence", builtin("Int")),
        ("W.Alias", named("W.Id")),
        ("W.Maybe", generic("Option", builtin("String"))),
        (
            "W.Nested",
            generic("Option", generic("Option", builtin("String"))),
        ),
        ("W.Some", generic("Some", builtin("String"))),
        ("W.List", generic("List", builtin("Int"))),
        ("W.Dctx0Option", generic("Dctx0Option", builtin("String"))),
    ] {
        data.insert(
            id.into(),
            DataIr {
                id: id.into(),
                stable_id: None,
                alias: Some(ty),
                fields: vec![],
                variants: vec![],
                keys: vec![],
            },
        );
    }
    for name in [
        "Unit",
        "Boolean",
        "String",
        "UUID",
        "Timestamp",
        "Decimal",
        "Float",
        "Duration",
        "Bytes",
        "None",
    ] {
        let id = format!("W.{name}");
        data.insert(
            id.clone(),
            DataIr {
                id,
                stable_id: None,
                alias: Some(builtin(name)),
                fields: vec![],
                variants: vec![],
                keys: vec![],
            },
        );
    }
    data.insert(
        "W.Result".into(),
        DataIr {
            id: "W.Result".into(),
            stable_id: None,
            alias: Some(TypeRef {
                name: "Dctx0Result".into(),
                symbol_id: None,
                arguments: vec![builtin("String"), builtin("Int")],
            }),
            fields: vec![],
            variants: vec![],
            keys: vec![],
        },
    );
    for (id, variants) in [
        (
            "W.Status",
            vec![
                VariantIr {
                    name: "Open".into(),
                    payload: None,
                },
                VariantIr {
                    name: "Closed".into(),
                    payload: None,
                },
            ],
        ),
        (
            "W.Choice",
            vec![
                VariantIr {
                    name: "Empty".into(),
                    payload: None,
                },
                VariantIr {
                    name: "Count".into(),
                    payload: Some(builtin("Int")),
                },
            ],
        ),
    ] {
        data.insert(
            id.into(),
            DataIr {
                id: id.into(),
                stable_id: None,
                alias: None,
                fields: vec![],
                variants,
                keys: vec![],
            },
        );
    }
    for (id, fields) in [
        (
            "W.Packet",
            vec![
                FieldIr {
                    name: "id".into(),
                    ty: named("W.Id"),
                },
                FieldIr {
                    name: "status".into(),
                    ty: named("W.Status"),
                },
                FieldIr {
                    name: "maybe".into(),
                    ty: named("W.Maybe"),
                },
            ],
        ),
        (
            "W.Tree",
            vec![FieldIr {
                name: "children".into(),
                ty: generic("List", named("W.Tree")),
            }],
        ),
    ] {
        data.insert(
            id.into(),
            DataIr {
                id: id.into(),
                stable_id: None,
                alias: None,
                fields,
                variants: vec![],
                keys: vec![],
            },
        );
    }
    data
}

#[test]
fn selection_closure_and_explicit_representations_reject_invalid_configuration() {
    let data = declarations();
    let ir = WorkspaceIr {
        schema_version: 1,
        compiler_version: "test".into(),
        workspace: "W".into(),
        symbols: BTreeMap::new(),
        data: data.clone(),
        operations: BTreeMap::new(),
        nodes: vec![],
        references: vec![],
    };
    assert_eq!(codec_declarations(&ir, &Default::default()).unwrap(), data);
    assert_eq!(codec_declarations(&ir, &["W".into()].into()).unwrap(), data);
    let selected = codec_declarations(&ir, &["W.Packet".into()].into()).unwrap();
    assert_eq!(selected.len(), 4);
    assert!(!selected.contains_key("W.Sequence"));
    assert_eq!(
        codec_declarations(&ir, &["W.Tree".into()].into())
            .unwrap()
            .len(),
        1
    );
    assert!(codec_declarations(&ir, &["Missing".into()].into()).is_err());
    let mut broken = ir.clone();
    broken.data.remove("W.Id");
    assert!(codec_declarations(&broken, &["W.Packet".into()].into()).is_err());
    let mut options = CodecOptions::default();
    for (name, rep) in [
        ("W.Id", CodecRepresentation::Bigint),
        ("W.Alias", CodecRepresentation::Number),
        ("W.Maybe", CodecRepresentation::Undefined),
        ("W.Status", CodecRepresentation::Tagged),
    ] {
        options.types.insert(name.into(), rep);
    }
    options.validate(&data).unwrap();
    for (name, rep) in [
        ("Missing", CodecRepresentation::Number),
        ("W.Packet", CodecRepresentation::Number),
        ("W.Id", CodecRepresentation::String),
        ("W.Maybe", CodecRepresentation::Number),
        ("W.Status", CodecRepresentation::Number),
        ("W.Choice", CodecRepresentation::String),
        ("W.Float", CodecRepresentation::Bigint),
    ] {
        let mut invalid = options.clone();
        invalid.types.insert(name.into(), rep);
        assert!(invalid.validate(&data).is_err(), "{name}");
    }
    for field in ["schema_version", "integer", "option", "enum"] {
        let mut value = serde_json::to_value(&options).unwrap();
        value[field] = if field == "schema_version" {
            serde_json::json!(2)
        } else {
            serde_json::json!("tagged")
        };
        if field == "enum" {
            value[field] = serde_json::json!("bigint");
        }
        let invalid: CodecOptions = serde_json::from_value(value).unwrap();
        assert!(generate_codecs(&data, &invalid, "snapshot").is_err());
    }
    let mut cycle = data.clone();
    cycle.get_mut("W.Id").unwrap().alias = Some(TypeRef {
        name: "W.Alias".into(),
        symbol_id: Some("W.Alias".into()),
        arguments: vec![],
    });
    assert!(options.validate(&cycle).is_err());
    let json = serde_json::to_string(&CodecOptions::default()).unwrap();
    assert!(
        serde_json::from_str::<CodecOptions>(&json.replace(
            "\"types\":{}",
            "\"types\":{\"W.Id\":\"number\",\"W.Id\":\"bigint\"}"
        ))
        .is_err()
    );
    assert!(
        serde_json::from_str::<CodecOptions>(&json.replace("\"types\":{}", "\"types\":[]"))
            .is_err()
    );
    assert!(
        serde_json::from_str::<CodecOptions>(&json.replace(
            "\"schema_version\":1",
            "\"schema_version\":1,\"extra\":true"
        ))
        .is_err()
    );
}

#[test]
fn generated_codecs_execute_independent_native_wire_pairs_and_typecheck() {
    let root =
        std::env::temp_dir().join(format!("dctx0-codecs-conformance-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let data = declarations();
    let defaults = CodecOptions::default();
    let mut configured = CodecOptions {
        integer: CodecRepresentation::Bigint,
        option: CodecRepresentation::Undefined,
        r#enum: CodecRepresentation::Tagged,
        ..Default::default()
    };
    configured
        .types
        .insert("W.Id".into(), CodecRepresentation::Number);
    configured
        .types
        .insert("W.Alias".into(), CodecRepresentation::Bigint);
    configured
        .types
        .insert("W.Maybe".into(), CodecRepresentation::Null);
    configured
        .types
        .insert("W.Status".into(), CodecRepresentation::String);
    for (name, options) in [("codecs.ts", defaults), ("configured.ts", configured)] {
        let artifact = generate_codecs(&data, &options, "snapshot\n\"safe").unwrap();
        assert_eq!(
            artifact,
            generate_codecs(&data, &options, "snapshot\n\"safe").unwrap()
        );
        fs::write(root.join(name), artifact.contents).unwrap();
    }
    fs::write(
        root.join("contract.ts"),
        BunHarness
            .generate(&dctx0_target_sdk::HarnessRequest {
                adapter_import: "./adapter.ts".into(),
                subjects: Default::default(),
            })
            .unwrap()
            .artifacts[0]
            .contents
            .clone(),
    )
    .unwrap();
    fs::write(
        root.join("codecs.test.ts"),
        include_str!("fixtures/codecs.test.ts"),
    )
    .unwrap();
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let compiler_started = std::time::Instant::now();
    let typecheck = Command::new("bun")
        .arg(workspace.join("examples/cinema/node_modules/typescript/bin/tsc"))
        .args([
            "--noEmit",
            "--strict",
            "--target",
            "ES2022",
            "--module",
            "ESNext",
            "--moduleResolution",
            "Bundler",
            "--allowImportingTsExtensions",
            "--skipLibCheck",
            "--ignoreDeprecations",
            "6.0",
            "codecs.ts",
            "configured.ts",
            "contract.ts",
        ])
        .current_dir(&root)
        .output()
        .expect("Bun and the installed Cinema TypeScript compiler are required");
    let compiler_elapsed_ms = compiler_started.elapsed().as_secs_f64() * 1000.0;
    assert!(
        typecheck.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&typecheck.stdout),
        String::from_utf8_lossy(&typecheck.stderr)
    );
    let report = workspace
        .join("target/codec-conformance")
        .join(std::process::id().to_string());
    fs::create_dir_all(&report).unwrap();
    let output = Command::new("bun")
        .args([
            "test",
            "--coverage",
            "--coverage-reporter=lcov",
            "--coverage-dir",
        ])
        .arg(&report)
        .arg("codecs.test.ts")
        .current_dir(&root)
        .output()
        .expect("Bun is required for codec conformance");
    fs::write(report.join("stdout.log"), &output.stdout).unwrap();
    fs::write(report.join("stderr.log"), &output.stderr).unwrap();
    let helper_report = root.join("native-performance.json");
    if helper_report.exists() {
        let mut metrics: serde_json::Value =
            serde_json::from_slice(&fs::read(helper_report).unwrap()).unwrap();
        metrics["compiler"] = serde_json::json!({"elapsed_ms":compiler_elapsed_ms,"peak_rss_bytes":null,"memory_scope":"Compiler peak RSS unavailable from portable Rust Command; outer validation watchdog measures the aggregate tree separately"});
        metrics["status"] = serde_json::json!(if output.status.success() {
            "pass"
        } else {
            "fail"
        });
        fs::write(
            report.join("performance.json"),
            serde_json::to_vec_pretty(&metrics).unwrap(),
        )
        .unwrap();
    }

    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
