//! Preflight constructive state contracts without executing an implementation.
use crate::operation_state::marker;
use dctx0_core::*;

pub(super) fn issues(ir: &WorkspaceIr, clauses: &[SemanticNode]) -> Vec<VerificationIssue> {
    let mut issues = Vec::new();
    for node in clauses
        .iter()
        .filter(|node| node.kind == StatementKind::State)
    {
        let entity = ir.references.iter().find(|reference| {
            reference.from == node.owner
                && reference.kind == ReferenceKind::State
                && reference.span.file == node.span.file
                && reference.span.start.offset >= node.span.start.offset
                && reference.span.end.offset <= node.span.end.offset
        });
        let mode = node.terms.first().map(|term| term.text.as_str());
        if mode == Some("replaces")
            && (!marker(node, "from")
                || !marker(node, "by")
                || marker(node, "where")
                || entity
                    .and_then(|reference| ir.data.get(&reference.target))
                    .is_some_and(|data| node.expressions.len() != data.keys.len() + 1))
        {
            issues.push(VerificationIssue {
                code: "STATE_REPLACEMENT_SOURCE_REQUIRED".into(),
                message: format!(
                    "{}: replaces requires a complete identity selector and an explicit complete entity source: state replaces Entity by key from Entity {{ ... }}. Observed after state and postconditions do not supply a replacement",
                    node.owner
                ),
                span: Some(node.span.clone()),
            });
        }
        if mode == Some("observes")
            && let Some(entity) = entity
            && ir.references.iter().any(|reference| {
                reference.from == node.owner
                    && reference.kind == ReferenceKind::Writes
                    && reference.target == entity.target
            })
        {
            issues.push(VerificationIssue {
                code: "STATE_OBSERVES_WRITE_CONFLICT".into(),
                message: format!(
                    "{}: state observes {} requires its complete observed entity state to remain unchanged on success, but this operation declares write access to the same entity. For an intended change, use replaces (existing entity) or upserts (insert or replace) with an explicit complete source; otherwise remove the write declaration. Postconditions alone do not define a state update",
                    node.owner, entity.target
                ),
                span: Some(node.span.clone()),
            });
        }
    }
    issues
}
