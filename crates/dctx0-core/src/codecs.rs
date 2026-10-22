//! Explicit native representations for editable testing codecs; no execution evidence.
use crate::{DataIr, TypeRef, WorkspaceIr};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodecRepresentation {
    Number,
    Bigint,
    Null,
    Undefined,
    String,
    Tagged,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodecOptions {
    pub schema_version: u32,
    pub integer: CodecRepresentation,
    pub option: CodecRepresentation,
    pub r#enum: CodecRepresentation,
    #[serde(deserialize_with = "unique_types")]
    pub types: BTreeMap<String, CodecRepresentation>,
}
impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            schema_version: 1,
            integer: CodecRepresentation::Number,
            option: CodecRepresentation::Null,
            r#enum: CodecRepresentation::String,
            types: BTreeMap::new(),
        }
    }
}
impl CodecOptions {
    pub fn validate(&self, declarations: &BTreeMap<String, DataIr>) -> Result<(), String> {
        use CodecRepresentation as R;
        if self.schema_version != 1
            || !matches!(self.integer, R::Number | R::Bigint)
            || !matches!(self.option, R::Null | R::Undefined)
            || !matches!(self.r#enum, R::String | R::Tagged)
        {
            return Err("Unsupported codec configuration version or default representation".into());
        }
        for (name, representation) in &self.types {
            let mut seen = BTreeSet::new();
            let mut current = name.as_str();
            let valid = loop {
                if !seen.insert(current) {
                    break false;
                }
                let data = declarations
                    .get(current)
                    .ok_or_else(|| format!("Unknown or unselected codec type: {current}"))?;
                if let Some(alias) = &data.alias {
                    if let Some(symbol) = &alias.symbol_id {
                        current = symbol;
                        continue;
                    }
                    break match alias.name.as_str() {
                        "Int" => matches!(representation, R::Number | R::Bigint),
                        "Option" | "Dctx0Option" => {
                            matches!(representation, R::Null | R::Undefined)
                        }
                        _ => false,
                    };
                }
                break !data.variants.is_empty()
                    && data
                        .variants
                        .iter()
                        .all(|variant| variant.payload.is_none())
                    && matches!(representation, R::String | R::Tagged);
            };
            if !valid {
                return Err(format!("Inappropriate codec representation for {name}"));
            }
        }
        Ok(())
    }
}

/// Select exact declarations or namespace descendants and their complete type closure.
pub fn codec_declarations(
    ir: &WorkspaceIr,
    subjects: &BTreeSet<String>,
) -> Result<BTreeMap<String, DataIr>, String> {
    let mut pending = BTreeSet::new();
    if subjects.is_empty() {
        pending.extend(ir.data.keys().cloned());
    }
    for subject in subjects {
        let selected: Vec<_> = ir
            .data
            .keys()
            .filter(|id| *id == subject || id.starts_with(&format!("{subject}.")))
            .cloned()
            .collect();
        if selected.is_empty() {
            return Err(format!("No codec data declarations selected by {subject}"));
        }
        pending.extend(selected);
    }
    let mut selected = BTreeMap::new();
    while let Some(id) = pending.pop_first() {
        if selected.contains_key(&id) {
            continue;
        }
        let data = ir
            .data
            .get(&id)
            .ok_or_else(|| format!("Missing codec data declaration: {id}"))?;
        for ty in data
            .alias
            .iter()
            .chain(data.fields.iter().map(|field| &field.ty))
            .chain(
                data.variants
                    .iter()
                    .filter_map(|variant| variant.payload.as_ref()),
            )
        {
            collect_types(ty, &mut pending);
        }
        selected.insert(id, data.clone());
    }
    Ok(selected)
}
fn collect_types(ty: &TypeRef, pending: &mut BTreeSet<String>) {
    if let Some(id) = &ty.symbol_id {
        pending.insert(id.clone());
    }
    for argument in &ty.arguments {
        collect_types(argument, pending);
    }
}

fn unique_types<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, CodecRepresentation>, D::Error> {
    struct UniqueTypes;
    impl<'de> serde::de::Visitor<'de> for UniqueTypes {
        type Value = BTreeMap<String, CodecRepresentation>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map of unique qualified type overrides")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut types = BTreeMap::new();
            while let Some((name, representation)) =
                map.next_entry::<String, CodecRepresentation>()?
            {
                if types.insert(name.clone(), representation).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "Duplicate codec type override: {name}"
                    )));
                }
            }
            Ok(types)
        }
    }
    deserializer.deserialize_map(UniqueTypes)
}
