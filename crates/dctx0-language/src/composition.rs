use crate::{kinds::is_data_declaration as data, parse};
use dctx0_core::{
    COMPILER_VERSION, Diagnostic, IR_VERSION, RelatedDiagnostic, SemanticNode, Source, Span,
    Statement, StatementKind as K, Symbol, Syntax, WorkspaceIr,
};
use std::collections::BTreeMap;

pub(crate) fn declared(kind: K) -> bool {
    data(kind)
        || matches!(
            kind,
            K::Workspace
                | K::Domain
                | K::Module
                | K::External
                | K::Surface
                | K::Port
                | K::Operation
                | K::Decision
                | K::Example
                | K::Invariant
                | K::Lifecycle
                | K::Proof
                | K::Test
                | K::Review
                | K::Policy
        )
}
pub(crate) fn problem(
    diagnostics: &mut Vec<Diagnostic>,
    code: &str,
    message: impl Into<String>,
    span: &Span,
) {
    diagnostics.push(Diagnostic {
        code: code.into(),
        message: message.into(),
        span: span.clone(),
        related: Vec::new(),
    });
}

pub(crate) fn compose(sources: &[Source]) -> Result<WorkspaceIr, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut ordered: Vec<_> = sources.iter().collect();
    ordered.sort_by(|a, b| a.file.cmp(&b.file));
    let mut statements = Vec::new();
    for source in ordered {
        let result = parse(&source.file, &source.text);
        diagnostics.extend(result.diagnostics);
        statements.extend(result.ast.statements);
    }
    let workspaces: Vec<_> = statements
        .iter()
        .filter(|s| s.kind == K::Workspace)
        .collect();
    let fallback = statements
        .first()
        .map(|s| s.span.clone())
        .unwrap_or_else(|| Span {
            file: "<workspace>".into(),
            start: dctx0_core::Position {
                offset: 0,
                line: 1,
                column: 1,
            },
            end: dctx0_core::Position {
                offset: 0,
                line: 1,
                column: 1,
            },
        });
    if workspaces.len() != 1 || workspaces[0].name.as_ref().is_none_or(String::is_empty) {
        problem(
            &mut diagnostics,
            "DCTX0_WORKSPACE_COUNT",
            "Expected exactly one named workspace",
            &fallback,
        );
        return Err(diagnostics);
    }
    let root = workspaces[0].name.clone().unwrap();
    let mut ir = WorkspaceIr {
        schema_version: IR_VERSION,
        compiler_version: COMPILER_VERSION.into(),
        workspace: root.clone(),
        symbols: BTreeMap::new(),
        data: BTreeMap::new(),
        operations: BTreeMap::new(),
        nodes: Vec::new(),
        references: Vec::new(),
    };
    let mut counters = BTreeMap::new();
    add(
        workspaces[0],
        &root,
        None,
        &mut ir,
        &mut counters,
        &mut diagnostics,
    );
    let mut fragments = Vec::new();
    for statement in &statements {
        if statement.kind == K::Fragment {
            fragments.push(statement);
        } else if statement.kind != K::Workspace {
            add(
                statement,
                &root,
                Some(&root),
                &mut ir,
                &mut counters,
                &mut diagnostics,
            );
        }
    }
    while !fragments.is_empty() {
        let mut remaining = Vec::new();
        let mut progressed = false;
        for fragment in fragments {
            let target = fragment
                .terms
                .iter()
                .map(|t| t.text.as_str())
                .collect::<String>();
            let target = if target == root || target.starts_with(&(root.clone() + ".")) {
                target
            } else {
                format!("{root}.{target}")
            };
            if let Some(symbol) = ir.symbols.get(&target) {
                if !matches!(
                    symbol.kind,
                    K::Workspace | K::Domain | K::Module | K::Surface | K::Port | K::External
                ) {
                    problem(
                        &mut diagnostics,
                        "DCTX0_FRAGMENT_TARGET_KIND",
                        format!("Cannot extend {}", symbol.kind.as_str()),
                        &fragment.span,
                    );
                    continue;
                }
                if let Some(Syntax::Fragment {
                    target: spelling,
                    target_span,
                }) = &fragment.syntax
                {
                    ir.references.push(dctx0_core::Reference {
                        from: target.clone(),
                        target: target.clone(),
                        kind: dctx0_core::ReferenceKind::Fragment,
                        spelling: spelling.clone(),
                        span: target_span.clone(),
                    });
                }
                for child in &fragment.children {
                    add(
                        child,
                        &target,
                        Some(&target),
                        &mut ir,
                        &mut counters,
                        &mut diagnostics,
                    );
                }
                progressed = true;
            } else {
                remaining.push(fragment);
            }
        }
        if !progressed {
            for fragment in remaining {
                problem(
                    &mut diagnostics,
                    "DCTX0_FRAGMENT_TARGET_UNKNOWN",
                    "Unknown fragment target",
                    &fragment.span,
                );
            }
            break;
        }
        fragments = remaining;
    }
    if diagnostics.is_empty() {
        Ok(ir)
    } else {
        Err(diagnostics)
    }
}
fn add(
    statement: &Statement,
    owner: &str,
    parent_node: Option<&str>,
    ir: &mut WorkspaceIr,
    counters: &mut BTreeMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if statement.kind == K::Fragment {
        problem(
            diagnostics,
            "DCTX0_FRAGMENT_NESTED",
            "Fragments must be at source root",
            &statement.span,
        );
        return;
    }
    let id = if declared(statement.kind) {
        if statement.name.as_ref().is_none_or(String::is_empty) {
            problem(
                diagnostics,
                "DCTX0_NAME_REQUIRED",
                "Declaration requires a name",
                &statement.span,
            );
            return;
        }
        if statement.kind == K::Workspace {
            owner.to_owned()
        } else {
            format!("{owner}.{}", statement.name.as_ref().unwrap())
        }
    } else {
        let prefix = format!(
            "{}#{}",
            parent_node.unwrap_or(owner),
            statement.kind.as_str()
        );
        let ordinal = counters.entry(prefix.clone()).or_default();
        *ordinal += 1;
        format!("{prefix}:{ordinal}")
    };
    let node_owner = if declared(statement.kind) {
        id.as_str()
    } else {
        owner
    };
    if declared(statement.kind) {
        let symbol = Symbol {
            id: id.clone(),
            name: statement.name.clone().unwrap(),
            kind: statement.kind,
            parent: if statement.kind == K::Workspace {
                None
            } else {
                Some(owner.into())
            },
            span: statement.span.clone(),
            name_span: statement.terms[0].span.clone(),
        };
        if let Some(previous) = ir.symbols.get(&id) {
            let mut diagnostic = Diagnostic {
                code: "DCTX0_DUPLICATE".into(),
                message: format!("Duplicate symbol {id}"),
                span: symbol.name_span.clone(),
                related: Vec::new(),
            };
            diagnostic.related.push(RelatedDiagnostic {
                message: "Originally declared here".into(),
                span: previous.name_span.clone(),
            });
            diagnostics.push(diagnostic);
            return;
        }
        ir.symbols.insert(id.clone(), symbol);
    }
    ir.nodes.push(SemanticNode {
        id: id.clone(),
        owner: node_owner.into(),
        parent: parent_node.map(str::to_owned),
        kind: statement.kind,
        name: statement.name.clone(),
        terms: statement.terms.clone(),
        syntax: statement.syntax.clone(),
        span: statement.span.clone(),
        expressions: Vec::new(),
    });
    for child in &statement.children {
        add(child, node_owner, Some(&id), ir, counters, diagnostics);
    }
}
