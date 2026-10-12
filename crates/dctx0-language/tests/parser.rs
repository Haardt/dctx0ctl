use dctx0_core::{Statement, StatementKind as K, Syntax, TokenKind as T};
use dctx0_language::{lex, parse};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn normalize(statement: &Statement) -> Value {
    let mut terms: Vec<String> = statement
        .terms
        .iter()
        .map(|t| {
            if matches!(t.kind, T::String | T::TripleString) {
                serde_json::to_string(&t.text).unwrap()
            } else {
                t.text.clone()
            }
        })
        .collect();
    if statement.kind == K::Field {
        terms.splice(0..0, [statement.name.clone().unwrap(), ":".to_owned()]);
    }
    json!({"kind": statement.kind.as_str(), "name": statement.name,
        "terms": terms, "children": statement.children.iter().map(normalize).collect::<Vec<_>>()})
}

#[test]
fn both_complete_example_workspaces_match_reference_ast() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut count = 0;
    {
        let (id, directory) = ("cinema", "fixtures/semantic/original-cinema");
        let mut paths = fs::read_dir(root.join(directory))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|s| s == "dctx0"))
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            count += 1;
            let source = fs::read_to_string(&path).unwrap();
            let result = parse(path.to_str().unwrap(), &source);
            assert!(
                result.diagnostics.is_empty(),
                "{}: {:?}",
                path.display(),
                result.diagnostics
            );
            let expected: Value = serde_json::from_str(
                &fs::read_to_string(root.join(format!(
                    "fixtures/parser/{id}-{}.json",
                    path.file_name().unwrap().to_str().unwrap()
                )))
                .unwrap(),
            )
            .unwrap();
            let actual = json!(
                result
                    .ast
                    .statements
                    .iter()
                    .map(normalize)
                    .collect::<Vec<_>>()
            );
            assert_eq!(actual, expected, "{}", path.display());
            check_spans(&result.ast.statements, &source);
        }
    }
    assert_eq!(count, 2);
}
fn check_spans(statements: &[Statement], source: &str) {
    for statement in statements {
        assert!(source.is_char_boundary(statement.span.start.offset));
        assert!(source.is_char_boundary(statement.span.end.offset));
        assert!(statement.span.start.offset <= statement.span.end.offset);
        assert!(statement.span.end.offset <= source.len());
        check_spans(&statement.children, source);
    }
}
fn valid(source: &str) -> dctx0_core::SourceAst {
    let result = parse("test.dctx0", source);
    assert!(
        result.diagnostics.is_empty(),
        "{source}: {:?}",
        result.diagnostics
    );
    result.ast
}
fn errors(source: &str, codes: &[&str]) {
    let result = parse("test.dctx0", source);
    for code in codes {
        assert!(
            result.diagnostics.iter().any(|d| d.code == *code),
            "{source}: missing {code}: {:?}",
            result.diagnostics
        );
    }
    for diagnostic in result.diagnostics {
        assert!(diagnostic.span.end.offset <= source.len());
        assert!(!diagnostic.message.is_empty());
    }
}

#[test]
fn lexer_preserves_numbers_operators_strings_and_unicode_positions() {
    let source = "// ignored\n /* comment\n */ value _a-b.c = 12..34 1.2e-3 4E+5 -> != <= >= ==\n\"ä🙂\\n\\t\\\"\\\\\\q\" \"\"\"multi\nline\"\"\" @";
    let result = lex("utf8.dctx0", source);
    assert!(result.diagnostics.is_empty());
    let texts: Vec<_> = result.tokens.iter().map(|t| t.text.as_str()).collect();
    for text in [
        "_a-b.c",
        "12",
        "..",
        "34",
        "1.2e-3",
        "4E+5",
        "->",
        "!=",
        "<=",
        ">=",
        "==",
        "ä🙂\n\t\"\\q",
        "multi\nline",
        "@",
    ] {
        assert!(texts.contains(&text), "{text:?}");
    }
    let string = result.tokens.iter().find(|t| t.kind == T::String).unwrap();
    assert_eq!(
        &source[string.span.start.offset..string.span.end.offset],
        "\"ä🙂\\n\\t\\\"\\\\\\q\""
    );
    let eof = result.tokens.last().unwrap();
    assert_eq!(eof.kind, T::Eof);
    assert_eq!(eof.span.start.offset, source.len());
    assert_eq!(lex("empty", "").tokens.len(), 1);
    assert_eq!(lex("position", "\"ä🙂\" x").tokens[1].span.start.column, 6);
}

