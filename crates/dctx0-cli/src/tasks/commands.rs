use super::args::{TaskArgs, Verb};
use super::render::{Body, HistorySummary, Output, Wave, markdown};
use crate::{error_message, render::json};
use dctx0_engine::Project;
use dctx0_tasks::*;
use std::{collections::BTreeMap, fs, io::Write, path::Path, result::Result};
fn required<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str, String> {
    value.ok_or_else(|| format!("{name} is required"))
}
fn identifier(value: &str) -> Result<i64, String> {
    let raw = value.trim_start_matches(['T', 'P', 'G']);
    let id = raw.parse::<i64>().map_err(error_message)?;
    if id <= 0 {
        Err("ID must be positive".into())
    } else {
        Ok(id)
    }
}
fn enumeration<T: serde::de::DeserializeOwned>(value: Option<&str>) -> Result<Option<T>, String> {
    value
        .map(|s| serde_json::from_value(serde_json::Value::String(s.into())).map_err(error_message))
        .transpose()
}
fn read<T: serde::de::DeserializeOwned>(path: Option<&Path>) -> Result<T, String> {
    let path = path.ok_or("--file is required")?;
    serde_json::from_slice(&fs::read(path).map_err(error_message)?).map_err(error_message)
}
fn profiles_for<'a>(
    session: &Session,
    task: i64,
    profiles: &'a dctx0_engine::Result<Option<dctx0_engine::LoadedProfiles>>,
) -> Result<Option<&'a dctx0_engine::LoadedProfiles>, String> {
    if session.states[&task].execution_bundle.is_some()
        || matches!(
            session.definitions[&task].adjustment,
            Some(AdjustmentReason::Profile { .. })
        )
    {
        return Ok(None);
    }
    Ok(profiles.as_ref().map_err(error_message)?.as_ref())
}
pub(crate) fn run(
    args: TaskArgs,
    db: Option<&Path>,
    global_json: bool,
    output: &mut dyn Write,
) -> Result<(), String> {
    if args
        .format
        .as_deref()
        .is_some_and(|s| !matches!(s, "markdown" | "json"))
        || global_json && args.format.as_deref() == Some("markdown")
    {
        return Err("Use --format markdown/json; --json conflicts with markdown".into());
    }
    if args.dry_run && !matches!(args.action, Verb::Plan | Verb::Preview) {
        return Err("--dry-run is available for planning only".into());
    }
    let project = Project::new(&args.input, db).map_err(error_message)?;
    let profiles = project.profiles(args.project.as_deref(), args.target.as_deref());
    if args.target.is_some() {
        profiles.as_ref().map_err(error_message)?;
    }
    let cursor = args
        .cursor
        .as_ref()
        .map(|s| serde_json::from_str::<Cursor>(s).map_err(error_message))
        .transpose()?;
    let filters = Filters {
        wave: args.wave,
        state: enumeration(args.state.as_deref())?,
        check: enumeration(args.check.as_deref())?,
        kind: enumeration(args.kind.as_deref())?,
        subject: args.subject.clone(),
        domain: args.domain.clone(),
        actor: args.actor.clone(),
    };
    let task = || identifier(required(args.id.as_deref(), "Task ID")?);
    let actor = || required(args.actor.as_deref(), "--actor").map(str::to_owned);
    let claim = || args.claim.ok_or_else(|| "--claim is required".to_owned());
    let version = || {
        args.version
            .ok_or_else(|| "--version is required".to_owned())
    };
    let mut report = Output {
        schema_version: 2,
        plan_id: None,
        snapshot_id: None,
        event_seq: 0,
        body: Body::Next {
            task: None,
            reason: String::new(),
            context: None,
        },
    };
    report.body = match args.action {
        Verb::ProfileChanges => {
            let profiles = profiles
                .as_ref()
                .map_err(error_message)?
                .as_ref()
                .ok_or("Profile configuration is required")?;
            let preview = project
                .task_profile_changes(
                    profiles,
                    ProfileUpdateSelection {
                        tasks: args
                            .tasks
                            .iter()
                            .map(|s| identifier(s))
                            .collect::<Result<Vec<_>, _>>()?,
                        domain: args.domain.clone(),
                        semantic: args.semantic.clone(),
                        all_affected: args.all_affected,
                    },
                )
                .map_err(error_message)?;
            report.plan_id = Some(preview.plan_id);
            report.snapshot_id = Some(preview.snapshot_id.clone());
            report.event_seq = preview.event_seq;
            Body::ProfileChanges(preview)
        }
        Verb::ProfileUpdate => {
            let value: serde_json::Value = read(args.file.as_deref())?;
            let preview: ProfileUpdatePreview =
                serde_json::from_value(value.pointer("/body/data").unwrap_or(&value).clone())
                    .map_err(error_message)?;
            let profiles = profiles
                .as_ref()
                .map_err(error_message)?
                .as_ref()
                .ok_or("Profile configuration is required")?;
            Body::Plan(
                project
                    .task_profile_update(
                        &preview,
                        profiles,
                        required(args.reason.as_deref(), "--reason")?,
                    )
                    .map_err(error_message)?,
            )
        }
        Verb::Preview => Body::Preview(
            project
                .task_preview(&args.snapshot)
                .map_err(error_message)?,
        ),
        Verb::Plan => {
            if args.dry_run {
                Body::Preview(
                    project
                        .task_preview(&args.snapshot)
                        .map_err(error_message)?,
                )
            } else {
                Body::Plan(project.task_plan(&args.snapshot).map_err(error_message)?)
            }
        }
        Verb::RenewReview => Body::Plan(
            project
                .task_renew_review(
                    task()?,
                    args.project.as_deref(),
                    required(args.reason.as_deref(), "--reason")?,
                )
                .map_err(error_message)?,
        ),
        Verb::FollowUp => Body::Plan(
            project
                .task_follow_up(
                    task()?,
                    args.finding.as_deref(),
                    required(args.reason.as_deref(), "--reason")?,
                )
                .map_err(error_message)?,
        ),
        Verb::Start
        | Verb::ClaimNext
        | Verb::Release
        | Verb::Complete
        | Verb::ClaimSlice
        | Verb::ReleaseSlice
        | Verb::RecordCheck
        | Verb::RecordReview
        | Verb::AddNote => {
            let action = match args.action {
                Verb::Start => Action::Start {
                    task: task()?,
                    actor: actor()?,
                    reservation: args.reservation,
                },
                Verb::ClaimNext => Action::ClaimNext {
                    actor: actor()?,
                    filters,
                },
                Verb::Release => Action::Release {
                    task: task()?,
                    actor: actor()?,
                    claim: claim()?,
                    version: version()?,
                    reason: required(args.reason.as_deref(), "--reason")?.into(),
                },
                Verb::Complete => {
                    let result = if args.file.is_some() {
                        read(args.file.as_deref())?
                    } else {
                        TaskResult {
                            summary: required(args.summary.as_deref(), "--summary or --file")?
                                .into(),
                            artifacts: args.artifact,
                            decisions: vec![],
                            implementation: args.implementation,
                        }
                    };
                    Action::Complete {
                        task: task()?,
                        actor: actor()?,
                        claim: claim()?,
                        version: version()?,
                        result,
                    }
                }
                Verb::ClaimSlice => Action::ClaimSlice {
                    anchor: task()?,
                    actor: actor()?,
                    domain: args.domain,
                },
                Verb::ReleaseSlice => Action::ReleaseSlice {
                    group: task()?,
                    actor: actor()?,
                    reason: required(args.reason.as_deref(), "--reason")?.into(),
                },
                Verb::RecordCheck => Action::RecordCheck {
                    task: task()?,
                    evidence: read(args.file.as_deref())?,
                },
                Verb::RecordReview => Action::RecordReview {
                    task: task()?,
                    actor: actor()?,
                    claim: claim()?,
                    version: version()?,
                    report: read(args.file.as_deref())?,
                },
                _ => Action::Note {
                    task: task()?,
                    note: required(args.reason.as_deref(), "--reason")?.into(),
                },
            };
            Body::Receipt(
                project
                    .task_action_selected(action, args.project.as_deref(), args.target.as_deref())
                    .map_err(error_message)?,
            )
        }
        _ => {
            let selected_plan = args
                .plan
                .as_ref()
                .map(|s| identifier(s))
                .transpose()?
                .or(cursor.as_ref().map(|c| c.plan_id));
            let as_of = cursor.as_ref().map(|c| c.event_seq);
            let detail = matches!(
                args.action,
                Verb::Show | Verb::Context | Verb::History | Verb::Dependencies | Verb::Slice
            );
            let session = if detail {
                project.task_show_session(task()?, selected_plan, as_of)
            } else {
                project.task_session(selected_plan, as_of)
            }
            .map_err(error_message)?;
            report.plan_id = Some(session.plan.id);
            report.snapshot_id = Some(session.plan.snapshot_id.clone());
            report.event_seq = session.event_seq;
            let query = |tag: &str| {
                dctx0_changes::hash(&(tag, &args.id, &args.direction, args.transitive, &filters))
            };
            match args.action {
                Verb::List => {
                    if args.all_plans {
                        let mut rows = vec![];
                        let mut cached = BTreeMap::new();
                        let store = project
                            .store(dctx0_store::AccessMode::ReadOnly)
                            .map_err(error_message)?;
                        for id in session.definitions.keys() {
                            let plan = store
                                .task_membership(&project.key(), *id, Some(session.event_seq))
                                .map_err(error_message)?;
                            if let std::collections::btree_map::Entry::Vacant(entry) =
                                cached.entry(plan)
                            {
                                let historical = project
                                    .task_session(Some(plan), Some(session.event_seq))
                                    .map_err(error_message)?;
                                entry.insert(
                                    historical
                                        .overviews()
                                        .into_iter()
                                        .map(|r| (r.task_id, r))
                                        .collect::<BTreeMap<_, _>>(),
                                );
                            }
                            let row = &cached[&plan][id];
                            if Session::matches(row, &filters) {
                                rows.push(row.clone());
                            }
                        }
                        let q = query("all-plans");
                        let offset = validate_cursor(&session, cursor.as_ref(), &q)
                            .map_err(error_message)?;
                        Body::List(
                            paginate(&session, rows, args.limit, offset, q)
                                .map_err(error_message)?,
                        )
                    } else {
                        Body::List(
                            session
                                .list(&filters, args.limit, cursor.as_ref())
                                .map_err(error_message)?,
                        )
                    }
                }
                Verb::Waves => {
                    let mut waves = BTreeMap::<usize, Wave>::new();
                    for row in session.overviews() {
                        let wave = waves.entry(row.wave).or_insert(Wave {
                            wave: row.wave,
                            total: 0,
                            open: 0,
                            progress: 0,
                            done: 0,
                            pass: 0,
                            fail: 0,
                            ready: 0,
                        });
                        wave.total += 1;
                        match row.work {
                            WorkState::Open => wave.open += 1,
                            WorkState::Progress => wave.progress += 1,
                            WorkState::Done => wave.done += 1,
                        }
                        wave.pass += usize::from(row.check == CheckStatus::Pass);
                        wave.fail += usize::from(row.check == CheckStatus::Fail);
                        wave.ready += usize::from(row.ready);
                    }
                    Body::Waves(waves.into_values().collect())
                }
                Verb::Next => {
                    let task = session.next(&filters, args.actor.as_deref());
                    let assignment = if let Some(task) = &task {
                        let snapshot = project
                            .load(&session.plan.snapshot_id)
                            .map_err(error_message)?;
                        Some(Box::new(
                            project
                                .task_context(
                                    &session,
                                    &snapshot,
                                    task.task_id,
                                    &args.section,
                                    (args.budget, cursor.as_ref()),
                                    profiles_for(&session, task.task_id, &profiles)?,
                                )
                                .map_err(error_message)?,
                        ))
                    } else {
                        None
                    };
                    Body::Next {
                        reason: if task.is_some() {
                            "Read-only selection; this task has not been claimed.".into()
                        } else {
                            "Inspect prerequisites, filters and claims for blockers.".into()
                        },
                        task,
                        context: assignment,
                    }
                }
                Verb::Show => {
                    let id = task()?;
                    let snapshot = project
                        .load(&session.plan.snapshot_id)
                        .map_err(error_message)?;
                    Body::Show {
                        overview: session
                            .overviews()
                            .into_iter()
                            .find(|r| r.task_id == id)
                            .unwrap(),
                        definition: Box::new(session.definitions[&id].clone()),
                        state: Box::new(session.states[&id].clone()),
                        context: Box::new(
                            project
                                .task_context(
                                    &session,
                                    &snapshot,
                                    id,
                                    &args.section,
                                    (args.budget, cursor.as_ref()),
                                    profiles_for(&session, id, &profiles)?,
                                )
                                .map_err(error_message)?,
                        ),
                    }
                }
                Verb::Context => {
                    let snapshot = project
                        .load(&session.plan.snapshot_id)
                        .map_err(error_message)?;
                    Body::Context(
                        project
                            .task_context(
                                &session,
                                &snapshot,
                                task()?,
                                &args.section,
                                (args.budget, cursor.as_ref()),
                                profiles_for(&session, task()?, &profiles)?,
                            )
                            .map_err(error_message)?,
                    )
                }
                Verb::History => {
                    let id = task()?;
                    let links = session
                        .history_links(id, &args.direction)
                        .map_err(error_message)?;
                    let mut ids = std::collections::BTreeSet::from([id]);
                    for link in &links {
                        ids.insert(link.from);
                        ids.insert(link.to);
                    }
                    let events = session
                        .events
                        .iter()
                        .filter(|e| e.task_id.is_some_and(|id| ids.contains(&id)))
                        .cloned()
                        .collect();
                    let q = query("history");
                    let offset =
                        validate_cursor(&session, cursor.as_ref(), &q).map_err(error_message)?;
                    let tasks = ids
                        .into_iter()
                        .map(|id| HistorySummary {
                            task_id: id,
                            title: session.definitions[&id].candidate.title.clone(),
                            work: session.states[&id].work,
                            check: session.states[&id].check,
                            result: session.states[&id]
                                .result
                                .as_ref()
                                .map(|r| r.summary.clone()),
                        })
                        .collect();
                    Body::History {
                        links,
                        events: paginate(&session, events, args.limit, offset, q)
                            .map_err(error_message)?,
                        tasks,
                    }
                }
                Verb::Dependencies => {
                    if !matches!(
                        args.direction.as_str(),
                        "both" | "prerequisites" | "dependents"
                    ) {
                        return Err(
                            "Dependency direction must be prerequisites or dependents".into()
                        );
                    }
                    let rows = session
                        .related(task()?, args.direction == "dependents", args.transitive)
                        .map_err(error_message)?;
                    let q = query("dependencies");
                    let offset =
                        validate_cursor(&session, cursor.as_ref(), &q).map_err(error_message)?;
                    Body::Dependencies(
                        paginate(&session, rows, args.limit, offset, q).map_err(error_message)?,
                    )
                }
                Verb::Diff => {
                    let before = project
                        .task_session(Some(task()?), as_of)
                        .map_err(error_message)?;
                    let after_id = identifier(required(args.other.as_deref(), "After-plan ID")?)?;
                    let after = project
                        .task_session(Some(after_id), as_of)
                        .map_err(error_message)?;
                    Body::Diff(diff_plans(&before.plan, &after.plan, &after.links))
                }
                _ => {
                    let mut slice = session
                        .slice(task()?, args.domain.as_deref(), args.actor.as_deref())
                        .map_err(error_message)?;
                    let q = query("slice");
                    let offset =
                        validate_cursor(&session, cursor.as_ref(), &q).map_err(error_message)?;
                    let page = paginate(&session, slice.tasks.clone(), args.limit, offset, q)
                        .map_err(error_message)?;
                    slice.tasks = page.items.clone();
                    Body::Slice { slice, page }
                }
            }
        }
    };
    if let Body::Plan(plan) = &report.body {
        report.plan_id = Some(plan.id);
        report.snapshot_id = Some(plan.snapshot_id.clone());
        report.event_seq = project
            .task_session(Some(plan.id), None)
            .map_err(error_message)?
            .event_seq;
    }
    if global_json || args.format.as_deref() == Some("json") {
        json(output, &report)
    } else {
        output
            .write_all(markdown(&report).as_bytes())
            .map_err(error_message)
    }
}
