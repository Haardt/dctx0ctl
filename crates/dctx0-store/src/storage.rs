//! Versioned, content-addressed JSON objects. Public serialization is unchanged.
use crate::*;
pub(crate) mod chunks;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MIGRATION: &str = "
CREATE TABLE storage_objects(id INTEGER PRIMARY KEY,hash BLOB NOT NULL UNIQUE,encoding TEXT NOT NULL,raw_length INTEGER NOT NULL,encoded_length INTEGER NOT NULL,bytes BLOB NOT NULL);
CREATE TABLE storage_edges(parent INTEGER NOT NULL REFERENCES storage_objects(id),child INTEGER NOT NULL REFERENCES storage_objects(id),PRIMARY KEY(parent,child));
CREATE INDEX storage_children ON storage_edges(child);
CREATE TABLE storage_state_metadata(event_seq INTEGER PRIMARY KEY REFERENCES task_events(seq),task_id INTEGER NOT NULL REFERENCES tasks(id),data TEXT NOT NULL);
CREATE INDEX storage_state_history ON storage_state_metadata(task_id,event_seq);
CREATE TABLE storage_evidence_metadata(event_seq INTEGER NOT NULL REFERENCES task_events(seq),task_id INTEGER NOT NULL REFERENCES tasks(id),ordinal INTEGER NOT NULL,data TEXT NOT NULL,PRIMARY KEY(event_seq,ordinal));
CREATE TABLE verification_case_index_v7(run_id TEXT NOT NULL REFERENCES verification_runs(id),case_id TEXT NOT NULL,ordinal INTEGER NOT NULL,subject TEXT NOT NULL,kind TEXT NOT NULL,outcome TEXT NOT NULL,issue_code TEXT,observations INTEGER NOT NULL,body_hash TEXT NOT NULL,metadata TEXT NOT NULL,PRIMARY KEY(run_id,case_id),UNIQUE(run_id,ordinal));
INSERT INTO verification_case_index_v7 SELECT * FROM verification_case_index;
DROP TABLE verification_case_index;
ALTER TABLE verification_case_index_v7 RENAME TO verification_case_index;
CREATE INDEX verification_cases_subject ON verification_case_index(run_id,subject,outcome);
CREATE TABLE storage_snapshot_keys(id INTEGER PRIMARY KEY,snapshot TEXT NOT NULL UNIQUE REFERENCES snapshots(id));
CREATE TABLE storage_spec_texts(id INTEGER PRIMARY KEY,hash BLOB NOT NULL UNIQUE,text TEXT NOT NULL);
CREATE TABLE storage_spec_nodes(id INTEGER PRIMARY KEY,hash BLOB NOT NULL UNIQUE,subject TEXT NOT NULL,owner TEXT,kind TEXT NOT NULL,text_id INTEGER NOT NULL REFERENCES storage_spec_texts(id),file TEXT NOT NULL,start_line INTEGER NOT NULL,end_line INTEGER NOT NULL);
CREATE TABLE storage_spec_members(snapshot INTEGER NOT NULL REFERENCES storage_snapshot_keys(id),node INTEGER NOT NULL REFERENCES storage_spec_nodes(id),PRIMARY KEY(snapshot,node));
CREATE TABLE storage_spec_sources(snapshot INTEGER NOT NULL REFERENCES storage_snapshot_keys(id),file TEXT NOT NULL,hash TEXT NOT NULL,PRIMARY KEY(snapshot,file));
CREATE TABLE storage_spec_relations(id INTEGER PRIMARY KEY,source TEXT NOT NULL,target TEXT NOT NULL,kind TEXT NOT NULL,UNIQUE(source,target,kind));
CREATE TABLE storage_relation_members(snapshot INTEGER NOT NULL REFERENCES storage_snapshot_keys(id),relation INTEGER NOT NULL REFERENCES storage_spec_relations(id),PRIMARY KEY(snapshot,relation));
CREATE TABLE IF NOT EXISTS verification_run_parents(child TEXT NOT NULL REFERENCES verification_runs(id),parent TEXT NOT NULL REFERENCES verification_runs(id),PRIMARY KEY(child,parent));
CREATE TABLE storage_conversion(id INTEGER PRIMARY KEY CHECK(id=1),source_signature TEXT NOT NULL,status TEXT NOT NULL);
PRAGMA user_version=7;";
pub(crate) const PARENT_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS verification_run_parents(child TEXT NOT NULL REFERENCES verification_runs(id),parent TEXT NOT NULL REFERENCES verification_runs(id),PRIMARY KEY(child,parent));";
const PREFIX: &str = "@dctx0:7:";
#[derive(Clone, Serialize, Deserialize)]
enum Part {
    Inline(Value),
    Object(i64),
}
#[derive(Clone, Serialize, Deserialize)]
enum Node {
    Value(Value),
    Array(Vec<Part>),
    Map(BTreeMap<String, Part>),
    Text(Vec<i64>),
}
// A fork briefly inherits file descriptions until exec closes them. Bounded
// waiting tolerates that transient owner without replacing a live reader's file.
pub(crate) fn lock_exclusive(file: &std::fs::File) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
    loop {
        match fs2::FileExt::try_lock_exclusive(file) {
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            result => return result,
        }
    }
}
fn invalid(message: impl Into<String>) -> Error {
    Error::Verification(message.into())
}
pub(crate) fn version(c: &Connection) -> Result<i64> {
    Ok(c.pragma_query_value(None, "user_version", |r| r.get(0))?)
}
fn part(c: &Connection, value: Value, refs: &mut BTreeSet<i64>) -> Result<Part> {
    // Plan members recur across immutable plans even when their candidate is small.
    let threshold = if (value.get("candidate").is_some() && value.get("task_id").is_some())
        || (value.get("requirements_hash").is_some() && value.get("work_order_hash").is_some())
    {
        512
    } else {
        4096
    };
    if serde_json::to_vec(&value)?.len() < threshold {
        return Ok(Part::Inline(value));
    }
    let id = object(c, value)?;
    refs.insert(id);
    Ok(Part::Object(id))
}
fn object(c: &Connection, value: Value) -> Result<i64> {
    let original = serde_json::to_vec(&value)?;
    let hash = Sha256::digest(&original).to_vec();
    if let Some(id) = c
        .query_row(
            "SELECT id FROM storage_objects WHERE hash=?1",
            [&hash],
            |r| r.get(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    let mut refs = BTreeSet::new();
    let node = if original.len() >= 32768 || value.get("candidate").is_some() {
        match value {
            Value::Array(values) => Node::Array(
                values
                    .into_iter()
                    .map(|v| part(c, v, &mut refs))
                    .collect::<Result<_>>()?,
            ),
            Value::Object(values) => Node::Map(
                values
                    .into_iter()
                    .map(|(k, v)| Ok((k, part(c, v, &mut refs)?)))
                    .collect::<Result<_>>()?,
            ),
            Value::String(text) => Node::Text(chunks::store(c, &text)?),
            value => Node::Value(value),
        }
    } else {
        Node::Value(value)
    };
    let encoded = serde_json::to_vec(&node)?;
    let (encoding, bytes) = verification::codec::encode(&encoded)?;
    c.execute("INSERT INTO storage_objects(hash,encoding,raw_length,encoded_length,bytes) VALUES(?1,?2,?3,?4,?5)", params![hash,encoding,original.len() as i64,encoded.len() as i64,bytes])?;
    let id = c.last_insert_rowid();
    if let Node::Text(chunks) = node {
        for chunk in chunks.into_iter().collect::<BTreeSet<_>>() {
            c.execute(
                "INSERT INTO storage_chunk_refs VALUES(?1,?2)",
                params![id, chunk],
            )?;
        }
    }
    for child in refs {
        c.execute(
            "INSERT INTO storage_edges VALUES(?1,?2)",
            params![id, child],
        )?;
    }
    Ok(id)
}
pub(crate) fn store(c: &Connection, value: &impl Serialize) -> Result<String> {
    let raw = serde_json::to_string(value)?;
    if version(c)? < 7 || raw.len() < 4096 {
        return Ok(raw);
    }
    Ok(format!("{PREFIX}{}", object(c, decode(&raw)?)?))
}
struct Stored {
    hash: Vec<u8>,
    raw_length: i64,
    node: Node,
}
fn expand_part(
    part: Part,
    objects: &mut BTreeMap<i64, Stored>,
    active: &mut BTreeSet<i64>,
) -> Result<Value> {
    match part {
        Part::Inline(value) => Ok(value),
        Part::Object(id) => expand(id, objects, active),
    }
}
fn expand(
    id: i64,
    objects: &mut BTreeMap<i64, Stored>,
    active: &mut BTreeSet<i64>,
) -> Result<Value> {
    if active.len() >= 256 || !active.insert(id) {
        return Err(invalid("Cyclic or excessively nested storage object"));
    }
    let stored = objects
        .get(&id)
        .ok_or_else(|| invalid("Missing storage object"))?;
    // Nodes can be shared in a DAG; retain them for other parents.
    let node = stored.node.clone();
    let hash = stored.hash.clone();
    let length = stored.raw_length;
    let value = match node {
        Node::Value(v) => v,
        Node::Text(_) => return Err(invalid("Text chunks were not loaded")),
        Node::Array(parts) => Value::Array(
            parts
                .into_iter()
                .map(|p| expand_part(p, objects, active))
                .collect::<Result<_>>()?,
        ),
        Node::Map(parts) => Value::Object(
            parts
                .into_iter()
                .map(|(k, p)| Ok((k, expand_part(p, objects, active)?)))
                .collect::<Result<_>>()?,
        ),
    };
    active.remove(&id);
    let raw = serde_json::to_vec(&value)?;
    if raw.len() as i64 != length || Sha256::digest(&raw).as_slice() != hash {
        return Err(invalid("Storage object hash or length mismatch"));
    }
    Ok(value)
}
pub(crate) fn load<T: serde::de::DeserializeOwned>(c: &Connection, raw: &str) -> Result<T> {
    let Some(id) = raw.strip_prefix(PREFIX) else {
        return Ok(decode(raw)?);
    };
    let id: i64 = id
        .parse()
        .map_err(|_| invalid("Invalid storage object reference"))?;
    let mut stmt=c.prepare("WITH RECURSIVE needed(id) AS (VALUES(?1) UNION SELECT e.child FROM storage_edges e JOIN needed n ON e.parent=n.id) SELECT o.id,o.hash,o.encoding,o.raw_length,o.encoded_length,o.bytes FROM storage_objects o JOIN needed n USING(id)")?;
    let mut objects = BTreeMap::new();
    for row in stmt.query_map([id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Vec<u8>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Vec<u8>>(5)?,
        ))
    })? {
        let (id, hash, encoding, raw_length, length, bytes) = row?;
        let bytes = verification::codec::decode_bytes(&encoding, bytes, Some(length))?;
        objects.insert(
            id,
            Stored {
                hash,
                raw_length,
                node: decode(
                    std::str::from_utf8(&bytes).map_err(|_| invalid("Invalid storage UTF-8"))?,
                )?,
            },
        );
    }
    chunks::load(c, id, &mut objects)?;
    let value = expand(id, &mut objects, &mut BTreeSet::new())?;
    Ok(T::deserialize(serde_stacker::Deserializer::new(value))?)
}
pub(crate) fn state_metadata(
    c: &Connection,
    seq: i64,
    task: i64,
    state: &dctx0_tasks::State,
) -> Result<()> {
    if version(c)? < 7 {
        return Ok(());
    }
    let mut summary = state.clone();
    summary.evidence.clear();
    summary.reviews.clear();
    summary.execution_bundle = None;
    if let Some(result) = &mut summary.result {
        result.artifacts.clear();
        result.decisions.clear();
    }
    c.execute(
        "INSERT OR REPLACE INTO storage_state_metadata VALUES(?1,?2,?3)",
        params![seq, task, serde_json::to_string(&summary)?],
    )?;
    for (ordinal, evidence) in state.evidence.iter().enumerate() {
        let data = evidence_metadata(task, evidence);
        c.execute(
            "INSERT OR REPLACE INTO storage_evidence_metadata VALUES(?1,?2,?3,?4)",
            params![seq, task, ordinal as i64, data.to_string()],
        )?;
    }
    Ok(())
}

pub(crate) fn evidence_metadata(task: i64, evidence: &dctx0_tasks::CheckEvidence) -> Value {
    serde_json::json!({"task_id":task,"requirements_hash":evidence.requirements_hash,"implementation":evidence.implementation,"status":format!("{:?}",evidence.status).to_uppercase(),"verifier":evidence.verifier,"verifier_version":evidence.verifier_version,"started_at":evidence.started_at,"finished_at":evidence.finished_at,"report_hash":format!("{:x}",Sha256::digest(evidence.report.as_bytes())),"details_available":true})
}

#[cfg(test)]
mod tests;
