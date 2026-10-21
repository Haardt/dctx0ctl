use dctx0_tasks::*;
use serde::Serialize;
#[derive(Serialize)]
pub(super) struct Output {
    pub(super) schema_version: u32,
    pub(super) plan_id: Option<i64>,
    pub(super) snapshot_id: Option<String>,
    pub(super) event_seq: i64,
    pub(super) body: Body,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) verification_evidence: Vec<dctx0_store::VerificationRunSummary>,
}
#[derive(Serialize)]
#[serde(tag = "view", content = "data", rename_all = "kebab-case")]
pub(super) enum Body {
    Package(WorkPackage),
    PackageDelta(PackageDelta),
    Progress(ProgressReport),
    Preview(CandidatePlan),
    ProfileChanges(ProfileUpdatePreview),
    Plan(Plan),
    List(Page<Overview>),
    Waves(Vec<Wave>),
    Next {
        task: Option<Overview>,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        context: Option<Box<ContextPage>>,
    },
    Show {
        overview: Overview,
        definition: Box<Definition>,
        state: Box<State>,
        context: Box<ContextPage>,
    },
    Context(ContextPage),
    History {
        links: Vec<HistoryLink>,
        events: Page<Event>,
        tasks: Vec<HistorySummary>,
    },
    Dependencies(Page<DependencyView>),
    Diff(PlanDiff),
    Slice {
        slice: Slice,
        page: Page<Overview>,
    },
    Receipt(Receipt),
}
#[derive(Serialize)]
pub(super) struct Wave {
    pub(super) wave: usize,
    pub(super) total: usize,
    pub(super) open: usize,
    pub(super) progress: usize,
    pub(super) done: usize,
    pub(super) pass: usize,
    pub(super) fail: usize,
    pub(super) ready: usize,
}
#[derive(Serialize)]
pub(super) struct HistorySummary {
    pub(super) task_id: i64,
    pub(super) title: String,
    pub(super) work: WorkState,
    pub(super) check: CheckStatus,
    pub(super) result: Option<String>,
}
fn row(row: &Overview) -> String {
    format!(
        "- T{} · wave {} · {:?} · {:?} · {:?}: {}{}\n",
        row.task_id,
        row.wave,
        row.kind,
        row.work,
        row.check,
        row.title.replace('\n', " "),
        if row.blockers.is_empty() {
            String::new()
        } else {
            format!(" — {}", row.blockers.join("; "))
        }
    )
}
fn continuation<T>(page: &Page<T>) -> String {
    page.next_cursor
        .as_ref()
        .map(|c| {
            format!(
                "\nMore results: --cursor '{}'\n",
                serde_json::to_string(c).unwrap()
            )
        })
        .unwrap_or_default()
}
pub(crate) fn markdown(report: &Output) -> String {
    let mut out = String::from("# Tasks\n\n");
    if let Some(plan) = report.plan_id {
        out.push_str(&format!(
            "Pinned plan P{plan} · snapshot {} · event {}. Working files are not checked.\n\n",
            report.snapshot_id.as_deref().unwrap(),
            report.event_seq
        ));
    }
    match &report.body {
        Body::Package(package) => {
            out.push_str(&format!("## Work package `{}`\n\nComplete context: {}; dependency-closed: {}. No execution reserved.\n", package.id, package.complete, package.slice.closed));
            for task in &package.tasks {
                out.push_str(&row(&task.overview));
                out.push_str(&format!(
                    "  Context references: {}\n  Evidence: {:?}\n",
                    task.context.join(", "),
                    task.evidence
                ));
                if !task.profile_subjects.is_empty() {
                    out.push_str(&format!(
                        "  Profile subject attribution: {:?}\n",
                        task.profile_subjects
                    ));
                }
            }
            for limitation in &package.limitations {
                out.push_str(&format!("\nIncomplete: {limitation}\n"));
            }
            for (id, item) in &package.content {
                out.push_str(&format!(
                    "\n### {} · `{id}`\n\n{}\n",
                    item.title,
                    fence(&item.text)
                ));
            }
        }
        Body::PackageDelta(delta) => {
            out.push_str(&format!(
                "## Package delta\n\n{} → {}\n\nAdded tasks: {:?}\nRemoved tasks: {:?}\n",
                delta.before, delta.after, delta.added_tasks, delta.removed_tasks
            ));
            for task in &delta.changed_tasks {
                out.push_str(&format!("- T{}: reload context: {}; requirements: {}; prerequisites: {}; profiles: {}; state: {}; evidence: {}\n", task.task_id, task.reload_context, task.requirements_changed, task.prerequisites_changed, task.profiles_changed, task.state_changed, task.evidence_changed));
            }
            for (id, item) in &delta.added_content {
                out.push_str(&format!(
                    "\n### {} · `{id}`\n\n{}\n",
                    item.title,
                    fence(&item.text)
                ));
            }
        }
        Body::Progress(progress) => {
            out.push_str(&format!("## Progress P{} @ {} → P{} @ {}\n\nNew tasks: {:?}\nRemoved tasks: {:?}\nNewly Done: {:?}\nNewly PASS: {:?}\nLost active PASS: {:?}\nUnfinished: {:?}\nBlocked: {:?}\n\nNew tasks are reported separately; Done and PASS measure separate transitions. No feature-completeness claim.\n", progress.before.plan_id, progress.before.event_seq, progress.after.plan_id, progress.after.event_seq, progress.added, progress.removed, progress.newly_done, progress.newly_pass, progress.lost_pass, progress.unfinished, progress.blocked));
            out.push_str(&format!(
                "\nAdded tasks already Done: {:?}\nAdded tasks already PASS: {:?}\n",
                progress.added_done, progress.added_pass
            ));
            for e in &progress.events {
                out.push_str(&format!("- {}: {} — {}\n", e.seq, e.kind, e.message));
            }
        }
        Body::ProfileChanges(preview) => {
            out.push_str("## Profile change preview\n\nNo work or testing status has changed.\n");
            for change in &preview.changes {
                if change.unknown_origin || !change.changed_files.is_empty() {
                    out.push_str(&format!(
                        "\n- T{} {:?}: {} → {}{}{}\n",
                        change.task_id,
                        change.work,
                        change.old_target.as_deref().unwrap_or("unknown origin"),
                        change.desired.bundle.target,
                        if change.selected {
                            "; selected for adaptation"
                        } else {
                            ""
                        },
                        if change.unknown_origin {
                            "; legacy context is unknown, not drift"
                        } else {
                            ""
                        }
                    ));
                    for file in &change.changed_files {
                        out.push_str(&format!(
                            "  - {}: {:?} → {:?}\n",
                            file.path, file.before_hash, file.after_hash
                        ));
                    }
                    for path in &change.desired.missing_files {
                        out.push_str(&format!("  - Required profile missing: {path}\n"));
                    }
                }
            }
            out.push_str(&format!("\nSelected tasks: {:?}\nRequired integration/review revisions: {:?}\nPossible impacts (not selected): {:?}\n", preview.selected_tasks, preview.revised_assemblies, preview.possible_impacts));
        }

        Body::Preview(plan) => {
            out.push_str("Preview — nothing stored.\n\n");
            for t in &plan.tasks {
                out.push_str(&format!("- wave {} · {:?}: {}\n", t.wave, t.kind, t.title));
            }
            out.push_str(&format!(
                "\n{} verification obligations remain separate from implementation work.\n",
                plan.obligations.len()
            ));
        }
        Body::Plan(plan) => {
            out.push_str(&format!(
                "Published P{}: {} tasks.\n\n",
                plan.id,
                plan.members.len()
            ));
            for m in &plan.members {
                out.push_str(&format!(
                    "- T{} · wave {}: {}\n",
                    m.task_id, m.candidate.wave, m.candidate.title
                ));
            }
        }
        Body::List(page) => {
            for r in &page.items {
                out.push_str(&row(r));
            }
            out.push_str(&continuation(page));
        }
        Body::Waves(waves) => {
            for w in waves {
                out.push_str(&format!("- Wave {}: {} tasks; {} open, {} progress, {} done; {} PASS, {} FAIL; {} ready.\n",w.wave,w.total,w.open,w.progress,w.done,w.pass,w.fail,w.ready));
            }
        }
        Body::Next {
            task,
            reason,
            context,
        } => {
            if let Some(task) = task {
                out.push_str(&row(task));
            } else {
                out.push_str("No matching ready open task.\n");
            }
            out.push_str(&format!("\n{reason}\n"));
            if let Some(context) = context {
                out.push_str(&context_content_markdown(context));
            }
        }
        Body::Show {
            overview,
            definition: _,
            state,
            context,
        } => {
            out.push_str(&row(overview));
            out.push_str(&context_content_markdown(context));
            if let Some(claim) = state.claim {
                out.push_str(&format!("\nClaim {claim}; version {}.\n", state.version));
            }
        }
        Body::Context(context) => return context_markdown(context),
        Body::History {
            links,
            events,
            tasks,
        } => {
            for t in tasks {
                out.push_str(&format!(
                    "- T{}: {} · {:?} · {:?}; result: {}\n",
                    t.task_id,
                    t.title,
                    t.work,
                    t.check,
                    t.result.as_deref().unwrap_or("not delivered")
                ));
            }
            for l in links {
                out.push_str(&format!(
                    "- T{} → T{}: {} — {}\n",
                    l.from, l.to, l.kind, l.reason
                ));
            }
            out.push_str("\n## Events\n\n");
            for e in &events.items {
                out.push_str(&format!("- {}: {} — {}\n", e.seq, e.kind, e.message));
            }
            out.push_str(&continuation(events));
        }
        Body::Dependencies(page) => {
            for e in &page.items {
                out.push_str(&format!(
                    "- T{} needs T{} ({:?}): {}\n",
                    e.task_id, e.prerequisite_id, e.condition, e.reason
                ));
            }
            out.push_str(&continuation(page));
        }
        Body::Diff(diff) => {
            out.push_str(&format!(
                "P{} → P{}\n\nNew: {:?}\nKept: {:?}\nRemoved: {:?}\n",
                diff.before, diff.after, diff.added, diff.kept, diff.removed
            ));
            for l in &diff.links {
                out.push_str(&format!("\nT{} → T{}: {}\n", l.from, l.to, l.reason));
            }
        }
        Body::Slice { slice, page } => {
            out.push_str(&format!("Anchor T{}; dependency-closed: {}. This is structural readiness, not an implementation guarantee.\n\n",slice.anchor,slice.closed));
            for r in &page.items {
                out.push_str(&row(r));
            }
            for b in &slice.blockers {
                out.push_str(&format!("\nBlocked: {b}\n"));
            }
            out.push_str(&continuation(page));
        }
        Body::Receipt(receipt) => {
            out.push_str(&format!("{}\n", receipt.message));
            if let Some(id) = receipt.task_id {
                out.push_str(&format!("\nTask T{id}\n"));
            }
            if let Some(group) = receipt.group_id {
                out.push_str(&format!("\nReservation group G{group}\n"));
            }
            if let Some(state) = &receipt.state {
                out.push_str(&format!(
                    "\nState {:?}; check {:?}; version {}; claim {:?}\n",
                    state.work, state.check, state.version, state.claim
                ));
            }
        }
    }
    out
}
