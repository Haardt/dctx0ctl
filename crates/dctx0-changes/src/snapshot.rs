//! Named projections specify precisely what contributes to each fingerprint.
use crate::{Fingerprints, Snapshot, hash, semantic_value, source_hash};
use dctx0_core::{
    Expression, SemanticNode, Source, StatementKind as K, Symbol, Token, TokenKind, WorkspaceIr,
};
use dctx0_graph::{EdgeKind, SemanticGraph};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct DeclarationContract<'a> {
    kind: K,
    name: &'a str,
    parent: &'a Option<String>,
}
#[derive(Serialize)]
struct TermContent<'a> {
    kind: TokenKind,
    text: &'a str,
}
#[derive(Serialize)]
struct ClauseContent<'a> {
    kind: K,
    terms: Vec<TermContent<'a>>,
    expressions: &'a [Expression],
}
#[derive(Serialize)]
struct AnonymousBehavior<'a> {
    terms: Vec<TermContent<'a>>,
    expressions: &'a [Expression],
}
#[derive(Serialize)]
struct Exposure<'a> {
    exposure: Vec<&'a str>,
}
#[derive(Serialize)]
struct Location<'a> {
    file: &'a str,
    parent: &'a Option<String>,
}
#[derive(Serialize)]
struct SemanticIdentity<'a> {
    version: u32,
    compiler: &'a str,
    fingerprints: BTreeMap<&'a str, (&'a str, &'a str, &'a str)>,
    edges: Vec<(&'a str, &'a str, EdgeKind)>,
}
#[derive(Serialize)]
struct SnapshotIdentity<'a> {
    schema: u32,
    source: &'a str,
    semantic: &'a str,
}

// Convert projections to sorted JSON objects before hashing. Serializing a
// struct directly would change the established canonical key order.
fn canonical(value: &impl Serialize) -> Value {
    serde_json::to_value(value).expect("fingerprint projections are JSON-compatible")
}
fn terms(terms: &[Token]) -> Vec<TermContent<'_>> {
    terms
        .iter()
        .map(|t| TermContent {
            kind: t.kind,
            text: &t.text,
        })
        .collect()
}
fn declaration_fingerprints(
    symbol: &Symbol,
    nodes: &[&SemanticNode],
    ir: &WorkspaceIr,
) -> Fingerprints {
    let mut contract = vec![canonical(&DeclarationContract {
        kind: symbol.kind,
        name: &symbol.name,
        parent: &symbol.parent,
    })];
    let mut behavior = Vec::new();
    let mut facts = Vec::new();
    if let Some(data) = ir.data.get(&symbol.id) {
        contract.push(canonical(data));
    }
    if let Some(operation) = ir.operations.get(&symbol.id) {
        contract.push(canonical(operation));
    }
    for node in nodes {
        let content = canonical(&ClauseContent {
            kind: node.kind,
            terms: terms(&node.terms),
            expressions: &node.expressions,
        });
        if matches!(
            node.kind,
            K::Intent
                | K::Constraint
                | K::Choose
                | K::Because
                | K::Avoid
                | K::Criterion
                | K::Narrative
        ) {
            facts.push(semantic_value(content));
        } else if node.id != symbol.id && !matches!(node.kind, K::Field | K::StableId | K::Key) {
            behavior.push(semantic_value(content));
        }
        if node.kind == K::Surface {
            contract.push(canonical(&Exposure {
                exposure: node.terms.iter().skip(1).map(|t| t.text.as_str()).collect(),
            }));
        }
    }
    Fingerprints {
        contract: hash(&contract),
        behavior: hash(&behavior),
        facts: hash(&facts),
        location: hash(&canonical(&Location {
            file: &symbol.span.file,
            parent: &symbol.parent,
        })),
    }
}
fn anonymous_fingerprints(node: &SemanticNode) -> Fingerprints {
    Fingerprints {
        contract: hash(&node.kind),
        behavior: hash(&semantic_value(canonical(&AnonymousBehavior {
            terms: terms(&node.terms),
            expressions: &node.expressions,
        }))),
        facts: hash(&node.kind),
        location: hash(&node.span.file),
    }
}
pub fn snapshot(sources: &[Source], ir: WorkspaceIr, graph: SemanticGraph) -> Snapshot {
    let mut sources = sources.to_vec();
    sources.sort_by(|a, b| a.file.cmp(&b.file));
    let source_hash = source_hash(&sources);
    let mut owned = BTreeMap::<&str, Vec<&SemanticNode>>::new();
    for node in &ir.nodes {
        owned.entry(&node.owner).or_default().push(node);
    }
    let mut fingerprints = BTreeMap::new();
    for symbol in ir.symbols.values() {
        fingerprints.insert(
            symbol.id.clone(),
            declaration_fingerprints(
                symbol,
                owned.get(symbol.id.as_str()).map_or(&[], Vec::as_slice),
                &ir,
            ),
        );
    }
    // Anonymous clauses have independent graph impact, even with named owners.
    for node in ir.nodes.iter().filter(|n| !ir.symbols.contains_key(&n.id)) {
        fingerprints.insert(node.id.clone(), anonymous_fingerprints(node));
    }
    let semantic_hash = hash(&canonical(&SemanticIdentity {
        version: ir.schema_version,
        compiler: &ir.compiler_version,
        fingerprints: fingerprints
            .iter()
            .map(|(id, f)| {
                (
                    id.as_str(),
                    (f.contract.as_str(), f.behavior.as_str(), f.facts.as_str()),
                )
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), e.kind))
            .collect(),
    }));
    let id = hash(&canonical(&SnapshotIdentity {
        schema: 1,
        source: &source_hash,
        semantic: &semantic_hash,
    }));
    Snapshot {
        schema_version: 1,
        id,
        source_hash,
        semantic_hash,
        sources,
        ir,
        graph,
        fingerprints,
    }
}
