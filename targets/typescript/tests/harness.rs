use dctx0_target_sdk::*;
use dctx0_target_typescript::BunHarness;
use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};
#[test]
fn generated_worker_is_deterministic_scaffolds_are_unavailable_and_imports_cannot_escape() {
    let request = HarnessRequest {
        adapter_import: "./adapter.ts".into(),
        subjects: BTreeSet::from([
            "operation".into(),
            "value".into(),
            "Entity".into(),
            "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner".into(),
        ]),
    };
    let bundle = BunHarness.generate(&request).unwrap();
    assert_eq!(bundle, BunHarness.generate(&request).unwrap());
    assert_eq!(bundle.recipe.program, "bun");
    assert_eq!(bundle.sdk_version, SDK_VERSION);
    assert_eq!(bundle.artifacts.len(), 3);
    assert!(!bundle.artifacts[1].contents.contains("__ADAPTER_IMPORT__"));
    let scaffold = BunHarness.scaffold();
    assert!(scaffold.contents.contains("not implemented"));
    for path in [
        "adapter.ts",
        "/absolute.ts",
        "./../escape.ts",
        "./folder/../../escape.ts",
        "./a\\b.ts",
        "./a\nb.ts",
        "./folder//adapter.ts",
        "./adapter.js",
    ] {
        let mut invalid = request.clone();
        invalid.adapter_import = path.into();
        assert!(
            BunHarness
                .generate(&invalid)
                .unwrap_err()
                .to_string()
                .contains("Harness input")
        );
    }
    let mut slot = request.clone();
    slot.adapter_import = "./__SUBJECTS__.ts".into();
    slot.subjects.insert("__ADAPTER_IMPORT__".into());
    let generated = BunHarness.generate(&slot).unwrap();
    assert!(
        generated.artifacts[1]
            .contents
            .contains("\"./__SUBJECTS__.ts\"")
    );
    assert!(
        generated.artifacts[1]
            .contents
            .contains("\"__ADAPTER_IMPORT__\"")
    );
    let mut escaped = request.clone();
    escaped.adapter_import = "./quote\"adapter.ts".into();
    assert!(
        BunHarness.generate(&escaped).unwrap().artifacts[1]
            .contents
            .contains("quote\\\"adapter.ts")
    );
}
#[test]
fn real_bun_worker_conforms_to_sessions_observation_mock_transport_and_failures() {
    let root = std::env::temp_dir().join(format!("dctx0-bun-worker-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let bundle = BunHarness
        .generate(&HarnessRequest {
            adapter_import: "./adapter.ts".into(),
            subjects: BTreeSet::from([
                "operation".into(),
                "value".into(),
                "Entity".into(),
                "HomeCinema.MovieNight.ScreeningPlanning.VotingRules.candidateOrderWinner".into(),
            ]),
        })
        .unwrap();
    for artifact in &bundle.artifacts {
        fs::write(root.join(&artifact.path), &artifact.contents).unwrap();
    }
    fs::write(root.join("counter.ts"), include_str!("fixtures/counter.ts")).unwrap();
    fs::write(root.join("adapter.ts"), include_str!("fixtures/adapter.ts")).unwrap();
    fs::write(root.join("scaffold.ts"), BunHarness.scaffold().contents).unwrap();
    let scaffold = BunHarness
        .generate(&HarnessRequest {
            adapter_import: "./scaffold.ts".into(),
            subjects: BTreeSet::new(),
        })
        .unwrap();
    fs::write(
        root.join("scaffold-worker.ts"),
        &scaffold.artifacts[1].contents,
    )
    .unwrap();
    fs::write(
        root.join("transport.test.ts"),
        include_str!("fixtures/transport.test.ts"),
    )
    .unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cinema/src/domain");
    fs::create_dir_all(root.join("src/domain")).unwrap();
    for file in ["candidate-order-winner.ts", "film-id.ts", "voting-rules.ts"] {
        fs::copy(source.join(file), root.join("src/domain").join(file)).unwrap();
    }
    let output = Command::new("bun")
        .args(["test", "transport.test.ts"])
        .env("DCTX0_NATIVE_COVERAGE", "1")
        .current_dir(&root)
        .output()
        .expect("Bun is required for executable target conformance");
    let evidence =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-harness-conformance");
    fs::create_dir_all(&evidence).unwrap();
    if output.status.success() {
        let mut raw: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("native-performance.json")).unwrap())
                .unwrap();
        for message in raw["protocolSamples"].as_array().unwrap() {
            decode_message(&serde_json::to_vec(message).unwrap(), MAX_MESSAGE_BYTES).unwrap();
        }
        raw["conformance_status"] = serde_json::json!("pass");
        raw["native_stdout"] = serde_json::json!(String::from_utf8_lossy(&output.stdout));
        raw["native_stderr"] = serde_json::json!(String::from_utf8_lossy(&output.stderr));
        fs::write(
            evidence.join(format!("{}.json", std::process::id())),
            serde_json::to_vec_pretty(&raw).unwrap(),
        )
        .unwrap();
    }
    fs::remove_dir_all(root).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn real_esm_observer_records_native_attempts_preserves_values_and_rejects_changed_sources() {
    use std::io::Write;
    use std::process::Stdio;
    let root = std::env::temp_dir().join(format!("dctx0-esm-conformance-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let source = InspectionSource {
        path: "library.mjs".into(),
        language: InspectionLanguage::TypeScript,
        text: "// 🎬 UTF-8 argument offsets\nexport const answer=42; export function lazy(path){return import(path)}; export function getLoader(holder){return holder.require}".into(),
    };
    let request = HarnessRequest {
        adapter_import: "./adapter.ts".into(),
        subjects: ["operation".into()].into(),
    };
    let mut bundle = BunHarness.generate(&request).unwrap();
    bundle.artifacts.extend(
        BunHarness
            .module_observer_artifacts(std::slice::from_ref(&source), "./")
            .unwrap(),
    );
    for artifact in &bundle.artifacts {
        fs::write(root.join(&artifact.path), &artifact.contents).unwrap();
    }
    fs::write(root.join("loaded.mjs"), "export const answer=42;").unwrap();
    fs::write(root.join("observer-worker.test.ts"), "import {test} from 'bun:test'; test('actual module observer worker',async()=>{const worker=await import('./worker.ts');await worker.finished;},30000);").unwrap();
    let mut coverage = Vec::new();
    for mode in 0..7 {
        fs::write(
            root.join("library.mjs"),
            if mode == 2 {
                source.text.replace("answer=42", "answer=99")
            } else {
                source.text.clone()
            },
        )
        .unwrap();
        let invoke = match mode {
            6 => {
                "const marker=()=>42;let reads=0;const actual=getLoader({get require(){reads++;return marker}});return {kind:'returned',value:{kind:'integer',value:String(actual===marker&&reads===1?actual():-1)}}"
            }
            0 => "return {kind:'returned',value:{kind:'integer',value:String(answer)}}",
            4 => {
                "const result=await lazy({toString(){return './loaded.mjs'}});return {kind:'returned',value:{kind:'integer',value:String(result.answer)}}"
            }
            5 => {
                "for(let i=0;i<4097;i++)await lazy('./loaded.mjs');return {kind:'returned',value:{kind:'integer',value:'42'}}"
            }
            _ => {
                "const result=await lazy('./loaded.mjs');return {kind:'returned',value:{kind:'integer',value:String(result.answer)}}"
            }
        };
        let adapter = format!(
            "import {{answer,lazy,getLoader}} from './library.mjs'; export const adapter={{describe(){{return {{protocol_version:1,subjects:[],artifacts:['library.mjs'],capabilities:[],representations:{{}}}}}},begin(){{return {{async invoke(){{{invoke}}},close(){{}}}}}}}};"
        );
        fs::write(root.join("adapter.ts"), adapter).unwrap();
        let mut manifest: ModuleObserverManifest =
            serde_json::from_str(&bundle.artifacts.last().unwrap().contents).unwrap();
        if mode == 3 {
            manifest.sources[0].sites[0].argument_end = source.text.len() + 1;
        }
        fs::write(
            root.join("module-observer.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let mut child = Command::new("bun")
            .args([
                "test",
                "observer-worker.test.ts",
                "--coverage",
                "--coverage-reporter",
                "lcov",
                "--coverage-dir",
                &format!("coverage-{mode}"),
            ])
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let requests = [
            WorkerRequest::Describe,
            WorkerRequest::Begin {
                case_id: "case".into(),
            },
            WorkerRequest::Invoke {
                invocation: dctx0_core::Invocation {
                    subject: "operation".into(),
                    arguments: vec![],
                },
            },
            WorkerRequest::End,
            WorkerRequest::ObserveDependencies,
            WorkerRequest::Shutdown,
        ];
        let mut input = child.stdin.take().unwrap();
        for (index, request) in requests.into_iter().enumerate() {
            input
                .write_all(
                    &encode_request(
                        &RequestEnvelope {
                            protocol_version: 1,
                            id: index as u64 + 1,
                            request,
                        },
                        MAX_MESSAGE_BYTES,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        drop(input);
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let messages = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .filter(|line| line.starts_with("{\"protocol_version\":"))
            .map(|line| {
                decode_message(line.as_bytes(), MAX_MESSAGE_BYTES)
                    .unwrap()
                    .message
            })
            .collect::<Vec<_>>();
        if matches!(mode, 2 | 3) {
            assert!(
                messages
                    .iter()
                    .any(|message| matches!(message, WorkerMessage::TechnicalError { .. })),
                "{messages:?}"
            );
        } else {
            assert!(messages.iter().any(|message| matches!(message,WorkerMessage::Reply { reply:WorkerReply::Invoked { observation:dctx0_core::Observation::Returned { value:dctx0_core::VerificationValue::Integer(value) }, .. }, .. } if value=="42")), "{messages:?}");
            let evidence = messages
                .iter()
                .find_map(|message| match message {
                    WorkerMessage::Reply {
                        reply: WorkerReply::DependenciesObserved { evidence },
                        ..
                    } => Some(evidence),
                    _ => None,
                })
                .unwrap();
            assert_eq!(evidence.loaded_sources, vec!["library.mjs"]);
            assert_eq!(evidence.complete, mode < 4);
            if matches!(mode, 0 | 6) {
                assert!(evidence.attempts.is_empty());
            }
            if mode == 1 {
                assert_eq!(evidence.attempts.len(), 1);
                assert_eq!(evidence.attempts[0].specifier, "./loaded.mjs");
                assert!(
                    evidence.attempts[0]
                        .resolved
                        .as_ref()
                        .unwrap()
                        .ends_with("/loaded.mjs")
                );
            }
        }
        let lcov = fs::read_to_string(root.join(format!("coverage-{mode}/lcov.info"))).unwrap();
        assert!(lcov.contains("module-observer.ts"));
        coverage.push(serde_json::json!({"mode":mode,"raw_lcov":lcov}));
    }
    let evidence =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-harness-conformance");
    fs::create_dir_all(&evidence).unwrap();
    fs::write(evidence.join("module-observer-coverage.json"),serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,"test_status":"pass","scope":"Actual generated Bun ESM module observer; seven native scenarios including opaque loader access, source/range rejection and trace limits","instrumentation":"Bun test LCOV; raw function counters retain Bun identity limitations","reports":coverage,"performance_status":"deferred"})).unwrap()).unwrap();
    fs::remove_dir_all(root).unwrap();
}
