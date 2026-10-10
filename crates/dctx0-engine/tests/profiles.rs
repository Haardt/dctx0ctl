use dctx0_core::{ProfileRole, TargetProfileProvider};
use dctx0_engine::{Project, create_profiles};
use dctx0_tasks::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-profiles-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("dctx0")).unwrap();
        Self(root)
    }
    fn project(&self, spec: &str) -> Project {
        fs::write(self.0.join("dctx0/spec.dctx0"), spec).unwrap();
        let p = Project::new(&self.0.join("dctx0"), None).unwrap();
        assert!(p.check().unwrap().ok);
        p.task_plan("latest").unwrap();
        p
    }
    fn create(&self) {
        create_profiles(&self.0, &dctx0_target_rust::Provider, false, true, false).unwrap();
    }
    fn edit(&self, path: &str, text: &str) {
        fs::write(self.0.join(format!(".dctx0/profiles/rust/{path}")), text).unwrap();
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
const SPEC: &str = "workspace W { value Id = UUID; entity Item { id: Id; key id } message Input { item: Item } event Added { item: Item } module M { owns { Item } port Repo { operation save(input: Input) -> Added } surface Api { operation add(input: Input) -> Added; effects { call Repo.save } } } }";
fn complete(p: &Project, id: i64, profiles: Option<&dctx0_engine::LoadedProfiles>) {
    let receipt = p
        .task_action_with_profiles(
            Action::Start {
                task: id,
                actor: "worker".into(),
                reservation: None,
            },
            profiles,
        )
        .unwrap();
    let state = receipt.state.unwrap();
    p.task_action(Action::Complete {
        task: id,
        actor: "worker".into(),
        claim: state.claim.unwrap(),
        version: state.version,
        result: TaskResult {
            summary: "Delivered".into(),
            artifacts: vec!["src/domain.rs".into()],
            decisions: vec![],
            implementation: Some("revision1".into()),
        },
    })
    .unwrap();
}
#[test]
fn creation_is_resumable_preserves_edits_and_agents_text_and_dry_run_is_read_only() {
    let t = Temp::new();
    fs::write(t.0.join("AGENTS.md"), "# Existing conventions\n").unwrap();
    let dry = create_profiles(&t.0, &dctx0_target_rust::Provider, false, true, true).unwrap();
    assert_eq!(dry.files.len(), 40);
    assert!(!t.0.join(".dctx0").exists());
    t.create();
    t.edit("implement/value.md", "Custom value representation");
    let repeat = create_profiles(&t.0, &dctx0_target_rust::Provider, false, true, false).unwrap();
    assert!(repeat.files.is_empty());
    assert_eq!(
        fs::read_to_string(t.0.join(".dctx0/profiles/rust/implement/value.md")).unwrap(),
        "Custom value representation"
    );
    let agents = fs::read_to_string(t.0.join("AGENTS.md")).unwrap();
    assert!(agents.starts_with("# Existing conventions\n"));
    assert_eq!(agents.matches("<!-- dctx0 profiles begin -->").count(), 1);
    fs::remove_file(t.0.join(".dctx0/profiles/rust/use/value.md")).unwrap();
    let repaired = create_profiles(&t.0, &dctx0_target_rust::Provider, true, true, false).unwrap();
    assert_eq!(repaired.files, vec![".dctx0/profiles/rust/use/value.md"]);
    fs::write(t.0.join("AGENTS.md"), "<!-- dctx0 profiles end -->").unwrap();
    assert!(
        create_profiles(&t.0, &dctx0_target_rust::Provider, false, true, true)
            .unwrap_err()
            .to_string()
            .contains("Ambiguous")
    );
    fs::write(
        t.0.join("AGENTS.md"),
        "<!-- dctx0 profiles begin --><!-- dctx0 profiles begin --><!-- dctx0 profiles end -->",
    )
    .unwrap();
    assert!(create_profiles(&t.0, &dctx0_target_rust::Provider, false, true, false).is_err());
}
#[test]
fn semantic_profiles_are_pinned_budgeted_and_empty_files_are_intentional() {
    let t = Temp::new();
    t.create();
    let p = t.project(SPEC);
    let session = p.task_session(None, None).unwrap();
    let snapshot = p.load("latest").unwrap();
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let entity = session
        .plan
        .members
        .iter()
        .find(|m| m.candidate.subjects == ["W.Item"])
        .unwrap();
    let selection = select_profiles(
        &snapshot,
        &entity.candidate,
        &profiles.manifest,
        &profiles.documents,
    );
    assert!(selection.missing_files.is_empty());
    assert!(selection.bundle.documents.iter().any(|d| {
        d.key
            .as_ref()
            .is_some_and(|k| k.semantic == "entity" && k.role == ProfileRole::Implement)
    }));
    assert!(selection.bundle.documents.iter().any(|d| {
        d.key
            .as_ref()
            .is_some_and(|k| k.semantic == "value" && k.role == ProfileRole::Use)
    }));
    assert!(!selection.bundle.documents.iter().any(|d| {
        d.key
            .as_ref()
            .is_some_and(|k| k.semantic == "module" && k.role == ProfileRole::Implement)
    }));
    let page = p
        .task_context(
            &session,
            &snapshot,
            entity.task_id,
            "implementation",
            (1, None),
            Some(&profiles),
        )
        .unwrap();
    assert!(page.oversized_item);
    assert!(!page.page.complete);
    assert!(page.missing_sections.contains(&"implementation".into()));
    let cursor = page.page.next_cursor.unwrap();
    t.edit("use/value.md", "Changed representation");
    let updated = p.profiles(None, None).unwrap().unwrap();
    assert!(
        p.task_context(
            &session,
            &snapshot,
            entity.task_id,
            "implementation",
            (1, Some(&cursor)),
            Some(&updated)
        )
        .is_err()
    );
    t.edit("implement/review.md", "Unrelated edit");
    let unrelated = p.profiles(None, None).unwrap().unwrap();
    assert_eq!(
        select_profiles(
            &snapshot,
            &entity.candidate,
            &updated.manifest,
            &updated.documents
        )
        .bundle
        .hash,
        select_profiles(
            &snapshot,
            &entity.candidate,
            &unrelated.manifest,
            &unrelated.documents
        )
        .bundle
        .hash
    );
    t.edit("implement/entity.md", "");
    let empty = p.profiles(None, None).unwrap().unwrap();
    assert!(
        select_profiles(
            &snapshot,
            &entity.candidate,
            &empty.manifest,
            &empty.documents
        )
        .missing_files
        .is_empty()
    );
    fs::remove_file(t.0.join(".dctx0/profiles/rust/implement/entity.md")).unwrap();
    let missing = p.profiles(None, None).unwrap().unwrap();
    let selected = select_profiles(
        &snapshot,
        &entity.candidate,
        &missing.manifest,
        &missing.documents,
    );
    assert_eq!(
        selected.missing_files,
        [".dctx0/profiles/rust/implement/entity.md"]
    );
    let page = p
        .task_context(
            &session,
            &snapshot,
            entity.task_id,
            "core",
            (usize::MAX, None),
            Some(&missing),
        )
        .unwrap();
    assert!(context_markdown(&page).contains("Required profile missing"));
    // Prerequisites first; an incomplete profile never changes claim state.
    for m in session
        .plan
        .members
        .iter()
        .filter(|m| m.candidate.wave < entity.candidate.wave)
    {
        complete(&p, m.task_id, Some(&missing));
    }
    assert!(
        p.task_action_with_profiles(
            Action::Start {
                task: entity.task_id,
                actor: "worker".into(),
                reservation: None
            },
            Some(&missing)
        )
        .is_err()
    );
    assert_eq!(
        p.task_session(None, None).unwrap().states[&entity.task_id].work,
        WorkState::Open
    );
}
#[test]
fn claims_freeze_history_release_reloads_and_completed_context_survives_deletion() {
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { value Id = UUID }");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let receipt = p
        .task_action_with_profiles(
            Action::ClaimNext {
                actor: "a".into(),
                filters: Filters::default(),
            },
            Some(&profiles),
        )
        .unwrap();
    let id = receipt.task_id.unwrap();
    let state = receipt.state.unwrap();
    let old = state.execution_bundle.clone().unwrap();
    let before = p.task_session(None, None).unwrap().event_seq;
    t.edit("implement/value.md", "Use a newtype");
    let updated = p.profiles(None, None).unwrap().unwrap();
    let session = p.task_session(None, None).unwrap();
    let snapshot = p.load("latest").unwrap();
    let page = p
        .task_context(
            &session,
            &snapshot,
            id,
            "implementation",
            (usize::MAX, None),
            Some(&updated),
        )
        .unwrap();
    assert_eq!(page.implementation.unwrap().bundle_hash, old.hash);
    p.task_action(Action::Release {
        task: id,
        actor: "a".into(),
        claim: state.claim.unwrap(),
        version: state.version,
        reason: "Retry".into(),
    })
    .unwrap();
    assert!(
        p.task_session(None, None).unwrap().states[&id]
            .execution_bundle
            .is_none()
    );
    complete(&p, id, Some(&updated));
    let session = p.task_session(None, None).unwrap();
    let new = session.states[&id].execution_bundle.clone().unwrap();
    assert_ne!(new.hash, old.hash);
    assert_eq!(
        p.task_session(None, Some(before)).unwrap().states[&id]
            .execution_bundle
            .as_ref()
            .unwrap()
            .hash,
        old.hash
    );
    fs::remove_dir_all(t.0.join(".dctx0/profiles/rust")).unwrap();
    let page = p
        .task_context(
            &session,
            &snapshot,
            id,
            "implementation",
            (usize::MAX, None),
            None,
        )
        .unwrap();
    assert_eq!(page.implementation.unwrap().bundle_hash, new.hash);
    assert_eq!(session.states[&id].check, CheckStatus::Unverified);
}
#[test]
fn profile_updates_are_selective_stale_guarded_idempotent_and_survive_planning() {
    let t = Temp::new();
    t.create();
    let p=t.project("workspace W { value Id = UUID; type Other = Int; module M { surface Api { operation read(id: Id) -> Int { pure } } } }");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let mut session = p.task_session(None, None).unwrap();
    let id = session
        .plan
        .members
        .iter()
        .find(|m| m.candidate.subjects == ["W.Id"])
        .unwrap()
        .task_id;
    complete(&p, id, Some(&profiles));
    session = p.task_session(None, None).unwrap();
    let snapshot = p.load("latest").unwrap();
    t.edit("implement/value.md", "Adopt a new representation");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let none = profile_changes(
        &session,
        &snapshot,
        &profiles.manifest,
        &profiles.documents,
        ProfileUpdateSelection::default(),
    )
    .unwrap();
    assert!(none.selected_tasks.is_empty());
    assert!(p.task_profile_update(&none, &profiles, "Adopt").is_err());
    let preview = profile_changes(
        &session,
        &snapshot,
        &profiles.manifest,
        &profiles.documents,
        ProfileUpdateSelection {
            tasks: vec![id],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(preview.selected_tasks, [id]);
    assert!(!preview.revised_assemblies.is_empty());
    let before = session.plan.clone();
    assert!(p.task_profile_update(&preview, &profiles, "").is_err());
    let plan = p
        .task_profile_update(&preview, &profiles, "Adopt revised representation")
        .unwrap();
    assert_eq!(
        p.task_profile_update(&preview, &profiles, "Adopt revised representation")
            .unwrap()
            .id,
        plan.id
    );
    let replacement = plan
        .members
        .iter()
        .find(|m| m.candidate.subjects == ["W.Id"])
        .unwrap();
    assert_ne!(replacement.task_id, id);
    assert_eq!(
        replacement.candidate.requirements_hash,
        before
            .members
            .iter()
            .find(|m| m.task_id == id)
            .unwrap()
            .candidate
            .requirements_hash
    );
    for member in &before.members {
        if !preview.selected_tasks.contains(&member.task_id)
            && !preview.revised_assemblies.contains(&member.task_id)
        {
            assert!(plan.members.iter().any(|m| m.task_id == member.task_id));
        }
    }
    let session = p.task_session(None, None).unwrap();
    assert_eq!(session.states[&id].work, WorkState::Done);
    assert_eq!(session.states[&replacement.task_id].work, WorkState::Open);
    assert_eq!(
        session.states[&replacement.task_id].check,
        CheckStatus::Unverified
    );
    assert_eq!(p.task_plan("latest").unwrap().id, plan.id);
    t.edit("implement/value.md", "A third representation");
    let newer = p.profiles(None, None).unwrap().unwrap();
    assert!(
        p.task_profile_update(&preview, &newer, "Different request")
            .is_err()
    );
    // The requested revision is pinned in the immutable adaptation definition.
    let page = p
        .task_context(
            &session,
            &snapshot,
            replacement.task_id,
            "implementation",
            (usize::MAX, None),
            Some(&newer),
        )
        .unwrap();
    assert_eq!(
        page.implementation.unwrap().bundle_hash,
        preview
            .changes
            .iter()
            .find(|c| c.task_id == id)
            .unwrap()
            .desired
            .bundle
            .hash
    );
    complete(&p, replacement.task_id, Some(&newer));
    let session = p.task_session(None, None).unwrap();
    assert_eq!(
        session.states[&replacement.task_id]
            .execution_bundle
            .as_ref()
            .unwrap()
            .hash,
        preview
            .changes
            .iter()
            .find(|c| c.task_id == id)
            .unwrap()
            .desired
            .bundle
            .hash
    );
}
#[test]
fn slice_reservations_freeze_all_contexts_and_roll_back_on_missing_files() {
    let t = Temp::new();
    t.create();
    let p = t.project(SPEC);
    let session = p.task_session(None, None).unwrap();
    let anchor = session
        .plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Integration)
        .unwrap()
        .task_id;
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let mut missing = profiles.clone();
    missing
        .documents
        .retain(|d| d.key.as_ref().is_none_or(|k| k.semantic != "operation"));
    let seq = session.event_seq;
    assert!(
        p.task_action_with_profiles(
            Action::ClaimSlice {
                anchor,
                actor: "a".into(),
                domain: None
            },
            Some(&missing)
        )
        .is_err()
    );
    let current = p.task_session(None, None).unwrap();
    assert_eq!(current.event_seq, seq);
    assert!(current.states.values().all(|s| s.reservation.is_none()));
    let receipt = p
        .task_action_with_profiles(
            Action::ClaimSlice {
                anchor,
                actor: "a".into(),
                domain: None,
            },
            Some(&profiles),
        )
        .unwrap();
    let group = receipt.group_id.unwrap();
    let reserved = p.task_session(None, None).unwrap();
    assert!(
        reserved
            .states
            .values()
            .filter(|s| s.reservation == Some(group))
            .all(|s| s.execution_bundle.is_some())
    );
    t.edit("implement/value.md", "Changed later");
    let id = reserved
        .next(&Filters::default(), Some("a"))
        .unwrap()
        .task_id;
    let expected = reserved.states[&id].execution_bundle.clone().unwrap();
    let start = p
        .task_action_with_profiles(
            Action::Start {
                task: id,
                actor: "a".into(),
                reservation: Some(group),
            },
            None,
        )
        .unwrap();
    assert_eq!(
        start.state.unwrap().execution_bundle.unwrap().hash,
        expected.hash
    );
    p.task_action(Action::ReleaseSlice {
        group,
        actor: "a".into(),
        reason: "Stop unstarted work".into(),
    })
    .unwrap();
    let released = p.task_session(None, None).unwrap();
    assert!(
        released
            .states
            .iter()
            .filter(|(task, _)| **task != id)
            .all(|(_, s)| s.execution_bundle.is_none())
    );
}
#[test]
fn optional_profiles_and_legacy_selection_require_explicit_adoption() {
    let t = Temp::new();
    let p = t.project("workspace W { value Id = UUID }");
    assert!(p.profiles(None, None).unwrap().is_none());
    assert!(p.profiles(None, Some("rust")).is_err());
    assert!(p.profiles(Some(&t.0), None).unwrap().is_none());
    assert!(p.profiles(Some(&t.0), Some("rust")).is_err());
    t.create();
    t.edit("common.md", "Common");
    fs::write(
        t.0.join(".dctx0/profiles/rust/use/intent.md"),
        "Intent explains rationale only.",
    )
    .unwrap();
    let profiles = p.profiles(None, Some("rust")).unwrap().unwrap();
    let session = p.task_session(None, None).unwrap();
    let id = session.plan.members[0].task_id;
    complete(&p, id, None);
    let session = p.task_session(None, None).unwrap();
    let snapshot = p.load("latest").unwrap();
    let all = profile_changes(
        &session,
        &snapshot,
        &profiles.manifest,
        &profiles.documents,
        ProfileUpdateSelection {
            all_affected: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(all.selected_tasks.is_empty());
    assert!(all.changes[0].unknown_origin);
    let selected = profile_changes(
        &session,
        &snapshot,
        &profiles.manifest,
        &profiles.documents,
        ProfileUpdateSelection {
            domain: Some("W".into()),
            semantic: Some("value".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(selected.selected_tasks, [id]);
    p.task_profile_update(&selected, &profiles, "Adopt explicit architecture guidance")
        .unwrap();
    assert!(
        profile_changes(
            &session,
            &snapshot,
            &profiles.manifest,
            &profiles.documents,
            ProfileUpdateSelection {
                tasks: vec![id],
                all_affected: true,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        profile_changes(
            &session,
            &snapshot,
            &profiles.manifest,
            &profiles.documents,
            ProfileUpdateSelection {
                tasks: vec![9999],
                ..Default::default()
            }
        )
        .is_err()
    );
    let current = p.task_session(None, None).unwrap();
    assert!(
        profile_changes(
            &current,
            &snapshot,
            &profiles.manifest,
            &profiles.documents,
            ProfileUpdateSelection {
                tasks: vec![current.plan.members[0].task_id],
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn profiles_reject_path_and_metadata_conflicts() {
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { type A = Int }");
    assert!(p.profiles(None, Some("../rust")).is_err());
    assert!(p.profiles(None, Some("java")).is_err());
    fs::write(
        t.0.join(".dctx0/profiles/config.json"),
        r#"{"format_version":99,"default_target":"rust"}"#,
    )
    .unwrap();
    assert!(p.profiles(None, None).is_err());
    assert!(create_profiles(&t.0, &dctx0_target_rust::Provider, false, false, true).is_err());
    fs::write(
        t.0.join(".dctx0/profiles/config.json"),
        r#"{"format_version":1,"default_target":"rust"}"#,
    )
    .unwrap();
    let mut manifest = dctx0_target_rust::Provider.manifest();
    manifest.template_version = 2;
    fs::write(
        t.0.join(".dctx0/profiles/rust/manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(create_profiles(&t.0, &dctx0_target_rust::Provider, false, false, false).is_err());
    manifest.target = "go".into();
    fs::write(
        t.0.join(".dctx0/profiles/rust/manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(p.profiles(None, None).is_err());
    let t = Temp::new();
    fs::create_dir_all(t.0.join(".dctx0/profiles/rust/common.md")).unwrap();
    assert!(create_profiles(&t.0, &dctx0_target_rust::Provider, false, false, true).is_err());
    #[cfg(unix)]
    {
        let t = Temp::new();
        std::os::unix::fs::symlink(t.0.join("dctx0"), t.0.join(".dctx0")).unwrap();
        assert!(create_profiles(&t.0, &dctx0_target_rust::Provider, false, false, true).is_err());
    }
}

struct ModifiedProvider {
    target: String,
    paths: Vec<String>,
}
impl TargetProfileProvider for ModifiedProvider {
    fn manifest(&self) -> dctx0_core::TargetManifest {
        let mut m = dctx0_target_rust::Provider.manifest();
        m.target = self.target.clone();
        m
    }
    fn templates(&self) -> Vec<dctx0_core::ProfileTemplate> {
        self.paths
            .iter()
            .map(|p| dctx0_core::ProfileTemplate {
                path: p.clone(),
                text: "guidance".into(),
            })
            .collect()
    }
}
#[test]
fn creation_checks_all_paths_and_preserves_exact_agents_suffixes() {
    let t = Temp::new();
    let invalid = ModifiedProvider {
        target: "../rust".into(),
        paths: vec![],
    };
    assert!(create_profiles(&t.0, &invalid, false, false, true).is_err());
    for paths in [
        vec!["../escaped.md".into()],
        vec!["common.md".into(), "common.md".into()],
        vec!["/outside.md".into()],
    ] {
        assert!(
            create_profiles(
                &t.0,
                &ModifiedProvider {
                    target: "rust".into(),
                    paths
                },
                false,
                false,
                true
            )
            .is_err()
        );
    }
    assert!(!t.0.join(".dctx0").exists());
    fs::write(t.0.join(".dctx0"), "conflict").unwrap();
    assert!(create_profiles(&t.0, &dctx0_target_rust::Provider, false, false, true).is_err());
    fs::remove_file(t.0.join(".dctx0")).unwrap();
    fs::write(t.0.join("AGENTS.md"), "Original trailing whitespace  \n\n").unwrap();
    t.create();
    assert!(
        fs::read_to_string(t.0.join("AGENTS.md"))
            .unwrap()
            .starts_with("Original trailing whitespace  \n\n")
    );
    let p = t.project("workspace W { type A = Int }");
    fs::write(t.0.join(".dctx0/profiles/rust/use/unknown.md"), "invalid").unwrap();
    assert!(p.profiles(None, None).is_err());
    assert!(
        create_profiles(
            &t.0.join("AGENTS.md"),
            &dctx0_target_rust::Provider,
            false,
            false,
            true
        )
        .is_err()
    );
}
#[test]
fn concurrent_profile_claims_are_distinct_and_missing_catalog_blocks_only_new_work() {
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { value A = UUID; value B = UUID }");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers = (0..2)
        .map(|i| {
            let p = p.clone();
            let profiles = profiles.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                p.task_action_with_profiles(
                    Action::ClaimNext {
                        actor: format!("worker{i}"),
                        filters: Filters::default(),
                    },
                    Some(&profiles),
                )
                .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let receipts = workers
        .into_iter()
        .map(|w| w.join().unwrap())
        .collect::<Vec<_>>();
    assert_ne!(receipts[0].task_id, receipts[1].task_id);
    assert!(
        receipts
            .iter()
            .all(|r| r.state.as_ref().unwrap().execution_bundle.is_some())
    );
    // A reservation can start with its saved context after deleting the catalog.
    let t = Temp::new();
    t.create();
    let p = t.project(SPEC);
    let session = p.task_session(None, None).unwrap();
    let anchor = session
        .plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Integration)
        .unwrap()
        .task_id;
    let r = p
        .task_action_selected(
            Action::ClaimSlice {
                anchor,
                actor: "a".into(),
                domain: None,
            },
            None,
            None,
        )
        .unwrap();
    let group = r.group_id.unwrap();
    fs::remove_dir_all(t.0.join(".dctx0/profiles/rust")).unwrap();
    let session = p.task_session(None, None).unwrap();
    let id = session
        .next(&Filters::default(), Some("a"))
        .unwrap()
        .task_id;
    assert!(
        p.task_action_selected(
            Action::Start {
                task: id,
                actor: "a".into(),
                reservation: Some(group)
            },
            None,
            None
        )
        .unwrap()
        .state
        .unwrap()
        .execution_bundle
        .is_some()
    );
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { value Id = UUID }");
    fs::remove_dir_all(t.0.join(".dctx0/profiles/rust")).unwrap();
    assert!(
        p.task_action_selected(
            Action::ClaimNext {
                actor: "a".into(),
                filters: Filters::default()
            },
            None,
            None
        )
        .is_err()
    );
}

#[test]
fn complete_examples_select_all_required_profiles_and_real_call_boundaries() {
    let provider = dctx0_target_rust::Provider;
    let manifest = provider.manifest();
    let documents = provider
        .templates()
        .into_iter()
        .map(|template| {
            let key = template
                .path
                .strip_suffix(".md")
                .and_then(|path| path.split_once('/'))
                .map(|(role, semantic)| dctx0_core::ProfileKey {
                    role: if role == "implement" {
                        ProfileRole::Implement
                    } else {
                        ProfileRole::Use
                    },
                    semantic: semantic.into(),
                });
            dctx0_core::ProfileDocument {
                target: "rust".into(),
                key,
                path: format!(".dctx0/profiles/rust/{}", template.path),
                content_hash: dctx0_changes::hash(&template.text),
                text: template.text,
                subjects: vec![],
            }
        })
        .collect::<Vec<_>>();
    let mut semantics = std::collections::BTreeSet::new();
    for example in ["cinema", "runatron-next"] {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../examples/{example}/dctx0"));
        let p = Project::new(&root, None).unwrap();
        let (sources, compilation) = p.compile().unwrap();
        let ir = compilation.ir.unwrap();
        let graph = dctx0_graph::build(&ir);
        let snapshot = dctx0_changes::snapshot(&sources, ir, graph);
        let plan = plan(&snapshot).unwrap();
        for candidate in plan.tasks {
            let selected = select_profiles(&snapshot, &candidate, &manifest, &documents);
            assert!(selected.missing_files.is_empty());
            for doc in selected.bundle.documents {
                if let Some(key) = doc.key {
                    semantics.insert(key.semantic);
                }
            }
        }
    }
    for semantic in [
        "value",
        "entity",
        "message",
        "event",
        "example",
        "invariant",
        "lifecycle",
        "operation",
        "port",
        "surface",
    ] {
        assert!(semantics.contains(semantic), "{semantic}");
    }
}
#[test]
fn missing_current_files_disable_drift_and_unrelated_edits_do_not_stale_selected_updates() {
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { value Id = UUID }");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let id = p.task_session(None, None).unwrap().plan.members[0].task_id;
    complete(&p, id, Some(&profiles));
    let session = p.task_session(None, None).unwrap();
    let snapshot = p.load("latest").unwrap();
    fs::remove_file(t.0.join(".dctx0/profiles/rust/implement/value.md")).unwrap();
    let missing = p.profiles(None, None).unwrap().unwrap();
    let preview = profile_changes(
        &session,
        &snapshot,
        &missing.manifest,
        &missing.documents,
        ProfileUpdateSelection {
            all_affected: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(preview.selected_tasks.is_empty());
    assert_eq!(
        preview.changes[0].comparison,
        ProfileComparison::Unavailable
    );
    t.edit("implement/value.md", "Revised");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let preview = profile_changes(
        &session,
        &snapshot,
        &profiles.manifest,
        &profiles.documents,
        ProfileUpdateSelection {
            tasks: vec![id],
            ..Default::default()
        },
    )
    .unwrap();
    t.edit("implement/review.md", "Unrelated");
    assert!(p.task_profile_update(&preview, &profiles, "Adopt").is_ok());
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { value Id = UUID }");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let id = p.task_session(None, None).unwrap().plan.members[0].task_id;
    complete(&p, id, Some(&profiles));
    t.edit("implement/value.md", "Revised");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let session = p.task_session(None, None).unwrap();
    let snapshot = p.load("latest").unwrap();
    let preview = profile_changes(
        &session,
        &snapshot,
        &profiles.manifest,
        &profiles.documents,
        ProfileUpdateSelection {
            tasks: vec![id],
            ..Default::default()
        },
    )
    .unwrap();
    p.task_action(Action::Note {
        task: id,
        note: "Event sequence changed".into(),
    })
    .unwrap();
    assert!(p.task_profile_update(&preview, &profiles, "Adopt").is_err());
    fs::remove_file(t.0.join(".dctx0/profiles/config.json")).unwrap();
    assert!(p.task_profile_update(&preview, &profiles, "Adopt").is_err());
}

#[test]
fn catalog_validation_rejects_corruption_symlinks_and_missing_provider_templates() {
    let t = Temp::new();
    assert!(
        create_profiles(
            &t.0,
            &ModifiedProvider {
                target: "rust".into(),
                paths: vec!["common.md".into()]
            },
            false,
            false,
            true
        )
        .is_err()
    );
    t.create();
    let p = t.project("workspace W { value Id = UUID }");
    let path = t.0.join(".dctx0/profiles/rust/manifest.json");
    for mutate in [0, 1] {
        let mut m = dctx0_target_rust::Provider.manifest();
        if mutate == 0 {
            m.standard_keys.clear();
        } else {
            m.standard_keys.push(dctx0_core::ProfileKey {
                role: ProfileRole::Use,
                semantic: "unknown".into(),
            });
        }
        fs::write(&path, serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(p.profiles(None, None).is_err());
    }
    fs::write(&path, "broken JSON").unwrap();
    assert!(p.profiles(None, None).is_err());
    fs::write(
        &path,
        serde_json::to_vec(&dctx0_target_rust::Provider.manifest()).unwrap(),
    )
    .unwrap();
    fs::remove_file(t.0.join(".dctx0/profiles/rust/common.md")).unwrap();
    fs::remove_dir_all(t.0.join(".dctx0/profiles/rust/use")).unwrap();
    fs::write(
        t.0.join(".dctx0/profiles/rust/implement/notes.txt"),
        "Not a role profile",
    )
    .unwrap();
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let session = p.task_session(None, None).unwrap();
    let snapshot = p.load("latest").unwrap();
    let selected = select_profiles(
        &snapshot,
        &session.plan.members[0].candidate,
        &profiles.manifest,
        &profiles.documents,
    );
    assert!(
        selected
            .missing_files
            .contains(&".dctx0/profiles/rust/common.md".into())
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            t.0.join("dctx0/spec.dctx0"),
            t.0.join(".dctx0/profiles/rust/common.md"),
        )
        .unwrap();
        assert!(p.profiles(None, None).is_err());
    }
}
#[test]
fn profile_selection_handles_operation_boundaries_retirement_and_short_domain_filters() {
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { domain D { value Id = UUID } module M { port P { operation f(id: D.Id) -> Int } surface S { operation f(id: D.Id) -> Int; effects { call P.f } } } }");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let session = p.task_session(None, None).unwrap();
    let snapshot = p.load("latest").unwrap();
    let id = session
        .plan
        .members
        .iter()
        .find(|m| m.candidate.subjects == ["W.D.Id"])
        .unwrap()
        .task_id;
    complete(&p, id, Some(&profiles));
    let session = p.task_session(None, None).unwrap();
    let unchanged = profile_changes(
        &session,
        &snapshot,
        &profiles.manifest,
        &profiles.documents,
        ProfileUpdateSelection {
            domain: Some("D".into()),
            semantic: Some("value".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        unchanged
            .changes
            .iter()
            .find(|c| c.task_id == id)
            .unwrap()
            .comparison,
        ProfileComparison::Unchanged
    );
    assert!(unchanged.selected_tasks.is_empty());
    let free = session
        .plan
        .members
        .iter()
        .find(|m| m.candidate.kind == TaskKind::Operation && m.candidate.subjects == ["W.M.S.f"])
        .unwrap()
        .candidate
        .clone();
    let selected = select_profiles(&snapshot, &free, &profiles.manifest, &profiles.documents);
    assert!(
        selected
            .bundle
            .documents
            .iter()
            .any(|d| d.key.as_ref().is_some_and(|k| k.semantic == "operation"))
    );
    for semantic in ["port", "surface"] {
        assert!(selected.bundle.documents.iter().any(|d| {
            d.key
                .as_ref()
                .is_some_and(|k| k.semantic == semantic && k.role == ProfileRole::Use)
        }));
    }
    let old = session.member(id).unwrap().candidate.clone();
    fs::write(
        t.0.join("dctx0/spec.dctx0"),
        "workspace W { intent \"Retired\" }",
    )
    .unwrap();
    p.check().unwrap();
    let newer = p.load("latest").unwrap();
    let mut retired = old;
    retired.kind = TaskKind::Retire;
    retired.subjects.push("W.D.Missing".into());
    retired.facts.push("W.D.RemovedFact".into());
    let selected = select_profiles(&newer, &retired, &profiles.manifest, &profiles.documents);
    assert_eq!(selected.bundle.documents.len(), 1);
}

#[test]
fn repeated_checks_stale_previews_and_fresh_previews_pin_the_observed_occurrence() {
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { value Id = UUID }");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let id = p.task_session(None, None).unwrap().plan.members[0].task_id;
    complete(&p, id, Some(&profiles));
    t.edit("implement/value.md", "Revised");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let selection = ProfileUpdateSelection {
        tasks: vec![id],
        ..Default::default()
    };
    assert!(
        p.task_profile_changes(
            &profiles,
            ProfileUpdateSelection {
                tasks: vec![9999],
                ..Default::default()
            }
        )
        .is_err()
    );
    let preview = p
        .task_profile_changes(&profiles, selection.clone())
        .unwrap();
    p.check().unwrap();
    assert!(p.task_profile_update(&preview, &profiles, "Adopt").is_err());
    let fresh = p.task_profile_changes(&profiles, selection).unwrap();
    assert_ne!(fresh.check_run_id, preview.check_run_id);
    let plan = p.task_profile_update(&fresh, &profiles, "Adopt").unwrap();
    assert_eq!(plan.check_run_id, fresh.check_run_id);
    let mut invalid = fresh.clone();
    invalid.hash = "tampered".into();
    assert!(
        p.task_profile_update(&invalid, &profiles, "Different request")
            .is_err()
    );
    fs::write(
        t.0.join("dctx0/spec.dctx0"),
        "workspace W { value Id = String }",
    )
    .unwrap();
    p.check().unwrap();
    assert!(
        p.task_profile_changes(&profiles, ProfileUpdateSelection::default())
            .is_err()
    );
    assert!(
        p.task_profile_update(&fresh, &profiles, "After spec change")
            .is_err()
    );
}
#[test]
fn context_provenance_distinguishes_current_requested_stored_and_unknown_legacy_guidance() {
    let t = Temp::new();
    t.create();
    let p = t.project("workspace W { value Id = UUID }");
    let profiles = p.profiles(None, None).unwrap().unwrap();
    let session = p.task_session(None, None).unwrap();
    let id = session.plan.members[0].task_id;
    let snapshot = p.load("latest").unwrap();
    let current = p
        .task_context(
            &session,
            &snapshot,
            id,
            "implementation",
            (usize::MAX, None),
            Some(&profiles),
        )
        .unwrap();
    assert_eq!(
        current.implementation.unwrap().origin,
        ImplementationOrigin::CurrentLocal
    );
    complete(&p, id, None);
    let session = p.task_session(None, None).unwrap();
    let legacy = p
        .task_context(
            &session,
            &snapshot,
            id,
            "implementation",
            (usize::MAX, None),
            Some(&profiles),
        )
        .unwrap();
    assert_eq!(
        legacy.implementation.as_ref().unwrap().origin,
        ImplementationOrigin::UnknownLegacyExecution
    );
    assert!(context_markdown(&legacy).contains("historical execution profile context is unknown"));
    let preview = p
        .task_profile_changes(
            &profiles,
            ProfileUpdateSelection {
                tasks: vec![id],
                ..Default::default()
            },
        )
        .unwrap();
    let plan = p.task_profile_update(&preview, &profiles, "Adopt").unwrap();
    let new = plan.members[0].task_id;
    let session = p.task_session(None, None).unwrap();
    let requested = p
        .task_context(
            &session,
            &snapshot,
            new,
            "implementation",
            (usize::MAX, None),
            None,
        )
        .unwrap();
    assert_eq!(
        requested.implementation.as_ref().unwrap().origin,
        ImplementationOrigin::RequestedRevision
    );
    assert!(context_markdown(&requested).contains("Immutable requested profile revision"));
    complete(&p, new, None);
    let session = p.task_session(None, None).unwrap();
    let stored = context(&session, &snapshot, new, "implementation", usize::MAX, None).unwrap();
    assert_eq!(
        stored.implementation.as_ref().unwrap().origin,
        ImplementationOrigin::StoredExecution
    );
    assert!(context_markdown(&stored).contains("Stored execution profiles"));
}
