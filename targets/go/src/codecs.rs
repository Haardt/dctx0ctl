//! Structural testing representations; production calls remain authored.
use dctx0_core::{CodecOptions, CodecRepresentation, DataIr};
use dctx0_target_sdk::{Artifact, Error};
use std::collections::BTreeMap;
pub const CODEC_GENERATOR_VERSION: &str = "dctx0-go-codecs-1";
pub fn generate_codecs(
    declarations: &BTreeMap<String, DataIr>,
    options: &CodecOptions,
    snapshot: &str,
) -> Result<Artifact, Error> {
    options.validate(declarations).map_err(Error::Invalid)?;
    if options.option != CodecRepresentation::Null
        || options
            .types
            .values()
            .any(|v| *v == CodecRepresentation::Undefined)
    {
        return Err(Error::Invalid(
            "Go codecs require null options; undefined has no Go representation".into(),
        ));
    }
    let descriptor =
        serde_json::to_string(&serde_json::to_string(declarations).expect("IR serializes"))
            .expect("text serializes");
    let settings =
        serde_json::to_string(&serde_json::to_string(options).expect("options serialize"))
            .expect("text serializes");
    Ok(Artifact {
        path: "wire-codecs.go".into(),
        contents: format!(
            "// Generator: {CODEC_GENERATOR_VERSION}; snapshot: {snapshot}\n{}\nconst codecDescriptor = {descriptor}\nconst codecSettings = {settings}\n",
            include_str!("../harness/codecs.go")
        ),
    })
}
