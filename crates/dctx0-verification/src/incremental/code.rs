//! The implementation change key contains code, not build outputs or host metadata.
use super::*;

pub fn is_code_path(path: &str) -> bool {
    matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(|value| value.to_str()),
        Some(
            "go" | "rs"
                | "ts"
                | "tsx"
                | "mts"
                | "cts"
                | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "java"
                | "kt"
                | "kts"
                | "py"
                | "pyi"
                | "rb"
                | "php"
                | "cs"
                | "fs"
                | "fsx"
                | "c"
                | "cc"
                | "cpp"
                | "cxx"
                | "h"
                | "hpp"
                | "hh"
                | "hxx"
                | "s"
                | "S"
                | "m"
                | "mm"
                | "swift"
                | "scala"
                | "dart"
                | "lua"
                | "r"
                | "R"
                | "ex"
                | "exs"
                | "erl"
                | "hrl"
                | "pl"
                | "pm"
                | "sh"
                | "bash"
                | "zsh"
                | "fish"
                | "ps1"
                | "sql"
                | "html"
                | "css"
                | "scss"
                | "less"
                | "vue"
                | "svelte"
                | "graphql"
                | "gql"
                | "dctx0"
        )
    )
}

pub fn code_inputs(inputs: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    inputs
        .iter()
        .filter(|(path, _)| is_code_path(path))
        .map(|(path, hash)| (path.clone(), hash.clone()))
        .collect()
}

pub fn code_receipt(receipt: &ExternalInputReceipt) -> ExternalInputReceipt {
    ExternalInputReceipt {
        schema_version: receipt.schema_version,
        deterministic_units: receipt.deterministic_units.clone(),
        files: receipt
            .files
            .iter()
            .filter(|(path, _)| path.strip_prefix("file:").is_some_and(is_code_path))
            .map(|(path, hash)| (path.clone(), hash.clone()))
            .collect(),
        environment: BTreeMap::new(),
        incomplete: receipt
            .incomplete
            .iter()
            .filter(|gap| gap.code == "INPUT_FILE_UNAVAILABLE" && is_code_path(&gap.input))
            .cloned()
            .collect(),
    }
}
