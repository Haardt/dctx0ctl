//! Exact structural testing codecs; use an authored binding for production types.
use dctx0_core::{CodecOptions, CodecRepresentation, DataIr};
use dctx0_target_sdk::{Artifact, Error};
use std::collections::BTreeMap;
pub const CODEC_GENERATOR_VERSION: &str = "dctx0-rust-codecs-1";
pub fn generate_codecs(
    declarations: &BTreeMap<String, DataIr>,
    options: &CodecOptions,
    snapshot: &str,
) -> Result<Artifact, Error> {
    options.validate(declarations).map_err(Error::Invalid)?;
    if options.integer != CodecRepresentation::Number
        || options.option != CodecRepresentation::Null
        || options.types.values().any(|v| {
            matches!(
                v,
                CodecRepresentation::Bigint | CodecRepresentation::Undefined
            )
        })
    {
        return Err(Error::Invalid("Rust codecs require exact JSON number integers and null options; bigint/undefined need authored mappings".into()));
    }
    let descriptor = serde_json::to_string(declarations).expect("IR serializes");
    let settings = serde_json::to_string(options).expect("options serialize");
    Ok(Artifact {
        path: "wire_codecs.rs".into(),
        contents: format!(
            "// Generator: {CODEC_GENERATOR_VERSION}; snapshot: {snapshot}\n{}\nconst CODEC_DESCRIPTOR: &str = {descriptor:?};\nconst CODEC_SETTINGS: &str = {settings:?};\n",
            include_str!("../harness/codecs.rs")
        ),
    })
}
