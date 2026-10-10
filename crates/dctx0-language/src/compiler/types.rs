//! Declaration types, stable IDs, keys and alias validation.
use crate::{
    composition::problem,
    kinds::is_data_declaration as data,
    resolution::{SymbolResolver, type_ref},
};
use dctx0_core::*;
use std::collections::{BTreeMap, BTreeSet};
struct TypeContext<'a> {
    resolver: SymbolResolver<'a>,
    refs: Vec<Reference>,
    stable: BTreeMap<String, String>,
    diagnostics: &'a mut Vec<Diagnostic>,
}
pub(super) fn build_types(ir: &mut WorkspaceIr, diagnostics: &mut Vec<Diagnostic>) {
    let mut owned = BTreeMap::<&str, Vec<&SemanticNode>>::new();
    for node in &ir.nodes {
        owned.entry(&node.owner).or_default().push(node);
    }
    let mut context = TypeContext {
        resolver: SymbolResolver::new(&ir.symbols),
        refs: Vec::new(),
        stable: BTreeMap::new(),
        diagnostics,
    };
    for node in &ir.nodes {
        let children: &[&SemanticNode] = owned.get(node.id.as_str()).map_or(&[], Vec::as_slice);
        if let Some(data) = compile_data(node, children, &mut context) {
            ir.data.insert(node.id.clone(), data);
        }
        if let Some(operation) = compile_operation(node, children, &ir.symbols, &mut context) {
            ir.operations.insert(node.id.clone(), operation);
        }
        validate_surface(node, context.diagnostics);
    }
    validate_aliases(ir, context.diagnostics);
    ir.references.extend(context.refs);
}
fn compile_data(
    node: &SemanticNode,
    owned: &[&SemanticNode],
    context: &mut TypeContext<'_>,
) -> Option<DataIr> {
    if let (
        true,
        Some(Syntax::Data {
            alias,
            fields,
            variants,
        }),
    ) = (data(node.kind), &node.syntax)
    {
        let mut names = BTreeSet::new();
        for field in fields {
            if !names.insert(field.name.clone()) {
                problem(
                    context.diagnostics,
                    "DCTX0_FIELD_DUPLICATE",
                    format!("Duplicate field {}", field.name),
                    &field.span,
                );
            }
        }
        let clauses: Vec<_> = owned
            .iter()
            .filter(|n| n.owner == node.id && n.kind == StatementKind::StableId)
            .collect();
        let stable_id = clauses.first().and_then(|clause| match &clause.syntax {
            Some(Syntax::StableId {
                value,
                quoted: true,
                ..
            }) if clause.terms.len() == 1 && valid_stable(value) => Some(value.clone()),
            _ => {
                problem(
                    context.diagnostics,
                    "DCTX0_STABLE_ID_INVALID",
                    "Invalid stableId",
                    &clause.span,
                );
                None
            }
        });
        if clauses.len() > 1 {
            problem(
                context.diagnostics,
                "DCTX0_STABLE_ID_DUPLICATE",
                "Multiple stableId clauses",
                &clauses[1].span,
            );
        }
        if let Some(id) = &stable_id {
            if context.stable.insert(id.clone(), node.id.clone()).is_some() {
                problem(
                    context.diagnostics,
                    "DCTX0_STABLE_ID_DUPLICATE",
                    format!("Duplicate stableId {id}"),
                    &node.span,
                );
            }
        }
        let keys = owned
            .iter()
            .filter(|n| n.owner == node.id && n.kind == StatementKind::Key)
            .flat_map(|n| {
                n.terms
                    .iter()
                    .filter(|t| t.kind == TokenKind::Identifier)
                    .map(|t| t.text.clone())
            })
            .collect::<Vec<_>>();
        for key in &keys {
            if !fields.iter().any(|f| f.name == *key) {
                problem(
                    context.diagnostics,
                    "DCTX0_KEY_UNKNOWN",
                    format!("Unknown key field {key}"),
                    &node.span,
                );
            }
        }
        return Some(DataIr {
            id: node.id.clone(),
            stable_id,
            alias: alias.as_ref().map(|a| {
                type_ref(
                    a,
                    &node.id,
                    ReferenceKind::UsesType,
                    &context.resolver,
                    &mut context.refs,
                    context.diagnostics,
                )
            }),
            fields: fields
                .iter()
                .map(|f| FieldIr {
                    name: f.name.clone(),
                    ty: type_ref(
                        &f.ty,
                        &node.id,
                        ReferenceKind::UsesType,
                        &context.resolver,
                        &mut context.refs,
                        context.diagnostics,
                    ),
                })
                .collect(),
            variants: variants
                .iter()
                .map(|v| VariantIr {
                    name: v.name.clone(),
                    payload: v.payload.as_ref().map(|t| {
                        type_ref(
                            t,
                            &node.id,
                            ReferenceKind::UsesType,
                            &context.resolver,
                            &mut context.refs,
                            context.diagnostics,
                        )
                    }),
                })
                .collect(),
            keys,
        });
    }
    None
}
fn compile_operation(
    node: &SemanticNode,
    owned: &[&SemanticNode],
    symbols: &BTreeMap<String, Symbol>,
    context: &mut TypeContext<'_>,
) -> Option<OperationIr> {
    if let Some(Syntax::Operation {
        parameters,
        result,
        failures,
    }) = &node.syntax
    {
        let mut names = BTreeSet::new();
        for p in parameters {
            if !names.insert(&p.name) {
                problem(
                    context.diagnostics,
                    "DCTX0_PARAMETER_DUPLICATE",
                    "Duplicate parameter",
                    &p.span,
                );
            }
        }
        let result = result
            .as_ref()
            .map(|r| {
                type_ref(
                    r,
                    &node.id,
                    ReferenceKind::Returns,
                    &context.resolver,
                    &mut context.refs,
                    context.diagnostics,
                )
            })
            .expect("validated operation signature has a result type");
        let failures = failures
            .iter()
            .map(|f| {
                type_ref(
                    f,
                    &node.id,
                    ReferenceKind::FailsWith,
                    &context.resolver,
                    &mut context.refs,
                    context.diagnostics,
                )
            })
            .collect::<Vec<_>>();
        for failure in &failures {
            if failure
                .symbol_id
                .as_ref()
                .is_none_or(|id| symbols[id].kind != StatementKind::Failure)
            {
                problem(
                    context.diagnostics,
                    "DCTX0_UNKNOWN_FAILURE",
                    "Failure clause must reference a failure declaration",
                    &node.span,
                );
            }
        }
        return Some(OperationIr {
            id: node.id.clone(),
            parameters: parameters
                .iter()
                .map(|p| FieldIr {
                    name: p.name.clone(),
                    ty: type_ref(
                        &p.ty,
                        &node.id,
                        ReferenceKind::Accepts,
                        &context.resolver,
                        &mut context.refs,
                        context.diagnostics,
                    ),
                })
                .collect(),
            result,
            failures,
            pure: owned
                .iter()
                .any(|n| n.owner == node.id && n.kind == StatementKind::Pure),
        });
    }
    None
}
fn validate_surface(node: &SemanticNode, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind == StatementKind::Surface
        && node.terms.len() > 1
        && (node.terms.len() != 3
            || node.terms[1].text != "exposes"
            || !matches!(
                node.terms[2].text.as_str(),
                "requestReply" | "notification" | "stream"
            ))
    {
        problem(
            diagnostics,
            "DCTX0_SURFACE_EXPOSURE",
            "Invalid surface exchange",
            &node.span,
        );
    }
}
fn validate_aliases(ir: &WorkspaceIr, diagnostics: &mut Vec<Diagnostic>) {
    for declaration in ir.data.values() {
        let mut current = declaration.alias.as_ref();
        let mut visited = BTreeSet::new();
        visited.insert(declaration.id.as_str());
        while let Some(alias) = current {
            let Some(id) = &alias.symbol_id else {
                break;
            };
            if !visited.insert(id) {
                problem(
                    diagnostics,
                    "DCTX0_ALIAS_CYCLE",
                    "Cyclic type alias",
                    &ir.symbols[&declaration.id].span,
                );
                break;
            }
            current = ir.data.get(id).and_then(|d| d.alias.as_ref());
        }
    }
}
fn valid_stable(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '-'))
}
