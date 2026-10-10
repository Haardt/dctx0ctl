use super::*;

impl ExpressionParser<'_, '_> {
    pub(super) fn field(&mut self, target: Expression, name: &str, span: &Span) -> Expression {
        let base = expand(&target.ty, self.context.ir);
        let field = base
            .symbol_id
            .as_ref()
            .and_then(|id| self.context.ir.data.get(id))
            .and_then(|d| d.fields.iter().find(|f| f.name == name));
        let ty = field.map(|f| f.ty.clone()).unwrap_or_else(|| {
            problem(
                self.context.diagnostics,
                "DCTX0_UNKNOWN_FIELD",
                format!("Unknown field {name} on {}", target.ty.name),
                span,
            );
            TypeRef::builtin("Unit")
        });
        Expression {
            ty,
            span: span.clone(),
            value: ExpressionKind::Field {
                target: Box::new(target),
                name: name.into(),
            },
        }
    }
    pub(super) fn variable(
        &mut self,
        token: &Token,
        expected: Option<&TypeRef>,
    ) -> Option<Expression> {
        let mut parts = token.text.split('.');
        let root = parts.next()?;
        if let Some(ty) = self.context.variables.get(root).cloned() {
            let mut value = Expression {
                ty,
                span: token.span.clone(),
                value: ExpressionKind::Variable { name: root.into() },
            };
            for field in parts {
                value = self.field(value, field, &token.span);
            }
            return Some(value);
        }
        if token.text == "None" {
            return Some(Expression {
                ty: expected
                    .cloned()
                    .unwrap_or_else(|| TypeRef::builtin("Option")),
                span: token.span.clone(),
                value: ExpressionKind::Call {
                    name: "None".into(),
                    target: None,
                    arguments: Vec::new(),
                },
            });
        }
        if matches!(token.text.as_str(), "Down" | "Up" | "HalfUp" | "HalfEven") {
            return Some(Expression {
                ty: TypeRef::builtin("RoundingMode"),
                span: token.span.clone(),
                value: ExpressionKind::Variant {
                    name: token.text.clone(),
                    declaration: "RoundingMode".into(),
                },
            });
        }
        if let Some((declaration_name, variant_name)) = token.text.rsplit_once('.') {
            if let Ok(id) =
                self.context
                    .resolver
                    .resolve_symbol(declaration_name, self.context.owner, data)
            {
                if self.context.ir.data[&id]
                    .variants
                    .iter()
                    .any(|v| v.name == variant_name)
                {
                    let mut span = token.span.clone();
                    span.end.offset = span.start.offset + declaration_name.len();
                    span.end.column = span.start.column + declaration_name.len();
                    reference(
                        declaration_name,
                        &span,
                        self.context.owner,
                        ReferenceKind::UsesType,
                        self.context.resolver,
                        data,
                        (self.context.refs, self.context.diagnostics),
                    );
                    return Some(Expression {
                        ty: TypeRef {
                            name: self.context.ir.symbols[&id].name.clone(),
                            symbol_id: Some(id.clone()),
                            arguments: Vec::new(),
                        },
                        span: token.span.clone(),
                        value: ExpressionKind::Variant {
                            name: variant_name.into(),
                            declaration: id,
                        },
                    });
                }
            }
        }
        let expected_id = expected
            .map(|t| expand(t, self.context.ir))
            .and_then(|t| t.symbol_id);
        let mut variants = Vec::new();
        for declaration in self.context.ir.data.values() {
            if declaration.variants.iter().any(|v| v.name == token.text)
                || self.context.ir.symbols[&declaration.id].kind == StatementKind::Failure
                    && self.context.ir.symbols[&declaration.id].name == token.text
            {
                let score = self
                    .context
                    .owner
                    .split('.')
                    .zip(declaration.id.split('.'))
                    .take_while(|(a, b)| a == b)
                    .count();
                let score = score
                    + if expected_id.as_ref() == Some(&declaration.id) {
                        10000
                    } else {
                        0
                    };
                variants.push((score, declaration.id.clone()));
            }
        }
        variants.sort();
        if let Some((score, id)) = variants.last() {
            if variants.iter().filter(|(s, _)| s == score).count() > 1 {
                self.error(
                    "DCTX0_AMBIGUOUS_REFERENCE",
                    format!("Ambiguous variant {}", token.text),
                );
            }
            let id = id.clone();
            self.context.refs.push(Reference {
                from: self.context.owner.into(),
                target: id.clone(),
                kind: ReferenceKind::UsesType,
                spelling: token.text.clone(),
                span: token.span.clone(),
            });
            // Variant spelling is not a reference to the declaration's name;
            // rename uses the reference kind/spelling to distinguish it.
            return Some(Expression {
                ty: TypeRef {
                    name: self.context.ir.symbols[&id].name.clone(),
                    symbol_id: Some(id.clone()),
                    arguments: Vec::new(),
                },
                span: token.span.clone(),
                value: ExpressionKind::Variant {
                    name: token.text.clone(),
                    declaration: id,
                },
            });
        }
        if self.context.samples {
            let ty = expected
                .cloned()
                .unwrap_or_else(|| TypeRef::builtin("Sample"));
            self.context
                .variables
                .insert(token.text.clone(), ty.clone());
            return Some(Expression {
                ty,
                span: token.span.clone(),
                value: ExpressionKind::Variable {
                    name: token.text.clone(),
                },
            });
        }
        self.error(
            "DCTX0_UNBOUND_VARIABLE",
            format!("Unbound variable {}", token.text),
        );
        None
    }
    pub(super) fn record(
        &mut self,
        token: &Token,
        expected: Option<&TypeRef>,
    ) -> Option<Expression> {
        if self
            .context
            .resolver
            .resolve_symbol(&token.text, self.context.owner, data)
            .is_err()
        {
            let qualified = token.text.rsplit_once('.');
            let variant_name = qualified.map_or(token.text.as_str(), |(_, variant)| variant);
            let expected_id = expected
                .map(|ty| expand(ty, self.context.ir))
                .and_then(|ty| ty.symbol_id);
            let mut candidates = self
                .context
                .ir
                .data
                .values()
                .filter_map(|d| {
                    d.variants
                        .iter()
                        .find(|v| {
                            v.name == variant_name
                                && v.payload.is_some()
                                && qualified.is_none_or(|(declaration, _)| {
                                    d.id == declaration
                                        || d.id.ends_with(&format!(".{declaration}"))
                                })
                        })
                        .map(|v| (d.id.clone(), v.payload.clone().unwrap()))
                })
                .collect::<Vec<_>>();
            let rank = |id: &str| {
                self.context
                    .owner
                    .split('.')
                    .zip(id.split('.'))
                    .take_while(|(a, b)| a == b)
                    .count()
                    + if expected_id.as_deref() == Some(id) {
                        10000
                    } else {
                        0
                    }
            };
            candidates.sort_by_key(|(id, _)| std::cmp::Reverse(rank(id)));
            if let Some((id, payload)) = candidates.first().cloned() {
                if candidates
                    .iter()
                    .filter(|(other, _)| rank(other) == rank(&id))
                    .count()
                    > 1
                {
                    self.error(
                        "DCTX0_AMBIGUOUS_REFERENCE",
                        format!("Ambiguous variant constructor {}", token.text),
                    );
                    return None;
                }
                self.require("{");
                self.require("value");
                self.require("=");
                let value = self.expression(0, Some(&payload))?;
                self.check(&payload, &value);
                self.require("}");
                let mut span = token.span.clone();
                let spelling = if let Some((declaration, _)) = qualified {
                    span.end.offset = span.start.offset + declaration.len();
                    span.end.column = span.start.column + declaration.len();
                    declaration
                } else {
                    variant_name
                };
                self.context.refs.push(Reference {
                    from: self.context.owner.into(),
                    target: id.clone(),
                    kind: ReferenceKind::UsesType,
                    spelling: spelling.into(),
                    span,
                });
                return Some(Expression {
                    ty: TypeRef {
                        name: self.context.ir.symbols[&id].name.clone(),
                        symbol_id: Some(id.clone()),
                        arguments: Vec::new(),
                    },
                    span: token.span.clone(),
                    value: ExpressionKind::VariantRecord {
                        declaration: id,
                        variant: variant_name.into(),
                        payload: Box::new(value),
                    },
                });
            }
        }
        let id = reference(
            &token.text,
            &token.span,
            self.context.owner,
            ReferenceKind::UsesType,
            self.context.resolver,
            data,
            (self.context.refs, self.context.diagnostics),
        )?;
        let declaration = self.context.ir.data.get(&id)?.clone();
        self.require("{");
        let mut fields = Vec::new();
        let mut seen = BTreeSet::new();
        while !self.is("}") && self.index < self.tokens.len() {
            let name = self.tokens.get(self.index)?.clone();
            self.index += 1;
            self.require("=");
            let expected = declaration
                .fields
                .iter()
                .find(|f| f.name == name.text)
                .map(|f| f.ty.clone());
            if expected.is_none() {
                self.error(
                    "DCTX0_UNKNOWN_FIELD",
                    format!("Unknown field {}", name.text),
                );
            }
            let value = self.expression(0, expected.as_ref())?;
            if let Some(ty) = &expected {
                self.check(ty, &value);
            }
            if !seen.insert(name.text.clone()) {
                self.error("DCTX0_FIELD_DUPLICATE", "Duplicate record field");
            }
            fields.push((name.text, value));
            self.eat(",");
            self.eat(";");
        }
        self.require("}");
        for field in &declaration.fields {
            if !seen.contains(&field.name) {
                self.error(
                    "DCTX0_RECORD_FIELD_MISSING",
                    format!("Missing field {}", field.name),
                );
            }
        }
        Some(Expression {
            ty: TypeRef {
                name: self.context.ir.symbols[&id].name.clone(),
                symbol_id: Some(id.clone()),
                arguments: Vec::new(),
            },
            span: token.span.clone(),
            value: ExpressionKind::Record {
                declaration: id,
                fields,
            },
        })
    }
}
