//! Reviewed dataflow from actual successful setup results into typed arguments.
use crate::OperationCaseInput;
use dctx0_core::{Invocation, InvocationObservation, Observation, VerificationValue, WorkspaceIr};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationArgumentBinding {
    pub setup: usize,
    pub source_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_setup: Option<usize>,
    pub parameter: String,
    pub target_path: Vec<String>,
}

pub fn validate_argument_bindings(
    ir: &WorkspaceIr,
    input: &OperationCaseInput,
) -> Result<(), String> {
    if input.argument_bindings.len() > 32 {
        return Err("Setup argument bindings exceed their finite limit".into());
    }
    for (index, binding) in input.argument_bindings.iter().enumerate() {
        if binding.setup >= binding.target_setup.unwrap_or(input.setup.len())
            || binding.source_path.len() > 16
            || binding.target_path.len() > 16
        {
            return Err(
                "Argument bindings require a preceding setup result and bounded paths".into(),
            );
        }
        let target = match binding.target_setup {
            Some(index) => input.setup.get(index),
            None => Some(&input.invocation),
        }
        .ok_or("Argument binding target setup is absent")?;
        let operation = ir
            .operations
            .get(&target.subject)
            .ok_or("Argument binding target operation is undeclared")?;
        if !operation
            .parameters
            .iter()
            .any(|parameter| parameter.name == binding.parameter)
        {
            return Err("Argument binding parameter is absent".into());
        }
        if input.argument_bindings[..index].iter().any(|other| {
            other.target_setup == binding.target_setup
                && other.parameter == binding.parameter
                && (other.target_path.starts_with(&binding.target_path)
                    || binding.target_path.starts_with(&other.target_path))
        }) {
            return Err("Argument bindings may not overwrite overlapping targets".into());
        }
    }
    Ok(())
}

pub fn resolve_operation_arguments(
    ir: &WorkspaceIr,
    input: &OperationCaseInput,
    observed: &[InvocationObservation],
    target_setup: Option<usize>,
) -> Result<Invocation, String> {
    validate_argument_bindings(ir, input)?;
    let mut invocation = match target_setup {
        Some(index) => input.setup.get(index),
        None => Some(&input.invocation),
    }
    .ok_or("Argument target invocation is absent")?
    .clone();
    let operation = ir
        .operations
        .get(&invocation.subject)
        .ok_or("Argument target operation is undeclared")?;
    for binding in input
        .argument_bindings
        .iter()
        .filter(|binding| binding.target_setup == target_setup)
    {
        let source = observed
            .get(binding.setup)
            .ok_or("Actual setup result is absent")?;
        if source.invocation.subject != input.setup[binding.setup].subject {
            return Err("Actual setup result has another subject".into());
        }
        let Observation::Returned { value } = &source.observation else {
            return Err("Actual setup did not return a value".into());
        };
        if matches!(value, VerificationValue::Failure { .. }) {
            return Err("A failed setup cannot supply argument values".into());
        }
        let value = record_path(value, &binding.source_path)?.clone();
        let index = operation
            .parameters
            .iter()
            .position(|parameter| parameter.name == binding.parameter)
            .ok_or("Argument parameter is absent")?;
        let mut target = invocation
            .arguments
            .get_mut(index)
            .ok_or("Argument target is absent")?;
        for field in &binding.target_path {
            let VerificationValue::Record(fields) = target else {
                return Err("Argument target path is not a record".into());
            };
            target = fields
                .get_mut(field)
                .ok_or("Argument target field is absent")?;
        }
        *target = value;
    }
    Ok(invocation)
}
fn record_path<'a>(
    mut value: &'a VerificationValue,
    path: &[String],
) -> Result<&'a VerificationValue, String> {
    for field in path {
        value = match value {
            VerificationValue::Record(fields) => {
                fields.get(field).ok_or("Setup result field is absent")?
            }
            VerificationValue::List(items) => {
                let index = field
                    .parse::<usize>()
                    .map_err(|_| "Setup result list index is invalid")?;
                if index.to_string() != *field {
                    return Err("Setup result list index must be canonical".into());
                }
                items.get(index).ok_or("Setup result list item is absent")?
            }
            _ => return Err("Setup result path is not a record or list".into()),
        };
    }
    Ok(value)
}
