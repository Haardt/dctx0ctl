//! Reviewed native connections and required roles. Neither is execution evidence.
use crate::{DataIr, OperationIr, Span, VerificationIssue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const BINDINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingRole {
    Operation,
    Validator,
    EntityObserver,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingRequirement {
    pub subject: String,
    pub role: BindingRole,
    pub span: Option<Span>,
    /// Present for invocation roles, including helper and preparation calls.
    pub operation: Option<OperationIr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingRequirements {
    pub schema_version: u32,
    pub snapshot: String,
    pub check_occurrence: u64,
    pub selected_subjects: BTreeSet<String>,
    pub requirements: Vec<BindingRequirement>,
    pub declarations: BTreeMap<String, DataIr>,
    pub capabilities: BTreeSet<String>,
    pub issues: Vec<VerificationIssue>,
}

/// An exported native entry point. Source paths are relative to a registered root;
/// symbols are names, never executable expressions or JSON-embedded code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEntryPoint {
    pub source_root: Option<String>,
    pub source: String,
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionBinding {
    pub factory: NativeEntryPoint,
    pub cleanup: NativeEntryPoint,
    pub native_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NativeInvocation {
    Function {
        entry: NativeEntryPoint,
    },
    Method {
        receiver: Vec<String>,
        method: String,
    },
    Hook {
        entry: NativeEntryPoint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArgumentBinding {
    pub parameter: String,
    /// Optional projection through named fields; order in this list is native order.
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureBinding {
    pub subject: String,
    pub mapper: NativeEntryPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationBinding {
    pub subject: String,
    pub invoke: NativeInvocation,
    pub arguments: Vec<ArgumentBinding>,
    pub result_hook: Option<NativeEntryPoint>,
    pub failures: Vec<FailureBinding>,
}

/// A validator must receive invalid input before strict native conversion. A hook
/// translates raw values to the real production parser; it must not replace it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValidatorInput {
    Wire,
    Json,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorBinding {
    pub subject: String,
    pub entry: NativeEntryPoint,
    pub input: ValidatorInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityObserverBinding {
    pub subject: String,
    pub entry: NativeEntryPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectObserverBindings {
    pub events: Option<NativeEntryPoint>,
    pub calls: Option<NativeEntryPoint>,
    pub resources: Option<NativeEntryPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTypeBinding {
    pub subject: String,
    pub native_type: String,
    pub decode: NativeEntryPoint,
    pub encode: NativeEntryPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingDescription {
    pub schema_version: u32,
    pub target: String,
    pub session: SessionBinding,
    pub operations: Vec<OperationBinding>,
    pub validators: Vec<ValidatorBinding>,
    pub entities: Vec<EntityObserverBinding>,
    pub effects: EffectObserverBindings,
    pub types: Vec<NativeTypeBinding>,
}

/// Portable relative file paths, independent of filesystem existence. Effects
/// adapters must additionally reject symlinks while opening these paths.
pub fn is_binding_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

impl NativeEntryPoint {
    fn validate(&self) -> Result<(), String> {
        if !is_binding_path(&self.source)
            || self
                .source_root
                .as_deref()
                .is_some_and(|id| !identifier(id))
            || !identifier(&self.symbol)
        {
            return Err(format!(
                "Invalid native entry point: {} ({})",
                self.symbol, self.source
            ));
        }
        Ok(())
    }
}

impl BindingDescription {
    /// Checks description structure only. Native signatures and representation
    /// support belong to the target; successful validation establishes no PASS.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != BINDINGS_SCHEMA_VERSION
            || self.target.is_empty()
            || !self
                .target
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || self.session.native_type.trim().is_empty()
        {
            return Err("Unsupported binding description version, target or session type".into());
        }
        self.session.factory.validate()?;
        self.session.cleanup.validate()?;
        unique(
            self.operations.iter().map(|b| b.subject.as_str()),
            "operation",
        )?;
        unique(
            self.validators.iter().map(|b| b.subject.as_str()),
            "validator",
        )?;
        unique(self.entities.iter().map(|b| b.subject.as_str()), "entity")?;
        unique(self.types.iter().map(|b| b.subject.as_str()), "type")?;
        for binding in &self.operations {
            match &binding.invoke {
                NativeInvocation::Function { entry } | NativeInvocation::Hook { entry } => {
                    entry.validate()?
                }
                NativeInvocation::Method { receiver, method } => {
                    if receiver.iter().any(|part| !identifier(part)) || !identifier(method) {
                        return Err(format!("Invalid method binding: {}", binding.subject));
                    }
                }
            }
            for argument in &binding.arguments {
                if !identifier(&argument.parameter)
                    || argument.fields.iter().any(|part| !identifier(part))
                {
                    return Err(format!("Invalid argument binding: {}", binding.subject));
                }
            }
            if let Some(hook) = &binding.result_hook {
                hook.validate()?;
            }
            unique(
                binding.failures.iter().map(|b| b.subject.as_str()),
                "failure",
            )?;
            for failure in &binding.failures {
                failure.mapper.validate()?;
            }
        }
        for binding in &self.validators {
            binding.entry.validate()?;
        }
        for binding in &self.entities {
            binding.entry.validate()?;
        }
        for entry in [
            &self.effects.events,
            &self.effects.calls,
            &self.effects.resources,
        ]
        .into_iter()
        .flatten()
        {
            entry.validate()?;
        }
        for binding in &self.types {
            if binding.native_type.trim().is_empty() {
                return Err(format!("Missing native type: {}", binding.subject));
            }
            binding.decode.validate()?;
            binding.encode.validate()?;
        }
        Ok(())
    }

    /// Missing roles remain positioned open requirements, rather than fabricated
    /// binding candidates. Surplus reviewed bindings do not block a smaller scope.
    pub fn missing(&self, requirements: &BindingRequirements) -> Vec<VerificationIssue> {
        requirements
            .requirements
            .iter()
            .filter_map(|requirement| {
                let bound = match requirement.role {
                    BindingRole::Operation => self
                        .operations
                        .iter()
                        .any(|b| b.subject == requirement.subject),
                    BindingRole::Validator => self
                        .validators
                        .iter()
                        .any(|b| b.subject == requirement.subject),
                    BindingRole::EntityObserver => self
                        .entities
                        .iter()
                        .any(|b| b.subject == requirement.subject),
                };
                (!bound).then(|| VerificationIssue {
                    code: "MISSING_BINDING".into(),
                    message: format!(
                        "{}: {:?} binding required",
                        requirement.subject, requirement.role
                    ),
                    span: requirement.span.clone(),
                })
            })
            .collect()
    }
}

fn unique<'a>(names: impl Iterator<Item = &'a str>, role: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for name in names {
        if name.trim().is_empty() || !seen.insert(name) {
            return Err(format!("Empty or duplicate {role} binding: {name}"));
        }
    }
    Ok(())
}
