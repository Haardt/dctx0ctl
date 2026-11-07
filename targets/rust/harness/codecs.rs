//! Structural testing representations, never production validators.
use serde_json::{Value, json};
fn codec_error(path: &str, message: &str) -> String {
    format!("wire codec {path}: {message}")
}
fn reference(name: &str) -> Value {
    json!({"symbol_id":name,"arguments":[]})
}
fn descriptor() -> Value {
    serde_json::from_str(CODEC_DESCRIPTOR).expect("generated descriptor")
}
fn settings() -> Value {
    serde_json::from_str(CODEC_SETTINGS).expect("generated settings")
}
pub fn decode(name: &str, wire: &Value) -> Result<Value, String> {
    convert_selected(name, wire, true, false)
}
pub fn decode_input(name: &str, wire: &Value) -> Result<Value, String> {
    convert_selected(name, wire, true, true)
}
pub fn encode(name: &str, native: &Value) -> Result<Value, String> {
    convert_selected(name, native, false, false)
}
fn convert_selected(
    name: &str,
    value: &Value,
    decoding: bool,
    input: bool,
) -> Result<Value, String> {
    let data = descriptor();
    if data.get(name).is_none() {
        return Err(codec_error(name, "unknown selected declaration"));
    }
    convert(
        &data,
        &settings(),
        &reference(name),
        value,
        decoding,
        input,
        None,
        name,
        0,
    )
}
#[allow(clippy::too_many_arguments)]
fn convert(
    data: &Value,
    config: &Value,
    ty: &Value,
    value: &Value,
    decoding: bool,
    input: bool,
    override_: Option<&str>,
    path: &str,
    depth: usize,
) -> Result<Value, String> {
    if depth > 128 {
        return Err(codec_error(path, "conversion depth exceeds 128"));
    }
    let id = ty["symbol_id"].as_str();
    let declaration = id.and_then(|id| data.get(id));
    let mode = override_.or_else(|| id.and_then(|id| config["types"][id].as_str()));
    if id.is_some() && declaration.is_none() {
        return Err(codec_error(path, "unknown declaration"));
    }
    if let Some(alias) = declaration
        .and_then(|d| d.get("alias"))
        .filter(|v| !v.is_null())
    {
        return convert(
            data,
            config,
            alias,
            value,
            decoding,
            input,
            mode,
            path,
            depth + 1,
        );
    }
    let name = ty["name"].as_str().unwrap_or("");
    let variants = declaration.and_then(|d| d["variants"].as_array());
    let expected = if declaration.is_some() {
        if variants.is_some_and(|v| !v.is_empty()) {
            "variant"
        } else {
            "record"
        }
    } else {
        match name {
            "Int" => "integer",
            "String" | "UUID" | "Timestamp" => "string",
            "Boolean" => "boolean",
            "Float" | "Duration" => "float",
            "Decimal" => "decimal",
            "List" | "Bytes" => "list",
            "Option" | "Dctx0Option" | "Some" | "None" => "option",
            "Unit" => "unit",
            _ => return Err(codec_error(path, &format!("unsupported builtin {name}"))),
        }
    };
    let kind = if decoding {
        value["kind"]
            .as_str()
            .ok_or_else(|| codec_error(path, "missing wire kind"))?
    } else {
        expected
    };
    if decoding && !input && kind != expected {
        return Err(codec_error(
            path,
            &format!("expected {expected}, received {kind}"),
        ));
    }
    let native = if decoding { &value["value"] } else { value };
    let child = |ty: &Value, value: &Value, suffix: &str| {
        convert(
            data,
            config,
            ty,
            value,
            decoding,
            input,
            None,
            &format!("{path}{suffix}"),
            depth + 1,
        )
    };
    let result = match kind {
        "unit" => {
            if !native.is_null() {
                return Err(codec_error(path, "expected null Unit"));
            };
            Value::Null
        }
        "string" => Value::String(
            native
                .as_str()
                .ok_or_else(|| codec_error(path, "expected text"))?
                .into(),
        ),
        "boolean" => Value::Bool(
            native
                .as_bool()
                .ok_or_else(|| codec_error(path, "expected boolean"))?,
        ),
        "integer" => {
            let text = if decoding {
                native
                    .as_str()
                    .ok_or_else(|| codec_error(path, "expected exact integer text"))?
                    .to_owned()
            } else {
                native
                    .as_number()
                    .ok_or_else(|| codec_error(path, "expected exact JSON number"))?
                    .to_string()
            };
            let digits = text.strip_prefix('-').unwrap_or(&text);
            if text.len() > 65536
                || digits.is_empty()
                || !digits.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(codec_error(path, "invalid exact integer text"));
            }
            if decoding {
                let normalized = format!(
                    "{}{}",
                    if text.starts_with('-') && digits.bytes().any(|b| b != b'0') {
                        "-"
                    } else {
                        ""
                    },
                    {
                        let d = digits.trim_start_matches('0');
                        if d.is_empty() { "0" } else { d }
                    }
                );
                let exact: Value = serde_json::from_str(&normalized).map_err(|_| {
                    codec_error(
                        path,
                        "exact integer unavailable; enable arbitrary_precision",
                    )
                })?;
                if exact.to_string() != normalized {
                    return Err(codec_error(
                        path,
                        "exact integer unavailable; enable arbitrary_precision",
                    ));
                }
                exact
            } else {
                Value::String(text)
            }
        }
        "decimal" => {
            let text = native
                .as_str()
                .ok_or_else(|| codec_error(path, "expected decimal text"))?;
            let unsigned = text.strip_prefix('-').unwrap_or(text);
            let parts: Vec<_> = unsigned.split('.').collect();
            if text.len() > 65536
                || parts.len() > 2
                || parts
                    .iter()
                    .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
            {
                return Err(codec_error(path, "invalid exact decimal text"));
            };
            Value::String(text.into())
        }
        "float" => {
            let n = if decoding {
                native.as_str().and_then(|s| s.parse::<f64>().ok())
            } else {
                native.as_f64()
            }
            .filter(|n| n.is_finite())
            .ok_or_else(|| codec_error(path, "expected finite Float"))?;
            if decoding {
                json!(n)
            } else {
                let original = native.to_string();
                if original
                    .strip_prefix('-')
                    .unwrap_or(&original)
                    .bytes()
                    .all(|b| b.is_ascii_digit())
                    && format!("{n:.0}") != original
                {
                    return Err(codec_error(
                        path,
                        "integer cannot be represented exactly as Float",
                    ));
                }
                Value::String(n.to_string())
            }
        }
        "record" => {
            let fields = native
                .as_object()
                .ok_or_else(|| codec_error(path, "expected record fields"))?;
            let declared = declaration.and_then(|d| d["fields"].as_array());
            if !input {
                let declared =
                    declared.ok_or_else(|| codec_error(path, "record descriptor missing"))?;
                if fields.len() != declared.len()
                    || declared
                        .iter()
                        .any(|f| !fields.contains_key(f["name"].as_str().unwrap_or("")))
                {
                    return Err(codec_error(path, "record fields differ"));
                }
            }
            let mut out = serde_json::Map::new();
            for (key, v) in fields {
                let hint = declared
                    .and_then(|fields| fields.iter().find(|f| f["name"] == key.as_str()))
                    .map(|f| f["ty"].clone())
                    .unwrap_or_else(|| fallback(v));
                out.insert(key.clone(), child(&hint, v, &format!(".{key}"))?);
            }
            Value::Object(out)
        }
        "list" => {
            let values = native
                .as_array()
                .ok_or_else(|| codec_error(path, "expected list"))?;
            let mut out = vec![];
            for (i, v) in values.iter().enumerate() {
                let hint = if name == "Bytes" {
                    json!({"name":"Int"})
                } else {
                    ty["arguments"]
                        .get(0)
                        .cloned()
                        .unwrap_or_else(|| fallback(v))
                };
                let n = child(&hint, v, &format!("[{i}]"))?;
                if name == "Bytes" && !input {
                    let byte = if decoding {
                        n.as_u64()
                    } else {
                        n["value"].as_str().and_then(|s| s.parse::<u64>().ok())
                    };
                    if !byte.is_some_and(|n| n <= 255) {
                        return Err(codec_error(path, "byte outside 0..255"));
                    }
                }
                out.push(n)
            }
            Value::Array(out)
        }
        "option" => {
            if native.is_null() {
                if name == "Some" && !input {
                    return Err(codec_error(path, "expected present option"));
                }
                Value::Null
            } else {
                if name == "None" && !input {
                    return Err(codec_error(path, "expected absent option"));
                };
                let hint = ty["arguments"]
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| fallback(native));
                let n = child(&hint, native, ".Some")?;
                if decoding && n.is_null() {
                    return Err(codec_error(path, "present option collapses into absence"));
                };
                n
            }
        }
        "variant" => {
            if id.is_none() || variants.is_none_or(|v| v.is_empty()) {
                return Err(codec_error(
                    path,
                    "unexpected variant needs an explicit native mapping",
                ));
            }
            let variants = variants.map(Vec::as_slice).unwrap_or(&[]);
            let tagged = variants.iter().any(|v| !v["payload"].is_null())
                || mode.unwrap_or(config["enum"].as_str().unwrap_or("string")) == "tagged";
            if decoding && id.is_some_and(|id| native["declaration"] != id) {
                return Err(codec_error(path, "foreign variant declaration"));
            }
            let alt = if decoding {
                native["name"].as_str()
            } else if tagged {
                native["kind"].as_str()
            } else {
                native.as_str()
            }
            .ok_or_else(|| codec_error(path, "expected alternative name"))?;
            let variant = variants.iter().find(|v| v["name"] == alt);
            if !input && variant.is_none() {
                return Err(codec_error(path, "unknown alternative"));
            }
            let payload = &native["payload"];
            let hint = variant.map(|v| &v["payload"]).filter(|v| !v.is_null());
            if !decoding
                && tagged
                && native
                    .as_object()
                    .is_none_or(|o| o.len() != 2 || !o.contains_key("payload"))
            {
                return Err(codec_error(path, "expected kind and payload fields"));
            }
            let converted = if !decoding && hint.is_some() {
                child(hint.expect("checked hint"), payload, &format!(".{alt}"))?
            } else if payload.is_null() {
                if hint.is_some() && !input {
                    return Err(codec_error(path, "missing variant payload"));
                };
                Value::Null
            } else {
                if !tagged {
                    return Err(codec_error(
                        path,
                        "payload cannot be represented as string enum",
                    ));
                };
                let hint = match hint {
                    Some(h) => h.clone(),
                    None if input => fallback(payload),
                    _ => return Err(codec_error(path, "unexpected variant payload")),
                };
                child(&hint, payload, &format!(".{alt}"))?
            };
            if decoding {
                if tagged {
                    json!({"kind":alt,"payload":converted})
                } else {
                    json!(alt)
                }
            } else {
                return Ok(
                    json!({"kind":"variant","value":{"declaration":id,"name":alt,"payload":converted}}),
                );
            }
        }
        _ => return Err(codec_error(path, &format!("unsupported wire kind {kind}"))),
    };
    if decoding {
        Ok(result)
    } else if kind == "unit" {
        Ok(json!({"kind":"unit"}))
    } else {
        Ok(json!({"kind":kind,"value":result}))
    }
}
fn fallback(wire: &Value) -> Value {
    json!({"name":match wire["kind"].as_str(){Some("integer")=>"Int",Some("decimal")=>"Decimal",Some("float")=>"Float",Some("boolean")=>"Boolean",Some("list")=>"List",Some("option")=>"Option",Some("unit")=>"Unit",_=>"String"}})
}
// Assertions deliberately accept technical Result errors separately from observations.
pub fn returned_record(
    observation: Result<&Value, &crate::contract::TechnicalError>,
) -> Result<Value, String> {
    let o = observation.map_err(|_| "expected returned record; technical failure".to_owned())?;
    if o["kind"] != "returned" || o["value"]["kind"] != "record" {
        return Err("expected returned record".into());
    }
    o["value"]["value"]
        .as_object()
        .ok_or("missing record fields".into())
        .map(|v| Value::Object(v.clone()))
}
pub fn declared_failure(
    observation: Result<&Value, &crate::contract::TechnicalError>,
    declaration: &str,
) -> Result<Value, String> {
    let o = observation
        .map_err(|_| format!("expected declared failure {declaration}; technical failure"))?;
    if o["kind"] != "returned" || o["value"]["kind"] != "failure" {
        return Err(format!("expected declared failure {declaration}"));
    }
    let f = &o["value"]["value"];
    if f["declaration"] != declaration {
        return Err(format!("wrong declared failure; expected {declaration}"));
    }
    if f["payload"].is_null() {
        let data = descriptor();
        let d = data.get(declaration).ok_or("unknown failure declaration")?;
        if !d["alias"].is_null()
            || d["fields"].as_array().is_none_or(|f| !f.is_empty())
            || d["variants"].as_array().is_none_or(|v| !v.is_empty())
        {
            return Err("missing failure payload".into());
        }
    } else {
        decode(declaration, &f["payload"])?;
    }
    Ok(f.clone())
}
pub fn string_field(record: &Value, name: &str, expected: &str) -> Result<(), String> {
    if record["kind"] != "record" || record["value"][name]["kind"] != "string" {
        return Err(format!("expected string field {name} on record"));
    }
    if record["value"][name]["value"] != expected {
        return Err(format!("string field {name} differs from expected value"));
    }
    Ok(())
}

pub fn technical_failure(
    error: &crate::contract::TechnicalError,
    category: &str,
) -> Result<(), String> {
    if error.cause.category() != category {
        return Err(format!("expected technical failure category {category}"));
    }
    Ok(())
}
pub fn unavailable_observation(
    observation: Result<&Value, &crate::contract::TechnicalError>,
) -> Result<(), String> {
    let o = observation.map_err(|_| "expected unavailable observation; technical failure")?;
    if o["kind"] != "unavailable" || o["reason"].as_str().is_none_or(str::is_empty) {
        return Err("expected unavailable observation with a reason".into());
    }
    Ok(())
}
pub fn recorded_calls(
    calls: Option<&[Value]>,
    invocation: &Value,
    count: usize,
) -> Result<(), String> {
    let calls = calls.ok_or("actual collaborator observations unavailable")?;
    let found = calls
        .iter()
        .filter(|c| c["invocation"] == *invocation)
        .count();
    if found != count {
        return Err(format!(
            "expected {count} actual collaborator calls; received {found}"
        ));
    }
    Ok(())
}
