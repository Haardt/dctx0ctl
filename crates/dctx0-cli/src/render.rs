use crate::error_message;
use std::io::Write;

pub(crate) fn json(output: &mut dyn Write, value: &impl serde::Serialize) -> Result<(), String> {
    serde_json::to_writer_pretty(&mut *output, value).map_err(error_message)?;
    writeln!(output).map_err(error_message)?;
    Ok(())
}
pub(crate) fn display_changes(
    output: &mut dyn Write,
    changes: &dctx0_changes::Changes,
) -> Result<(), String> {
    if changes.before.is_none() {
        writeln!(output, "Initial valid snapshot").map_err(error_message)?;
    } else {
        writeln!(
            output,
            "{} changes; source changed: {}",
            changes.changes.len(),
            changes.source_changed
        )
        .map_err(error_message)?;
    }
    for change in &changes.changes {
        writeln!(
            output,
            "{:?} {} -> {} [{}]; {} affected areas",
            change.kind,
            change.old.as_deref().unwrap_or("-"),
            change.new.as_deref().unwrap_or("-"),
            change
                .facets
                .iter()
                .map(|facet| match facet {
                    dctx0_changes::ChangeFacet::Contract => "contract",
                    dctx0_changes::ChangeFacet::Behavior => "behavior",
                    dctx0_changes::ChangeFacet::Facts => "facts",
                    dctx0_changes::ChangeFacet::Location => "location",
                })
                .collect::<Vec<_>>()
                .join(", "),
            change.impacts.len()
        )
        .map_err(error_message)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_reports_split_large_collections_into_searchable_lines() {
        let value = serde_json::json!({"items": (0..10_000).collect::<Vec<_>>()});
        let mut output = Vec::new();
        json(&mut output, &value).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.ends_with('\n'));
        assert!(text.lines().count() > 10_000);
        assert!(text.lines().all(|line| line.len() < 80));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).unwrap(),
            value
        );
    }
}
