//! Lossless SDK decoding and checked conversion to actual Rust API inputs.
use crate::contract::Value;
use dctx0_core::VerificationValue;
use serde_json::json;
pub fn decode(wire: &Value) -> Result<Value, String> {
    let value: VerificationValue =
        serde_json::from_value(wire.clone()).map_err(|e| e.to_string())?;
    fn inner(value: VerificationValue) -> Result<Value, String> {
        Ok(match value {
            VerificationValue::Unit => Value::Null,
            VerificationValue::Integer(text) => json!(
                text.parse::<i64>()
                    .map_err(|_| "Integer does not fit the Rust i64 API")?
            ),
            VerificationValue::Boolean(value) => json!(value),
            VerificationValue::String(value) => json!(value),
            VerificationValue::List(values) => {
                Value::Array(values.into_iter().map(inner).collect::<Result<_, _>>()?)
            }
            VerificationValue::Record(values) => Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| Ok((key, inner(value)?)))
                    .collect::<Result<_, String>>()?,
            ),
            VerificationValue::Option(value) => match value {
                None => Value::Null,
                Some(value) => inner(*value)?,
            },
            VerificationValue::Variant {
                name,
                payload: None,
                ..
            } => json!(name),
            _ => return Err("Unsupported Rust API representation".into()),
        })
    }
    inner(value)
}
pub fn model() -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read("model.json").map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn encode(model: &Value, value: &Value, ty: &Value) -> Result<Value, String> {
    if let Some(symbol) = ty["symbol_id"].as_str() {
        let data = model["data"]
            .get(symbol)
            .ok_or("Missing output descriptor")?;
        if !data["alias"].is_null() {
            return encode(model, value, &data["alias"]);
        }
        if data["variants"].as_array().is_some_and(|v| !v.is_empty()) {
            return Ok(
                json!({"kind":"variant","value":{"declaration":symbol,"name":value.as_str().ok_or("Expected variant")?,"payload":null}}),
            );
        }
        let fields = data["fields"].as_array().ok_or("Missing fields")?;
        let object = value.as_object().ok_or("Expected output record")?;
        if fields.len() != object.len() {
            return Err("Unexpected output fields".into());
        }
        let mut out = serde_json::Map::new();
        for field in fields {
            let name = field["name"].as_str().ok_or("Missing field name")?;
            out.insert(
                name.into(),
                encode(
                    model,
                    object.get(name).ok_or("Missing actual output field")?,
                    &field["ty"],
                )?,
            );
        }
        return Ok(json!({"kind":"record","value":out}));
    }
    let (kind, encoded) = match ty["name"].as_str().ok_or("Missing output type")? {
        "Option" => (
            "option",
            if value.is_null() {
                Value::Null
            } else {
                encode(model, value, &ty["arguments"][0])?
            },
        ),
        "List" => (
            "list",
            Value::Array(
                value
                    .as_array()
                    .ok_or("Expected list")?
                    .iter()
                    .map(|v| encode(model, v, &ty["arguments"][0]))
                    .collect::<Result<_, _>>()?,
            ),
        ),
        "String" => ("string", json!(value.as_str().ok_or("Expected text")?)),
        "Boolean" => ("boolean", json!(value.as_bool().ok_or("Expected Boolean")?)),
        "Int" => (
            "integer",
            json!(value.as_i64().ok_or("Expected i64")?.to_string()),
        ),
        "Unit" if value.is_null() => return Ok(json!({"kind":"unit"})),
        _ => return Err("Unsupported output type".into()),
    };
    Ok(json!({"kind":kind,"value":encoded}))
}