#[test]
fn lexical_failures_retain_precise_diagnostics() {
    for (source, code) in [
        ("/* open", "DCTX0_LEX_COMMENT"),
        ("\"open", "DCTX0_LEX_STRING"),
        ("\"open\\", "DCTX0_LEX_STRING"),
        ("\"\"\"open", "DCTX0_LEX_TRIPLE_STRING"),
        ("1.", "DCTX0_LEX_NUMBER"),
        ("1e", "DCTX0_LEX_NUMBER"),
        ("1e+", "DCTX0_LEX_NUMBER"),
    ] {
        let result = lex("bad", source);
        assert_eq!(result.diagnostics[0].code, code);
        assert_eq!(result.diagnostics[0].span.start.offset, 0);
        assert_eq!(result.diagnostics[0].span.end.offset, source.len());
    }
    assert_eq!(lex("whitespace", " \t\r\n").tokens[0].kind, T::Newline);
}

#[test]
fn parses_typed_fields_unions_operation_parameters_and_failures() {
    let ast = valid(
        "domain D {\n type Envelope { data: Map<String, List<Option<Int>>>, count: Int; }\n type Choice = None | Some(Envelope)\n surface Api { operation get(id: UUID, query: Option<String>)\n -> List<Envelope>\n ! Missing ! Busy { pure } }\n}",
    );
    let children = &ast.statements[0].children;
    let Some(Syntax::Data { fields, .. }) = &children[0].syntax else {
        panic!("data syntax")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].ty.name, "Map");
    assert_eq!(
        fields[0].ty.arguments[1].arguments[0].arguments[0].name,
        "Int"
    );
    let Some(Syntax::Data { variants, .. }) = &children[1].syntax else {
        panic!("union syntax")
    };
    assert_eq!(variants[1].payload.as_ref().unwrap().name, "Envelope");
    let Some(Syntax::Operation {
        parameters,
        result,
        failures,
    }) = &children[2].children[0].syntax
    else {
        panic!("operation syntax")
    };
    assert_eq!(parameters.len(), 2);
    assert_eq!(result.as_ref().unwrap().name, "List");
    assert_eq!(
        failures.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        ["Missing", "Busy"]
    );
}

#[test]
fn contexts_relations_effects_fragments_and_narratives_are_preserved() {
    let ast = valid(
        "fragment D.M {\n owns { Item; Other }\n requires { Store }\n operation run() -> Unit\n effects { read Item write Item call Store.save as saved emit Changed(id: saved.id, value: 2) observe Clock.now }\n}\nreview R { intent \"Text\"\n inspect { src/app.ts }\n criteria { \"correct\" }\n question Why { \"\"\"Details\"\"\" }\n}\nentity Item { stableId \"item\"; id: UUID; key id }\n decision Choose { choose \"\"\"Rust\"\"\"; because \"Fast\"; avoid \"Slow\" }\n policy Release { require true }\n external Clock { surface Api {} }\n",
    );
    let fragment = &ast.statements[0];
    assert!(matches!(&fragment.syntax, Some(Syntax::Fragment { target, .. }) if target == "D.M"));
    assert_eq!(fragment.children[0].children[0].kind, K::Own);
    assert_eq!(fragment.children[1].children[0].kind, K::Dependency);
    assert_eq!(fragment.children[2].children[0].kind, K::Effects);
    let effects = &fragment.children[2].children[0].children;
    assert_eq!(effects.len(), 5);
    assert!(
        matches!(&effects[2].syntax, Some(Syntax::Effect { result_alias: Some(alias), .. }) if alias == "saved")
    );
    assert!(
        matches!(&effects[3].syntax, Some(Syntax::Effect { payload: Some(dctx0_core::EffectPayload::Record { fields }), .. }) if fields.len() == 2)
    );
    assert_eq!(ast.statements[1].children[1].children[0].kind, K::Artifact);
    assert_eq!(ast.statements[1].children[2].children[0].kind, K::Criterion);
    assert_eq!(ast.statements[1].children[3].children[0].kind, K::Narrative);
    assert!(matches!(
        ast.statements[2].children[0].syntax,
        Some(Syntax::StableId { quoted: true, .. })
    ));
}

