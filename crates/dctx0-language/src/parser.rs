use crate::kinds::{is_data_declaration as data, is_effect_clause as effect};
use crate::{lex, syntax};
use dctx0_core::{
    Diagnostic, SourceAst, Span, Statement, StatementKind as K, Token, TokenKind as T,
};

#[derive(Debug)]
pub struct ParseResult {
    pub ast: SourceAst,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse one source file with recovery. Diagnostics must be checked before
/// using the AST as valid input. Expressions remain tokens until IR compilation.
pub fn parse(file: &str, source: &str) -> ParseResult {
    let lexical = lex(file, source);
    let mut parser = Parser {
        tokens: lexical.tokens,
        index: 0,
        diagnostics: lexical.diagnostics,
    };
    let statements = parser.block(None, None, 0);
    context(&statements, None, &mut parser.diagnostics);
    ParseResult {
        ast: SourceAst {
            file: file.to_owned(),
            statements,
        },
        diagnostics: parser.diagnostics,
    }
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    diagnostics: Vec<Diagnostic>,
}
impl Parser {
    fn current(&self) -> &Token {
        &self.tokens[self.index.min(self.tokens.len() - 1)]
    }
    // Consumed tokens are never revisited. Preserve positions for statement
    // endpoints while transferring their allocated strings into the AST.
    fn take_current(&mut self) -> Token {
        let token = &mut self.tokens[self.index];
        Token {
            kind: token.kind,
            text: std::mem::take(&mut token.text),
            span: Span {
                file: std::mem::take(&mut token.span.file),
                start: token.span.start,
                end: token.span.end,
            },
        }
    }
    fn text(&self) -> &str {
        &self.current().text
    }
    fn symbol(&self, text: &str) -> bool {
        self.current().kind == T::Symbol && self.text() == text
    }
    fn eof(&self) -> bool {
        self.current().kind == T::Eof
    }
    fn next(&self) -> Option<&Token> {
        self.tokens.get(self.index + 1)
    }
    fn significant(&self) -> &Token {
        self.tokens[self.index + 1..]
            .iter()
            .find(|t| t.kind != T::Newline)
            .expect("newline is followed by the EOF sentinel")
    }
    fn problem(&mut self, code: &'static str, message: String, span: Span) {
        self.diagnostics.push(Diagnostic {
            related: Vec::new(),
            code: code.into(),
            message,
            span,
        });
    }
    fn block(&mut self, parent: Option<K>, opening: Option<Span>, depth: usize) -> Vec<Statement> {
        let mut statements: Vec<Statement> = Vec::new();
        while !self.eof() {
            while self.current().kind == T::Newline
                || self.symbol(";")
                || parent.is_some_and(data) && self.symbol(",")
            {
                self.index += 1;
            }
            if self.eof() || opening.is_some() && self.symbol("}") {
                break;
            }
            let before = self.index;
            if let Some(statement) = self.statement(parent, depth) {
                if statement.kind == K::Effects
                    && statements.last().is_some_and(|s| s.kind == K::Operation)
                {
                    statements.last_mut().unwrap().children.push(statement);
                } else {
                    statements.push(statement);
                }
            } else {
                self.recover(parent);
            }
            if self.index == before {
                self.index += 1;
            }
        }
        if let Some(span) = opening {
            if self.symbol("}") {
                self.index += 1;
            } else {
                self.problem(
                    "DCTX0_PARSE_BLOCK_UNCLOSED",
                    "Block is missing a closing '}'".into(),
                    span,
                );
            }
        }
        statements
    }
    fn statement(&mut self, parent: Option<K>, depth: usize) -> Option<Statement> {
        let start = self.current().clone();
        if parent.is_some_and(data)
            && start.kind == T::Identifier
            && self.next().is_some_and(|t| t.text == ":")
        {
            return Some(self.field(start));
        }
        let relation = match parent {
            Some(K::Owns) => Some(K::Own),
            Some(K::Requires) => Some(K::Dependency),
            Some(K::Inspect) => Some(K::Artifact),
            Some(K::Criteria) if start.kind == T::String => Some(K::Criterion),
            Some(K::Question) if start.kind == T::TripleString => Some(K::Narrative),
            Some(K::Invariant) if !keyword(&start) => Some(K::Assertion),
            _ => None,
        };
        if let Some(kind) = relation {
            return Some(self.relation(kind, start));
        }
        let Some(mut kind) = K::from_keyword(&start.text).filter(|_| keyword(&start)) else {
            self.problem(
                "DCTX0_PARSE_KEYWORD",
                format!("Unknown statement keyword {:?}", start.text),
                start.span,
            );
            return None;
        };
        self.index += 1;
        if parent == Some(K::Invariant) && kind == K::Assert {
            kind = K::Assertion;
        }
        let effect_relation = parent == Some(K::Effects) && effect(kind);
        let expression = matches!(
            kind,
            K::Given
                | K::When
                | K::Expect
                | K::Fails
                | K::Emits
                | K::Require
                | K::State
                | K::Ensures
                | K::Where
                | K::Assertion
        ) || kind == K::Requires && !self.symbol("{");
        let terms = self.terms(kind, expression, effect_relation);
        let has_body = self.symbol("{");
        let children = if has_body {
            let opening = self.current().span.clone();
            self.index += 1;
            if depth >= 128 {
                self.problem(
                    "DCTX0_PARSE_DEPTH",
                    "Block nesting exceeds 128".into(),
                    opening,
                );
                self.skip_block();
                Vec::new()
            } else {
                self.block(Some(kind), Some(opening), depth + 1)
            }
        } else {
            if self.current().kind == T::Newline || self.symbol(";") {
                self.index += 1;
            }
            Vec::new()
        };
        let end = &self.tokens[self.index.saturating_sub(1)];
        let span = merge(&start.span, &end.span);
        let name = if named(kind) {
            terms.first().map(|t| t.text.clone())
        } else {
            None
        };
        let syntax = syntax::statement(kind, &terms, &children, &span, &mut self.diagnostics);
        if kind == K::Pure && (!terms.is_empty() || has_body) {
            self.problem(
                "DCTX0_PURE_CLAUSE",
                "pure must be parameterless and have no block".into(),
                span.clone(),
            );
        }
        Some(Statement {
            kind,
            name,
            terms,
            children,
            span,
            syntax,
        })
    }
    fn skip_block(&mut self) {
        let mut depth = 1;
        while !self.eof() {
            if self.symbol("{") {
                depth += 1;
            }
            if self.symbol("}") {
                depth -= 1;
            }
            self.index += 1;
            if depth == 0 {
                break;
            }
        }
    }
    fn terms(&mut self, kind: K, expression: bool, effect_relation: bool) -> Vec<Token> {
        let mut terms = Vec::new();
        let mut depth: usize = 0;
        while !self.eof() {
            let token = self.current();
            if depth == 0
                && (token.kind == T::Symbol
                    && (matches!(token.text.as_str(), "}" | ";")
                        || !expression && token.text == "{")
                    || effect_relation && effect_text(&token.text))
            {
                break;
            }
            if depth == 0 && token.kind == T::Newline {
                if effect_relation || !continues(kind, expression, &terms, self.significant()) {
                    break;
                }
                self.index += 1;
                continue;
            }
            if token.kind == T::Symbol {
                depth = adjusted(depth, &token.text, expression);
            }
            terms.push(self.take_current());
            self.index += 1;
        }
        terms.retain(|t| t.kind != T::Newline);
        terms
    }
    fn field(&mut self, start: Token) -> Statement {
        self.index += 2;
        let mut terms = Vec::new();
        let mut depth = 0;
        while !self.eof() {
            let token = self.current();
            if depth == 0
                && (token.kind == T::Newline
                    || token.kind == T::Symbol && matches!(token.text.as_str(), "," | ";" | "}"))
            {
                break;
            }
            if token.kind == T::Symbol {
                depth = adjusted(depth, &token.text, false);
            }
            terms.push(self.take_current());
            self.index += 1;
        }
        let span = merge(&start.span, &terms.last().unwrap_or(&start).span);
        if self.current().kind == T::Newline || matches!(self.text(), "," | ";") {
            self.index += 1;
        }
        let ty = syntax::parse_type(&terms, &span, &mut self.diagnostics);
        let fields = ty
            .map(|ty| dctx0_core::Parameter {
                name: start.text.clone(),
                ty,
                span: span.clone(),
            })
            .into_iter()
            .collect();
        Statement {
            kind: K::Field,
            name: Some(start.text),
            terms,
            children: Vec::new(),
            span,
            syntax: Some(dctx0_core::Syntax::Data {
                alias: None,
                fields,
                variants: Vec::new(),
            }),
        }
    }
    fn relation(&mut self, kind: K, start: Token) -> Statement {
        let mut terms = Vec::new();
        while !self.eof()
            && self.current().kind != T::Newline
            && !(self.current().kind == T::Symbol && matches!(self.text(), ";" | "}"))
        {
            terms.push(self.take_current());
            self.index += 1;
        }
        let span = merge(&start.span, &terms.last().unwrap_or(&start).span);
        if self.current().kind == T::Newline || self.symbol(";") {
            self.index += 1;
        }
        Statement {
            kind,
            name: None,
            terms,
            children: Vec::new(),
            span,
            syntax: None,
        }
    }
    fn recover(&mut self, parent: Option<K>) {
        while !self.eof() {
            if self.current().kind == T::Newline || self.symbol(";") {
                self.index += 1;
                break;
            }
            if self.symbol("}")
                || keyword(self.current())
                || parent.is_some_and(data) && self.next().is_some_and(|t| t.text == ":")
            {
                break;
            }
            self.index += 1;
        }
    }
}

pub(crate) fn merge(start: &Span, end: &Span) -> Span {
    Span {
        file: start.file.clone(),
        start: start.start,
        end: end.end,
    }
}
fn effect_text(text: &str) -> bool {
    matches!(text, "read" | "write" | "call" | "emit" | "observe")
}
fn keyword(token: &Token) -> bool {
    token.kind == T::Identifier
        && K::from_keyword(&token.text).is_some_and(|k| {
            !matches!(
                k,
                K::Field
                    | K::Own
                    | K::Dependency
                    | K::Artifact
                    | K::Criterion
                    | K::Narrative
                    | K::Assertion
            )
        })
}
fn named(kind: K) -> bool {
    !matches!(
        kind,
        K::Read
            | K::Write
            | K::Call
            | K::Emit
            | K::Observe
            | K::Owns
            | K::Requires
            | K::Effects
            | K::Given
            | K::When
            | K::Expect
            | K::Fails
            | K::Emits
            | K::Inspect
            | K::Criteria
            | K::Require
            | K::Choose
            | K::Because
            | K::Avoid
            | K::Forall
            | K::Range
            | K::Length
            | K::Format
            | K::Where
            | K::Key
            | K::Ensures
            | K::StableId
            | K::Pure
            | K::Assertion
    )
}
pub(crate) fn adjusted(depth: usize, text: &str, expression: bool) -> usize {
    let opening = if expression {
        ["(", "[", "{"]
    } else {
        ["(", "[", "<"]
    };
    let closing = if expression {
        [")", "]", "}"]
    } else {
        [")", "]", ">"]
    };
    if opening.contains(&text) {
        depth + 1
    } else if closing.contains(&text) {
        depth.saturating_sub(1)
    } else {
        depth
    }
}
fn continues(kind: K, expression: bool, terms: &[Token], next: &Token) -> bool {
    if next.kind == T::Eof || next.kind == T::Symbol && next.text == "}" {
        return false;
    }
    if expression {
        return kind == K::State && matches!(next.text.as_str(), "by" | "where" | "from")
            || terms
                .last()
                .is_some_and(|t| matches!(t.text.as_str(), "=" | "==" | "returns"))
            || next.text == "returns"
            || !keyword(next);
    }
    match kind {
        K::Operation => matches!(next.text.as_str(), "->" | "!" | "|"),
        K::Transition => matches!(next.text.as_str(), "from" | "to" | "when"),
        _ => data(kind) && next.text == "|",
    }
}
fn broad(kind: K) -> bool {
    data(kind)
        || matches!(
            kind,
            K::Domain
                | K::Module
                | K::External
                | K::Surface
                | K::Port
                | K::Decision
                | K::Example
                | K::Invariant
                | K::Lifecycle
                | K::Proof
                | K::Test
                | K::Review
                | K::Policy
                | K::Constraint
                | K::Intent
        )
}
fn allowed(parent: Option<K>, child: K) -> bool {
    match parent {
        None => broad(child) || matches!(child, K::Workspace | K::Fragment),
        Some(K::Workspace | K::Domain) => broad(child),
        Some(K::Fragment) => broad(child) || matches!(child, K::Owns | K::Requires | K::Operation),
        Some(K::Module) => matches!(
            child,
            K::Intent
                | K::Decision
                | K::Owns
                | K::Requires
                | K::Port
                | K::Surface
                | K::Example
                | K::Invariant
                | K::Lifecycle
                | K::Review
                | K::Constraint
        ),
        Some(K::External) => matches!(child, K::Intent | K::Surface),
        Some(K::Surface | K::Port) => matches!(child, K::Intent | K::Operation),
        Some(K::Operation) => {
            effect(child)
                || matches!(
                    child,
                    K::Intent | K::Pure | K::Effects | K::Requires | K::Ensures | K::State
                )
        }
        Some(K::Effects) => effect(child),
        Some(K::Owns) => child == K::Own,
        Some(K::Requires) => child == K::Dependency,
        Some(K::Decision) => matches!(child, K::Choose | K::Because | K::Avoid),
        Some(K::Example) => matches!(child, K::Given | K::When | K::Expect | K::Fails | K::Emits),
        Some(K::Invariant) => matches!(child, K::Forall | K::Assertion),
        Some(K::Lifecycle) => matches!(child, K::State | K::Transition),
        Some(K::Review) => matches!(child, K::Intent | K::Inspect | K::Criteria | K::Question),
        Some(K::Inspect) => child == K::Artifact,
        Some(K::Criteria) => child == K::Criterion,
        Some(K::Question) => child == K::Narrative,
        Some(K::Policy) => child == K::Require,
        Some(kind) if data(kind) => {
            matches!(
                child,
                K::Field | K::Range | K::Length | K::Format | K::Where | K::StableId
            ) || kind == K::Entity && child == K::Key
        }
        _ => false,
    }
}
fn context(statements: &[Statement], parent: Option<K>, diagnostics: &mut Vec<Diagnostic>) {
    for statement in statements {
        if !allowed(parent, statement.kind) {
            diagnostics.push(Diagnostic {
                related: Vec::new(),
                code: "DCTX0_STATEMENT_CONTEXT".into(),
                message: format!(
                    "{} is not allowed in {}",
                    statement.kind.as_str(),
                    parent.map_or("the source root", K::as_str)
                ),
                span: statement.span.clone(),
            });
        }
        context(&statement.children, Some(statement.kind), diagnostics);
    }
}
