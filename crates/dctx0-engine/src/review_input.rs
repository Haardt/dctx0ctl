//! Bound wire input without retaining a second complete JSON byte buffer.
use crate::{Error, Result};
use serde::de::DeserializeOwned;
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

pub const DEFAULT_REVIEW_INPUT_BYTES: u64 = 64 * 1024 * 1024;

pub fn read_review_input<T: DeserializeOwned>(path: &Path, limit: u64, label: &str) -> Result<T> {
    let probe_limit = limit.checked_add(1).filter(|_| limit > 0).ok_or_else(|| {
        Error::Invalid("--max-input-bytes must be between 1 and u64::MAX - 1".to_owned())
    })?;
    let file = File::open(path)?;
    let too_large = || {
        format!(
            "{label} input exceeds {limit} bytes; adjust --max-input-bytes for a larger reviewed group"
        )
    };
    let metadata = file.metadata()?;
    if metadata.is_file() && metadata.len() > limit {
        return Err(Error::Invalid(too_large()));
    }
    decode(file, limit, probe_limit, label)
}

fn decode<T: DeserializeOwned>(
    reader: impl Read,
    limit: u64,
    probe_limit: u64,
    label: &str,
) -> Result<T> {
    let mut bounded = reader.take(probe_limit);
    let value = serde_json::from_reader(BufReader::new(&mut bounded));
    if probe_limit - bounded.limit() > limit {
        return Err(Error::Invalid(format!(
            "{label} input exceeds {limit} bytes; adjust --max-input-bytes for a larger reviewed group"
        )));
    }
    value.map_err(|error| Error::Invalid(format!("Invalid {label} JSON: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn complete_rust_api_bounds_notes_before_publishing_a_check() {
        let root = std::env::temp_dir().join(format!("dctx0-review-limit-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let notes = root.join("notes.json");
        File::create(&notes)
            .unwrap()
            .set_len(DEFAULT_REVIEW_INPUT_BYTES + 1)
            .unwrap();
        let project = crate::Project::new(&root, None).unwrap();
        let error = project
            .complete_verification(
                &root,
                &notes,
                &dctx0_target_typescript::Provider,
                &dctx0_target_typescript::BunHarness,
                &crate::NativeCancellation::default(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("67108864 bytes"));
        assert!(!project.database.exists());
        std::fs::write(&notes, b"{}").unwrap();
        let error = project
            .complete_verification_with_input_limit(
                &root,
                &notes,
                &dctx0_target_typescript::Provider,
                &dctx0_target_typescript::BunHarness,
                &crate::NativeCancellation::default(),
                1,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("exceeds 1 bytes"));
        assert!(!project.database.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_stream_rejects_growth_and_trailing_invalid_data() {
        let exact: Vec<String> = decode(&b"[]"[..], 2, 3, "Inspection").unwrap();
        assert!(exact.is_empty());
        let grown = decode::<Vec<String>>(&b"[] "[..], 2, 3, "Inspection").unwrap_err();
        assert!(grown.to_string().contains("exceeds 2 bytes"));
        assert!(
            decode::<Vec<String>>(&b"[] x"[..], 4, 5, "Attestation")
                .unwrap_err()
                .to_string()
                .contains("Invalid Attestation JSON")
        );
        assert!(
            read_review_input::<Vec<String>>(Path::new("unused"), 0, "Inspection")
                .unwrap_err()
                .to_string()
                .contains("--max-input-bytes")
        );
        assert!(
            read_review_input::<Vec<String>>(Path::new("unused"), u64::MAX, "Inspection")
                .unwrap_err()
                .to_string()
                .contains("--max-input-bytes")
        );
    }
}
