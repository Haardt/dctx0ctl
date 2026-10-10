use dctx0_core::Source;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub fn hash(value: &impl Serialize) -> String {
    // serde_json's default map representation orders keys; canonical IR lists
    // are built deterministically and semantically ordered arrays stay ordered.
    let bytes =
        serde_json::to_vec(value).expect("snapshot structures contain only JSON-compatible values");
    format!("{:x}", Sha256::digest(bytes))
}
pub fn source_hash(sources: &[Source]) -> String {
    let mut ordered = sources.to_vec();
    ordered.sort_by(|a, b| a.file.cmp(&b.file));
    hash(&ordered)
}
/// Strip syntax locations recursively. A literal/field named `span` in source
/// remains data inside its typed representation, not an object key here.
pub fn semantic_value(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "span" | "name_span" | "target_span" | "value_span"
                    )
                })
                .map(|(key, value)| (key, semantic_value(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(semantic_value).collect()),
        other => other,
    }
}
