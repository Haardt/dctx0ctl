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

/// Buffer only up to the explicit response budget; never emit a partial JSON value.
pub(crate) fn bounded_json(
    output: &mut dyn Write,
    value: &impl serde::Serialize,
    max: u64,
) -> Result<(), String> {
    with_output_limit(output, max, |buffer| json(buffer, value))
}

pub(crate) fn with_output_limit<T>(
    output: &mut dyn Write,
    max: u64,
    action: impl FnOnce(&mut dyn Write) -> Result<T, String>,
) -> Result<T, String> {
    struct Buffer {
        bytes: Vec<u8>,
        max: u64,
    }
    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len() as u64 + bytes.len() as u64 > self.max {
                return Err(std::io::Error::other(
                    "OUTPUT_LIMIT_EXCEEDED: use --max-output-bytes or verify export",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Buffer { bytes: vec![], max };
    let result = action(&mut buffer)?;
    output.write_all(&buffer.bytes).map_err(error_message)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flushing_or_exceeding_a_response_budget_never_publishes_partial_output() {
        let mut published = b"existing".to_vec();
        let result: Result<(), String> = with_output_limit(&mut published, 100, |buffer| {
            buffer.write_all(b"partial").map_err(error_message)?;
            buffer.flush().map_err(error_message)?;
            Err("cancelled".into())
        });
        assert!(result.is_err());
        assert_eq!(published, b"existing");
        assert!(
            with_output_limit(&mut published, 3, |buffer| {
                buffer.write_all(b"large").map_err(error_message)
            })
            .is_err()
        );
        assert_eq!(published, b"existing");
        with_output_limit(&mut published, 4, |buffer| {
            buffer.write_all(b"done").map_err(error_message)?;
            buffer.flush().map_err(error_message)
        })
        .unwrap();
        assert_eq!(published, b"existingdone");
    }

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
