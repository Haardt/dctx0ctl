//! Effects, architecture, obligations, state references and refinements.
use crate::{
    composition::problem,
    kinds::{is_data_declaration as data, is_observable_target, is_verification_target},
    resolution::{SymbolResolver, reference, type_ref},
    syntax::parse_type,
};
use dctx0_core::*;
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn build_relations(ir: &mut WorkspaceIr, diagnostics: &mut Vec<Diagnostic>) {
    let resolver = SymbolResolver::new(&ir.symbols);
    let mut refs = Vec::new();
    let mut ownership = BTreeMap::new();
    for node in &ir.nodes {
        compile_effect(node, &resolver, &mut refs, diagnostics, ir);
        compile_architecture(node, &resolver, &mut refs, diagnostics, &mut ownership);
        compile_obligations(node, &resolver, &mut refs, diagnostics);
        compile_operation_state(node, &resolver, &mut refs, diagnostics, ir);
        if node.kind == StatementKind::Lifecycle {
            validate_lifecycle(node, ir, diagnostics);
        }
        validate_refinements(node, diagnostics);
    }
    ir.references.extend(refs);
}
fn compile_effect(
    node: &SemanticNode,
    resolver: &SymbolResolver<'_>,
    refs: &mut Vec<Reference>,
    diagnostics: &mut Vec<Diagnostic>,
    ir: &WorkspaceIr,
) {
    if let Some(Syntax::Effect { target, .. }) = &node.syntax {
        let kind = match node.kind {
            StatementKind::Read => ReferenceKind::Reads,
            StatementKind::Write => ReferenceKind::Writes,
            StatementKind::Call => ReferenceKind::Calls,
            StatementKind::Emit => ReferenceKind::Emits,
            _ => ReferenceKind::Observes,
        };
        let token = node.terms.first().expect("validated effect has a target");
        reference(
            target,
            &token.span,
            &node.owner,
            kind,
            resolver,
            |k| {
                if node.kind == StatementKind::Call {
                    k == StatementKind::Operation
                } else if node.kind == StatementKind::Emit {
                    k == StatementKind::Event
                } else if node.kind == StatementKind::Observe {
                    is_observable_target(k)
                } else {
                    data(k)
                }
            },
            (refs, diagnostics),
        );
        if ir.operations.get(&node.owner).is_some_and(|o| o.pure) {
            problem(
                diagnostics,
                "DCTX0_PURE_EFFECT",
                "Pure operations cannot declare effects",
                &node.span,
            );
        }
    }
}

