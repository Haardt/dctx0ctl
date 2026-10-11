use super::*;

impl ExpressionParser<'_, '_> {
    pub(super) fn arguments(&mut self, expected: &[TypeRef]) -> Option<Vec<Expression>> {
        let mut args = Vec::new();
        while !self.is(")") && self.index < self.tokens.len() {
            let value = self.expression(0, expected.get(args.len()))?;
            if let Some(ty) = expected.get(args.len()) {
                self.check(ty, &value);
            }
            args.push(value);
            if !self.eat(",") {
                break;
            }
        }
        self.require(")");
        Some(args)
    }
    pub(super) fn call(&mut self, token: &Token, expected: Option<&TypeRef>) -> Option<Expression> {
        self.require("(");
        if matches!(
            token.text.as_str(),
            "count"
                | "sum"
                | "all"
                | "any"
                | "contains"
                | "distinct"
                | "min"
                | "max"
                | "filter"
                | "map"
                | "sortBy"
                | "sortDescBy"
        ) {
            return self.collection(token);
        }
        let operation = self
            .context
            .resolver
            .resolve_symbol(&token.text, self.context.owner, |k| {
                k == StatementKind::Operation
            })
            .ok();
        let parameters = operation
            .as_ref()
            .map(|id| {
                self.context.ir.operations[id]
                    .parameters
                    .iter()
                    .map(|p| p.ty.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                if token.text == "Some" {
                    expected
                        .map(|t| expand(t, self.context.ir))
                        .and_then(|t| t.arguments.first().cloned())
                        .into_iter()
                        .collect()
                } else if token.text == "ifElse" {
                    let mut parameters = vec![TypeRef::builtin("Boolean")];
                    if let Some(expected) = expected {
                        parameters.extend([expected.clone(), expected.clone()]);
                    }
                    parameters
                } else {
                    Vec::new()
                }
            });
        let args = self.arguments(&parameters)?;
        let ty = if let Some(id) = &operation {
            reference(
                &token.text,
                &token.span,
                self.context.owner,
                ReferenceKind::Calls,
                self.context.resolver,
                |k| k == StatementKind::Operation,
                (self.context.refs, self.context.diagnostics),
            );
            if args.len() != parameters.len() {
                self.error("DCTX0_EXPRESSION_ARITY", "Wrong operation argument count");
            }
            self.context.ir.operations[id].result.clone()
        } else {
            let (arity, result) = match token.text.as_str() {
                "Some" => (1, "Option"),
                "first" => (1, "Option"),
                "single" => (1, "Unit"),
                "ifElse" => (3, "Unit"),
                "mean" => (1, "Option"),
                "indexOf" => (2, "Int"),
                "getOrElse" => (2, "Unit"),
                "None" => (0, "Option"),
                "lowercase" | "uppercase" | "trim" => (1, "String"),
                "length" => (1, "Int"),
                "startsWith" | "endsWith" | "includesText" => (2, "Boolean"),
                "slice" => (3, "List"),
                "reverse" => (1, "List"),
                "divide" => (4, "Decimal"),
                "round" => (3, "Decimal"),
                _ => {
                    self.error(
                        "DCTX0_UNKNOWN_FUNCTION",
                        format!("Unknown function {}", token.text),
                    );
                    (args.len(), "Unit")
                }
            };
            if args.len() != arity {
                self.error(
                    "DCTX0_EXPRESSION_ARITY",
                    format!("{} expects {arity} arguments", token.text),
                );
            }
            match token.text.as_str() {
                "ifElse" => {
                    if let Some(condition) = args.first() {
                        self.check(&TypeRef::builtin("Boolean"), condition);
                    }
                    let ty = args
                        .get(1)
                        .map(|value| value.ty.clone())
                        .unwrap_or_else(|| TypeRef::builtin("Unit"));
                    if let Some(alternative) = args.get(2) {
                        self.check(&ty, alternative);
                    }
                    ty
                }
                "slice" => {
                    if let Some(list) = args.first() {
                        self.element(list);
                    }
                    for value in args.iter().skip(1) {
                        self.check(&TypeRef::builtin("Int"), value);
                    }
                    args.first()
                        .map(|value| value.ty.clone())
                        .unwrap_or_else(|| TypeRef::builtin("List"))
                }
                "single" => args
                    .first()
                    .map(|value| self.element(value))
                    .unwrap_or_else(|| TypeRef::builtin("Unit")),
                "mean" => {
                    if let Some(value) = args.first() {
                        let element = self.element(value);
                        if !matches!(
                            expand(&element, self.context.ir).name.as_str(),
                            "Int" | "Decimal"
                        ) {
                            self.error(
                                "DCTX0_EXPRESSION_TYPE",
                                "mean requires exact numeric elements",
                            );
                        }
                    }
                    TypeRef {
                        name: "Option".into(),
                        symbol_id: None,
                        arguments: vec![TypeRef::builtin("Float")],
                    }
                }
                "indexOf" => {
                    if let Some(list) = args.first() {
                        let element = self.element(list);
                        if let Some(value) = args.get(1) {
                            self.check(&element, value);
                        }
                    }
                    TypeRef::builtin("Int")
                }
                "getOrElse" => {
                    let option = args.first().map(|value| expand(&value.ty, self.context.ir));
                    if option
                        .as_ref()
                        .is_none_or(|ty| ty.name != "Option" || ty.arguments.len() != 1)
                    {
                        self.error("DCTX0_EXPRESSION_TYPE", "getOrElse requires an Option");
                    }
                    let ty = option
                        .and_then(|ty| ty.arguments.into_iter().next())
                        .unwrap_or_else(|| TypeRef::builtin("Unit"));
                    if let Some(fallback) = args.get(1) {
                        self.check(&ty, fallback);
                    }
                    ty
                }
                "first" => TypeRef {
                    name: "Option".into(),
                    symbol_id: None,
                    arguments: args
                        .first()
                        .map(|value| self.element(value))
                        .into_iter()
                        .collect(),
                },
                "Some" => TypeRef {
                    name: "Option".into(),
                    symbol_id: None,
                    arguments: args.first().map(|a| a.ty.clone()).into_iter().collect(),
                },
                "reverse" => {
                    if let Some(a) = args.first() {
                        if expand(&a.ty, self.context.ir).name != "List" {
                            self.error("DCTX0_EXPRESSION_TYPE", "reverse requires a List");
                        }
                        a.ty.clone()
                    } else {
                        TypeRef::builtin("List")
                    }
                }
                "lowercase" | "uppercase" | "trim" | "startsWith" | "endsWith" | "includesText" => {
                    for a in &args {
                        self.check(&TypeRef::builtin("String"), a);
                    }
                    TypeRef::builtin(result)
                }
                "length" => {
                    if args.first().is_some_and(|a| {
                        !matches!(
                            expand(&a.ty, self.context.ir).name.as_str(),
                            "String" | "List"
                        )
                    }) {
                        self.error("DCTX0_EXPRESSION_TYPE", "length requires String or List");
                    }
                    TypeRef::builtin(result)
                }
                _ => TypeRef::builtin(result),
            }
        };
        Some(Expression {
            ty,
            span: token.span.clone(),
            value: ExpressionKind::Call {
                name: token.text.clone(),
                target: operation,
                arguments: args,
            },
        })
    }
    pub(super) fn collection(&mut self, token: &Token) -> Option<Expression> {
        let marker = top_level(self.tokens, self.index, "for");
        if let Some(marker) = marker
            .filter(|_| matches!(token.text.as_str(), "sum" | "map" | "sortBy" | "sortDescBy"))
        {
            let projection = &self.tokens[self.index..marker];
            let binder = self.tokens.get(marker + 1)?.text.clone();
            self.index = marker + 2;
            self.require("in");
            let collection = self.expression(0, None)?;
            let element = self.element(&collection);
            let previous = self.context.variables.insert(binder.clone(), element);
            let value = parse_expression(projection, self.context, None)?;
            restore(&mut self.context.variables, &binder, previous);
            if matches!(token.text.as_str(), "sortBy" | "sortDescBy")
                && !matches!(
                    expand(&value.ty, self.context.ir).name.as_str(),
                    "Int" | "Decimal" | "Float" | "String" | "UUID"
                )
            {
                self.error(
                    "DCTX0_EXPRESSION_TYPE",
                    "sortBy requires a numeric or textual key",
                );
            }
            self.require(")");
            return Some(Expression {
                ty: if token.text == "map" {
                    TypeRef {
                        name: "List".into(),
                        symbol_id: None,
                        arguments: vec![value.ty.clone()],
                    }
                } else if matches!(token.text.as_str(), "sortBy" | "sortDescBy") {
                    collection.ty.clone()
                } else {
                    value.ty.clone()
                },
                span: token.span.clone(),
                value: ExpressionKind::Collection {
                    operator: token.text.clone(),
                    binder: Some(binder),
                    collection: Box::new(collection),
                    predicate: None,
                    projection: Some(Box::new(value)),
                },
            });
        }
        if matches!(token.text.as_str(), "map" | "sortBy" | "sortDescBy") {
            self.error(
                "DCTX0_EXPRESSION_ARITY",
                "map requires a projection followed by for and a bound collection",
            );
            return None;
        }
        if self
            .tokens
            .get(self.index + 1)
            .is_some_and(|t| t.text == "in")
        {
            let binder = self.text().to_owned();
            self.index += 2;
            let collection = self.expression(0, None)?;
            let element = self.element(&collection);
            let previous = self.context.variables.insert(binder.clone(), element);
            let predicate = if self.eat("where") {
                let p = self.expression(0, None)?;
                self.check(&TypeRef::builtin("Boolean"), &p);
                Some(Box::new(p))
            } else {
                None
            };
            restore(&mut self.context.variables, &binder, previous);
            self.require(")");
            return Some(Expression {
                ty: if token.text == "filter" {
                    collection.ty.clone()
                } else {
                    TypeRef::builtin(if token.text == "count" {
                        "Int"
                    } else {
                        "Boolean"
                    })
                },
                span: token.span.clone(),
                value: ExpressionKind::Collection {
                    operator: token.text.clone(),
                    binder: Some(binder),
                    collection: Box::new(collection),
                    predicate,
                    projection: None,
                },
            });
        }
        if token.text == "filter" {
            self.error(
                "DCTX0_EXPRESSION_ARITY",
                "filter requires a bound element and collection",
            );
            return None;
        }
        let args = self.arguments(&[])?;
        if args.len() != if token.text == "contains" { 2 } else { 1 } {
            self.error("DCTX0_EXPRESSION_ARITY", "Wrong collection argument count");
        }
        let collection = args.first()?.clone();
        let element = self.element(&collection);
        if let Some(value) = args.get(1) {
            self.check(&element, value);
        }
        let ty = match token.text.as_str() {
            "count" => TypeRef::builtin("Int"),
            "all" | "any" | "contains" => TypeRef::builtin("Boolean"),
            "distinct" => collection.ty.clone(),
            _ => element,
        };
        Some(Expression {
            ty,
            span: token.span.clone(),
            value: ExpressionKind::Collection {
                operator: token.text.clone(),
                binder: None,
                collection: Box::new(collection),
                predicate: args.get(1).cloned().map(Box::new),
                projection: None,
            },
        })
    }
    pub(super) fn element(&mut self, collection: &Expression) -> TypeRef {
        let ty = expand(&collection.ty, self.context.ir);
        if ty.name != "List" {
            self.error("DCTX0_EXPRESSION_TYPE", "Collection requires List");
        }
        ty.arguments
            .first()
            .cloned()
            .unwrap_or_else(|| TypeRef::builtin("Unit"))
    }
}
