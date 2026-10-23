//! Shared public spec metadata; source excerpts are stored once and stay searchable.
use crate::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
pub(crate) fn project(c: &Connection, snapshot: &Snapshot) -> Result<()> {
    let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM storage_snapshot_keys k JOIN query_snapshots q ON q.snapshot=k.snapshot WHERE k.snapshot=?1)",[&snapshot.id],|r|r.get(0))?;
    if exists {
        return Ok(());
    }
    c.execute(
        "INSERT OR IGNORE INTO storage_snapshot_keys(snapshot) VALUES(?1)",
        [&snapshot.id],
    )?;
    let key: i64 = c.query_row(
        "SELECT id FROM storage_snapshot_keys WHERE snapshot=?1",
        [&snapshot.id],
        |r| r.get(0),
    )?;
    for source in &snapshot.sources {
        c.execute(
            "INSERT OR IGNORE INTO storage_spec_sources VALUES(?1,?2,?3)",
            params![
                key,
                source.file,
                format!("{:x}", Sha256::digest(source.text.as_bytes()))
            ],
        )?;
    }
    for node in &snapshot.ir.nodes {
        let text = snapshot
            .sources
            .iter()
            .find(|s| s.file == node.span.file)
            .and_then(|s| s.text.get(node.span.start.offset..node.span.end.offset))
            .unwrap_or("");
        let hash = Sha256::digest(text.as_bytes()).to_vec();
        c.execute(
            "INSERT OR IGNORE INTO storage_spec_texts(hash,text) VALUES(?1,?2)",
            params![hash, text],
        )?;
        let text_id: i64 = c.query_row(
            "SELECT id FROM storage_spec_texts WHERE hash=?1",
            [hash],
            |r| r.get(0),
        )?;
        let kind = serde_json::to_value(node.kind)?
            .as_str()
            .ok_or_else(|| Error::Verification("Invalid spec kind".into()))?
            .to_owned();
        let identity = serde_json::to_vec(&(
            &node.id,
            &node.owner,
            &kind,
            text_id,
            &node.span.file,
            node.span.start.line,
            node.span.end.line,
        ))?;
        let hash = Sha256::digest(identity).to_vec();
        c.execute("INSERT OR IGNORE INTO storage_spec_nodes(hash,subject,owner,kind,text_id,file,start_line,end_line) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![hash,node.id,node.owner,kind,text_id,node.span.file,node.span.start.line as i64,node.span.end.line as i64])?;
        let id: i64 = c.query_row(
            "SELECT id FROM storage_spec_nodes WHERE hash=?1",
            [hash],
            |r| r.get(0),
        )?;
        c.execute(
            "INSERT OR IGNORE INTO storage_spec_members VALUES(?1,?2)",
            params![key, id],
        )?;
    }
    for edge in &snapshot.graph.edges {
        let value = serde_json::to_value(edge.kind)?;
        let kind = value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string());
        c.execute(
            "INSERT OR IGNORE INTO storage_spec_relations(source,target,kind) VALUES(?1,?2,?3)",
            params![edge.from, edge.to, kind],
        )?;
        let id: i64 = c.query_row(
            "SELECT id FROM storage_spec_relations WHERE source=?1 AND target=?2 AND kind=?3",
            params![edge.from, edge.to, kind],
            |r| r.get(0),
        )?;
        c.execute(
            "INSERT OR IGNORE INTO storage_relation_members VALUES(?1,?2)",
            params![key, id],
        )?;
    }
    c.execute(
        "INSERT OR REPLACE INTO query_snapshots VALUES(?1,1)",
        [&snapshot.id],
    )?;
    c.execute(
        "DELETE FROM query_spec_rows WHERE snapshot=?1",
        [&snapshot.id],
    )?;
    Ok(())
}
pub(crate) fn populate(
    source: &Connection,
    target: &Connection,
    snapshot: &str,
    needed: &BTreeSet<String>,
) -> Result<()> {
    let key: i64 = source.query_row(
        "SELECT id FROM storage_snapshot_keys WHERE snapshot=?1",
        [snapshot],
        |r| r.get(0),
    )?;
    if needed.iter().any(|v| {
        matches!(
            v.as_str(),
            "dctx0_subjects" | "dctx0_facts" | "dctx0_source_refs"
        )
    }) {
        let mut q=source.prepare("SELECT n.subject,n.owner,n.kind,t.text,n.file,n.start_line,n.end_line,s.hash FROM storage_spec_members m JOIN storage_spec_nodes n ON n.id=m.node JOIN storage_spec_texts t ON t.id=n.text_id LEFT JOIN storage_spec_sources s ON s.snapshot=m.snapshot AND s.file=n.file WHERE m.snapshot=?1 ORDER BY n.id")?;
        let mut rows = q.query([key])?;
        while let Some(r) = rows.next()? {
            let subject: String = r.get(0)?;
            let owner: Option<String> = r.get(1)?;
            let kind: String = r.get(2)?;
            let text: String = r.get(3)?;
            if needed.contains("dctx0_subjects") {
                query::put(
                    target,
                    "dctx0_subjects",
                    json!({"snapshot":snapshot,"subject":subject,"kind":kind,"text":text}),
                )?;
            }
            if needed.contains("dctx0_facts") {
                query::put(
                    target,
                    "dctx0_facts",
                    json!({"snapshot":snapshot,"subject":subject,"owner":owner,"kind":kind,"text":text}),
                )?;
            }
            if needed.contains("dctx0_source_refs") {
                query::put(
                    target,
                    "dctx0_source_refs",
                    json!({"snapshot":snapshot,"subject":subject,"origin":"pinned-spec","file":r.get::<_,String>(4)?,"start_line":r.get::<_,i64>(5)?,"end_line":r.get::<_,i64>(6)?,"source_hash":r.get::<_,Option<String>>(7)?}),
                )?;
            }
        }
    }
    if needed.contains("dctx0_relations") {
        let mut q=source.prepare("SELECT r.source,r.target,r.kind FROM storage_relation_members m JOIN storage_spec_relations r ON r.id=m.relation WHERE m.snapshot=?1 ORDER BY r.id")?;
        for row in q.query_map([key],|r|Ok(json!({"snapshot":snapshot,"source":r.get::<_,String>(0)?,"target":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?})))? {query::put(target,"dctx0_relations",row?)?;}
    }
    Ok(())
}
