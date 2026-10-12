//! Representation conversion only; model metadata supplies types, never answers.
use crate::contract::Value;
use serde_json::json;
pub const ROOT: &str = "HomeCinema.MovieNight.";
pub fn named(name: &str) -> Value {
    json!({"name":name,"symbol_id":format!("{ROOT}{name}"),"arguments":[]})
}
pub fn model() -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read("model.json").map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn decode(wire: &Value) -> Result<Value, String> {
    let value = &wire["value"];
    match wire["kind"].as_str().ok_or("Missing wire kind")? {
        "unit" => Ok(Value::Null),
        "integer" => {
            let text = value.as_str().ok_or("Invalid exact integer")?;
            let number: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            cinema::domain::exact_integer(&number).map_err(|e| e.to_string())?;
            Ok(number)
        }
        "float" => {
            let n = value
                .as_str()
                .ok_or("Missing float text")?
                .parse::<f64>()
                .map_err(|e| e.to_string())?;
            if !n.is_finite() {
                return Err("Unrepresentable float".into());
            }
            Ok(json!(n))
        }
        "string" if value.is_string() => Ok(value.clone()),
        "boolean" if value.is_boolean() => Ok(value.clone()),
        "option" => {
            if value.is_null() {
                Ok(Value::Null)
            } else {
                decode(value)
            }
        }
        "list" => value
            .as_array()
            .ok_or("Expected wire list")?
            .iter()
            .map(decode)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        "record" => value
            .as_object()
            .ok_or("Expected wire record")?
            .iter()
            .map(|(name, item)| Ok((name.clone(), decode(item)?)))
            .collect::<Result<serde_json::Map<_, _>, String>>()
            .map(Value::Object),
        "variant" => {
            if !value["payload"].is_null() {
                return Err("Payload variant unsupported".into());
            }
            value["name"]
                .as_str()
                .map(|n| json!(n))
                .ok_or("Missing variant name".into())
        }
        _ => Err("Unsupported wire representation".into()),
    }
}
pub fn encode(model: &Value, value: &Value, ty: &Value) -> Result<Value, String> {
    if let Some(symbol) = ty["symbol_id"].as_str() {
        let data = model["data"].get(symbol).ok_or("Missing type descriptor")?;
        if !data["alias"].is_null() {
            return encode(model, value, &data["alias"]);
        }
        if data["variants"].as_array().is_some_and(|v| !v.is_empty()) {
            return Ok(
                json!({"kind":"variant","value":{"declaration":symbol,"name":value.as_str().ok_or("Expected alternative")?,"payload":null}}),
            );
        }
        let fields = data["fields"].as_array().ok_or("Missing fields")?;
        let object = value.as_object().ok_or("Expected application record")?;
        if fields.len() != object.len() {
            return Err("Unexpected application fields".into());
        }
        let mut out = serde_json::Map::new();
        for field in fields {
            let name = field["name"].as_str().ok_or("Missing field name")?;
            out.insert(
                name.into(),
                encode(
                    model,
                    object.get(name).ok_or("Missing actual field")?,
                    &field["ty"],
                )?,
            );
        }
        return Ok(json!({"kind":"record","value":out}));
    }
    let (kind, encoded) = match ty["name"].as_str().ok_or("Missing type name")? {
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
                    .ok_or("Expected application list")?
                    .iter()
                    .map(|item| encode(model, item, &ty["arguments"][0]))
                    .collect::<Result<_, _>>()?,
            ),
        ),
        "Int" => (
            "integer",
            json!(
                cinema::domain::exact_integer(value)
                    .map_err(|e| e.to_string())?
                    .to_string()
            ),
        ),
        "Float" => {
            let n = value
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or("Expected finite application float")?;
            ("float", json!(n.to_string()))
        }
        "String" | "UUID" | "Timestamp" => (
            "string",
            json!(value.as_str().ok_or("Expected application text")?),
        ),
        "Boolean" => (
            "boolean",
            json!(value.as_bool().ok_or("Expected application boolean")?),
        ),
        "Unit" if value.is_null() => return Ok(json!({"kind":"unit"})),
        _ => return Err("Unsupported application type".into()),
    };
    Ok(json!({"kind":kind,"value":encoded}))
}
pub fn outcome(
    model: &Value,
    result: Result<Value, cinema::domain::Error>,
    ty: &Value,
) -> Result<Value, String> {
    match result {
        Ok(value) => Ok(json!({"kind":"returned","value":encode(model,&value,ty)?})),
        Err(cinema::domain::Error::Failure { code, payload }) => Ok(
            json!({"kind":"returned","value":{"kind":"failure","value":{"declaration":format!("{ROOT}{code}"),"payload":encode(model,&payload,&named(&code))?}}}),
        ),
        Err(error) => Ok(json!({"kind":"threw","message":error.to_string()})),
    }
}
