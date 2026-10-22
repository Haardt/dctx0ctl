//! Deterministic editable testing codecs; domain validation stays in production.
use dctx0_core::{CodecOptions, DataIr};
use dctx0_target_sdk::{Artifact, Error};
use std::collections::BTreeMap;

pub const CODEC_GENERATOR_VERSION: &str = "dctx0-typescript-codecs-2";

pub fn generate_codecs(
    declarations: &BTreeMap<String, DataIr>,
    options: &CodecOptions,
    snapshot: &str,
) -> Result<Artifact, Error> {
    options.validate(declarations).map_err(Error::Invalid)?;
    // These derived models have only JSON-safe fields and string map keys;
    // serialization cannot fail on user input after representation validation.
    let descriptor = serde_json::to_string(declarations).expect("codec IR is JSON-safe");
    let configuration = serde_json::to_string(options).expect("codec options are JSON-safe");
    let snapshot = serde_json::to_string(snapshot).expect("snapshot text is JSON-safe");
    Ok(Artifact {
        path: "wire-codecs.ts".into(),
        contents: format!(
            "/** Editable testing codecs. Generator: {CODEC_GENERATOR_VERSION}.\n * Representation conversion only; does not replace production contract validation. */\nimport type {{ Value }} from './contract.ts';\nexport const snapshot = {snapshot};\nconst declarations: Record<string, Data> = {descriptor};\nexport const configuration: Configuration = {configuration};\n{}",
            include_str!("../harness/codecs.ts")
        ),
    })
}
