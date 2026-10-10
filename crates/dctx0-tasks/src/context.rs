use crate::*;
use dctx0_changes::{Snapshot, hash};
use dctx0_core::{Span, StatementKind as K};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileAttribution {
    pub target: String,
    pub key: Option<dctx0_core::ProfileKey>,
    pub path: String,
    pub content_hash: String,
    pub subjects: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ImplementationOrigin {
    CurrentLocal,
    StoredExecution,
    RequestedRevision,
    UnknownLegacyExecution,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationContext {
    pub origin: ImplementationOrigin,
    pub target: String,
    pub bundle_hash: String,
    pub missing_files: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileAttribution>,
    pub section: String,
    pub title: String,
    pub text: String,
    pub source: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_definition: Option<dctx0_core::DataIr>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub operation_contracts: Vec<dctx0_core::OperationIr>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPage {
    pub page: Page<ContextItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<ImplementationContext>,
    pub required_sections: Vec<String>,
    pub missing_sections: Vec<String>,
    pub utf8_bytes: usize,
    pub characters: usize,
    pub token_count: Option<usize>,
    pub tokenizer: Option<String>,
    pub oversized_item: bool,
}
pub fn context(
    session: &Session,
    snapshot: &Snapshot,
    id: i64,
    section: &str,
    budget: usize,
    cursor: Option<&Cursor>,
) -> Result<ContextPage> {
    context_with_profiles(session, snapshot, id, section, budget, cursor, None)
}
/// Profile content participates in pagination and cursor identity before budgeting.
pub fn context_with_profiles(
    session: &Session,
    snapshot: &Snapshot,
    id: i64,
    section: &str,
    budget: usize,
    cursor: Option<&Cursor>,
    implementation: Option<ProfileSelection>,
) -> Result<ContextPage> {
    if budget == 0 || snapshot.id != session.plan.snapshot_id {
        return Err(Error::Invalid("Context budget or snapshot mismatch".into()));
    }
    let member = session.member(id)?;
    let c = &member.candidate;
    let definition = &session.definitions[&id];
    let state = &session.states[&id];
    let (implementation, origin) = if let Some(bundle) = &state.execution_bundle {
        (
            Some(ProfileSelection {
                bundle: bundle.clone(),
                missing_files: vec![],
            }),
            ImplementationOrigin::StoredExecution,
        )
    } else if let Some(AdjustmentReason::Profile { desired, .. }) = &definition.adjustment {
        (
            Some(ProfileSelection {
                bundle: desired.clone(),
                missing_files: vec![],
            }),
            ImplementationOrigin::RequestedRevision,
        )
    } else {
        (
            implementation,
            if state.work == WorkState::Open {
                ImplementationOrigin::CurrentLocal
            } else {
                ImplementationOrigin::UnknownLegacyExecution
            },
        )
    };

    let index = SpecIndex::new(&snapshot.ir);
    let allowed = [
        "core",
        "implementation",
        "goal",
        "acceptance",
        "contract",
        "types",
        "rules",
        "examples",
        "decisions",
        "changes",
        "dependencies",
        "history",
        "result",
    ];
    if !allowed.contains(&section) {
        return Err(Error::Invalid(format!("Unknown context section {section}")));
    }
    let (instruction, acceptance) = assignment(snapshot, c);
    let mut items = vec![
        ContextItem {
            profile: None,
            section: "goal".into(),
            title: format!("Assignment: {}", c.title),
            text: instruction,
            type_definition: None,
            operation_contracts: vec![],
            source: None,
        },
        ContextItem {
            profile: None,
            section: "acceptance".into(),
            title: "Acceptance".into(),
            text: acceptance
                .iter()
                .map(|a| format!("- {a}"))
                .collect::<Vec<_>>()
                .join("\n"),
            type_definition: None,
            operation_contracts: vec![],
            source: None,
        },
    ];
    let mut covered = vec![];
    if c.kind == TaskKind::Retire {
        items.push(ContextItem {
            profile: None,
            section: "contract".into(),
            title: "Removed contract".into(),
            text: c.contract.clone(),
            source: None,
            type_definition: None,
            operation_contracts: vec![],
        });
    } else {
        for subject in &c.subjects {
            let (item, raw) = specification(snapshot, &index, subject, "contract");
            if raw && matches!(section, "core" | "contract") {
                covered.push(item.source.clone().unwrap());
            }
            items.push(item);
        }
    }
    for ty in &c.types {
        // Own types already have their complete specification above. Explicit
        // type-only queries still include them.
        if c.subjects.contains(ty) && section != "types" {
            continue;
        }
        let (item, raw) = specification(snapshot, &index, ty, "types");
        if raw && matches!(section, "core" | "types") {
            covered.push(item.source.clone().unwrap());
        }
        items.push(item);
    }
    let mut seen = BTreeSet::new();
    for fact in &c.facts {
        let node = index.nodes[fact.as_str()];
        if covered.iter().any(|span: &Span| {
            span.file == node.span.file
                && span.start.offset <= node.span.start.offset
                && span.end.offset >= node.span.end.offset
        }) {
            continue;
        }
        if matches!(
            node.kind,
            K::Effects | K::Field | K::Key | K::StableId | K::Inspect | K::Criteria
        ) {
            continue;
        }
        let scopes = index.ancestors(&node.owner);
        let category = if node.kind == K::Example
            || scopes
                .iter()
                .any(|s| snapshot.ir.symbols[s].kind == K::Example)
        {
            "examples"
        } else if node.kind == K::Decision
            || scopes
                .iter()
                .any(|s| snapshot.ir.symbols[s].kind == K::Decision)
        {
            "decisions"
        } else {
            "rules"
        };
        // Parent statements already contain their children in the source excerpt.
        if node.parent.as_ref().is_some_and(|p| seen.contains(p)) {
            seen.insert(node.id.clone());
            continue;
        }
        items.push(ContextItem {
            profile: None,
            section: category.into(),
            title: format!("{} — {}", node.kind.as_str(), node.owner),
            text: node_text(snapshot, node),
            type_definition: None,
            operation_contracts: vec![],
            source: Some(node.span.clone()),
        });
        seen.insert(node.id.clone());
    }
    if definition.before_snapshot.is_some() {
        items.push(ContextItem {
            profile: None,
            section: "changes".into(),
            title: "Change request".into(),
            text: definition.reason.clone(),
            source: None,
            type_definition: None,
            operation_contracts: vec![],
        });
    }
    for change in &definition.changes {
        items.push(ContextItem {
            profile: None,
            section: "changes".into(),
            title: format!("{}: {}", change.facet, change.subject),
            text: format!(
                "Before:\n{}\n\nAfter:\n{}",
                change.before.as_deref().unwrap_or("not required"),
                change.after.as_deref().unwrap_or("removed")
            ),
            type_definition: None,
            operation_contracts: vec![],
            source: None,
        });
    }
    let rows = session.overviews();
    for dep in session.related(id, false, false)? {
        let r = rows
            .iter()
            .find(|r| r.task_id == dep.prerequisite_id)
            .unwrap();
        let provider = &session.member(r.task_id)?.candidate;
        let contracts = provider
            .subjects
            .iter()
            .filter_map(|s| snapshot.ir.operations.get(s).cloned())
            .collect::<Vec<_>>();
        items.push(ContextItem {
            profile: None,
            section: "dependencies".into(),
            title: format!("T{} — {}", r.task_id, r.title),
            text: format!(
                "{}; {:?}; {:?}. Result: {}. Details: tasks show T{}",
                dep.reason,
                r.work,
                r.check,
                r.result_summary.as_deref().unwrap_or("not delivered"),
                r.task_id
            ) + &session.states[&r.task_id]
                .result
                .as_ref()
                .map(|result| {
                    format!(
                        "\nArtifacts: {}. Decisions: {}.",
                        result.artifacts.join(", "),
                        result.decisions.join("; ")
                    )
                })
                .unwrap_or_default()
                + &contracts
                    .iter()
                    .map(|op| format!("\nUsed contract: {}", contract_text(&snapshot.ir, &op.id)))
                    .collect::<String>(),
            type_definition: None,
            operation_contracts: contracts,
            source: None,
        });
    }
    for link in session.links.iter().filter(|l| l.to == id) {
        let d = &session.definitions[&link.from];
        let state = &session.states[&link.from];
        items.push(ContextItem {
            profile: None,
            section: "history".into(),
            title: format!("T{} — {}", link.from, d.candidate.title),
            text: format!(
                "{}: {}. Result: {}. Load historical task explicitly.",
                link.kind,
                link.reason,
                state
                    .result
                    .as_ref()
                    .map_or("not delivered", |r| r.summary.as_str())
            ),
            type_definition: None,
            operation_contracts: vec![],
            source: None,
        });
    }
    if let Some(result) = &session.states[&id].result {
        items.push(ContextItem {
            profile: None,
            section: "result".into(),
            title: "Delivered result".into(),
            text: format!(
                "{}\nArtifacts: {}\nDecisions: {}",
                result.summary,
                result.artifacts.join(", "),
                result.decisions.join("; ")
            ),
            type_definition: None,
            operation_contracts: vec![],
            source: None,
        });
    }
    if let Some(selection) = &implementation {
        for document in &selection.bundle.documents {
            items.push(ContextItem {
                profile: Some(ProfileAttribution {
                    target: document.target.clone(),
                    key: document.key.clone(),
                    path: document.path.clone(),
                    content_hash: document.content_hash.clone(),
                    subjects: document.subjects.clone(),
                }),
                section: "implementation".into(),
                title: format!(
                    "Profile: {} — {}",
                    document.path,
                    document.subjects.join(", ")
                ),
                text: document.text.clone(),
                source: None,
                type_definition: None,
                operation_contracts: vec![],
            });
        }
        for path in &selection.missing_files {
            items.push(ContextItem {
                profile: None,
                section: "implementation".into(),
                title: "Required profile missing".into(),
                text: path.clone(),
                source: None,
                type_definition: None,
                operation_contracts: vec![],
            });
        }
    }
    // Put goals and binding rules before potentially large type catalogs.
    items.sort_by_key(|item| match item.section.as_str() {
        "goal" => 0,
        "acceptance" => 1,
        "contract" => 2,
        "changes" => 2,
        "implementation" => 3,
        "rules" => 3,
        "decisions" => 4,
        "types" => 5,
        "examples" => 6,
        "dependencies" => 7,
        "history" => 8,
        _ => 9,
    });
    let required_sections = items
        .iter()
        .filter(|i| {
            matches!(
                i.section.as_str(),
                "implementation"
                    | "goal"
                    | "acceptance"
                    | "contract"
                    | "types"
                    | "rules"
                    | "examples"
                    | "decisions"
                    | "changes"
                    | "dependencies"
            )
        })
        .map(|i| i.section.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected = items
        .into_iter()
        .filter(|i| section == "core" || i.section == section)
        .collect::<Vec<_>>();
    // The item layout is part of cursor identity: older presentations cannot
    // safely resume at the same numeric offset after a CLI upgrade.
    let query = hash(&(
        3_u32,
        id,
        section,
        budget,
        implementation.as_ref().map(|p| &p.bundle.hash),
    ));
    let offset = validate_cursor(session, cursor, &query)?;
    if offset > selected.len() {
        return Err(Error::Invalid(
            "Context cursor offset exceeds content".into(),
        ));
    }
    let mut count = 0;
    let mut bytes = 0;
    let mut chars = 0;
    for item in selected.iter().skip(offset) {
        let size = item.text.len() + item.title.len();
        if count > 0 && bytes + size > budget {
            break;
        }
        bytes += size;
        chars += item.text.chars().count() + item.title.chars().count();
        count += 1;
    }
    let remaining = selected
        .iter()
        .skip(offset + count)
        .map(|i| i.section.clone())
        .collect::<BTreeSet<_>>();
    let missing_sections = if section == "core" {
        required_sections
            .iter()
            .filter(|s| remaining.contains(*s))
            .cloned()
            .collect()
    } else {
        required_sections
            .iter()
            .filter(|s| s.as_str() != section || remaining.contains(*s))
            .cloned()
            .collect()
    };
    let page = paginate(session, selected, count.max(1), offset, query)?;
    Ok(ContextPage {
        implementation: implementation.map(|p| ImplementationContext {
            origin,
            target: p.bundle.target,
            bundle_hash: p.bundle.hash,
            missing_files: p.missing_files,
        }),
        page,
        required_sections,
        missing_sections,
        utf8_bytes: bytes,
        characters: chars,
        token_count: None,
        tokenizer: None,
        oversized_item: bytes > budget,
    })
}
/// Exact declaration blocks retain field annotations and clauses that a compact
/// typed signature does not express. Missing source text is explicitly marked.
fn specification(
    snapshot: &Snapshot,
    index: &SpecIndex<'_>,
    subject: &str,
    section: &str,
) -> (ContextItem, bool) {
    let node = index.nodes.get(subject);
    let raw = node.and_then(|node| {
        snapshot
            .sources
            .iter()
            .find(|s| s.file == node.span.file)
            .and_then(|s| s.text.get(node.span.start.offset..node.span.end.offset))
    });
    let text = raw.map(str::to_owned).unwrap_or_else(|| {
        let clauses = index
            .owned
            .get(subject)
            .into_iter()
            .flatten()
            .filter(|n| n.id != subject)
            .map(|n| node_text(snapshot, n))
            .collect::<Vec<_>>();
        format!(
            "Raw specification unavailable; reconstructed contract and clauses:\n{}\n{}",
            contract_text(&snapshot.ir, subject),
            clauses.join("\n")
        )
    });
    (
        ContextItem {
            profile: None,
            section: section.into(),
            title: format!("Specification: {subject}"),
            text,
            source: node.map(|n| n.span.clone()),
            type_definition: snapshot.ir.data.get(subject).cloned(),
            operation_contracts: snapshot
                .ir
                .operations
                .get(subject)
                .cloned()
                .into_iter()
                .collect(),
        },
        raw.is_some(),
    )
}
fn node_text(snapshot: &Snapshot, node: &dctx0_core::SemanticNode) -> String {
    snapshot
        .sources
        .iter()
        .find(|s| s.file == node.span.file)
        .and_then(|s| s.text.get(node.span.start.offset..node.span.end.offset))
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!(
                "{} {}",
                node.kind.as_str(),
                node.terms
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
}
/// Fences accommodate arbitrary spec narratives without corrupting Markdown.
pub fn fence(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for ch in text.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let delimiter = "`".repeat((longest + 1).max(3));
    format!("{delimiter}\n{text}\n{delimiter}")
}
pub fn context_markdown(context: &ContextPage) -> String {
    let mut output = format!(
        "# Task Context\n\nPlan P{} · snapshot {} · event {}\n",
        context.page.plan_id, context.page.snapshot_id, context.page.event_seq
    );
    output.push_str(&context_content_markdown(context));
    output
}
/// Render context sections inside a command's existing provenance header.
pub fn context_content_markdown(context: &ContextPage) -> String {
    let mut output = String::new();
    if let Some(profiles) = &context.implementation {
        let origin = match profiles.origin {
            ImplementationOrigin::CurrentLocal => {
                "Current local profiles; execution context is not yet bound"
            }
            ImplementationOrigin::StoredExecution => "Stored execution profiles",
            ImplementationOrigin::RequestedRevision => "Immutable requested profile revision",
            ImplementationOrigin::UnknownLegacyExecution => {
                "Current guidance; historical execution profile context is unknown"
            }
        };
        output.push_str(&format!(
            "\n{origin} · target {} · bundle {}.\n",
            profiles.target, profiles.bundle_hash
        ));
    }

    for item in &context.page.items {
        output.push_str(&format!(
            "\n## {}\n\n{}\n",
            item.title.replace('\n', " "),
            if matches!(
                item.section.as_str(),
                "goal"
                    | "acceptance"
                    | "changes"
                    | "dependencies"
                    | "history"
                    | "result"
                    | "implementation"
            ) {
                item.text.clone()
            } else {
                fence(&item.text)
            }
        ));
        if let Some(span) = &item.source {
            output.push_str(&format!("\nSource: {}:{}\n", span.file, span.start.line));
        }
    }
    if !context.missing_sections.is_empty() {
        output.push_str(&format!(
            "\nRequired content still to load: {}.\n",
            context.missing_sections.join(", ")
        ));
    }
    if let Some(cursor) = &context.page.next_cursor {
        output.push_str(&format!(
            "\nContinue with --cursor '{}'.\n",
            serde_json::to_string(cursor).unwrap()
        ));
    }
    output
}
