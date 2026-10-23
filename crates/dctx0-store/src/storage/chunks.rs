//! Content-defined byte chunks preserve exact report strings, including whitespace.
use super::*;
pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS storage_chunks(id INTEGER PRIMARY KEY,hash BLOB NOT NULL UNIQUE,encoding TEXT NOT NULL,raw_length INTEGER NOT NULL,bytes BLOB NOT NULL); CREATE TABLE IF NOT EXISTS storage_chunk_refs(parent INTEGER NOT NULL REFERENCES storage_objects(id),chunk INTEGER NOT NULL REFERENCES storage_chunks(id),PRIMARY KEY(parent,chunk)); CREATE INDEX IF NOT EXISTS storage_chunk_users ON storage_chunk_refs(chunk);";
pub(super) fn store(c: &Connection, text: &str) -> Result<Vec<i64>> {
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut rolling = 0u64;
    let mut ids = Vec::new();
    for (position, byte) in bytes.iter().enumerate() {
        rolling = rolling
            .wrapping_shl(1)
            .wrapping_add((u64::from(*byte) + 1).wrapping_mul(0x9e3779b185ebca87));
        let end = position + 1;
        let length = end - start;
        if end == bytes.len() || length >= 32768 || (length >= 2048 && rolling & 8191 == 0) {
            let part = &bytes[start..end];
            let hash = Sha256::digest(part).to_vec();
            let old: Option<i64> = c
                .prepare_cached("SELECT id FROM storage_chunks WHERE hash=?1")?
                .query_row([&hash], |r| r.get(0))
                .optional()?;
            let id = if let Some(id) = old {
                id
            } else {
                let (encoding, encoded) = verification::codec::encode(part)?;
                c.execute("INSERT INTO storage_chunks(hash,encoding,raw_length,bytes) VALUES(?1,?2,?3,?4)",params![hash,encoding,part.len() as i64,encoded])?;
                c.last_insert_rowid()
            };
            ids.push(id);
            start = end;
            rolling = 0;
        }
    }
    Ok(ids)
}
pub(super) fn load(c: &Connection, root: i64, objects: &mut BTreeMap<i64, Stored>) -> Result<()> {
    if !objects
        .values()
        .any(|stored| matches!(stored.node, Node::Text(_)))
    {
        return Ok(());
    }
    let mut chunks = BTreeMap::new();
    let mut q=c.prepare("WITH RECURSIVE needed(id) AS (VALUES(?1) UNION SELECT e.child FROM storage_edges e JOIN needed n ON e.parent=n.id) SELECT DISTINCT b.id,b.hash,b.encoding,b.raw_length,b.bytes FROM needed n JOIN storage_chunk_refs r ON r.parent=n.id JOIN storage_chunks b ON b.id=r.chunk")?;
    let mut rows = q.query([root])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let hash: Vec<u8> = row.get(1)?;
        let encoding: String = row.get(2)?;
        let length: i64 = row.get(3)?;
        let bytes = verification::codec::decode_bytes(&encoding, row.get(4)?, Some(length))?;
        if Sha256::digest(&bytes).as_slice() != hash {
            return Err(invalid("Stored text chunk hash mismatch"));
        }
        chunks.insert(id, bytes);
    }
    for stored in objects.values_mut() {
        if let Node::Text(ids) = &stored.node {
            let mut bytes = Vec::new();
            for id in ids {
                bytes.extend_from_slice(
                    chunks
                        .get(id)
                        .ok_or_else(|| invalid("Missing stored text chunk"))?,
                );
            }
            stored.node = Node::Value(Value::String(
                String::from_utf8(bytes).map_err(|_| invalid("Invalid stored text UTF-8"))?,
            ));
        }
    }
    Ok(())
}
