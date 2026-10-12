use crate::TextIndex;
use dctx0_core::{
    Compilation, Source, SourceAst, Span, Statement, StatementKind as K, Symbol, Token, TokenKind,
};
use lsp_types::{Position, Range};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const TOKEN_TYPES: &[&str] = &[
    "namespace",
    "type",
    "class",
    "interface",
    "method",
    "event",
    "keyword",
    "comment",
    "string",
    "number",
    "operator",
    "variable",
];
pub const TOKEN_MODIFIERS: &[&str] = &["declaration", "defaultLibrary"];
const BUILTINS: &[&str] = &[
    "String",
    "Boolean",
    "Int",
    "Float",
    "Decimal",
    "UUID",
    "Timestamp",
    "Duration",
    "Bytes",
    "Unit",
    "Option",
    "List",
    "Some",
    "None",
    "Dctx0Option",
    "Dctx0Result",
];
const EXTRA_KEYWORDS: &[&str] = &[
    "exposes",
    "requestReply",
    "notification",
    "stream",
    "returns",
    "from",
    "to",
    "initial",
    "terminal",
    "Some",
    "None",
    "true",
    "false",
    "is",
    "and",
    "or",
    "not",
    "implies",
    "in",
    "for",
    "uses",
    "as",
    "result",
];

const BUILTIN_FUNCTIONS: &str = "count sum all any contains distinct min max filter map sortBy sortDescBy first single ifElse mean indexOf getOrElse lowercase uppercase trim length startsWith endsWith includesText slice reverse divide round";

