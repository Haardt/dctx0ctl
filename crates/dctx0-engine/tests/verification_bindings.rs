use dctx0_core::*;
use dctx0_engine::*;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const DESCRIPTION: &str = include_str!("../../../fixtures/conformance/bindings/description.json");
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dctx0-bindings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("spec")).unwrap();
        fs::create_dir_all(root.join("app/.dctx0/testing")).unwrap();
        fs::write(
            root.join("spec/w.dctx0"),
            "workspace W { value Id = Int { range 1..3 } }",
        )
        .unwrap();
        Self(root)
    }
    fn project(&self) -> Project {
        Project::new(&self.0.join("spec"), None).unwrap()
    }
    fn description(&self, text: &str) -> dctx0_engine::Result<BindingDescription> {
        fs::write(self.0.join("app/.dctx0/testing/bindings.json"), text).unwrap();
        read_verification_bindings(&self.0.join("app"), ".dctx0/testing/bindings.json")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn binding_plan_works_without_application_and_never_publishes_or_migrates_history() {
    let fixture = Fixture::new();
    let project = fixture.project();
    let select = || VerificationSelection {
        subjects: ["W.Id".into()].into(),
        ..Default::default()
    };
    assert!(
        project
            .verification_binding_requirements(select(), &Default::default(), Default::default())
            .is_err()
    );
    assert!(!project.database.exists());
    project.check().unwrap();
    let original = fs::read(&project.database).unwrap();
    let report = project
        .verification_binding_requirements(select(), &Default::default(), Default::default())
        .unwrap();
    assert_eq!(report.requirements.len(), 1);
    assert_eq!(report.requirements[0].subject, "W.Id");
    assert_eq!(report.requirements[0].role, BindingRole::Validator);
    assert_eq!(
        report.requirements[0].span.as_ref().unwrap().file,
        "w.dctx0"
    );
    assert_eq!(report.declarations.len(), 1);
    assert!(report.issues.is_empty());
    assert_eq!(fs::read(&project.database).unwrap(), original);
    fs::write(
        fixture.0.join("spec/w.dctx0"),
        "workspace W { value Id = String }",
    )
    .unwrap();
    assert!(
        project
            .verification_binding_requirements(select(), &Default::default(), Default::default())
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    assert_eq!(fs::read(&project.database).unwrap(), original);
}

#[test]
fn descriptions_reject_unknown_duplicate_and_escaping_inputs_before_native_access() {
    let fixture = Fixture::new();
    let valid = fixture.description(DESCRIPTION).unwrap();
    assert_eq!(valid.target, "test-native");
    for (pointer, invalid) in [
        ("/schema_version", json!(2)),
        ("/target", json!("")),
        ("/target", json!("Go")),
        ("/session/native_type", json!(" ")),
        ("/session/factory/source", json!("../outside")),
        ("/session/cleanup/source", json!("C:\\outside")),
        ("/session/factory/symbol", json!("create()")),
        ("/session/factory/source_root", json!("bad/root")),
        ("/operations/0/invoke/entry/source", json!("/outside")),
        ("/operations/0/arguments/0/parameter", json!("")),
        ("/operations/0/arguments/0/fields/0", json!("id()")),
        ("/operations/0/result_hook/symbol", json!("$()")),
        ("/operations/0/failures/0/mapper/source", json!("a/./b")),
        ("/validators/0/entry/source", json!("a//b")),
        ("/entities/0/entry/source", json!("a\nb")),
        ("/effects/events/source", json!("a/../b")),
        ("/types/0/native_type", json!("")),
        ("/types/0/decode/source", json!("")),
        ("/types/0/encode/symbol", json!("1bad")),
    ] {
        let mut value: Value = serde_json::from_str(DESCRIPTION).unwrap();
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            fixture.description(&value.to_string()).is_err(),
            "{pointer}: {value}"
        );
    }
    for pointer in [
        "/operations",
        "/validators",
        "/entities",
        "/types",
        "/operations/0/failures",
    ] {
        let mut value: Value = serde_json::from_str(DESCRIPTION).unwrap();
        let entries = value.pointer_mut(pointer).unwrap().as_array_mut().unwrap();
        entries.push(entries[0].clone());
        assert!(
            fixture
                .description(&value.to_string())
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
    }
    for malformed in [
        DESCRIPTION.replacen(
            "\"schema_version\": 1",
            "\"schema_version\": 1, \"schema_version\": 1",
            1,
        ),
        DESCRIPTION.replacen(
            "\"symbol\": \"create\"",
            "\"symbol\": \"create\", \"symbol\": \"other\"",
            1,
        ),
        DESCRIPTION.replacen(
            "\"schema_version\": 1",
            "\"schema_version\": 1, \"unknown\": true",
            1,
        ),
        "{invalid}".into(),
        " ".repeat(524_289),
    ] {
        assert!(fixture.description(&malformed).is_err());
    }
    assert!(read_verification_bindings(&fixture.0.join("app"), "../bindings.json").is_err());
    assert!(read_verification_bindings(&fixture.0.join("app"), "missing.json").is_err());
    assert!(!fixture.project().database.exists());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            fixture.0.join("app/.dctx0/testing/bindings.json"),
            fixture.0.join("app/linked.json"),
        )
        .unwrap();
        assert!(read_verification_bindings(&fixture.0.join("app"), "linked.json").is_err());
    }
}

#[test]
fn descriptions_keep_methods_hooks_and_missing_roles_explicit() {
    let fixture = Fixture::new();
    let mut binding = fixture.description(DESCRIPTION).unwrap();
    let project = fixture.project();
    project.check().unwrap();
    let mut requirements = project
        .verification_binding_requirements(
            VerificationSelection {
                subjects: ["W.Id".into()].into(),
                ..Default::default()
            },
            &Default::default(),
            Default::default(),
        )
        .unwrap();
    assert!(binding.missing(&requirements).is_empty());
    for role in [
        BindingRole::Operation,
        BindingRole::Validator,
        BindingRole::EntityObserver,
    ] {
        requirements.requirements[0].role = role;
        requirements.requirements[0].subject = "W.Missing".into();
        let missing = binding.missing(&requirements);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].span, requirements.requirements[0].span);
        assert!(missing[0].message.contains("W.Missing"));
    }
    for (role, subject) in [
        (BindingRole::Operation, "W.Api.apply"),
        (BindingRole::EntityObserver, "W.Item"),
    ] {
        requirements.requirements[0].role = role;
        requirements.requirements[0].subject = subject.into();
        assert!(binding.missing(&requirements).is_empty());
    }
    binding.operations[0].invoke = NativeInvocation::Method {
        receiver: vec!["service".into()],
        method: "apply".into(),
    };
    assert!(binding.validate().is_ok());
    for (receiver, method) in [(vec!["service()".into()], "apply"), (vec![], "apply()")] {
        binding.operations[0].invoke = NativeInvocation::Method {
            receiver,
            method: method.into(),
        };
        assert!(binding.validate().is_err());
    }
    binding.operations[0].invoke = NativeInvocation::Hook {
        entry: binding.session.factory.clone(),
    };
    assert!(binding.validate().is_ok());
    binding.operations[0].subject.clear();
    assert!(binding.validate().is_err());
}
