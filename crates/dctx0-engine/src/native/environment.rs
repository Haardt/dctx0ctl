//! Explicit child environments. Selection never silently drops inherited inputs.
use super::*;
use std::{collections::BTreeSet, ffi::OsString};

pub(crate) fn environment_value<'a>(
    entries: &'a BTreeMap<OsString, OsString>,
    name: &str,
) -> Option<&'a OsString> {
    #[cfg(windows)]
    return entries
        .iter()
        .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case(name))
        .map(|(_, value)| value);
    #[cfg(not(windows))]
    entries.get(std::ffi::OsStr::new(name))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NativeEnvironment {
    #[default]
    Inherited,
    Controlled {
        #[serde(default)]
        values: BTreeMap<String, String>,
        #[serde(default)]
        inherit: BTreeSet<String>,
    },
}

impl NativeEnvironment {
    pub(crate) fn entries(&self) -> Result<BTreeMap<OsString, OsString>, NativeFailure> {
        let Self::Controlled { values, inherit } = self else {
            return Ok(std::env::vars_os().collect());
        };
        if values.len() + inherit.len() > 4096
            || values.keys().chain(inherit).any(|name| {
                name.is_empty() || name.contains(['=', '\0']) || name == "DCTX0_NATIVE_OWNERS"
            })
            || values.values().any(|value| value.contains('\0'))
        {
            return Err(NativeFailure::new(
                NativeFailureKind::Environment,
                "Invalid controlled environment names, values or size",
            ));
        }
        let mut entries = inherit
            .iter()
            .filter_map(|name| std::env::var_os(name).map(|value| (name.into(), value)))
            .collect::<BTreeMap<OsString, OsString>>();
        for (name, value) in values {
            #[cfg(windows)]
            entries.retain(|key, _| !key.to_string_lossy().eq_ignore_ascii_case(name));
            entries.insert(name.into(), value.into());
        }
        Ok(entries)
    }

    pub(crate) fn forwarded_names(&self) -> Option<BTreeSet<String>> {
        match self {
            Self::Inherited => None,
            Self::Controlled { values, inherit } => Some(
                inherit
                    .iter()
                    .filter(|name| {
                        #[cfg(windows)]
                        return !values.keys().any(|fixed| fixed.eq_ignore_ascii_case(name));
                        #[cfg(not(windows))]
                        !values.contains_key(*name)
                    })
                    .cloned()
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_selection_is_explicit_and_fixed_values_override_forwarding() {
        let mixed_case = BTreeMap::from([(OsString::from("Path"), OsString::from("search-root"))]);
        assert!(environment_value(&mixed_case, "Path").is_some());
        assert_eq!(
            environment_value(&mixed_case, "PATH").is_some(),
            cfg!(windows)
        );
        let inherited = NativeEnvironment::Inherited.entries().unwrap();
        assert_eq!(inherited, std::env::vars_os().collect());
        let policy = NativeEnvironment::Controlled {
            values: [("PATH".into(), "fixed-search-path".into())].into(),
            inherit: ["PATH".into(), "DCTX0_ABSENT_ENVIRONMENT_9914".into()].into(),
        };
        assert_eq!(
            policy.entries().unwrap(),
            [("PATH".into(), "fixed-search-path".into())].into()
        );
        assert_eq!(
            policy.forwarded_names().unwrap(),
            ["DCTX0_ABSENT_ENVIRONMENT_9914".into()].into()
        );
        let encoded = serde_json::to_vec(&policy).unwrap();
        assert_eq!(
            serde_json::from_slice::<NativeEnvironment>(&encoded).unwrap(),
            policy
        );
        assert!(
            serde_json::from_str::<NativeEnvironment>(r#"{"mode":"controlled","extra":true}"#)
                .is_err()
        );
        for name in ["", "A=B", "A\0B", "DCTX0_NATIVE_OWNERS"] {
            assert!(
                NativeEnvironment::Controlled {
                    values: [(name.into(), "v".into())].into(),
                    inherit: Default::default()
                }
                .entries()
                .is_err()
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launched_child_receives_only_controlled_values_and_runtime_receipt_hides_secrets() {
        let policy = NativeEnvironment::Controlled {
            values: [("BOUND_INPUT".into(), "private-input-value".into())].into(),
            inherit: Default::default(),
        };
        let recipe = ExecutionRecipe {
            program: "/bin/sh".into(),
            arguments: vec![
                "-c".into(),
                "test -z \"${CODEX_TOOL_CALL_ID+x}\" && test -n \"$BOUND_INPUT\"".into(),
            ],
        };
        let mut command = NativeCommand::spawn(
            &recipe,
            &std::env::temp_dir(),
            NativeLimits {
                environment: policy,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let identity = command.runtime_identity();
        assert_eq!(identity.schema_version, 3);
        assert!(identity.still_current().unwrap());
        let serialized = serde_json::to_string(&identity).unwrap();
        assert!(!serialized.contains("private-input-value"));
        assert_eq!(
            command
                .wait(&NativeCancellation::default())
                .await
                .unwrap()
                .exit_code,
            Some(0)
        );
    }
}
