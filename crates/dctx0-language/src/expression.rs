//! Pratt expression parsing; scoped variables and typed construction.
mod calls;
mod clauses;
mod constructors;
mod state;
mod types;
use crate::{
    composition::problem,
    kinds::is_data_declaration as data,
    resolution::{SymbolResolver, reference, type_ref},
    syntax::parse_type,
};
pub(crate) use clauses::compile_clauses;
use dctx0_core::*;
use std::collections::{BTreeMap, BTreeSet};
use types::{compatible, expand, expression_depth};

fn precedence(operator: &str) -> Option<u8> {
    Some(match operator {
        "implies" => 1,
        "or" => 2,
        "and" => 3,
        "==" | "!=" | "is" => 4,
        "<" | "<=" | ">" | ">=" | "in" => 5,
        ".." => 6,
        "+" | "-" => 7,
        "*" | "/" | "%" => 8,
        _ => return None,
    })
}
struct ExpressionContext<'a> {
    ir: &'a WorkspaceIr,
    resolver: &'a SymbolResolver<'a>,
    owner: &'a str,
    variables: BTreeMap<String, TypeRef>,
    samples: bool,
    allow_after: bool,
    refs: &'a mut Vec<Reference>,
    diagnostics: &'a mut Vec<Diagnostic>,
}
struct ExpressionParser<'a, 'b> {
    tokens: &'a [Token],
    index: usize,
    context: &'b mut ExpressionContext<'a>,
    depth: usize,
}
impl ExpressionParser<'_, '_> {
    fn text(&self) -> &str {
        self.tokens.get(self.index).map_or("", |t| t.text.as_str())
    }
    fn is(&self, text: &str) -> bool {
        self.text() == text
            && self
                .tokens
                .get(self.index)
                .is_some_and(|t| !matches!(t.kind, TokenKind::String | TokenKind::TripleString))
    }
    fn eat(&mut self, text: &str) -> bool {
        if self.is(text) {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn error(&mut self, code: &str, message: impl Into<String>) {
        if let Some(token) = self.tokens.get(self.index).or_else(|| self.tokens.last()) {
            problem(self.context.diagnostics, code, message, &token.span);
        }
    }
    fn require(&mut self, text: &str) {
        if !self.eat(text) {
            self.error("DCTX0_INVALID_EXPRESSION", format!("Expected {text}"));
        }
    }
    fn expression(&mut self, min: u8, expected: Option<&TypeRef>) -> Option<Expression> {
        if self.depth >= 128 {
            self.error("DCTX0_EXPRESSION_DEPTH", "Expression nesting exceeds 128");
            return None;
        }
        self.depth += 1;
        let mut left = self.prefix(expected)?;
        while let Some(p) = precedence(self.text()) {
            if p < min {
                break;
            }
            let operator = self.text().to_owned();
            self.index += 1;
            let right = self.expression(
                if operator == "implies" { p } else { p + 1 },
                Some(&left.ty),
            )?;
            let a = expand(&left.ty, self.context.ir);
            let b = expand(&right.ty, self.context.ir);
            let numeric = |t: &TypeRef| matches!(t.name.as_str(), "Int" | "Decimal" | "Float");
            let valid = match operator.as_str() {
                "and" | "or" | "implies" => a.name == "Boolean" && b.name == "Boolean",
                "is" => true,
                "in" => b.name == "List" || b.name == "Range",
                "+" => numeric(&a) && numeric(&b) || a.name == "String" && b.name == "String",
                "-" | "*" | "/" | "%" | ".." | "<" | "<=" | ">" | ">=" => {
                    numeric(&a) && numeric(&b)
                        || a.name == b.name && matches!(a.name.as_str(), "Timestamp" | "Duration")
                }
                _ => {
                    compatible(&left.ty, &right.ty, self.context.ir)
                        || compatible(&right.ty, &left.ty, self.context.ir)
                }
            };
            if !valid {
                self.error(
                    "DCTX0_EXPRESSION_TYPE",
                    format!("Invalid {operator} operands {} and {}", a.name, b.name),
                );
            }
            let ty = if matches!(
                operator.as_str(),
                "==" | "!=" | "<" | "<=" | ">" | ">=" | "in" | "and" | "or" | "implies" | "is"
            ) {
                TypeRef::builtin("Boolean")
            } else if operator == ".." {
                TypeRef {
                    name: "Range".into(),
                    symbol_id: None,
                    arguments: vec![left.ty.clone()],
                }
            } else if a.name == "Float" || b.name == "Float" {
                TypeRef::builtin("Float")
            } else if a.name == "Decimal" || b.name == "Decimal" {
                TypeRef::builtin("Decimal")
            } else {
                a
            };
            if expression_depth(&left).max(expression_depth(&right)) >= 128 {
                self.error(
                    "DCTX0_EXPRESSION_DEPTH",
                    "Expression tree exceeds 128 levels",
                );
                return None;
            }
            let span = crate::parser::merge(&left.span, &right.span);
            left = Expression {
                ty,
                span,
                value: ExpressionKind::Binary {
                    operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
        }
        self.depth -= 1;
        Some(left)
    }
    fn prefix(&mut self, expected: Option<&TypeRef>) -> Option<Expression> {
        let token = self.tokens.get(self.index)?.clone();
        self.index += 1;
        if token.kind == TokenKind::Identifier && token.text.matches('.').count() >= 128 {
            self.error("DCTX0_EXPRESSION_DEPTH", "Field path exceeds 128 levels");
            return None;
        }
        let mut expression = if token.kind == TokenKind::String
            || token.kind == TokenKind::Number
            || matches!(token.text.as_str(), "true" | "false")
        {
            let kind = if token.kind == TokenKind::String {
                "String"
            } else if token.kind == TokenKind::Number {
                if token.text.contains(['e', 'E']) {
                    "Float"
                } else if token.text.contains('.') {
                    "Decimal"
                } else {
                    "Int"
                }
            } else {
                "Boolean"
            };
            Expression {
                ty: TypeRef::builtin(kind),
                span: token.span.clone(),
                value: ExpressionKind::Literal {
                    text: token.text.clone(),
                    literal_kind: kind.into(),
                },
            }
        } else if matches!(token.text.as_str(), "not" | "-") {
            let operand = self.expression(9, expected)?;
            let base = expand(&operand.ty, self.context.ir);
            if (token.text == "not" && base.name != "Boolean")
                || (token.text == "-" && !matches!(base.name.as_str(), "Int" | "Float" | "Decimal"))
            {
                self.error("DCTX0_EXPRESSION_TYPE", "Invalid unary operand");
            }
            Expression {
                ty: if token.text == "not" {
                    TypeRef::builtin("Boolean")
                } else {
                    operand.ty.clone()
                },
                span: token.span.clone(),
                value: ExpressionKind::Unary {
                    operator: token.text.clone(),
                    operand: Box::new(operand),
                },
            }
        } else if token.text == "(" {
            let value = self.expression(0, expected)?;
            self.require(")");
            value
        } else if token.text == "[" {
            let element = expected
                .map(|t| expand(t, self.context.ir))
                .filter(|t| t.name == "List")
                .and_then(|t| t.arguments.first().cloned());
            let mut values = Vec::new();
            while !self.is("]") && self.index < self.tokens.len() {
                values.push(self.expression(0, element.as_ref())?);
                if !self.eat(",") {
                    break;
                }
            }
            self.require("]");
            let element = element
                .or_else(|| values.first().map(|v| v.ty.clone()))
                .unwrap_or_else(|| TypeRef::builtin("Unit"));
            for value in &values {
                self.check(&element, value);
            }
            Expression {
                ty: TypeRef {
                    name: "List".into(),
                    symbol_id: None,
                    arguments: vec![element],
                },
                span: token.span.clone(),
                value: ExpressionKind::List { values },
            }
        } else if matches!(
            token.text.as_str(),
            "exists" | "preserves" | "creates" | "updates" | "removes" | "calls" | "emits"
        ) {
            self.trace(&token)?
        } else if token.text.starts_with("before.") || token.text.starts_with("after.") {
            self.state(&token)?
        } else if self.is("{") {
            self.record(&token, expected)?
        } else if self.is("(") {
            self.call(&token, expected)?
        } else {
            self.variable(&token, expected)?
        };
        while self.eat(".") {
            if expression_depth(&expression) >= 128 {
                self.error(
                    "DCTX0_EXPRESSION_DEPTH",
                    "Field expression exceeds 128 levels",
                );
                return None;
            }
            let field = self.tokens.get(self.index)?.clone();
            self.index += 1;
            for name in field.text.split('.') {
                expression = self.field(expression, name, &field.span);
            }
        }
        if let Some(last) = self.tokens.get(self.index.saturating_sub(1)) {
            expression.span = crate::parser::merge(&token.span, &last.span);
        }
        Some(expression)
    }
    fn check(&mut self, expected: &TypeRef, value: &Expression) {
        if !compatible(expected, &value.ty, self.context.ir) {
            self.error(
                "DCTX0_EXPRESSION_TYPE",
                format!("Expected {}, found {}", expected.name, value.ty.name),
            );
        }
    }
}
fn restore(variables: &mut BTreeMap<String, TypeRef>, binder: &str, previous: Option<TypeRef>) {
    if let Some(ty) = previous {
        variables.insert(binder.into(), ty);
    } else {
        variables.remove(binder);
    }
}
fn top_level(tokens: &[Token], start: usize, marker: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, t) in tokens.iter().enumerate().skip(start) {
        if matches!(t.text.as_str(), "(" | "[" | "{") {
            depth += 1;
        }
        if matches!(t.text.as_str(), ")" | "]" | "}") {
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        if depth == 0 && t.kind == TokenKind::Identifier && t.text == marker {
            return Some(i);
        }
    }
    None
}
fn parse_expression<'a>(
    tokens: &'a [Token],
    context: &mut ExpressionContext<'a>,
    expected: Option<&TypeRef>,
) -> Option<Expression> {
    if tokens.is_empty() {
        problem(
            context.diagnostics,
            "DCTX0_INVALID_EXPRESSION",
            "Expression is empty",
            &context.ir.symbols[context.owner].span,
        );
        return None;
    }
    let mut parser = ExpressionParser {
        tokens,
        index: 0,
        context,
        depth: 0,
    };
    let mut result = parser.expression(0, expected);
    if result
        .as_ref()
        .is_some_and(|value| expression_depth(value) > 128)
    {
        parser.error(
            "DCTX0_EXPRESSION_DEPTH",
            "Expression tree exceeds 128 levels",
        );
        result = None;
    }
    if result.is_none() || parser.index != tokens.len() {
        parser.error(
            "DCTX0_INVALID_EXPRESSION",
            "Expression was not completely consumed",
        );
    }
    result
}