pub struct Analysis {
    sources: BTreeMap<String, String>,
    ast: BTreeMap<String, SourceAst>,
    tokens: BTreeMap<String, Vec<Token>>,
    compilation: Compilation,
}
impl Analysis {
    pub fn new(sources: Vec<Source>) -> Self {
        let sources: BTreeMap<_, _> = sources.into_iter().map(|s| (s.file, s.text)).collect();
        let sorted: Vec<_> = sources
            .iter()
            .map(|(file, text)| Source {
                file: file.clone(),
                text: text.clone(),
            })
            .collect();
        let compilation = dctx0_language::compile(&sorted);
        let ast = sources
            .iter()
            .map(|(file, text)| (file.clone(), dctx0_language::parse(file, text).ast))
            .collect();
        let tokens = sources
            .iter()
            .map(|(file, text)| (file.clone(), dctx0_language::lex(file, text).tokens))
            .collect();
        Self {
            sources,
            ast,
            tokens,
            compilation,
        }
    }
    pub fn files(&self) -> impl Iterator<Item = &String> {
        self.sources.keys()
    }
    pub fn source(&self, file: &str) -> Option<&str> {
        self.sources.get(file).map(String::as_str)
    }
    pub fn range(&self, span: &Span) -> Range {
        TextIndex::new(self.source(&span.file).unwrap_or(""))
            .range(span.start.offset, span.end.offset)
    }
    pub fn diagnostics(&self, file: &str) -> Value {
        json!(self.compilation.diagnostics.iter().filter(|d| d.span.file == file).map(|d| json!({
            "range": self.range(&d.span), "severity": 1, "code": d.code, "source": "dctx0", "message": d.message,
            "relatedInformation": d.related.iter().map(|r| json!({"location": {"uri": r.span.file, "range": self.range(&r.span)}, "message": r.message})).collect::<Vec<_>>()
        })).collect::<Vec<_>>())
    }
    pub fn document_symbols(&self, file: &str) -> Value {
        self.ast
            .get(file)
            .map_or(json!([]), |ast| json!(self.symbols(&ast.statements)))
    }
    fn symbols(&self, statements: &[Statement]) -> Vec<Value> {
        statements.iter().flat_map(|s| {
            let children = self.symbols(&s.children);
            let Some(name) = &s.name else { return children; };
            let name_span = self.tokens.get(&s.span.file).into_iter().flatten().find(|t| t.kind == TokenKind::Identifier && &t.text == name && t.span.start.offset >= s.span.start.offset && t.span.end.offset <= s.span.end.offset).map_or(&s.span, |t| &t.span);
            vec![json!({"name": name, "detail": s.kind.as_str(), "kind": symbol_kind(s.kind), "range": self.range(&s.span), "selectionRange": self.range(name_span), "children": children})]
        }).collect()
    }
    pub fn workspace_symbols(&self, query: &str) -> Value {
        fn flatten(items: &Value, uri: &str, query: &str, out: &mut Vec<Value>) {
            if let Some(items) = items.as_array() {
                for s in items {
                    if s["detail"] != "fragment"
                        && s["name"]
                            .as_str()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(query)
                    {
                        out.push(json!({"name": s["name"], "kind": s["kind"], "location": {"uri": uri, "range": s["selectionRange"]}}));
                    }
                    flatten(&s["children"], uri, query, out);
                }
            }
        }
        let mut out = Vec::new();
        for file in self.files() {
            flatten(
                &self.document_symbols(file),
                file,
                &query.to_lowercase(),
                &mut out,
            );
        }
        json!(out)
    }
    fn selected(&self, file: &str, offset: usize) -> Option<&Symbol> {
        let ir = self.compilation.ir.as_ref()?;
        if let Some(symbol) = ir
            .symbols
            .values()
            .find(|s| contains(&s.name_span, file, offset))
        {
            return Some(symbol);
        }
        let reference = ir
            .references
            .iter()
            .filter(|r| contains(&r.span, file, offset))
            .min_by_key(|r| r.span.end.offset - r.span.start.offset)?;
        ir.symbols.get(&reference.target)
    }
    pub fn definition(&self, file: &str, position: Position) -> Value {
        let Some(offset) = self
            .source(file)
            .and_then(|s| TextIndex::new(s).offset(position))
        else {
            return Value::Null;
        };
        self.selected(file, offset).map_or(
            Value::Null,
            |s| json!({"uri": s.name_span.file, "range": self.range(&s.name_span)}),
        )
    }
    pub fn references(&self, file: &str, position: Position, include_declaration: bool) -> Value {
        let Some(offset) = self
            .source(file)
            .and_then(|s| TextIndex::new(s).offset(position))
        else {
            return json!([]);
        };
        let Some(symbol) = self.selected(file, offset) else {
            return json!([]);
        };
        let Some(ir) = &self.compilation.ir else {
            return json!([]);
        };
        let mut spans: Vec<_> = ir
            .references
            .iter()
            .filter(|r| r.target == symbol.id)
            .map(|r| &r.span)
            .collect();
        if include_declaration {
            spans.push(&symbol.name_span);
        }
        spans.sort_by_key(|s| (&s.file, s.start.offset, s.end.offset));
        spans.dedup();
        json!(
            spans
                .iter()
                .map(|s| json!({"uri": s.file, "range": self.range(s)}))
                .collect::<Vec<_>>()
        )
    }
    pub fn hover(&self, file: &str, position: Position) -> Value {
        let Some(offset) = self
            .source(file)
            .and_then(|s| TextIndex::new(s).offset(position))
        else {
            return Value::Null;
        };
        let Some(symbol) = self.selected(file, offset) else {
            return Value::Null;
        };
        let mut markdown = format!("**{}** `{}`", symbol.kind.as_str(), symbol.id);
        if let Some(ir) = &self.compilation.ir {
            if let Some(op) = ir.operations.get(&symbol.id) {
                markdown.push_str(if op.pure { "\n\n**Purity contract:** explicitly pure. Determinism and absence of effects are requirements, not proof of the implementation." } else { "\n\nNo explicit `pure` declaration." });
            }
            for n in &ir.nodes {
                if n.kind == K::Intent
                    && (n.owner == symbol.id || symbol.id.starts_with(&format!("{}.", n.owner)))
                {
                    let text = n
                        .terms
                        .iter()
                        .filter(|t| matches!(t.kind, TokenKind::String | TokenKind::TripleString))
                        .map(|t| t.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    markdown.push_str(&format!(
                        "\n\n**Architecture intent · {}:** {}",
                        n.owner,
                        escape_markdown(&text)
                    ));
                }
            }
        }
        json!({"contents": {"kind": "markdown", "value": markdown}, "range": self.range(&symbol.name_span)})
    }
    pub fn completions(&self, file: &str, position: Position) -> Value {
        let Some(source) = self.source(file) else {
            return json!([]);
        };
        let Some(offset) = TextIndex::new(source).offset(position) else {
            return json!([]);
        };
        let start = source[..offset]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
            .map_or(0, |i| i + 1);
        let prefix = &source[start..offset];
        let mut items = BTreeMap::new();
        // Use the compiler's keyword enum rather than a divergent language catalog.
        for token in KEYWORDS
            .split_whitespace()
            .chain(EXTRA_KEYWORDS.iter().copied())
        {
            if K::from_keyword(token).is_some() || EXTRA_KEYWORDS.contains(&token) {
                items.insert(
                    token.to_owned(),
                    json!({"label":token,"kind":14,"detail":"dctx0 keyword"}),
                );
            }
        }
        items.insert("intent".into(), json!({"label":"intent","kind":14,"insertText":"intent \"\"\"${1:Explain this boundary.}\"\"\"","insertTextFormat":2}));
        for label in BUILTIN_FUNCTIONS.split_whitespace() {
            items.insert(
                label.into(),
                json!({"label": label, "kind": 3, "detail": "built-in expression function"}),
            );
        }
        items.insert("exposes".into(), json!({"label":"exposes", "kind":14, "insertText":"exposes ${1|requestReply,notification,stream|}", "insertTextFormat":2}));
        for label in BUILTINS {
            items.insert(
                (*label).into(),
                json!({"label":label,"kind":7,"detail":"built-in type"}),
            );
        }
        if let Some(ir) = &self.compilation.ir {
            for s in ir.symbols.values() {
                for label in [&s.name, &s.id] {
                    items.insert(label.clone(), json!({"label":label,"kind":if s.kind == K::Operation {3} else {7},"detail":format!("{} {}",s.kind.as_str(),s.id)}));
                }
            }
        } else {
            fn partial(statements: &[Statement], items: &mut BTreeMap<String, Value>) {
                for s in statements {
                    if let Some(name) = &s.name {
                        items.entry(name.clone()).or_insert_with(|| json!({"label":name,"kind":7,"detail":"syntax candidate; workspace is not valid"}));
                    }
                    partial(&s.children, items);
                }
            }
            for ast in self.ast.values() {
                partial(&ast.statements, &mut items);
            }
        }
        json!(
            items
                .into_iter()
                .filter(|(label, _)| label.to_lowercase().starts_with(&prefix.to_lowercase()))
                .map(|(_, item)| item)
                .collect::<Vec<_>>()
        )
    }
    pub fn semantic_tokens(&self, file: &str) -> Value {
        let Some(source) = self.source(file) else {
            return json!({"data":[]});
        };
        let Some(tokens) = self.tokens.get(file) else {
            return json!({"data":[]});
        };
        let index = TextIndex::new(source);
        let mut segments = Vec::new();
        fn headers(statements: &[Statement], offsets: &mut std::collections::BTreeSet<usize>) {
            for s in statements {
                offsets.insert(s.span.start.offset);
                headers(&s.children, offsets);
            }
        }
        let mut header_offsets = std::collections::BTreeSet::new();
        if let Some(ast) = self.ast.get(file) {
            headers(&ast.statements, &mut header_offsets);
        }
        let mut covered = 0;
        for token in tokens {
            // Comments are skipped by the compiler lexer; inspect only gaps, never strings.
            comment_segments(
                source,
                covered,
                token.span.start.offset,
                &index,
                &mut segments,
            );
            covered = token.span.end.offset;
            let (ty, modifiers) = match token.kind {
                TokenKind::String | TokenKind::TripleString => (8, 0),
                TokenKind::Number => (9, 0),
                TokenKind::Identifier
                    if token.text == "value"
                        && !header_offsets.contains(&token.span.start.offset) =>
                {
                    (11, 0)
                }
                TokenKind::Identifier
                    if BUILTIN_FUNCTIONS
                        .split_whitespace()
                        .any(|f| f == token.text)
                        && source[token.span.end.offset..]
                            .trim_start()
                            .starts_with('(') =>
                {
                    self.selected(file, token.span.start.offset)
                        .map_or((4, 2), |s| {
                            (
                                semantic_kind(s.kind),
                                u32::from(contains(&s.name_span, file, token.span.start.offset)),
                            )
                        })
                }
                TokenKind::Identifier
                    if K::from_keyword(&token.text).is_some()
                        || EXTRA_KEYWORDS.contains(&token.text.as_str()) =>
                {
                    (6, 0)
                }
                TokenKind::Identifier if BUILTINS.contains(&token.text.as_str()) => (1, 2),
                TokenKind::Identifier => {
                    self.selected(file, token.span.start.offset)
                        .map_or((11, 0), |s| {
                            (
                                semantic_kind(s.kind),
                                u32::from(contains(&s.name_span, file, token.span.start.offset)),
                            )
                        })
                }
                TokenKind::Symbol
                    if [
                        ".", "->", "..", "!=", "<=", ">=", "==", "=", "!", "|", "<", ">", "+", "-",
                        "*", "/", "%",
                    ]
                    .contains(&token.text.as_str()) =>
                {
                    (10, 0)
                }
                _ => continue,
            };
            push_segments(
                source,
                token.span.start.offset,
                token.span.end.offset,
                ty,
                modifiers,
                &index,
                &mut segments,
            );
        }
        segments.sort_unstable();
        let mut data = Vec::new();
        let (mut line, mut col) = (0, 0);
        for (next_line, next_col, len, ty, modifiers) in segments {
            data.extend([
                next_line - line,
                if next_line == line {
                    next_col - col
                } else {
                    next_col
                },
                len,
                ty,
                modifiers,
            ]);
            (line, col) = (next_line, next_col);
        }
        json!({"data":data})
    }
}
const KEYWORDS: &str = "workspace domain scalar value type failure entity message event module surface port external owns requires operation effects read write call emit observe intent decision choose because avoid constraint example given when expect fails emits invariant forall proof test review inspect criteria question policy require range length format where key ensures state lifecycle transition assert fragment stableId pure";
fn contains(span: &Span, file: &str, offset: usize) -> bool {
    span.file == file && span.start.offset <= offset && offset < span.end.offset
}
fn escape_markdown(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if "\\`*_[]<>".contains(c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}
fn symbol_kind(k: K) -> u32 {
    match k {
        K::Workspace | K::Domain | K::Module | K::Fragment => 3,
        K::Operation => 6,
        K::Entity => 5,
        K::Surface | K::Port | K::External => 11,
        K::Event => 24,
        _ => 23,
    }
}
fn semantic_kind(k: K) -> u32 {
    match k {
        K::Workspace | K::Domain | K::Module => 0,
        K::Entity => 2,
        K::Surface | K::Port | K::External => 3,
        K::Operation => 4,
        K::Event => 5,
        _ => 1,
    }
}
type Segment = (u32, u32, u32, u32, u32);
fn push_segments(
    source: &str,
    start: usize,
    end: usize,
    ty: u32,
    modifiers: u32,
    index: &TextIndex<'_>,
    out: &mut Vec<Segment>,
) {
    let mut byte = start;
    for part in source[start..end].split_inclusive('\n') {
        let text = part.trim_end_matches(['\r', '\n']);
        if !text.is_empty() {
            let p = index.position(byte);
            out.push((
                p.line,
                p.character,
                text.encode_utf16().count() as u32,
                ty,
                modifiers,
            ));
        }
        byte += part.len();
    }
}
fn comment_segments(
    source: &str,
    start: usize,
    end: usize,
    index: &TextIndex<'_>,
    out: &mut Vec<Segment>,
) {
    let gap = &source[start..end];
    let mut cursor = 0;
    while cursor < gap.len() {
        let Some(relative) = gap[cursor..].find('/') else {
            break;
        };
        cursor += relative;
        let finish = if gap[cursor..].starts_with("//") {
            // The lexer ends this gap at its newline token (or EOF).
            gap.len()
        } else if gap[cursor..].starts_with("/*") {
            gap[cursor + 2..]
                .find("*/")
                .map_or(gap.len(), |i| cursor + 2 + i + 2)
        } else {
            cursor += 1;
            continue;
        };
        push_segments(source, start + cursor, start + finish, 7, 0, index, out);
        cursor = finish;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_retains_related_diagnostics_and_classifies_actual_helper_declarations() {
        let uri = "file:///projection.dctx0";
        let source = "workspace W { domain D {\n value N = Int\n value N = String\n} }";
        let analysis = Analysis::new(vec![Source {
            file: uri.into(),
            text: source.into(),
        }]);
        assert!(
            analysis
                .diagnostics(uri)
                .as_array()
                .unwrap()
                .iter()
                .any(|d| !d["relatedInformation"].as_array().unwrap().is_empty())
        );
        let source = "workspace W { domain D {\n value Text = String { where length(trim(value)) > 0 }\n surface S { operation trim(input: String) -> String { pure\n ensures result == input } }\n /* block comment */\n} }";
        let analysis = Analysis::new(vec![Source {
            file: uri.into(),
            text: source.into(),
        }]);
        assert_eq!(analysis.diagnostics(uri), json!([]));
        assert!(
            !analysis.semantic_tokens(uri)["data"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let tokens = analysis.tokens.get(uri).unwrap();
        let variable = tokens
            .iter()
            .find(|t| t.text == "value" && t.span.start.line == 2 && t.span.start.column > 20)
            .unwrap();
        let pos = TextIndex::new(source).position(variable.span.start.offset);
        let data = analysis.semantic_tokens(uri)["data"]
            .as_array()
            .unwrap()
            .clone();
        let (mut line, mut col) = (0, 0);
        let mut found = false;
        for token in data.chunks_exact(5) {
            let delta = token[0].as_u64().unwrap() as u32;
            line += delta;
            col = if delta == 0 {
                col + token[1].as_u64().unwrap() as u32
            } else {
                token[1].as_u64().unwrap() as u32
            };
            if (line, col) == (pos.line, pos.character) {
                assert_eq!(token[3], 11);
                found = true;
            }
        }
        assert!(found);
        let analysis = Analysis::new(vec![Source {
            file: uri.into(),
            text: "workspace W { /* unterminated".into(),
        }]);
        assert!(
            !analysis.semantic_tokens(uri)["data"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}