#[test]
fn expression_record_literals_comparisons_and_continuations_do_not_swallow_clauses() {
    let ast = valid(
        "example E {\n given Store.get(id) returns\n { value: 3 }\n when result = Api.get(id); expect result == { value: 3 }; emits Changed\n}\ninvariant Bounds { forall value: Int; assert value < 10; assert value > -10; candidate != 0 }\nlifecycle Flow { state Open initial\n transition close\n from Open\n to Closed\n when Api.close\n}\nsurface Api { operation get() -> Int { state Item\n by id\n where true\n requires true\n and false\n ensures true } }",
    );
    assert_eq!(ast.statements[0].children.len(), 4);
    assert_eq!(
        ast.statements[1]
            .children
            .iter()
            .map(|s| s.kind)
            .collect::<Vec<_>>(),
        [K::Forall, K::Assertion, K::Assertion, K::Assertion]
    );
    assert_eq!(ast.statements[2].children[1].terms.len(), 7);
    assert_eq!(ast.statements[3].children[0].children.len(), 3);
}

#[test]
fn malformed_input_recovers_to_later_declarations() {
    let result = parse(
        "broken",
        "? bad; value Good = Int\n nonsense x\n value Other = String\n}",
    );
    assert_eq!(
        result
            .ast
            .statements
            .iter()
            .map(|s| s.name.as_deref())
            .collect::<Vec<_>>(),
        [Some("Good"), Some("Other")]
    );
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == "DCTX0_PARSE_KEYWORD")
            .count(),
        3
    );
    errors("workspace W { domain D {", &["DCTX0_PARSE_BLOCK_UNCLOSED"]);
    errors("workspace W { ensures true }", &["DCTX0_STATEMENT_CONTEXT"]);
    errors(
        "surface Api { operation run() -> Unit { key id } }",
        &["DCTX0_STATEMENT_CONTEXT"],
    );
    errors(
        "domain D { pure value }",
        &["DCTX0_PURE_CLAUSE", "DCTX0_STATEMENT_CONTEXT"],
    );
    errors(
        "surface Api { operation x() -> Unit { pure {} } }",
        &["DCTX0_PURE_CLAUSE"],
    );
}

#[test]
fn invalid_type_and_union_syntax_is_never_silently_accepted() {
    for source in [
        "value X =",
        "value X = 42",
        "value X = List<>",
        "value X = List<Int,>",
        "value X = List<,Int>",
        "type X { field: }",
        "surface Api { operation run(x) -> Unit }",
    ] {
        errors(source, &["DCTX0_TYPE_EXPECTED"]);
    }
    errors("value X = Int extra", &["DCTX0_TYPE_TRAILING_TOKENS"]);
    errors(
        "value X = List<Int String>",
        &["DCTX0_TYPE_TRAILING_TOKENS"],
    );
    errors("value X = List<Int", &["DCTX0_TYPE_ARGUMENTS_UNCLOSED"]);
    errors("value X = List<42>", &["DCTX0_TYPE_EXPECTED"]);
    errors("type X = A | A", &["DCTX0_UNION_VARIANT_DUPLICATE"]);
    for source in [
        "type X = A |",
        "type X = A | B()",
        "type X = A | B extra",
        "type X = A | B(Int",
        "type X = A | 42",
    ] {
        errors(source, &["DCTX0_UNION_VARIANT_INVALID"]);
    }
    for source in [
        "surface Api { operation run -> Unit }",
        "surface Api { operation run( }",
        "surface Api { operation run() -> }",
        "surface Api { operation run() -> Int uses Store }",
    ] {
        errors(source, &["DCTX0_OPERATION_SIGNATURE"]);
    }
}

