//! Explicit diagnostic export; existing adapter setup and execution are unchanged.
use dctx0_verification::{ProgramPlan, application_plan::*};
use std::{
    io::{Read, Write},
    path::Path,
};

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(crate::error_message)?
        .take(524_289)
        .read_to_end(&mut bytes)
        .map_err(crate::error_message)?;
    if bytes.len() > 524_288 {
        return Err("Application-test configuration/catalog exceeds 512 KiB".into());
    }
    serde_json::from_slice(&bytes).map_err(crate::error_message)
}

pub(crate) fn render(
    program: &ProgramPlan,
    configuration: &Path,
    catalog: &Path,
    json: bool,
    output: &mut dyn Write,
) -> Result<bool, String> {
    let configuration = read(configuration)?;
    let catalog: Vec<TestRegistration> = read(catalog)?;
    let plan = application_test_plan(program, &configuration, &catalog).map_err(|issues| {
        issues
            .iter()
            .map(|issue| format!("{}: {}", issue.code, issue.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    if json {
        crate::render::json(output, &plan)?;
    } else {
        writeln!(output, "# Application-test plan\n\nSnapshot: `{}`; check occurrence: {}\n\nDiagnostic planning only; no native execution or task adoption.\n\n| Group | Backend | Target | Mandatory cases |\n| --- | --- | --- | ---: |", program.snapshot, program.check_occurrence).map_err(crate::error_message)?;
        for group in &configuration.groups {
            writeln!(
                output,
                "| {} | {:?} | {} | {} |",
                group.id,
                group.backend,
                group.target,
                plan.case_groups
                    .values()
                    .filter(|id| *id == &group.id)
                    .count()
            )
            .map_err(crate::error_message)?;
        }
        writeln!(output, "\n{} expected registrations; {} open manifest obligations. Registration does not establish execution completeness.", plan.registrations.len(), plan.manifest.open_obligations.len()).map_err(crate::error_message)?;
    }
    Ok(program.issues.is_empty())
}
