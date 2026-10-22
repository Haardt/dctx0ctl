//! Skip embedded run bodies while parsing: exported aggregates can be hundreds of MB.
use super::*;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::{cell::Cell, collections::BTreeMap, rc::Rc};

#[derive(Debug, Serialize)]
pub struct ExportGcResult {
    pub path: PathBuf,
    pub status: String,
    pub bytes_before: u64,
    pub bytes_after: u64,
}
#[derive(Clone)]
struct Reduce<'a> {
    summaries: &'a BTreeMap<String, VerificationRunSummary>,
    changed: Rc<Cell<bool>>,
}
impl<'de> DeserializeSeed<'de> for Reduce<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        de: D,
    ) -> std::result::Result<Value, D::Error> {
        de.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Reduce<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a verification report")
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> std::result::Result<Value, E> {
        Ok(v.into())
    }
    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(self.clone())? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut items = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let summary = items
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| self.summaries.get(id));
            if key == "run"
                && let Some(summary) = summary
            {
                map.next_value::<IgnoredAny>()?;
                items.insert(
                    "summary".into(),
                    serde_json::to_value(summary).map_err(serde::de::Error::custom)?,
                );
                items.insert("report_kind".into(), "verification-reference".into());
                self.changed.set(true);
            } else {
                items.insert(key, map.next_value_seed(self.clone())?);
            }
        }
        Ok(Value::Object(items))
    }
}
fn files(root: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            files(&entry.path(), result)?;
        } else if ty.is_file() && entry.path().extension().is_some_and(|s| s == "json") {
            result.push(entry.path());
        }
    }
    Ok(())
}
pub(super) fn clean(
    project: &Project,
    root: Option<&Path>,
    dry_run: bool,
) -> Result<Vec<ExportGcResult>> {
    let Some(root) = root else {
        return Ok(vec![]);
    };
    validate(Some(root))?;
    let mut summaries = BTreeMap::new();
    let mut offset = 0;
    loop {
        let page = project.verification_history(offset, 200)?;
        let count = page.len();
        summaries.extend(page.into_iter().map(|s| (s.id.clone(), s)));
        if count < 200 {
            break;
        }
        offset += 200;
    }
    let mut paths = Vec::new();
    files(root, &mut paths)?;
    paths.sort();
    let mut results = Vec::new();
    for path in paths {
        let before = fs::metadata(&path)?;
        let changed = Rc::new(Cell::new(false));
        let seed = Reduce {
            summaries: &summaries,
            changed: changed.clone(),
        };
        let mut de = serde_json::Deserializer::from_reader(BufReader::new(File::open(&path)?));
        let parsed = seed.deserialize(&mut de).and_then(|value| {
            de.end()?;
            Ok(value)
        });
        let (status, after) = match parsed {
            Ok(value) if changed.get() => {
                if fs::metadata(&path)?.modified()? != before.modified()? {
                    return Err(Error::Invalid("Export changed during gc".into()));
                }
                // After reduction this contains metadata, not the original embedded case corpus.
                let mut bytes = serde_json::to_vec_pretty(&value)?;
                bytes.push(b'\n');
                if !dry_run {
                    atomic_write(&path, &bytes)?;
                }
                (
                    if dry_run { "would-reduce" } else { "reduced" }.to_string(),
                    bytes.len() as u64,
                )
            }
            Ok(_) => ("unchanged: no recognized embedded run".into(), before.len()),
            Err(error) => (format!("skipped: {error}"), before.len()),
        };
        results.push(ExportGcResult {
            path,
            status,
            bytes_before: before.len(),
            bytes_after: after,
        });
    }
    Ok(results)
}
pub(super) fn validate(root: Option<&Path>) -> Result<()> {
    if let Some(root) = root
        && !fs::symlink_metadata(root)?.is_dir()
    {
        return Err(Error::Invalid(
            "Export directory must be an actual directory, not a symlink".into(),
        ));
    }
    Ok(())
}