#[test]
fn invalid_effect_syntax_is_diagnosed() {
    for (clause, code) in [
        ("read", "DCTX0_EFFECT_TARGET"),
        ("write Item extra", "DCTX0_EFFECT_SYNTAX"),
        ("call Store.get alias", "DCTX0_CALL_RESULT_ALIAS"),
        ("call Store.get as 3", "DCTX0_CALL_RESULT_ALIAS"),
        ("emit Changed invalid", "DCTX0_EMIT_PAYLOAD_SYNTAX"),
        ("emit Changed(value:)", "DCTX0_EMIT_PAYLOAD_SYNTAX"),
        ("emit Changed(value: 3,)", "DCTX0_EMIT_PAYLOAD_SYNTAX"),
    ] {
        errors(
            &format!("surface Api {{ operation run() -> Unit {{ {clause} }} }}"),
            &[code],
        );
    }
    let ast = valid(
        "surface Api { operation run() -> Unit { call Store.get; emit Changed(3); emit Empty; read Item; observe Clock.now } }",
    );
    assert!(matches!(
        ast.statements[0].children[0].children[1].syntax,
        Some(Syntax::Effect {
            payload: Some(dctx0_core::EffectPayload::Scalar { .. }),
            ..
        })
    ));
}

#[test]
fn excessive_block_and_type_nesting_is_rejected_without_stack_overflow() {
    errors(
        &format!("{}{}", "domain D {".repeat(140), "}".repeat(140)),
        &["DCTX0_PARSE_DEPTH"],
    );
    errors(
        &format!("value Deep = {}Int{}", "List<".repeat(140), ">".repeat(140)),
        &["DCTX0_TYPE_DEPTH"],
    );
}

#[test]
fn all_statement_kind_names_round_trip_and_serialize() {
    let words = "workspace domain scalar value type failure entity message event module surface port external owns requires operation effects read write call emit observe intent decision choose because avoid constraint example given when expect fails emits invariant forall proof test review inspect criteria question policy require range length format where key ensures state lifecycle transition assert fragment stableId pure field own dependency artifact criterion narrative assertion";
    for word in words.split_whitespace() {
        let kind = K::from_keyword(word).unwrap();
        assert_eq!(kind.as_str(), word);
        assert_eq!(serde_json::to_value(kind).unwrap(), word);
    }
    assert_eq!(K::from_keyword("unknown"), None);
}

#[test]
fn recovery_preserves_record_fields_and_reports_illegal_nested_clauses() {
    let result = parse(
        "recover",
        "type T { ? field: Int; }\nfragment {}\nentity Item { stableId }\nintent \"x\" { value Child = Int }\n",
    );
    assert_eq!(
        result.ast.statements[0].children[0].name.as_deref(),
        Some("field")
    );
    assert!(
        matches!(result.ast.statements[1].syntax, Some(Syntax::Fragment { ref target, .. }) if target.is_empty())
    );
    assert!(
        matches!(result.ast.statements[2].children[0].syntax, Some(Syntax::StableId { ref value, .. }) if value.is_empty())
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "DCTX0_STATEMENT_CONTEXT")
    );
}

#[test]
fn strings_containing_delimiters_are_values_and_nested_payload_calls_are_preserved() {
    let ast = valid(
        "review R { criteria { \"}\"; \";\"; \"{\" } }\nsurface Api { operation run() -> Unit { emit Changed(\")\"); emit Changed(combine(1, 2)) } }",
    );
    assert_eq!(ast.statements[0].children[0].children.len(), 3);
    assert_eq!(ast.statements[1].children[0].children.len(), 2);
    assert!(
        matches!(&ast.statements[1].children[0].children[0].syntax, Some(Syntax::Effect { payload: Some(dctx0_core::EffectPayload::Scalar { expression }), .. }) if expression[0].text == ")")
    );
}

#[test]
fn illegal_children_are_rejected_in_each_architectural_and_behavioral_context() {
    for source in [
        "module M { value Hidden = Int }",
        "external Service { value Hidden = Int }",
        "surface Api { value Hidden = Int }",
        "fragment D.M { workspace W {} }",
        "decision Choice { value Hidden = Int }",
        "example Example { value Hidden = Int }",
        "invariant Check { value Hidden = Int }",
        "lifecycle Flow { value Hidden = Int }",
        "review Review { value Hidden = Int }",
        "operation run() -> Unit",
        "surface Api { operation run() -> Unit { effects { value Hidden = Int } } }",
    ] {
        errors(source, &["DCTX0_STATEMENT_CONTEXT"]);
    }
}
