use crate::*;
use dctx0_changes::{Snapshot, hash, semantic_value};
use dctx0_core::{SemanticNode, StatementKind as K, TypeRef, WorkspaceIr};
use petgraph::{algo::kosaraju_scc, graph::DiGraph};
use std::collections::{BTreeMap, BTreeSet};

/// Snapshot-local indexes avoid repeated workspace scans and serialization.
pub struct SpecIndex<'a> {
    pub ir: &'a WorkspaceIr,
    pub nodes: BTreeMap<&'a str, &'a SemanticNode>,
    pub owned: BTreeMap<&'a str, Vec<&'a SemanticNode>>,
    references: BTreeMap<&'a str, Vec<&'a dctx0_core::Reference>>,
    decisions: BTreeMap<&'a str, Vec<&'a SemanticNode>>,
    criteria: BTreeMap<&'a str, Vec<&'a SemanticNode>>,
    related: BTreeMap<&'a str, BTreeSet<&'a str>>,
    fact_hashes: BTreeMap<&'a str, String>,
    contract_hashes: BTreeMap<&'a str, String>,
}
impl<'a> SpecIndex<'a> {
    pub fn new(ir: &'a WorkspaceIr) -> Self {
        let mut owned = BTreeMap::<&str, Vec<_>>::new();
        let mut decisions = BTreeMap::<&str, Vec<_>>::new();
        let mut criteria = BTreeMap::<&str, Vec<_>>::new();
        let mut related = BTreeMap::<&str, BTreeSet<&str>>::new();
        let mut references = BTreeMap::<&str, Vec<_>>::new();
        for node in &ir.nodes {
            owned.entry(&node.owner).or_default().push(node);
            if let Some(symbol) = ir.symbols.get(&node.owner) {
                if let Some(parent) = symbol.parent.as_deref() {
                    if symbol.kind == K::Decision {
                        decisions.entry(parent).or_default().push(node);
                    }
                    if symbol.kind == K::Review && node.kind == K::Criterion {
                        let targets = ir
                            .references
                            .iter()
                            .filter(|r| {
                                r.from == symbol.id && r.kind == dctx0_core::ReferenceKind::Verifies
                            })
                            .map(|r| r.target.as_str())
                            .collect::<Vec<_>>();
                        if targets.is_empty() {
                            criteria.entry(parent).or_default().push(node);
                        } else {
                            for target in targets {
                                criteria.entry(target).or_default().push(node);
                            }
                        }
                    }
                }
            }
            if node.kind == K::When {
                for expression in &node.expressions {
                    if let Some(target) = invocation_target(expression) {
                        related.entry(target).or_default().insert(&node.owner);
                    }
                }
            }
        }
        for r in &ir.references {
            references.entry(&r.from).or_default().push(r);
            if ir
                .symbols
                .get(&r.from)
                .is_some_and(|s| matches!(s.kind, K::Invariant | K::Lifecycle))
            {
                related.entry(&r.target).or_default().insert(&r.from);
            }
        }
        let fact_hashes = ir
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), hash(&fact_value(n))))
            .collect();
        let mut contract_hashes = BTreeMap::new();
        for symbol in ir.symbols.values() {
            let value = if let Some(data) = ir.data.get(&symbol.id) {
                serde_json::to_value(data).unwrap()
            } else if let Some(op) = ir.operations.get(&symbol.id) {
                serde_json::to_value(op).unwrap()
            } else {
                serde_json::json!({"subject":symbol.id,"kind":symbol.kind})
            };
            contract_hashes.insert(symbol.id.as_str(), hash(&value));
        }
        Self {
            ir,
            nodes: ir.nodes.iter().map(|n| (n.id.as_str(), n)).collect(),
            owned,
            references,
            decisions,
            criteria,
            related,
            fact_hashes,
            contract_hashes,
        }
    }
    pub fn ancestors(&self, subject: &str) -> Vec<String> {
        let mut result = vec![subject.to_owned()];
        let mut next = self
            .ir
            .symbols
            .get(subject)
            .and_then(|s| s.parent.as_deref());
        while let Some(id) = next {
            result.push(id.into());
            next = if let Some(symbol) = self.ir.symbols.get(id) {
                symbol.parent.as_deref()
            } else {
                self.nodes
                    .get(id)
                    .map(|node| node.owner.as_str())
                    .filter(|owner| *owner != id)
            };
        }
        result
    }
    pub fn facts(&self, subject: &str, implementation: bool) -> Vec<String> {
        let mut ids = self
            .owned
            .get(subject)
            .into_iter()
            .flatten()
            .filter(|n| n.id != subject)
            .map(|n| n.id.clone())
            .collect::<BTreeSet<_>>();
        for scope in self.ancestors(subject) {
            ids.extend(
                self.owned
                    .get(scope.as_str())
                    .into_iter()
                    .flatten()
                    .filter(|n| matches!(n.kind, K::Intent | K::Constraint))
                    .map(|n| n.id.clone()),
            );
            ids.extend(
                self.decisions
                    .get(scope.as_str())
                    .into_iter()
                    .flatten()
                    .map(|n| n.id.clone()),
            );
            if implementation {
                ids.extend(
                    self.criteria
                        .get(scope.as_str())
                        .into_iter()
                        .flatten()
                        .map(|n| n.id.clone()),
                );
            }
        }
        if implementation {
            for owner in self.related.get(subject).into_iter().flatten() {
                ids.insert((*owner).into());
                ids.extend(
                    self.owned
                        .get(owner)
                        .into_iter()
                        .flatten()
                        .map(|n| n.id.clone()),
                );
            }
        }
        ids.into_iter().collect()
    }
    pub fn type_closure(&self, subjects: &[String], facts: &[String]) -> Vec<String> {
        let mut found = BTreeSet::new();
        let mut pending = Vec::new();
        let owners = subjects
            .iter()
            .map(String::as_str)
            .chain(facts.iter().filter_map(|id| {
                self.nodes
                    .get(id.as_str())
                    .filter(|n| {
                        !n.expressions.is_empty()
                            || matches!(n.kind, K::Example | K::Invariant | K::Lifecycle)
                    })
                    .map(|n| n.owner.as_str())
            }))
            .collect::<BTreeSet<_>>();
        for owner in owners {
            if self.ir.data.contains_key(owner) {
                pending.push(owner.to_owned());
            }
            for r in self.references.get(owner).into_iter().flatten() {
                if self.ir.data.contains_key(&r.target) {
                    pending.push(r.target.clone());
                }
            }
        }
        // A called operation's signature and failures are part of the used contract.
        for subject in subjects {
            for call in self
                .references
                .get(subject.as_str())
                .into_iter()
                .flatten()
                .filter(|r| self.ir.operations.contains_key(&r.target))
            {
                for r in self
                    .references
                    .get(call.target.as_str())
                    .into_iter()
                    .flatten()
                {
                    if self.ir.data.contains_key(&r.target) {
                        pending.push(r.target.clone());
                    }
                }
            }
        }
        while let Some(id) = pending.pop() {
            if found.insert(id.clone()) {
                for r in self.references.get(id.as_str()).into_iter().flatten() {
                    if self.ir.data.contains_key(&r.target) {
                        pending.push(r.target.clone());
                    }
                }
            }
        }
        found.into_iter().collect()
    }
    pub fn requirements_hash(
        &self,
        subjects: &[String],
        facts: &[String],
        types: &[String],
    ) -> String {
        let contracts = subjects
            .iter()
            .chain(types)
            .map(|id| (id, &self.contract_hashes[id.as_str()]))
            .collect::<BTreeMap<_, _>>();
        let mut normalized = facts
            .iter()
            .map(|id| &self.fact_hashes[id.as_str()])
            .collect::<BTreeSet<_>>();
        for id in types {
            normalized.extend(
                self.owned
                    .get(id.as_str())
                    .into_iter()
                    .flatten()
                    .filter(|n| n.id != *id)
                    .map(|n| &self.fact_hashes[n.id.as_str()]),
            );
        }
        let sequences = facts
            .iter()
            .filter(|id| {
                matches!(
                    self.nodes[id.as_str()].kind,
                    K::Example | K::Lifecycle | K::Invariant
                )
            })
            .map(|id| {
                (
                    id,
                    self.owned
                        .get(id.as_str())
                        .into_iter()
                        .flatten()
                        .map(|n| &self.fact_hashes[n.id.as_str()])
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let imports = subjects
            .iter()
            .flat_map(|subject| self.references.get(subject.as_str()).into_iter().flatten())
            .filter(|r| self.ir.operations.contains_key(&r.target))
            .map(|r| {
                let rules = self
                    .owned
                    .get(r.target.as_str())
                    .into_iter()
                    .flatten()
                    .filter(|n| {
                        matches!(
                            n.kind,
                            K::Requires
                                | K::Ensures
                                | K::State
                                | K::Read
                                | K::Write
                                | K::Call
                                | K::Emit
                                | K::Observe
                        )
                    })
                    .map(|n| &self.fact_hashes[n.id.as_str()])
                    .collect::<BTreeSet<_>>();
                (&r.target, (&self.contract_hashes[r.target.as_str()], rules))
            })
            .collect::<BTreeMap<_, _>>();
        hash(&(contracts, normalized, sequences, imports))
    }
}
fn invocation_target(expression: &dctx0_core::Expression) -> Option<&str> {
    match &expression.value {
        dctx0_core::ExpressionKind::Call { target, .. } => target.as_deref(),
        _ => None,
    }
}
fn fact_value(node: &SemanticNode) -> serde_json::Value {
    semantic_value(
        serde_json::json!({"kind":node.kind,"terms":node.terms.iter().map(|t| (&t.kind,&t.text)).collect::<Vec<_>>(),"expressions":node.expressions}),
    )
}
pub fn type_name(ty: &TypeRef) -> String {
    let name = ty.symbol_id.as_ref().unwrap_or(&ty.name);
    if ty.arguments.is_empty() {
        name.clone()
    } else {
        format!(
            "{}<{}>",
            name,
            ty.arguments
                .iter()
                .map(type_name)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
pub fn contract_text(ir: &WorkspaceIr, id: &str) -> String {
    if let Some(op) = ir.operations.get(id) {
        format!(
            "{}({}) -> {}{}{}",
            id,
            op.parameters
                .iter()
                .map(|p| format!("{}: {}", p.name, type_name(&p.ty)))
                .collect::<Vec<_>>()
                .join(", "),
            type_name(&op.result),
            if op.failures.is_empty() {
                String::new()
            } else {
                format!(
                    " ! {}",
                    op.failures
                        .iter()
                        .map(type_name)
                        .collect::<Vec<_>>()
                        .join(" | ")
                )
            },
            if op.pure { " [pure]" } else { "" }
        )
    } else if let Some(data) = ir.data.get(id) {
        let mut text = format!("{} {}", ir.symbols[id].kind.as_str(), id);
        if let Some(alias) = &data.alias {
            text.push_str(&format!(" = {}", type_name(alias)));
        }
        for field in &data.fields {
            text.push_str(&format!("\n  {}: {}", field.name, type_name(&field.ty)));
        }
        for variant in &data.variants {
            text.push_str(&format!(
                "\n  | {}{}",
                variant.name,
                variant
                    .payload
                    .as_ref()
                    .map(|p| format!("({})", type_name(p)))
                    .unwrap_or_default()
            ));
        }
        if !data.keys.is_empty() {
            text.push_str(&format!("\n  key {}", data.keys.join(", ")));
        }
        if let Some(stable) = &data.stable_id {
            text.push_str(&format!("\n  stableId {stable:?}"));
        }
        text
    } else {
        id.into()
    }
}
fn candidate(
    index: &SpecIndex<'_>,
    kind: TaskKind,
    key: String,
    subjects: Vec<String>,
    implementation: bool,
) -> Candidate {
    let mut facts = BTreeSet::new();
    for subject in &subjects {
        facts.extend(index.facts(subject, implementation));
    }
    let facts = facts.into_iter().collect::<Vec<_>>();
    let types = index.type_closure(&subjects, &facts);
    let requirements_hash = index.requirements_hash(&subjects, &facts, &types);
    Candidate {
        review_obligations: vec![],
        key,
        kind,
        title: format!("{:?}: {}", kind, subjects.join(", ")),
        contract: subjects
            .iter()
            .map(|s| contract_text(index.ir, s))
            .collect::<Vec<_>>()
            .join("\n\n"),
        subjects,
        facts,
        types,
        work_order_hash: requirements_hash.clone(),
        requirements_hash,
        prerequisites: vec![],
        wave: 1,
        acceptance: kind.acceptance(),
    }
}

pub fn plan(snapshot: &Snapshot) -> Result<CandidatePlan> {
    let ir = &snapshot.ir;
    let index = SpecIndex::new(ir);
    let contract_ids = ir
        .data
        .keys()
        .chain(ir.operations.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut graph = DiGraph::<String, ()>::new();
    let indices = contract_ids
        .iter()
        .map(|id| (id.clone(), graph.add_node(id.clone())))
        .collect::<BTreeMap<_, _>>();
    for r in &ir.references {
        if contract_ids.contains(&r.from) && contract_ids.contains(&r.target) {
            graph.add_edge(indices[&r.from], indices[&r.target], ());
        }
    }
    let mut groups = kosaraju_scc(&graph)
        .into_iter()
        .map(|group| {
            let mut ids = group
                .into_iter()
                .map(|i| graph[i].clone())
                .collect::<Vec<_>>();
            ids.sort();
            ids
        })
        .collect::<Vec<_>>();
    groups.sort();
    let mut producer = BTreeMap::new();
    let mut tasks = BTreeMap::new();
    for group in groups {
        let key = if group.len() == 1 {
            format!("contract:{}", group[0])
        } else {
            format!("contract-group:{}", hash(&group))
        };
        for id in &group {
            producer.insert(id.clone(), key.clone());
        }
        let mut task = candidate(
            &index,
            TaskKind::Contract,
            key.clone(),
            group.clone(),
            false,
        );
        if group.len() > 1 {
            task.acceptance.push(format!(
                "Publish these recursive contracts together: {}",
                group.join(", ")
            ));
        }
        tasks.insert(key, task);
    }
    for op in ir.operations.keys() {
        let scopes = index.ancestors(op);
        if scopes.iter().any(|id| ir.symbols[id].kind == K::External) {
            continue;
        }
        let kind = if scopes.iter().any(|id| ir.symbols[id].kind == K::Port) {
            TaskKind::Adapter
        } else {
            TaskKind::Operation
        };
        let key = format!("implementation:{op}");
        let task = candidate(&index, kind, key.clone(), vec![op.clone()], true);
        tasks.insert(key, task);
    }
    for task in tasks.values_mut() {
        let mut needs = BTreeSet::new();
        for id in &task.types {
            needs.insert(producer[id].clone());
        }
        for id in &task.subjects {
            if task.kind != TaskKind::Contract {
                needs.insert(producer[id].clone());
            }
            for r in ir.references.iter().filter(|r| &r.from == id) {
                if let Some(key) = producer.get(&r.target) {
                    needs.insert(key.clone());
                }
            }
        }
        needs.remove(&task.key);
        task.prerequisites = needs
            .into_iter()
            .map(|key| Prerequisite {
                key,
                condition: Condition::Delivered,
                reason: "Required modeled type or operation contract".into(),
            })
            .collect();
    }
    let modules = ir
        .symbols
        .values()
        .filter(|s| s.kind == K::Module)
        .map(|s| s.id.clone())
        .collect::<Vec<_>>();
    for module in modules {
        let implementations = tasks
            .values()
            .filter(|t| {
                matches!(t.kind, TaskKind::Operation | TaskKind::Adapter)
                    && t.subjects
                        .iter()
                        .any(|s| index.ancestors(s).contains(&module))
            })
            .map(|t| t.key.clone())
            .collect::<Vec<_>>();
        if implementations.is_empty() {
            continue;
        }
        let key = format!("integration:{module}");
        let mut task = candidate(
            &index,
            TaskKind::Integration,
            key.clone(),
            vec![module.clone()],
            false,
        );
        // Integration includes the module's examples, invariants and lifecycle rules.
        for node in &ir.nodes {
            if matches!(node.kind, K::Example | K::Invariant | K::Lifecycle)
                && (index.ancestors(&node.id).contains(&module)
                    || ir
                        .references
                        .iter()
                        .any(|r| r.from == node.id && index.ancestors(&r.target).contains(&module)))
            {
                task.facts.push(node.id.clone());
                task.facts.extend(
                    index
                        .owned
                        .get(node.id.as_str())
                        .into_iter()
                        .flatten()
                        .map(|n| n.id.clone()),
                );
            }
        }
        task.facts.sort();
        task.facts.dedup();
        task.types = index.type_closure(&task.subjects, &task.facts);
        task.requirements_hash = index.requirements_hash(&task.subjects, &task.facts, &task.types);
        task.prerequisites = implementations
            .into_iter()
            .map(|key| Prerequisite {
                key,
                condition: Condition::Delivered,
                reason: "Assemble this module's delivered operation or adapter".into(),
            })
            .collect();
        for ty in &task.types {
            let key = producer[ty].clone();
            if !task.prerequisites.iter().any(|p| p.key == key) {
                task.prerequisites.push(Prerequisite {
                    key,
                    condition: Condition::Delivered,
                    reason: "Publish the owned or scenario type contract before integration".into(),
                });
            }
        }
        let mut collaborators = BTreeSet::new();
        for own in &task.prerequisites {
            // Every prerequisite above names an existing producer.
            let implementation = &tasks[&own.key];
            for subject in &implementation.subjects {
                for reference in ir
                    .references
                    .iter()
                    .filter(|r| r.from == *subject && r.kind == dctx0_core::ReferenceKind::Calls)
                {
                    let key = format!("implementation:{}", reference.target);
                    if tasks.contains_key(&key) {
                        collaborators.insert(key);
                    }
                }
            }
        }
        for key in collaborators {
            if !task.prerequisites.iter().any(|p| p.key == key) {
                task.prerequisites.push(Prerequisite {
                    key,
                    condition: Condition::Delivered,
                    reason:
                        "Deliver the called operation implementation before assembling the module"
                            .into(),
                });
            }
        }
        task.prerequisites.sort_by(|a, b| a.key.cmp(&b.key));
        tasks.insert(key, task);
    }
    for review in ir.symbols.values().filter(|s| s.kind == K::Review) {
        let scope = review.parent.as_ref().unwrap_or(&ir.workspace);
        let explicit = ir
            .references
            .iter()
            .filter(|r| r.from == review.id && r.kind == dctx0_core::ReferenceKind::Verifies)
            .map(|r| r.target.clone())
            .collect::<Vec<_>>();
        let key = format!("review:{}", review.id);
        let mut task = candidate(
            &index,
            TaskKind::Review,
            key.clone(),
            vec![review.id.clone()],
            false,
        );
        task.facts.extend(
            ir.nodes
                .iter()
                .filter(|n| index.ancestors(&n.owner).contains(&review.id))
                .map(|n| n.id.clone()),
        );
        task.facts.sort();
        task.facts.dedup();
        task.requirements_hash = index.requirements_hash(&task.subjects, &task.facts, &task.types);
        task.prerequisites = tasks
            .values()
            .filter(|t| {
                if !explicit.is_empty() {
                    return t.subjects.iter().any(|s| {
                        explicit
                            .iter()
                            .any(|target| s == target || index.ancestors(s).contains(target))
                    });
                }
                t.kind == TaskKind::Integration
                    && t.subjects
                        .iter()
                        .any(|s| index.ancestors(s).contains(scope))
                    || !tasks.values().any(|t| {
                        t.kind == TaskKind::Integration
                            && t.subjects
                                .iter()
                                .any(|s| index.ancestors(s).contains(scope))
                    }) && t
                        .subjects
                        .iter()
                        .any(|s| index.ancestors(s).contains(scope))
            })
            .map(|t| Prerequisite {
                key: t.key.clone(),
                condition: Condition::Delivered,
                reason: "Deliver the review's explicitly scoped subjects".into(),
            })
            .collect();
        tasks.insert(key, task);
    }
    // Cross-module obligations have their own assembly task.
    for node in ir
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, K::Example | K::Invariant | K::Lifecycle))
    {
        let involved = ir
            .references
            .iter()
            .filter(|r| r.from == node.id)
            .filter_map(|r| {
                index.ancestors(&r.target).into_iter().find(|s| {
                    ir.symbols
                        .get(s)
                        .is_some_and(|symbol| symbol.kind == K::Module)
                })
            })
            .collect::<BTreeSet<_>>();
        if involved.len() > 1 {
            let key = format!("integration:{}", node.id);
            let mut task = candidate(
                &index,
                TaskKind::Integration,
                key.clone(),
                vec![node.id.clone()],
                false,
            );
            task.prerequisites = involved
                .into_iter()
                .map(|s| Prerequisite {
                    key: format!("integration:{s}"),
                    condition: Condition::Delivered,
                    reason: "Deliver the modules participating in this cross-module obligation"
                        .into(),
                })
                .collect();
            tasks.insert(key, task);
        }
    }
    assign_conformance_reviews(&index, &mut tasks);
    assign_waves(&mut tasks)?;
    // Assembly and reviews track delivered work revisions as well as spec facts.
    let ordered = tasks
        .values()
        .map(|t| (t.wave, t.key.clone()))
        .collect::<BTreeSet<_>>();
    for (_, key) in ordered {
        if matches!(tasks[&key].kind, TaskKind::Integration | TaskKind::Review) {
            let inputs = tasks[&key]
                .prerequisites
                .iter()
                .map(|p| (&p.key, &tasks[&p.key].work_order_hash))
                .collect::<Vec<_>>();
            let work = hash(&(&tasks[&key].requirements_hash, inputs));
            tasks.get_mut(&key).unwrap().work_order_hash = work;
        }
    }
    Ok(CandidatePlan {
        snapshot_id: snapshot.id.clone(),
        tasks: tasks.into_values().collect(),
        obligations: ir
            .symbols
            .values()
            .filter(|s| matches!(s.kind, K::Proof | K::Test | K::Policy))
            .map(|s| s.id.clone())
            .collect(),
    })
}

/// Every textual requirement and non-execution suite has one explicit review owner.
/// Existing scoped reviews are reused; uncovered scopes get a separate review task.
fn assign_conformance_reviews(index: &SpecIndex<'_>, tasks: &mut BTreeMap<String, Candidate>) {
    let ir = index.ir;
    let obligations = ir
        .nodes
        .iter()
        .filter(|n| n.kind == K::Constraint)
        .map(|n| (n.id.clone(), n.owner.clone()))
        .chain(
            ir.symbols
                .values()
                .filter(|s| {
                    matches!(
                        s.kind,
                        K::Decision | K::Review | K::Policy | K::Proof | K::Test
                    ) && !(matches!(s.kind, K::Proof | K::Test)
                        && matches!(s.name.as_str(), "Examples" | "Properties"))
                })
                .map(|s| {
                    (
                        s.id.clone(),
                        s.parent.clone().unwrap_or_else(|| ir.workspace.clone()),
                    )
                }),
        )
        .collect::<BTreeMap<_, _>>();
    let original = tasks.clone();
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for (subject, owner) in obligations {
        let ancestors = index.ancestors(&owner);
        let existing = original
            .values()
            .filter(|task| task.kind == TaskKind::Review)
            .filter_map(|task| {
                if task.subjects.contains(&subject) {
                    return Some((usize::MAX, task.key.clone()));
                }
                let scope = ir.symbols.get(&task.subjects[0])?.parent.as_ref()?;
                (scope == &owner || ancestors.contains(scope))
                    .then(|| (scope.split('.').count(), task.key.clone()))
            })
            .max();
        let key = existing
            .map(|(_, key)| key)
            .unwrap_or_else(|| format!("conformance:{owner}"));
        groups.entry(key).or_default().push(subject);
    }
    for (key, subjects) in groups {
        let declared = subjects
            .iter()
            .filter(|subject| ir.symbols.contains_key(*subject))
            .cloned()
            .collect::<Vec<_>>();
        let mut task = tasks.remove(&key).unwrap_or_else(|| {
            let mut task = candidate(
                index,
                TaskKind::Review,
                key.clone(),
                if declared.is_empty() {
                    vec![key.trim_start_matches("conformance:").to_owned()]
                } else {
                    declared.clone()
                },
                false,
            );
            let scope = key.trim_start_matches("conformance:");
            let scoped = original
                .values()
                .filter(|task| {
                    task.subjects.iter().any(|subject| {
                        index
                            .ancestors(subject)
                            .iter()
                            .any(|ancestor| ancestor == scope)
                    })
                })
                .collect::<Vec<_>>();
            let assembled = scoped.iter().any(|task| task.kind == TaskKind::Integration);
            task.prerequisites = scoped
                .into_iter()
                .filter(|t| {
                    !assembled || matches!(t.kind, TaskKind::Integration | TaskKind::Review)
                })
                .map(|t| Prerequisite {
                    key: t.key.clone(),
                    condition: Condition::Delivered,
                    reason:
                        "Deliver implementation and scoped reviews before conformance assessment"
                            .into(),
                })
                .collect();
            task
        });
        task.review_obligations = subjects;
        task.subjects.extend(declared);
        task.subjects.sort();
        task.subjects.dedup();
        for subject in &task.review_obligations {
            task.facts.extend(
                ir.nodes
                    .iter()
                    .filter(|n| n.id == *subject || n.owner == *subject)
                    .map(|n| n.id.clone()),
            );
        }
        task.facts.sort();
        task.facts.dedup();
        task.types = index.type_closure(&task.subjects, &task.facts);
        task.requirements_hash = hash(&(
            index.requirements_hash(&task.subjects, &task.facts, &task.types),
            &task.review_obligations,
        ));
        task.work_order_hash = task.requirements_hash.clone();
        task.acceptance.push("Record a source-bound conformance review for every assigned manual requirement and suite, with concrete evidence and limitations; approval is separate from automatic test PASS.".into());
        tasks.insert(key, task);
    }
}

pub fn conformance_subjects(_ir: &WorkspaceIr, candidate: &Candidate) -> Vec<String> {
    if candidate.kind != TaskKind::Review {
        return vec![];
    }
    candidate.review_obligations.clone()
}
/// A separate DAG rejects missing producers and explains cycles.
pub fn assign_waves(tasks: &mut BTreeMap<String, Candidate>) -> Result<()> {
    let mut remaining = tasks.keys().cloned().collect::<BTreeSet<_>>();
    let mut waves = BTreeMap::new();
    while !remaining.is_empty() {
        let mut progress = false;
        for key in remaining.clone() {
            let mut wave = 1;
            let mut waiting = false;
            for dep in &tasks[&key].prerequisites {
                if !tasks.contains_key(&dep.key) {
                    return Err(Error::Invalid(format!(
                        "Missing prerequisite {} for {key}: {}",
                        dep.key, dep.reason
                    )));
                }
                if let Some(w) = waves.get(&dep.key) {
                    wave = wave.max(w + 1);
                } else {
                    waiting = true;
                }
            }
            if !waiting {
                waves.insert(key.clone(), wave);
                remaining.remove(&key);
                progress = true;
            }
        }
        if !progress {
            return Err(Error::Invalid(format!(
                "Hard task cycle: {}",
                remaining
                    .iter()
                    .map(|key| format!("{key} -> {:?}", tasks[key].prerequisites))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
    }
    for (key, wave) in waves {
        tasks.get_mut(&key).unwrap().wave = wave;
    }
    Ok(())
}
