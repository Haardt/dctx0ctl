use crate::{
    kinds::{is_data_declaration as data, is_effect_clause as effect},
    parser::{adjusted, merge},
};
use dctx0_core::{
    Diagnostic, EffectPayload, Parameter, Span, Statement, StatementKind as K, Syntax, Token,
    TokenKind as T, TypeSyntax, Variant,
};

fn problem(diagnostics: &mut Vec<Diagnostic>, code: &'static str, message: &str, span: &Span) {
    diagnostics.push(Diagnostic {
        related: Vec::new(),
        code: code.into(),
        message: message.to_owned(),
        span: span.clone(),
    });
}

pub(crate) fn parse_type(
    tokens: &[Token],
    fallback: &Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypeSyntax> {
    let mut cursor = 0;
    let result = ty(tokens, &mut cursor, fallback, diagnostics, 0);
    if result.is_some() && cursor < tokens.len() {
        problem(
            diagnostics,
            "DCTX0_TYPE_TRAILING_TOKENS",
            "Unexpected tokens after type",
            &merge(&tokens[cursor].span, &tokens.last().unwrap().span),
        );
    }
    result
}
fn ty(
    tokens: &[Token],
    cursor: &mut usize,
    fallback: &Span,
    diagnostics: &mut Vec<Diagnostic>,
    depth: usize,
) -> Option<TypeSyntax> {
    if depth >= 128 {
        problem(
            diagnostics,
            "DCTX0_TYPE_DEPTH",
            "Type nesting exceeds 128",
            fallback,
        );
        *cursor = tokens.len();
        return None;
    }
    let Some(name) = tokens.get(*cursor).filter(|t| t.kind == T::Identifier) else {
        problem(
            diagnostics,
            "DCTX0_TYPE_EXPECTED",
            "Expected a type name",
            tokens.get(*cursor).map_or(fallback, |t| &t.span),
        );
        return None;
    };
    *cursor += 1;
    let mut arguments = Vec::new();
    if tokens.get(*cursor).is_some_and(|t| t.text == "<") {
        let opening = &tokens[*cursor].span;
        *cursor += 1;
        let mut expects = true;
        while *cursor < tokens.len() && tokens[*cursor].text != ">" {
            if !expects {
                problem(
                    diagnostics,
                    "DCTX0_TYPE_TRAILING_TOKENS",
                    "Expected ',' or '>' after type argument",
                    &tokens[*cursor].span,
                );
                break;
            }
            if tokens[*cursor].text == "," {
                problem(
                    diagnostics,
                    "DCTX0_TYPE_EXPECTED",
                    "Type arguments cannot be empty",
                    &tokens[*cursor].span,
                );
                *cursor += 1;
                continue;
            }
            let before = *cursor;
            if let Some(argument) = ty(tokens, cursor, fallback, diagnostics, depth + 1) {
                arguments.push(argument);
            }
            expects = false;
            if *cursor == before {
                break;
            }
            if tokens.get(*cursor).is_some_and(|t| t.text == ",") {
                *cursor += 1;
                expects = true;
            }
        }
        if tokens.get(*cursor).is_some_and(|t| t.text == ">") {
            if expects {
                problem(
                    diagnostics,
                    "DCTX0_TYPE_EXPECTED",
                    "Type arguments cannot be empty",
                    &tokens[*cursor].span,
                );
            }
            *cursor += 1;
        } else {
            problem(
                diagnostics,
                "DCTX0_TYPE_ARGUMENTS_UNCLOSED",
                "Type arguments are not closed",
                opening,
            );
        }
    }
    let end = tokens.get(cursor.saturating_sub(1)).unwrap_or(name);
    Some(TypeSyntax {
        name: name.text.clone(),
        arguments,
        span: merge(&name.span, &end.span),
    })
}

pub(crate) fn statement(
    kind: K,
    terms: &[Token],
    children: &[Statement],
    span: &Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Syntax> {
    if data(kind) {
        return Some(data_syntax(terms, children, span, diagnostics));
    }
    if effect(kind) {
        return effect_syntax(kind, terms, span, diagnostics);
    }
    match kind {
        K::Operation => Some(operation(terms, span, diagnostics)),
        K::Fragment => Some(Syntax::Fragment {
            target: terms.iter().map(|t| t.text.as_str()).collect(),
            target_span: terms.first().map_or_else(
                || span.clone(),
                |first| merge(&first.span, &terms.last().unwrap().span),
            ),
        }),
        K::StableId => Some(Syntax::StableId {
            value: terms.first().map_or_else(String::new, |t| t.text.clone()),
            value_span: terms
                .first()
                .map_or_else(|| span.clone(), |t| t.span.clone()),
            quoted: terms.first().is_some_and(|t| t.kind == T::String),
        }),
        K::Intent | K::Choose | K::Because | K::Avoid | K::Constraint => Some(Syntax::Narrative {
            text: terms.first().map_or_else(String::new, |t| t.text.clone()),
            narrative: terms.first().is_some_and(|t| t.kind == T::TripleString),
        }),
        _ => None,
    }
}
fn split<'a>(tokens: &'a [Token], separator: &str) -> Vec<&'a [Token]> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate() {
        if token.text == separator && token.kind == T::Symbol && depth == 0 {
            result.push(&tokens[start..index]);
            start = index + 1;
        } else if token.kind == T::Symbol {
            depth = adjusted(depth, &token.text, false);
        }
    }
    result.push(&tokens[start..]);
    result
}
fn closing(tokens: &[Token], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if token.kind == T::Symbol && token.text == "(" {
            depth += 1;
        }
        if token.kind == T::Symbol && token.text == ")" {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}
fn parameters(tokens: &[Token], span: &Span, diagnostics: &mut Vec<Diagnostic>) -> Vec<Parameter> {
    if tokens.is_empty() {
        return Vec::new();
    }
    split(tokens, ",")
        .into_iter()
        .filter_map(|part| {
            if part.len() < 2 || part[0].kind != T::Identifier || part[1].text != ":" {
                problem(
                    diagnostics,
                    "DCTX0_TYPE_EXPECTED",
                    "Parameters must use name: Type syntax",
                    part.first().map_or(span, |t| &t.span),
                );
                return None;
            }
            parse_type(&part[2..], span, diagnostics).map(|ty| Parameter {
                name: part[0].text.clone(),
                ty,
                span: merge(&part[0].span, &part.last().unwrap().span),
            })
        })
        .collect()
}
fn operation(terms: &[Token], span: &Span, diagnostics: &mut Vec<Diagnostic>) -> Syntax {
    let open = terms.iter().position(|t| t.text == "(");
    let close = open.and_then(|i| closing(terms, i));
    let parameters = match (open, close) {
        (Some(o), Some(c)) => parameters(&terms[o + 1..c], span, diagnostics),
        _ => Vec::new(),
    };
    let arrow = terms.iter().position(|t| t.text == "->");
    let result = arrow.and_then(|a| {
        let end = terms[a + 1..]
            .iter()
            .position(|t| t.text == "!")
            .map_or(terms.len(), |i| a + 1 + i);
        parse_type(&terms[a + 1..end], span, diagnostics)
    });
    let failures = terms
        .iter()
        .enumerate()
        .filter(|(_, t)| t.text == "!")
        .filter_map(|(i, _)| {
            let end = terms[i + 1..]
                .iter()
                .position(|t| t.text == "!")
                .map_or(terms.len(), |j| i + 1 + j);
            parse_type(&terms[i + 1..end], span, diagnostics)
        })
        .collect();
    if open != Some(1)
        || close.is_none()
        || arrow != close.map(|c| c + 1)
        || result.is_none()
        || terms
            .iter()
            .any(|t| matches!(t.text.as_str(), "returns" | "uses"))
    {
        problem(
            diagnostics,
            "DCTX0_OPERATION_SIGNATURE",
            "Use operation name(parameter: Type) -> Result followed by optional ! Failure clauses",
            span,
        );
    }
    Syntax::Operation {
        parameters,
        result,
        failures,
    }
}
fn data_syntax(
    terms: &[Token],
    children: &[Statement],
    span: &Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Syntax {
    let body = terms
        .iter()
        .position(|t| t.text == "=")
        .map(|i| &terms[i + 1..]);
    let mut variants = Vec::new();
    let mut alias = None;
    if let Some(body) = body {
        if body.iter().any(|t| t.text == "|") {
            for part in split(body, "|") {
                let Some(name) = part.first().filter(|t| t.kind == T::Identifier) else {
                    problem(
                        diagnostics,
                        "DCTX0_UNION_VARIANT_INVALID",
                        "Union variants require a name",
                        span,
                    );
                    continue;
                };
                let payload = if part.len() == 1 {
                    None
                } else if part[1].text == "(" && closing(part, 1) == Some(part.len() - 1) {
                    let payload = parse_type(&part[2..part.len() - 1], span, diagnostics);
                    if payload.is_none() {
                        problem(
                            diagnostics,
                            "DCTX0_UNION_VARIANT_INVALID",
                            "Union variant requires a valid payload type",
                            span,
                        );
                        continue;
                    }
                    payload
                } else {
                    problem(
                        diagnostics,
                        "DCTX0_UNION_VARIANT_INVALID",
                        "Union variants must be Name or Name(PayloadType)",
                        span,
                    );
                    continue;
                };
                if variants.iter().any(|v: &Variant| v.name == name.text) {
                    problem(
                        diagnostics,
                        "DCTX0_UNION_VARIANT_DUPLICATE",
                        "Duplicate union variant",
                        span,
                    );
                }
                variants.push(Variant {
                    name: name.text.clone(),
                    payload,
                });
            }
        } else {
            alias = parse_type(body, span, diagnostics);
        }
    }
    let fields = children
        .iter()
        .filter_map(|child| match (child.kind, &child.syntax) {
            (K::Field, Some(Syntax::Data { fields, .. })) => Some(fields.as_slice()),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect();
    Syntax::Data {
        alias,
        fields,
        variants,
    }
}
fn effect_syntax(
    kind: K,
    terms: &[Token],
    span: &Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Syntax> {
    let Some(target) = terms.first().filter(|t| t.kind == T::Identifier) else {
        problem(
            diagnostics,
            "DCTX0_EFFECT_TARGET",
            "Effect requires a target",
            span,
        );
        return None;
    };
    let rest = &terms[1..];
    let mut result_alias = None;
    let mut payload = None;
    if !rest.is_empty() {
        match kind {
            K::Call => {
                if rest.len() == 2 && rest[0].text == "as" && rest[1].kind == T::Identifier {
                    result_alias = Some(rest[1].text.clone());
                } else {
                    problem(
                        diagnostics,
                        "DCTX0_CALL_RESULT_ALIAS",
                        "call must use optional 'as alias' syntax",
                        span,
                    );
                }
            }
            K::Emit => {
                if rest[0].text != "(" || closing(rest, 0) != Some(rest.len() - 1) {
                    problem(
                        diagnostics,
                        "DCTX0_EMIT_PAYLOAD_SYNTAX",
                        "emit payload must be enclosed in parentheses",
                        span,
                    );
                } else {
                    let body = &rest[1..rest.len() - 1];
                    let parts = split(body, ",");
                    payload = Some(
                        if parts
                            .iter()
                            .any(|p| p.iter().any(|t| t.kind == T::Symbol && t.text == ":"))
                        {
                            let fields = parts
                                .into_iter()
                                .filter_map(|part| {
                                    if part.len() < 3
                                        || part[0].kind != T::Identifier
                                        || part[1].text != ":"
                                    {
                                        problem(
                                            diagnostics,
                                            "DCTX0_EMIT_PAYLOAD_SYNTAX",
                                            "emit record fields must use field: expression",
                                            span,
                                        );
                                        None
                                    } else {
                                        Some((part[0].text.clone(), part[2..].to_vec()))
                                    }
                                })
                                .collect();
                            EffectPayload::Record { fields }
                        } else {
                            EffectPayload::Scalar {
                                expression: body.to_vec(),
                            }
                        },
                    );
                }
            }
            _ => problem(
                diagnostics,
                "DCTX0_EFFECT_SYNTAX",
                "Effect accepts exactly one target",
                span,
            ),
        }
    }
    Some(Syntax::Effect {
        target: target.text.clone(),
        result_alias,
        payload,
    })
}
