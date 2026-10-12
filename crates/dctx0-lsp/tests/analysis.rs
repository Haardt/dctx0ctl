use dctx0_core::Source;
use dctx0_lsp::{Analysis, TextIndex};
use lsp_types::Position;
fn sources() -> Vec<Source> {
    vec![
    Source { file:"file:///types.dctx0".into(),text:"workspace W { domain D { scalar Id is UUID } }".into() },
    Source { file:"file:///api.dctx0".into(),text:"fragment D {\n surface Api {\n intent \"emoji 😀 // not a comment\"\n operation echo(input: Id) -> Id {\n pure\n ensures result == input\n }\n }\n}".into() }
]
}
#[test]
fn utf16_positions_roundtrip_and_reject_half_surrogates_and_bad_lines() {
    let text = "a😀é\r\nb\n";
    let index = TextIndex::new(text);
    for offset in [0, 1, 5, 7, 9, 10, 11] {
        assert_eq!(index.offset(index.position(offset)), Some(offset));
    }
    assert_eq!(index.offset(Position::new(0, 2)), None);
    assert_eq!(index.offset(Position::new(9, 0)), None);
    assert_eq!(index.offset(Position::new(0, 5)), None);
    assert_eq!(index.position(3), Position::new(0, 1));
    assert_eq!(index.position(8), Position::new(0, 4));
}
#[test]
fn valid_multifile_analysis_uses_resolved_references_and_editor_features() {
    let input = sources();
    let text = &input[1].text;
    let analysis = Analysis::new(input.clone());
    assert_eq!(analysis.diagnostics(&input[1].file), serde_json::json!([]));
    let p = TextIndex::new(text).position(text.find("input: Id").unwrap() + 7);
    assert_eq!(analysis.definition(&input[1].file, p)["uri"], input[0].file);
    assert_eq!(
        analysis
            .references(&input[1].file, p, true)
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(
        !analysis
            .document_symbols(&input[1].file)
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !analysis
            .workspace_symbols("echo")
            .as_array()
            .unwrap()
            .is_empty()
    );
    let p = TextIndex::new(text).position(text.find("echo").unwrap());
    assert!(
        analysis.hover(&input[1].file, p)["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("requirements, not proof")
    );
    assert!(
        analysis.hover(&input[1].file, p)["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("emoji")
    );
    assert!(
        !analysis.semantic_tokens(&input[1].file)["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let p = TextIndex::new(text).position(text.find("input: Id").unwrap() + 8);
    assert!(
        analysis
            .completions(&input[1].file, p)
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["label"] == "Id")
    );
    assert_eq!(
        analysis.definition("unknown", Position::new(0, 0)),
        serde_json::Value::Null
    );
    assert_eq!(
        analysis.semantic_tokens("unknown"),
        serde_json::json!({"data":[]})
    );
}
#[test]
fn malformed_input_keeps_syntax_features_without_claiming_resolved_navigation() {
    let mut input = sources();
    input[1].text.push_str("\n scalar Broken is Missing {\n");
    let analysis = Analysis::new(input.clone());
    assert!(
        !analysis
            .diagnostics(&input[1].file)
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !analysis
            .document_symbols(&input[1].file)
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !analysis.semantic_tokens(&input[1].file)["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        analysis.definition(&input[1].file, Position::new(3, 24)),
        serde_json::Value::Null
    );
    assert!(
        !analysis
            .completions(&input[1].file, Position::new(0, 0))
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn semantic_comments_never_overlap_strings_and_multiline_tokens_are_split() {
    let text = "workspace W {\r\n intent \"\"\"😀 // text\r\nsecond line\"\"\"\r\n /* multiline\r\n comment */\r\n // line\r\n}\r\n";
    let analysis = Analysis::new(vec![Source {
        file: "file:///x.dctx0".into(),
        text: text.into(),
    }]);
    let data = analysis.semantic_tokens("file:///x.dctx0")["data"]
        .as_array()
        .unwrap()
        .clone();
    let mut line = 0;
    let mut col = 0;
    let mut comments = 0;
    let mut strings = 0;
    for token in data.chunks_exact(5) {
        let delta = token[0].as_u64().unwrap();
        line += delta;
        col = if delta == 0 {
            col + token[1].as_u64().unwrap()
        } else {
            token[1].as_u64().unwrap()
        };
        if token[3] == 7 {
            comments += 1;
            assert!(line >= 3);
        }
        if token[3] == 8 {
            strings += 1;
        }
        assert!(token[2].as_u64().unwrap() > 0);
        assert!(col < 100);
    }
    assert_eq!(comments, 3);
    assert_eq!(strings, 2);
}
