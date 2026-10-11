//! Build operation, invariant and example variable scopes.
use super::*;

/// Build the lexical variables available to one semantic clause.
pub(super) fn variables_for_clause(
    node: &SemanticNode,
    ir: &WorkspaceIr,
    resolver: &SymbolResolver<'_>,
    owned: &BTreeMap<&str, Vec<&SemanticNode>>,
    refs: &mut Vec<Reference>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, TypeRef> {
    let mut variables = BTreeMap::new();
    if let Some(operation) = ir.operations.get(&node.owner) {
        variables.extend(
            operation
                .parameters
                .iter()
                .map(|p| (p.name.clone(), p.ty.clone())),
        );
        variables.insert("result".into(), operation.result.clone());
        variables.insert("success".into(), TypeRef::builtin("Boolean"));
        variables.insert(
            "failure".into(),
            TypeRef {
                name: "Failure".into(),
                symbol_id: None,
                arguments: operation.failures.clone(),
            },
        );
        for effect in owned.get(node.owner.as_str()).into_iter().flatten() {
            if let Some(Syntax::Effect {
                target,
                result_alias: Some(alias),
                ..
            }) = &effect.syntax
            {
                if let Ok(id) =
                    resolver.resolve_symbol(target, &node.owner, |k| k == StatementKind::Operation)
                {
                    variables.insert(alias.clone(), ir.operations[&id].result.clone());
                }
            }
        }
    }
    if node.kind == StatementKind::Transition {
        if let Some(operation) = node
            .terms
            .first()
            .and_then(|token| {
                resolver
                    .resolve_symbol(&token.text, &node.owner, |kind| {
                        kind == StatementKind::Operation
                    })
                    .ok()
            })
            .and_then(|id| ir.operations.get(&id))
        {
            variables.extend(
                operation
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.ty.clone())),
            );
        }
    }
    if let Some(declaration) = ir.data.get(&node.owner) {
        variables.insert(
            "value".into(),
            declaration.alias.clone().unwrap_or_else(|| TypeRef {
                name: ir.symbols[&node.owner].name.clone(),
                symbol_id: Some(node.owner.clone()),
                arguments: Vec::new(),
            }),
        );
        variables.extend(
            declaration
                .fields
                .iter()
                .map(|f| (f.name.clone(), f.ty.clone())),
        );
    }
    if ir
        .symbols
        .get(&node.owner)
        .is_some_and(|s| s.kind == StatementKind::Invariant)
    {
        for quantifier in owned
            .get(node.owner.as_str())
            .into_iter()
            .flatten()
            .filter(|n| n.kind == StatementKind::Forall)
        {
            if let Some(ty) = parse_type(&quantifier.terms[2..], &quantifier.span, diagnostics) {
                let ty = type_ref(
                    &ty,
                    &node.owner,
                    ReferenceKind::UsesType,
                    resolver,
                    refs,
                    diagnostics,
                );
                variables.insert(quantifier.terms[0].text.clone(), ty);
            }
        }
    }
    if node.kind == StatementKind::State && ir.operations.contains_key(&node.owner) {
        let token = &node.terms[1];
        let id = resolver
            .resolve_symbol(&token.text, &node.owner, |k| k == StatementKind::Entity)
            .expect("state entity was resolved before expressions");
        variables.insert(
            "self".into(),
            TypeRef {
                name: ir.symbols[&id].name.clone(),
                symbol_id: Some(id),
                arguments: Vec::new(),
            },
        );
    }
    variables
}
