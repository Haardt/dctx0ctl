//! Logical units retain full setup/assertions; technical boundaries may merge them.
use super::*;
use crate::ProgramPlan;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct LogicalVerificationUnit {
    pub id: String,
    pub cases: BTreeSet<String>,
    pub requirements: BTreeSet<String>,
    pub description_hash: String,
}

// Only syntax locations are removed. VerificationValue payloads (including
// record keys literally named "span") are opaque data and remain byte-bound.
fn without_locations(value: Value) -> Value {
    match value {
        Value::Object(map)
            if map.len() == 2 && map.contains_key("kind") && map.contains_key("value") =>
        {
            Value::Object(map)
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, value)| {
                    !(matches!(
                        key.as_str(),
                        "span" | "name_span" | "target_span" | "value_span"
                    ) && value.as_object().is_some_and(|span| {
                        span.contains_key("file")
                            && span.contains_key("start")
                            && span.contains_key("end")
                    }))
                })
                .map(|(key, value)| (key, without_locations(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(without_locations).collect()),
        other => other,
    }
}

fn add_unit(
    units: &mut Vec<LogicalVerificationUnit>,
    id: String,
    cases: BTreeSet<String>,
    description: &impl Serialize,
    manifest: &VerificationCaseManifest,
) -> Result<(), serde_json::Error> {
    if cases.is_empty() {
        return Ok(());
    }
    let requirements = cases
        .iter()
        .flat_map(|id| {
            manifest
                .case_requirements
                .get(id)
                .into_iter()
                .flatten()
                .cloned()
        })
        .collect();
    units.push(LogicalVerificationUnit {
        id,
        cases,
        requirements,
        description_hash: evidence_digest(&without_locations(serde_json::to_value(description)?))?,
    });
    Ok(())
}

pub fn logical_unit_catalog(program: &ProgramPlan) -> Result<Vec<LogicalVerificationUnit>, String> {
    let manifest = crate::case_manifest(program);
    let mut units = Vec::new();
    let result = (|| -> Result<(), serde_json::Error> {
        for contract in &program.contracts {
            let cases = [
                ("value", contract.corpus.values.len()),
                ("rejection", contract.rejection_corpus.values.len()),
            ]
            .into_iter()
            .flat_map(|(kind, count)| {
                (0..count).map(move |index| format!("{}:{kind}:{index}", contract.subject))
            })
            .collect();
            add_unit(
                &mut units,
                format!("contract:{}", contract.subject),
                cases,
                contract,
                &manifest,
            )?;
            if !contract.identity_requirements.is_empty() {
                add_unit(
                    &mut units,
                    format!("identity:{}", contract.subject),
                    [format!("{}:identity", contract.subject)].into(),
                    contract,
                    &manifest,
                )?;
            }
        }
        for operation in &program.operations {
            let mut cases: BTreeSet<_> =
                operation.cases.iter().map(|case| case.id.clone()).collect();
            if crate::supports_generated_operation(operation) {
                cases.extend(
                    (0..operation.inputs.bindings.len())
                        .map(|i| format!("{}:operation:{i}", operation.operation.id)),
                );
            } else {
                cases.insert(format!("{}:operation", operation.operation.id));
            }
            add_unit(
                &mut units,
                format!("operation:{}", operation.operation.id),
                cases,
                operation,
                &manifest,
            )?;
        }
        for property in &program.properties {
            let cases = property
                .assertions
                .iter()
                .flat_map(|assertion| assertion.cases.iter().map(|case| case.id.clone()))
                .collect();
            add_unit(
                &mut units,
                format!("property:{}", property.subject),
                cases,
                property,
                &manifest,
            )?;
        }
        for example in &program.examples {
            add_unit(
                &mut units,
                format!("scenario:{}", example.subject),
                [example.subject.clone()].into(),
                example,
                &manifest,
            )?;
        }
        for lifecycle in &program.lifecycles {
            for path in &lifecycle.paths {
                add_unit(
                    &mut units,
                    format!("lifecycle:{}", path.id),
                    [path.id.clone()].into(),
                    &(
                        &lifecycle.entity,
                        &lifecycle.field,
                        &lifecycle.states,
                        &lifecycle.transitions,
                        path,
                        &lifecycle.issues,
                    ),
                    &manifest,
                )?;
            }
        }
        Ok(())
    })();
    result.map_err(|error| error.to_string())?;
    let boundaries: Vec<_> = units
        .iter()
        .map(|u| VerificationUnit {
            id: u.id.clone(),
            cases: u.cases.clone(),
            execute_always: false,
            boundary: UnitDependencyBoundary::Unknown {
                group: "catalog".into(),
                reasons: ["TECHNICAL_BOUNDARY_REQUIRED".into()].into(),
            },
        })
        .collect();
    validate_unit_partition(&manifest.required_cases, &boundaries)?;
    Ok(units)
}

/// Merge unknown logical units into their existing native execution cohort.
/// Shared-session/fixture analysis supplies the same group for connected units.
pub fn merge_unit_boundaries(
    logical: &[LogicalVerificationUnit],
    boundaries: &BTreeMap<String, UnitDependencyBoundary>,
) -> Result<Vec<VerificationUnit>, String> {
    let mut units = BTreeMap::<String, VerificationUnit>::new();
    for unit in logical {
        let boundary = boundaries
            .get(&unit.id)
            .ok_or_else(|| format!("UNIT_BOUNDARY_MISSING: {}", unit.id))?;
        let id = match boundary {
            UnitDependencyBoundary::Unknown { group, .. } => format!("cohort:{group}"),
            _ => unit.id.clone(),
        };
        let entry = units.entry(id.clone()).or_insert_with(|| VerificationUnit {
            id,
            cases: BTreeSet::new(),
            boundary: boundary.clone(),
            execute_always: false,
        });
        if let (
            UnitDependencyBoundary::Unknown { reasons, .. },
            UnitDependencyBoundary::Unknown { reasons: more, .. },
        ) = (&mut entry.boundary, boundary)
        {
            reasons.extend(more.iter().cloned());
        } else if &entry.boundary != boundary {
            return Err("CONFLICTING_UNIT_BOUNDARIES".into());
        }
        entry.cases.extend(unit.cases.iter().cloned());
    }
    Ok(units.into_values().collect())
}