fn compile_architecture(
    node: &SemanticNode,
    resolver: &SymbolResolver<'_>,
    refs: &mut Vec<Reference>,
    diagnostics: &mut Vec<Diagnostic>,
    ownership: &mut BTreeMap<String, String>,
) {
    if matches!(node.kind, StatementKind::Own | StatementKind::Dependency) {
        for token in node.terms.iter().filter(|t| {
            t.kind == TokenKind::Identifier
                && !matches!(
                    t.text.as_str(),
                    "scalar" | "value" | "type" | "failure" | "entity" | "message" | "event"
                )
        }) {
            let kind = if node.kind == StatementKind::Own {
                ReferenceKind::Owns
            } else {
                ReferenceKind::Requires
            };
            if let Some(target) = reference(
                &token.text,
                &token.span,
                &node.owner,
                kind,
                resolver,
                |k| {
                    if node.kind == StatementKind::Own {
                        data(k)
                    } else {
                        matches!(
                            k,
                            StatementKind::Module | StatementKind::Port | StatementKind::External
                        )
                    }
                },
                (refs, diagnostics),
            ) {
                if node.kind == StatementKind::Own {
                    if let Some(previous) = ownership.insert(target, node.owner.clone()) {
                        if previous != node.owner {
                            problem(
                                diagnostics,
                                "DCTX0_OWNERSHIP_DUPLICATE",
                                "Declaration has multiple owners",
                                &token.span,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn compile_obligations(
    node: &SemanticNode,
    resolver: &SymbolResolver<'_>,
    refs: &mut Vec<Reference>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let markers: &[&str] = match node.kind {
        StatementKind::Lifecycle => &["for"],
        StatementKind::Review | StatementKind::Proof | StatementKind::Test => &["for"],
        _ => &[],
    };
    for pair in node.terms.windows(2) {
        if markers.contains(&pair[0].text.as_str()) {
            reference(
                &pair[1].text,
                &pair[1].span,
                &node.owner,
                if node.kind == StatementKind::Lifecycle {
                    ReferenceKind::State
                } else {
                    ReferenceKind::Verifies
                },
                resolver,
                |_| true,
                (refs, diagnostics),
            );
        }
    }
    if node.kind == StatementKind::Transition {
        if let Some(token) = node.terms.first() {
            reference(
                &token.text,
                &token.span,
                &node.owner,
                ReferenceKind::Calls,
                resolver,
                |k| k == StatementKind::Operation,
                (refs, diagnostics),
            );
        }
    }
    if node.kind == StatementKind::Require {
        if node.terms.is_empty() {
            problem(
                diagnostics,
                "DCTX0_REFERENCE_REQUIRED",
                "require needs a proof, test, or review",
                &node.span,
            );
        }
        if let Some(token) = node.terms.first() {
            let name = token
                .text
                .strip_suffix(".highFindings")
                .or_else(|| token.text.strip_suffix(".status"))
                .unwrap_or(&token.text);
            let mut span = token.span.clone();
            span.end.offset = span.start.offset + name.len();
            span.end.column = span.start.column + name.len();
            reference(
                name,
                &span,
                &node.owner,
                ReferenceKind::Verifies,
                resolver,
                is_verification_target,
                (refs, diagnostics),
            );
        }
    }
    if node.kind == StatementKind::Test
        && !node.terms.iter().any(|t| {
            matches!(
                t.text.as_str(),
                "Examples" | "Properties" | "AdapterConformance" | "ExternalConformance"
            )
        })
    {
        problem(
            diagnostics,
            "DCTX0_UNKNOWN_TEST_SUITE",
            "Unknown test suite",
            &node.span,
        );
    }
    if node.kind == StatementKind::Forall {
        if node.terms.len() < 3 || node.terms[1].text != ":" {
            problem(
                diagnostics,
                "DCTX0_QUANTIFIER_SYNTAX",
                "Use forall name: Type",
                &node.span,
            );
        } else {
            if let Some(ty) = parse_type(&node.terms[2..], &node.span, diagnostics) {
                type_ref(
                    &ty,
                    &node.owner,
                    ReferenceKind::UsesType,
                    resolver,
                    refs,
                    diagnostics,
                );
            }
        }
    }
}

fn compile_operation_state(
    node: &SemanticNode,
    resolver: &SymbolResolver<'_>,
    refs: &mut Vec<Reference>,
    diagnostics: &mut Vec<Diagnostic>,
    ir: &WorkspaceIr,
) {
    if node.kind == StatementKind::State && ir.operations.contains_key(&node.owner) {
        if node.terms.len() < 2
            || !matches!(
                node.terms[0].text.as_str(),
                "observes" | "lists" | "inserts" | "upserts" | "replaces" | "removes"
            )
        {
            problem(
                diagnostics,
                "DCTX0_STATE_SYNTAX",
                "Invalid operation state clause",
                &node.span,
            );
        } else {
            reference(
                &node.terms[1].text,
                &node.terms[1].span,
                &node.owner,
                ReferenceKind::State,
                resolver,
                |k| k == StatementKind::Entity,
                (refs, diagnostics),
            );
        }
    }
}

fn validate_refinements(node: &SemanticNode, diagnostics: &mut Vec<Diagnostic>) {
    if matches!(node.kind, StatementKind::Range | StatementKind::Length) {
        let values: Vec<_> = node
            .terms
            .iter()
            .filter(|t| t.kind == TokenKind::Number)
            .collect();
        if values.len() != 2 || !node.terms.iter().any(|t| t.text == "..") {
            problem(
                diagnostics,
                "DCTX0_REFINEMENT_SYNTAX",
                "Expected minimum..maximum",
                &node.span,
            );
        }
    }
    if node.kind == StatementKind::Format
        && (node.terms.len() != 1
            || !matches!(
                node.terms[0].text.as_str(),
                "Email" | "UUID" | "Uri" | "Date" | "DateTime"
            ))
    {
        problem(
            diagnostics,
            "DCTX0_FORMAT_UNKNOWN",
            "Unknown format",
            &node.span,
        );
    }
}
fn validate_lifecycle(node: &SemanticNode, ir: &WorkspaceIr, diagnostics: &mut Vec<Diagnostic>) {
    let states: Vec<_> = ir
        .nodes
        .iter()
        .filter(|n| n.owner == node.id && n.kind == StatementKind::State)
        .collect();
    let names: Vec<_> = states
        .iter()
        .filter_map(|s| s.terms.first().map(|t| t.text.as_str()))
        .collect();
    let initial = states
        .iter()
        .filter(|s| s.terms.iter().any(|t| t.text == "initial"))
        .count();
    if initial != 1 {
        problem(
            diagnostics,
            "DCTX0_LIFECYCLE_INITIAL",
            "Lifecycle needs exactly one initial state",
            &node.span,
        );
    }
    let mut seen = BTreeSet::new();
    for name in &names {
        if !seen.insert(name) {
            problem(
                diagnostics,
                "DCTX0_LIFECYCLE_STATE_DUPLICATE",
                "Duplicate lifecycle state",
                &node.span,
            );
        }
    }
    for transition in ir
        .nodes
        .iter()
        .filter(|n| n.owner == node.id && n.kind == StatementKind::Transition)
    {
        for marker in ["from", "to"] {
            let state = transition
                .terms
                .windows(2)
                .find(|p| p[0].text == marker)
                .map(|p| p[1].text.as_str());
            if state.is_none_or(|name| !names.contains(&name)) {
                problem(
                    diagnostics,
                    "DCTX0_LIFECYCLE_STATE_UNKNOWN",
                    format!("Unknown {marker} state"),
                    &transition.span,
                );
            }
        }
    }
}
