//! Deterministic semantic dependency graph; cycles are legal.
use dctx0_core::{ReferenceKind, Span, StatementKind as K, WorkspaceIr};
use petgraph::{
    algo::kosaraju_scc,
    graph::{DiGraph, NodeIndex},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    Contains,
    Governs,
    Dependency(ReferenceKind),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: K,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub origins: Vec<Span>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticGraph {
    pub nodes: BTreeMap<String, GraphNode>,
    pub edges: Vec<GraphEdge>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Impact {
    pub subject: String,
    pub path: Vec<String>,
    pub reasons: Vec<EdgeKind>,
}

pub fn build(ir: &WorkspaceIr) -> SemanticGraph {
    let nodes = ir
        .nodes
        .iter()
        .map(|n| {
            (
                n.id.clone(),
                GraphNode {
                    id: n.id.clone(),
                    kind: n.kind,
                    span: n.span.clone(),
                },
            )
        })
        .collect();
    let mut edges = BTreeMap::<(String, String, EdgeKind), Vec<Span>>::new();
    for node in &ir.nodes {
        if let Some(parent) = &node.parent {
            edges
                .entry((parent.clone(), node.id.clone(), EdgeKind::Contains))
                .or_default()
                .push(node.span.clone());
        }
        if matches!(
            node.kind,
            K::Intent
                | K::Constraint
                | K::Decision
                | K::Example
                | K::Invariant
                | K::Lifecycle
                | K::Review
                | K::Proof
                | K::Test
                | K::Policy
        ) {
            let target = if ir.symbols.contains_key(&node.id) {
                ir.symbols[&node.id]
                    .parent
                    .as_deref()
                    .unwrap_or(&node.owner)
            } else {
                &node.owner
            };
            if node.id != target {
                edges
                    .entry((node.id.clone(), target.to_owned(), EdgeKind::Governs))
                    .or_default()
                    .push(node.span.clone());
            }
        }
    }
    for reference in &ir.references {
        if reference.kind != ReferenceKind::Fragment && reference.from != reference.target {
            edges
                .entry((
                    reference.from.clone(),
                    reference.target.clone(),
                    EdgeKind::Dependency(reference.kind),
                ))
                .or_default()
                .push(reference.span.clone());
        }
    }
    let edges = edges
        .into_iter()
        .map(|((from, to, kind), mut origins)| {
            origins.sort_by(|a, b| (&a.file, a.start.offset).cmp(&(&b.file, b.start.offset)));
            origins.dedup();
            GraphEdge {
                from,
                to,
                kind,
                origins,
            }
        })
        .collect();
    SemanticGraph { nodes, edges }
}
impl SemanticGraph {
    fn indexed(&self) -> (DiGraph<String, EdgeKind>, BTreeMap<String, NodeIndex>) {
        let mut graph = DiGraph::new();
        let mut indices = BTreeMap::new();
        for id in self.nodes.keys() {
            indices.insert(id.clone(), graph.add_node(id.clone()));
        }
        for edge in &self.edges {
            if let (Some(from), Some(to)) = (indices.get(&edge.from), indices.get(&edge.to)) {
                graph.add_edge(*from, *to, edge.kind);
            }
        }
        (graph, indices)
    }
    pub fn dependencies(&self, id: &str) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.from == id && matches!(e.kind, EdgeKind::Dependency(_)))
            .collect()
    }
    pub fn users(&self, id: &str) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.to == id && matches!(e.kind, EdgeKind::Dependency(_)))
            .collect()
    }
    /// Build once when evaluating several changed symbols in this snapshot.
    pub fn index(&self) -> GraphIndex<'_> {
        let mut incoming = BTreeMap::<&str, Vec<&GraphEdge>>::new();
        let mut outgoing = BTreeMap::<&str, Vec<&GraphEdge>>::new();
        for edge in &self.edges {
            incoming.entry(&edge.to).or_default().push(edge);
            outgoing.entry(&edge.from).or_default().push(edge);
        }
        GraphIndex {
            graph: self,
            incoming,
            outgoing,
        }
    }
    pub fn impact(&self, id: &str) -> Vec<Impact> {
        self.index().impact(id)
    }
    pub fn components(&self) -> Vec<Vec<String>> {
        let (graph, _) = self.indexed();
        let mut components = kosaraju_scc(&graph)
            .into_iter()
            .map(|indices| {
                let mut names = indices
                    .into_iter()
                    .map(|i| graph[i].clone())
                    .collect::<Vec<_>>();
                names.sort();
                names
            })
            .collect::<Vec<_>>();
        components.sort();
        components
    }
}

/// An ephemeral traversal index. Serialized graphs remain unchanged.
pub struct GraphIndex<'a> {
    graph: &'a SemanticGraph,
    incoming: BTreeMap<&'a str, Vec<&'a GraphEdge>>,
    outgoing: BTreeMap<&'a str, Vec<&'a GraphEdge>>,
}
impl GraphIndex<'_> {
    /// Incoming dependency edges propagate to consumers; governing facts
    /// propagate to their target and the target's named semantic subtree.
    pub fn impact(&self, id: &str) -> Vec<Impact> {
        if !self.graph.nodes.contains_key(id) {
            return Vec::new();
        }
        let mut queue = VecDeque::from([(id.to_owned(), vec![id.to_owned()], Vec::new())]);
        let mut visited = BTreeSet::from([id.to_owned()]);
        let mut results = Vec::new();
        while let Some((index, path, reasons)) = queue.pop_front() {
            let mut next = Vec::new();
            for edge in self.incoming.get(index.as_str()).into_iter().flatten() {
                if matches!(edge.kind, EdgeKind::Dependency(_)) {
                    next.push((edge.from.clone(), edge.kind));
                }
            }
            for edge in self.outgoing.get(index.as_str()).into_iter().flatten() {
                if edge.kind == EdgeKind::Governs
                    || reasons.contains(&EdgeKind::Governs) && edge.kind == EdgeKind::Contains
                {
                    next.push((edge.to.clone(), edge.kind));
                }
            }
            next.sort();
            for (next, reason) in next {
                let subject = next.clone();
                if !visited.insert(subject.clone()) {
                    continue;
                }
                let mut path = path.clone();
                path.push(subject.clone());
                let mut reasons = reasons.clone();
                reasons.push(reason);
                results.push(Impact {
                    subject,
                    path: path.clone(),
                    reasons: reasons.clone(),
                });
                queue.push_back((next, path, reasons));
            }
        }
        results.sort_by(|a, b| a.subject.cmp(&b.subject));
        results
    }
}
