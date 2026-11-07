use dctx0_core::CodecOptions;
use dctx0_engine::*;
use dctx0_target_sdk::Artifact;
use std::{collections::BTreeSet, fs};

#[test]
fn scaffolds_require_current_published_sources_and_never_overwrite_or_escape() {
    let root = std::env::temp_dir().join(format!("dctx0-codec-scaffold-{}", std::process::id()));
    let spec = root.join("spec");
    let app = root.join("app");
    fs::create_dir_all(&spec).unwrap();
    fs::create_dir_all(&app).unwrap();
    fs::write(spec.join("w.dctx0"), "workspace W { value Id = Int }").unwrap();
    let project = Project::new(&spec, None).unwrap();
    assert!(project.codec_inputs(&BTreeSet::new()).is_err());
    project.check().unwrap();
    let inputs = project.codec_inputs(&["W".into()].into()).unwrap();
    assert_eq!(inputs.declarations.len(), 1);
    assert!(project.codec_inputs(&["Missing".into()].into()).is_err());
    let database = fs::read(&project.database).unwrap();
    let artifact = Artifact {
        path: "wire-codecs.ts".into(),
        contents: "// proposed".into(),
    };
    let publish = |name, dry| {
        publish_codec_scaffold(
            &app,
            name,
            artifact.clone(),
            &inputs,
            CodecOptions::default(),
            "test-generator",
            dry,
        )
    };
    assert_eq!(publish("wire-codecs.ts", true).unwrap().status, "proposed");
    assert!(!app.join(".dctx0").exists());
    assert_eq!(publish("wire-codecs.ts", false).unwrap().status, "created");
    fs::write(
        app.join(".dctx0/testing/typescript/wire-codecs.ts"),
        "// edited",
    )
    .unwrap();
    assert_eq!(publish("wire-codecs.ts", true).unwrap().status, "preserved");
    assert_eq!(
        publish("wire-codecs.ts", false).unwrap().status,
        "preserved"
    );
    assert_eq!(
        fs::read_to_string(app.join(".dctx0/testing/typescript/wire-codecs.ts")).unwrap(),
        "// edited"
    );
    assert_eq!(publish("candidate.ts", false).unwrap().status, "created");
    for name in [
        "",
        "../escape.ts",
        "/escape.ts",
        "sub/codec.ts",
        "sub\\codec.ts",
        "x\n.ts",
        "x.js",
        "contract.ts",
        "adapter.ts",
        "worker.ts",
        "module-observer.ts",
    ] {
        assert!(publish(name, true).is_err(), "{name}");
    }
    for (target, extension) in [("go", "go"), ("rust", "rs")] {
        let filename = format!("generated.{extension}");
        let publish = |name: &str, dry| {
            publish_target_codec_scaffold(
                &app,
                target,
                name,
                artifact.clone(),
                &inputs,
                CodecOptions::default(),
                "test",
                dry,
            )
        };
        assert_eq!(publish(&filename, true).unwrap().status, "proposed");
        assert!(!app.join(format!(".dctx0/testing/{target}")).exists());
        assert_eq!(publish(&filename, false).unwrap().status, "created");
        let path = app.join(format!(".dctx0/testing/{target}/{filename}"));
        fs::write(&path, "authored edits").unwrap();
        assert_eq!(publish(&filename, false).unwrap().status, "preserved");
        assert_eq!(fs::read_to_string(&path).unwrap(), "authored edits");
        assert_eq!(
            publish(&format!("candidate.{filename}.txt"), false)
                .unwrap()
                .status,
            "created"
        );
        for protected in [
            format!("adapter.{extension}"),
            format!("contract.{extension}"),
            format!("../escape.{extension}"),
        ] {
            assert!(publish(&protected, false).is_err());
        }
    }
    assert_eq!(database, fs::read(&project.database).unwrap());
    fs::write(
        spec.join("w.dctx0"),
        "workspace W { value Id = Int }\n// drift",
    )
    .unwrap();
    assert!(
        project
            .codec_inputs(&BTreeSet::new())
            .unwrap_err()
            .to_string()
            .contains("run check")
    );
    fs::remove_dir_all(root).unwrap();
}
