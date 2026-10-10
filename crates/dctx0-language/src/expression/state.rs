use super::*;

impl ExpressionParser<'_, '_> {
    pub(super) fn trace(&mut self, token: &Token) -> Option<Expression> {
        let target = self.tokens.get(self.index)?.clone();
        self.index += 1;
        let kind = if token.text == "calls" {
            ReferenceKind::Calls
        } else if token.text == "emits" {
            ReferenceKind::Emits
        } else {
            ReferenceKind::State
        };
        let modeled = self
            .context
            .resolver
            .resolve_symbol(&target.text, self.context.owner, |k| match kind {
                ReferenceKind::Calls => k == StatementKind::Operation,
                ReferenceKind::Emits => k == StatementKind::Event,
                _ => k == StatementKind::Entity,
            });
        let id = match modeled {
            Ok(id) => {
                reference(
                    &target.text,
                    &target.span,
                    self.context.owner,
                    kind,
                    self.context.resolver,
                    |k| match kind {
                        ReferenceKind::Calls => k == StatementKind::Operation,
                        ReferenceKind::Emits => k == StatementKind::Event,
                        _ => k == StatementKind::Entity,
                    },
                    (self.context.refs, self.context.diagnostics),
                );
                Some(id)
            }
            Err(candidates) if candidates.is_empty() && kind == ReferenceKind::Calls => None,
            Err(_) => {
                self.error(
                    "DCTX0_UNKNOWN_REFERENCE",
                    format!("Invalid trace target {}", target.text),
                );
                return None;
            }
        };
        self.require("(");
        let expected = match &id {
            None => Vec::new(),
            Some(id) if kind == ReferenceKind::Calls => self.context.ir.operations[id]
                .parameters
                .iter()
                .map(|p| p.ty.clone())
                .collect::<Vec<_>>(),
            Some(id) => self.context.ir.data[id]
                .keys
                .iter()
                .filter_map(|key| {
                    self.context.ir.data[id]
                        .fields
                        .iter()
                        .find(|f| f.name == *key)
                        .map(|f| f.ty.clone())
                })
                .collect(),
        };
        let args = self.arguments(&expected)?;
        if kind == ReferenceKind::Calls || kind == ReferenceKind::Emits {
            self.require("exactly");
            let count = self.expression(9, Some(&TypeRef::builtin("Int")))?;
            self.check(&TypeRef::builtin("Int"), &count);
            return Some(Expression {
                ty: TypeRef::builtin("Boolean"),
                span: token.span.clone(),
                value: ExpressionKind::Trace {
                    trace_kind: token.text.clone(),
                    target: target.text,
                    modeled_target: id,
                    arguments: args,
                    count: Box::new(count),
                },
            });
        }
        if args.len() != expected.len() {
            self.error("DCTX0_EXPRESSION_ARITY", "Wrong entity key count");
        }
        Some(Expression {
            ty: TypeRef::builtin("Boolean"),
            span: token.span.clone(),
            value: ExpressionKind::Call {
                name: format!("{}:{}", token.text, id.as_deref().unwrap_or(&target.text)),
                target: id,
                arguments: args,
            },
        })
    }
    pub(super) fn state(&mut self, token: &Token) -> Option<Expression> {
        let (phase, rest) = token.text.split_once('.')?;
        if phase == "after" && !self.context.allow_after {
            self.error(
                "DCTX0_AFTER_STATE_CONTEXT",
                "after state is unavailable in preconditions",
            );
        }
        let parts = rest.split('.').collect::<Vec<_>>();
        let mut found = None;
        for count in (1..=parts.len()).rev() {
            let name = parts[..count].join(".");
            if let Ok(id) = self
                .context
                .resolver
                .resolve_symbol(&name, self.context.owner, |k| k == StatementKind::Entity)
            {
                found = Some((count, name, id));
                break;
            }
        }
        let Some((count, name, id)) = found else {
            self.error(
                "DCTX0_UNKNOWN_ENTITY",
                format!("Unknown state entity {rest}"),
            );
            return None;
        };
        let mut span = token.span.clone();
        span.start.offset += phase.len() + 1;
        span.start.column += phase.len() + 1;
        span.end.offset = span.start.offset + name.len();
        span.end.column = span.start.column + name.len();
        reference(
            &name,
            &span,
            self.context.owner,
            ReferenceKind::State,
            self.context.resolver,
            |k| k == StatementKind::Entity,
            (self.context.refs, self.context.diagnostics),
        );
        let declaration = &self.context.ir.data[&id];
        let expected = declaration
            .keys
            .iter()
            .filter_map(|k| {
                declaration
                    .fields
                    .iter()
                    .find(|f| f.name == *k)
                    .map(|f| f.ty.clone())
            })
            .collect::<Vec<_>>();
        let keyed = self.eat("(");
        let keys = if keyed {
            self.arguments(&expected)?
        } else {
            Vec::new()
        };
        if keyed && keys.len() != expected.len() {
            self.error("DCTX0_EXPRESSION_ARITY", "Wrong entity key count");
        }
        let entity = TypeRef {
            name: self.context.ir.symbols[&id].name.clone(),
            symbol_id: Some(id.clone()),
            arguments: Vec::new(),
        };
        let ty = if keyed {
            entity
        } else {
            TypeRef {
                name: "List".into(),
                symbol_id: None,
                arguments: vec![entity],
            }
        };
        let mut value = Expression {
            ty,
            span: token.span.clone(),
            value: ExpressionKind::State {
                phase: if phase == "after" {
                    StatePhase::After
                } else {
                    StatePhase::Before
                },
                entity: id,
                keys,
            },
        };
        for field in &parts[count..] {
            value = self.field(value, field, &token.span);
        }
        Some(value)
    }
}
