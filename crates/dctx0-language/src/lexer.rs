use dctx0_core::{Diagnostic, Position, Span, Token, TokenKind};

#[derive(Debug)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Tokenize UTF-8 source. Identifiers follow the reference ASCII grammar;
/// string contents and source positions support Unicode.
pub fn lex(file: &str, source: &str) -> LexResult {
    let mut scanner = Scanner {
        file,
        source,
        position: Position {
            offset: 0,
            line: 1,
            column: 1,
        },
        result: LexResult {
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        },
    };
    while let Some(ch) = scanner.peek() {
        let start = scanner.position;
        if matches!(ch, ' ' | '\t' | '\r') {
            scanner.advance();
        } else if scanner.rest().starts_with("//") {
            while scanner.peek().is_some_and(|c| c != '\n') {
                scanner.advance();
            }
        } else if scanner.rest().starts_with("/*") {
            scanner.consume(2);
            while scanner.peek().is_some() && !scanner.rest().starts_with("*/") {
                scanner.advance();
            }
            if scanner.peek().is_none() {
                scanner.problem("DCTX0_LEX_COMMENT", "Unterminated block comment", start);
            } else {
                scanner.consume(2);
            }
        } else if ch == '"' {
            scanner.string(start);
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let offset = scanner.position.offset;
            while scanner
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
            {
                scanner.advance();
            }
            let text = scanner.source[offset..scanner.position.offset].to_owned();
            scanner.push(TokenKind::Identifier, text, start);
        } else if ch.is_ascii_digit() {
            scanner.number(start);
        } else {
            let count = if ["->", "..", "!=", "<=", ">=", "=="]
                .iter()
                .any(|s| scanner.rest().starts_with(s))
            {
                2
            } else {
                1
            };
            let offset = scanner.position.offset;
            scanner.consume(count);
            let text = scanner.source[offset..scanner.position.offset].to_owned();
            scanner.push(
                if ch == '\n' {
                    TokenKind::Newline
                } else {
                    TokenKind::Symbol
                },
                text,
                start,
            );
        }
    }
    scanner.push(TokenKind::Eof, String::new(), scanner.position);
    scanner.result
}

struct Scanner<'a> {
    file: &'a str,
    source: &'a str,
    position: Position,
    result: LexResult,
}
impl Scanner<'_> {
    fn rest(&self) -> &str {
        &self.source[self.position.offset..]
    }
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn advance(&mut self) {
        // All callers check availability or consume a known delimiter.
        let ch = self
            .peek()
            .expect("scanner advances only over available characters");
        self.position.offset += ch.len_utf8();
        if ch == '\n' {
            self.position.line += 1;
            self.position.column = 1;
        } else {
            self.position.column += 1;
        }
    }
    fn consume(&mut self, count: usize) {
        for _ in 0..count {
            self.advance();
        }
    }
    fn span(&self, start: Position) -> Span {
        Span {
            file: self.file.to_owned(),
            start,
            end: self.position,
        }
    }
    fn push(&mut self, kind: TokenKind, text: String, start: Position) {
        self.result.tokens.push(Token {
            kind,
            text,
            span: self.span(start),
        });
    }
    fn problem(&mut self, code: &'static str, message: &str, start: Position) {
        self.result.diagnostics.push(Diagnostic {
            related: Vec::new(),
            code: code.into(),
            message: message.to_owned(),
            span: self.span(start),
        });
    }
    fn string(&mut self, start: Position) {
        let triple = self.rest().starts_with("\"\"\"");
        let delimiter = if triple { "\"\"\"" } else { "\"" };
        self.consume(delimiter.len());
        let mut text = String::new();
        while let Some(ch) = self.peek() {
            if self.rest().starts_with(delimiter) {
                break;
            }
            self.advance();
            if !triple && ch == '\\' && self.peek().is_some() {
                let escaped = self.peek().unwrap();
                text.push(match escaped {
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                });
                self.advance();
            } else {
                text.push(ch);
            }
        }
        if self.peek().is_some() {
            self.consume(delimiter.len());
        } else {
            self.problem(
                if triple {
                    "DCTX0_LEX_TRIPLE_STRING"
                } else {
                    "DCTX0_LEX_STRING"
                },
                "Unterminated string literal",
                start,
            );
        }
        self.push(
            if triple {
                TokenKind::TripleString
            } else {
                TokenKind::String
            },
            text,
            start,
        );
    }
    fn digits(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }
    }
    fn number(&mut self, start: Position) {
        self.digits();
        if self.rest().starts_with('.') && !self.rest().starts_with("..") {
            self.advance();
            let fraction = self.position.offset;
            self.digits();
            if fraction == self.position.offset {
                self.problem(
                    "DCTX0_LEX_NUMBER",
                    "Decimal requires digits after the decimal point",
                    start,
                );
            }
        }
        if self.peek().is_some_and(|c| matches!(c, 'e' | 'E')) {
            self.advance();
            if self.peek().is_some_and(|c| matches!(c, '+' | '-')) {
                self.advance();
            }
            let exponent = self.position.offset;
            self.digits();
            if exponent == self.position.offset {
                self.problem(
                    "DCTX0_LEX_NUMBER",
                    "Scientific literal requires an exponent",
                    start,
                );
            }
        }
        self.push(
            TokenKind::Number,
            self.source[start.offset..self.position.offset].to_owned(),
            start,
        );
    }
}
