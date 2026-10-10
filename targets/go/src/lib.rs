//! Embedded editable defaults for go.
use dctx0_core::{ProfileTemplate, TargetManifest, TargetProfileProvider};
pub struct Provider;
impl TargetProfileProvider for Provider {
    fn manifest(&self) -> TargetManifest {
        let standard_keys = dctx0_core::standard_profile_keys();
        TargetManifest {
            format_version: 1,
            target: "go".into(),
            template_version: 1,
            standard_keys,
        }
    }
    fn templates(&self) -> Vec<ProfileTemplate> {
        vec![
            ProfileTemplate {
                path: "common.md".into(),
                text: include_str!("../profiles/common.md").into(),
            },
            ProfileTemplate {
                path: "implement/scalar.md".into(),
                text: include_str!("../profiles/implement/scalar.md").into(),
            },
            ProfileTemplate {
                path: "implement/value.md".into(),
                text: include_str!("../profiles/implement/value.md").into(),
            },
            ProfileTemplate {
                path: "implement/type.md".into(),
                text: include_str!("../profiles/implement/type.md").into(),
            },
            ProfileTemplate {
                path: "implement/entity.md".into(),
                text: include_str!("../profiles/implement/entity.md").into(),
            },
            ProfileTemplate {
                path: "implement/message.md".into(),
                text: include_str!("../profiles/implement/message.md").into(),
            },
            ProfileTemplate {
                path: "implement/event.md".into(),
                text: include_str!("../profiles/implement/event.md").into(),
            },
            ProfileTemplate {
                path: "implement/failure.md".into(),
                text: include_str!("../profiles/implement/failure.md").into(),
            },
            ProfileTemplate {
                path: "implement/operation.md".into(),
                text: include_str!("../profiles/implement/operation.md").into(),
            },
            ProfileTemplate {
                path: "implement/port.md".into(),
                text: include_str!("../profiles/implement/port.md").into(),
            },
            ProfileTemplate {
                path: "implement/surface.md".into(),
                text: include_str!("../profiles/implement/surface.md").into(),
            },
            ProfileTemplate {
                path: "implement/module.md".into(),
                text: include_str!("../profiles/implement/module.md").into(),
            },
            ProfileTemplate {
                path: "implement/external.md".into(),
                text: include_str!("../profiles/implement/external.md").into(),
            },
            ProfileTemplate {
                path: "implement/example.md".into(),
                text: include_str!("../profiles/implement/example.md").into(),
            },
            ProfileTemplate {
                path: "implement/invariant.md".into(),
                text: include_str!("../profiles/implement/invariant.md").into(),
            },
            ProfileTemplate {
                path: "implement/lifecycle.md".into(),
                text: include_str!("../profiles/implement/lifecycle.md").into(),
            },
            ProfileTemplate {
                path: "implement/proof.md".into(),
                text: include_str!("../profiles/implement/proof.md").into(),
            },
            ProfileTemplate {
                path: "implement/test.md".into(),
                text: include_str!("../profiles/implement/test.md").into(),
            },
            ProfileTemplate {
                path: "implement/review.md".into(),
                text: include_str!("../profiles/implement/review.md").into(),
            },
            ProfileTemplate {
                path: "use/scalar.md".into(),
                text: include_str!("../profiles/use/scalar.md").into(),
            },
            ProfileTemplate {
                path: "use/value.md".into(),
                text: include_str!("../profiles/use/value.md").into(),
            },
            ProfileTemplate {
                path: "use/type.md".into(),
                text: include_str!("../profiles/use/type.md").into(),
            },
            ProfileTemplate {
                path: "use/entity.md".into(),
                text: include_str!("../profiles/use/entity.md").into(),
            },
            ProfileTemplate {
                path: "use/message.md".into(),
                text: include_str!("../profiles/use/message.md").into(),
            },
            ProfileTemplate {
                path: "use/event.md".into(),
                text: include_str!("../profiles/use/event.md").into(),
            },
            ProfileTemplate {
                path: "use/failure.md".into(),
                text: include_str!("../profiles/use/failure.md").into(),
            },
            ProfileTemplate {
                path: "use/operation.md".into(),
                text: include_str!("../profiles/use/operation.md").into(),
            },
            ProfileTemplate {
                path: "use/port.md".into(),
                text: include_str!("../profiles/use/port.md").into(),
            },
            ProfileTemplate {
                path: "use/surface.md".into(),
                text: include_str!("../profiles/use/surface.md").into(),
            },
            ProfileTemplate {
                path: "use/module.md".into(),
                text: include_str!("../profiles/use/module.md").into(),
            },
            ProfileTemplate {
                path: "use/external.md".into(),
                text: include_str!("../profiles/use/external.md").into(),
            },
            ProfileTemplate {
                path: "use/example.md".into(),
                text: include_str!("../profiles/use/example.md").into(),
            },
            ProfileTemplate {
                path: "use/invariant.md".into(),
                text: include_str!("../profiles/use/invariant.md").into(),
            },
            ProfileTemplate {
                path: "use/lifecycle.md".into(),
                text: include_str!("../profiles/use/lifecycle.md").into(),
            },
            ProfileTemplate {
                path: "use/proof.md".into(),
                text: include_str!("../profiles/use/proof.md").into(),
            },
            ProfileTemplate {
                path: "use/test.md".into(),
                text: include_str!("../profiles/use/test.md").into(),
            },
            ProfileTemplate {
                path: "use/review.md".into(),
                text: include_str!("../profiles/use/review.md").into(),
            },
        ]
    }
}
