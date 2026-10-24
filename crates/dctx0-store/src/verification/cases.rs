//! Independently addressed immutable case bodies and metadata-only read paths.
use super::*;
use dctx0_core::VerificationCaseRecord;

pub(crate) const MIGRATION: &str = "
CREATE TABLE verification_case_bodies(hash TEXT PRIMARY KEY,bytes BLOB NOT NULL,raw_length INTEGER NOT NULL);
CREATE TABLE verification_case_projections(run_id TEXT PRIMARY KEY REFERENCES verification_runs(id),version INTEGER NOT NULL);
CREATE TABLE verification_case_index(run_id TEXT NOT NULL REFERENCES verification_runs(id),case_id TEXT NOT NULL,ordinal INTEGER NOT NULL,subject TEXT NOT NULL,kind TEXT NOT NULL,outcome TEXT NOT NULL,issue_code TEXT,observations INTEGER NOT NULL,body_hash TEXT NOT NULL REFERENCES verification_case_bodies(hash),metadata TEXT NOT NULL,PRIMARY KEY(run_id,case_id),UNIQUE(run_id,ordinal));
CREATE INDEX verification_cases_subject ON verification_case_index(run_id,subject,outcome);
CREATE TABLE verification_requirements(run_id TEXT NOT NULL REFERENCES verification_runs(id),case_id TEXT NOT NULL,PRIMARY KEY(run_id,case_id));
CREATE TABLE query_snapshots(snapshot TEXT PRIMARY KEY REFERENCES snapshots(id),version INTEGER NOT NULL);
CREATE TABLE query_spec_rows(snapshot TEXT NOT NULL REFERENCES snapshots(id),view TEXT NOT NULL,data TEXT NOT NULL);
CREATE INDEX query_spec_snapshot ON query_spec_rows(snapshot,view);
PRAGMA user_version=6;";

pub(crate) fn prepared(c: &Connection, id: &str) -> Result<bool> {
    Ok(c.query_row(
        "SELECT EXISTS(SELECT 1 FROM verification_case_projections WHERE run_id=?1 AND version=1)",
        [id],
        |r| r.get(0),
    )?)
}
pub(crate) fn ids(c: &Connection, id: &str) -> Result<Vec<String>> {
    Ok(
        c.prepare("SELECT case_id FROM verification_case_index WHERE run_id=?1 ORDER BY ordinal")?
            .query_map([id], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?,
    )
}
pub(crate) fn requirements(c: &Connection, id: &str) -> Result<BTreeSet<String>> {
    Ok(
        c.prepare(
            "SELECT case_id FROM verification_requirements WHERE run_id=?1 ORDER BY case_id",
        )?
        .query_map([id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?,
    )
}
pub(crate) fn read(c: &Connection, run: &str, id: &str) -> Result<VerificationCaseRecord> {
    let (hash,bytes,length): (String,Vec<u8>,i64) = c.query_row("SELECT b.hash,b.bytes,b.raw_length FROM verification_case_index i JOIN verification_case_bodies b ON b.hash=i.body_hash WHERE i.run_id=?1 AND i.case_id=?2",params![run,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    decode_record(&hash, bytes, length, id)
}
/// Full reconstruction uses one ordered join instead of one statement per case.
pub(crate) fn read_all(c: &Connection, run: &str) -> Result<Vec<VerificationCaseRecord>> {
    let mut statement=c.prepare("SELECT i.case_id,b.hash,b.bytes,b.raw_length FROM verification_case_index i JOIN verification_case_bodies b ON b.hash=i.body_hash WHERE i.run_id=?1 ORDER BY i.ordinal")?;
    let mut rows = statement.query([run])?;
    let mut cases = Vec::new();
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let hash: String = row.get(1)?;
        cases.push(decode_record(&hash, row.get(2)?, row.get(3)?, &id)?);
    }
    Ok(cases)
}
fn decode_record(
    hash: &str,
    bytes: Vec<u8>,
    length: i64,
    id: &str,
) -> Result<VerificationCaseRecord> {
    let bytes = codec::decode_bytes("gzip", bytes, Some(length))?;
    if format!("{:x}", Sha256::digest(&bytes)) != hash {
        return Err(invalid("Stored case digest is corrupt"));
    }
    let case: VerificationCaseRecord =
        decode(std::str::from_utf8(&bytes).map_err(|_| invalid("Invalid case UTF-8"))?)?;
    if case.id != id {
        return Err(invalid("Stored case association is corrupt"));
    }
    Ok(case)
}

pub(crate) fn store(c: &Connection, id: &str, run: &VerificationRun) -> Result<()> {
    if prepared(c, id)? {
        return Ok(());
    }
    for (ordinal, case) in run.cases.iter().enumerate() {
        let (hash, length, bytes) = codec::compress_json(case)?;
        let inserted = c.execute("INSERT OR IGNORE INTO verification_case_bodies(hash,bytes,raw_length) VALUES(?1,?2,?3)",params![hash,bytes,length as i64])?;
        if inserted == 0 {
            let (stored, raw): (Vec<u8>, i64) = c.query_row(
                "SELECT bytes,raw_length FROM verification_case_bodies WHERE hash=?1",
                [&hash],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if raw != length as i64 {
                return Err(invalid("Stored case length is corrupt"));
            }
            if stored != bytes {
                let raw = codec::decode_bytes("gzip", stored, Some(raw))?;
                if format!("{:x}", Sha256::digest(&raw)) != hash {
                    return Err(invalid("Stored case digest is corrupt"));
                }
            }
        }
        let kind = serde_json::to_value(&case.kind)?;
        let kind = kind
            .as_str()
            .ok_or_else(|| invalid("Case kind must be a wire string"))?;
        let (outcome, issue) = match &case.result {
            VerificationResult::Pass => ("PASS", None),
            VerificationResult::Fail { issue } => ("FAIL", Some(&issue.code)),
            VerificationResult::Unknown { issue } => ("UNKNOWN", Some(&issue.code)),
        };
        let metadata = VerificationCaseRecord {
            id: case.id.clone(),
            subject: case.subject.clone(),
            kind: case.kind.clone(),
            bindings: Default::default(),
            setup: vec![],
            arguments: vec![],
            observations: vec![],
            events: None,
            resources: None,
            states: vec![],
            result: case.result.clone(),
            elapsed_us: case.elapsed_us,
        };
        c.execute(
            "INSERT INTO verification_case_index VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                id,
                case.id,
                ordinal as i64,
                case.subject,
                kind,
                outcome,
                issue,
                case.observations.len() as i64,
                hash,
                serde_json::to_string(&metadata)?
            ],
        )?;
    }
    for required in &run.required_cases {
        c.execute(
            "INSERT INTO verification_requirements VALUES(?1,?2)",
            params![id, required],
        )?;
    }
    c.execute(
        "INSERT INTO verification_case_projections VALUES(?1,1)",
        [id],
    )?;
    Ok(())
}
impl Store {
    /// Explicit, resumable conversion. Each run is committed independently.
    pub fn prepare_verification_queries(&mut self, project: &str) -> Result<usize> {
        self.require_write()?;
        let ids=self.connection.prepare("SELECT id FROM verification_runs WHERE project=?1 AND pruned_at IS NULL ORDER BY rowid")?.query_map([project],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
        let mut count = 0;
        for id in ids {
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            if !prepared(&tx, &id)? {
                let run = codec::read_run(&tx, project, &id)?.run;
                if codec::json_hash(&run)? != id {
                    return Err(invalid("Run digest is corrupt"));
                }
                maintenance::store_body(&tx, &id, &run)?;
                tx.execute("DELETE FROM verification_corpora WHERE NOT EXISTS(SELECT 1 FROM verification_runs r WHERE r.corpus_hash=verification_corpora.hash)",[])?;
                count += 1;
            }
            tx.commit()?;
        }
        let full: i64 = self.connection.query_row(
            "SELECT count(*) FROM verification_runs WHERE project=?1 AND pruned_at IS NULL",
            [project],
            |r| r.get(0),
        )?;
        if full > 3 && crate::storage::version(&self.connection)? < 7 {
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            maintenance::prune(&tx, project)?;
            tx.commit()?;
        }
        Ok(count)
    }
    /// Metadata-only filtering; only an explicit exact-case request opens a body.
    pub fn verification_diagnostics(
        &self,
        project: &str,
        id: &str,
        filter: dctx0_verification::diagnostics::DiagnosticFilter,
        limit: usize,
        cursor: Option<&dctx0_verification::diagnostics::DiagnosticCursor>,
        summary_only: bool,
    ) -> Result<dctx0_verification::diagnostics::DiagnosticPage> {
        let tx = self.connection.unchecked_transaction()?;
        let summary = self.verification_run_summary(project, id)?;
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if !summary.details_available && (version < 7 || !prepared(&self.connection, id)?) {
            return Err(Error::DetailsPruned(id.into()));
        }
        if version < 6 || !prepared(&self.connection, id)? {
            return Err(invalid(&format!(
                "QUERY_INDEX_MISSING: run projection missing; run `{}`",
                crate::query::preparation_command(project, None)
            )));
        }
        let data: String = self.connection.query_row(
            "SELECT data FROM verification_runs WHERE project=?1 AND id=?2",
            params![project, id],
            |r| r.get(0),
        )?;
        let mut run: VerificationRun = decode(&data)?;
        run.required_cases = requirements(&self.connection, id)?;
        let mut observations = std::collections::BTreeMap::new();
        let mut stmt=self.connection.prepare("SELECT metadata,observations FROM verification_case_index WHERE run_id=?1 ORDER BY ordinal")?;
        for row in stmt.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize))
        })? {
            let (data, n) = row?;
            let case: VerificationCaseRecord = decode(&data)?;
            observations.insert(case.id.clone(), n);
            run.cases.push(case);
        }
        let exact = filter.case.clone();
        let mut page = dctx0_verification::diagnostics::diagnostic_page(
            id,
            &run,
            filter,
            limit,
            cursor,
            summary_only,
        )
        .map_err(|e| invalid(&e))?;
        page.details_available = summary.details_available;
        for case in &mut page.cases {
            case.observations = observations[&case.id];
        }
        if !summary_only {
            if let Some(exact) = exact {
                if !summary.details_available {
                    return Err(Error::DetailsPruned(id.into()));
                }
                page.detail = Some(read(&self.connection, id, &exact)?);
            }
        }
        tx.commit()?;
        Ok(page)
    }
}

impl Store {
    /// The temporary sibling is synced before replacement; failed writes preserve the destination.
    pub fn export_verification(
        &self,
        project: &str,
        id: &str,
        case: Option<&str>,
        destination: &std::path::Path,
    ) -> Result<()> {
        use std::io::Write;
        let tx = self.connection.unchecked_transaction()?;
        let summary = self.verification_run_summary(project, id)?;
        if !summary.details_available {
            return Err(Error::DetailsPruned(id.into()));
        }
        let temp = destination.with_extension(format!(
            "dctx0-export-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| invalid("Invalid system time"))?
                .as_nanos()
        ));
        let result = (|| {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            let mut output = std::io::BufWriter::new(file);
            if let Some(case) = case {
                serde_json::to_writer(
                    &mut output,
                    &read_case(&self.connection, project, id, case)?,
                )?;
            } else {
                let version: i64 =
                    self.connection
                        .pragma_query_value(None, "user_version", |r| r.get(0))?;
                if version >= 6 && prepared(&self.connection, id)? {
                    let data: String = self.connection.query_row(
                        "SELECT data FROM verification_runs WHERE project=?1 AND id=?2",
                        params![project, id],
                        |r| r.get(0),
                    )?;
                    let header: VerificationRun = decode(&data)?;
                    let run = StreamingRun {
                        c: &self.connection,
                        id,
                        header: &header,
                    };
                    if codec::json_hash(&run)? != id {
                        return Err(invalid("Reconstructed run digest is corrupt"));
                    }
                    #[derive(Serialize)]
                    struct Export<'a> {
                        id: &'a str,
                        created_at: &'a str,
                        run: StreamingRun<'a>,
                    }
                    serde_json::to_writer(
                        &mut output,
                        &Export {
                            id,
                            created_at: &summary.created_at,
                            run,
                        },
                    )?;
                } else {
                    serde_json::to_writer(
                        &mut output,
                        &codec::read_run(&self.connection, project, id)?,
                    )?;
                }
            }
            output.flush()?;
            output.get_ref().sync_all()?;
            std::fs::rename(&temp, destination)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        tx.commit()?;
        result
    }
    pub fn verification_case(
        &self,
        project: &str,
        id: &str,
        case: &str,
    ) -> Result<VerificationCaseRecord> {
        let tx = self.connection.unchecked_transaction()?;
        let case = read_case(&tx, project, id, case)?;
        tx.commit()?;
        Ok(case)
    }
}

fn read_case(
    c: &Connection,
    project: &str,
    id: &str,
    case: &str,
) -> Result<VerificationCaseRecord> {
    let summary = summaries::summary(c, project, id)?;
    if !summary.details_available {
        return Err(Error::DetailsPruned(id.into()));
    }
    let version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version >= 6 && prepared(c, id)? {
        read(c, id, case)
    } else {
        codec::read_run(c, project, id)?
            .run
            .cases
            .into_iter()
            .find(|c| c.id == case)
            .ok_or_else(|| Error::Missing(case.into()))
    }
}

pub(crate) fn validate_header(run: &VerificationRun) -> Result<()> {
    if !run.cases.is_empty() || !run.required_cases.is_empty() {
        return Err(invalid("Indexed run header contains a second case corpus"));
    }
    Ok(())
}

struct StreamingRun<'a> {
    c: &'a Connection,
    id: &'a str,
    header: &'a VerificationRun,
}
impl Serialize for StreamingRun<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::{Error, SerializeSeq, SerializeStruct};
        struct Cases<'a>(&'a Connection, &'a str);
        impl Serialize for Cases<'_> {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                let ids = ids(self.0, self.1).map_err(S::Error::custom)?;
                let mut seq = serializer.serialize_seq(Some(ids.len()))?;
                for id in ids {
                    seq.serialize_element(&read(self.0, self.1, &id).map_err(S::Error::custom)?)?;
                }
                seq.end()
            }
        }
        let run = self.header;
        validate_header(run).map_err(S::Error::custom)?;
        let mut s = serializer.serialize_struct("VerificationRun", 15)?;
        macro_rules! field {
            ($name:ident) => {
                s.serialize_field(stringify!($name), &run.$name)?;
            };
        }
        field!(schema_version);
        field!(origin);
        field!(started_at_unix_ns);
        field!(elapsed_us);
        field!(provenance);
        field!(budgets);
        field!(selected_subjects);
        field!(required_subjects);
        field!(not_selected);
        s.serialize_field(
            "required_cases",
            &requirements(self.c, self.id).map_err(S::Error::custom)?,
        )?;
        s.serialize_field("cases", &Cases(self.c, self.id))?;
        field!(open_obligations);
        field!(bounded_obligations);
        field!(artifacts);
        field!(result);
        s.end()
    }
}

impl Store {
    /// Compare indexed metadata without decompressing native observations. Older
    /// unprepared stores need explicit query preparation; reads never migrate.
    pub fn verification_comparison(
        &self,
        project: &str,
        before: &str,
        after: &str,
        offset: usize,
        limit: usize,
    ) -> Result<dctx0_verification::comparison::RunComparison> {
        let tx = self.connection.unchecked_transaction()?;
        let load = |id: &str| -> Result<VerificationRun> {
            let summary = self.verification_run_summary(project, id)?;
            if !prepared(&self.connection, id)? {
                return Err(invalid(
                    "QUERY_INDEX_MISSING: prepare verification queries explicitly",
                ));
            }
            let data: String = self.connection.query_row(
                "SELECT data FROM verification_runs WHERE project=?1 AND id=?2",
                params![project, id],
                |r| r.get(0),
            )?;
            let mut run: VerificationRun = decode(&data)?;
            run.cases.clear();
            run.required_cases = requirements(&self.connection, id)?;
            let mut statement = self.connection.prepare(
                "SELECT metadata FROM verification_case_index WHERE run_id=?1 ORDER BY case_id",
            )?;
            for row in statement.query_map([id], |r| r.get::<_, String>(0))? {
                run.cases.push(decode(&row?)?);
            }
            if !summary.details_available {
                run.provenance
                    .incomplete
                    .push(dctx0_core::VerificationIssue {
                        code: "COMPARISON_DETAILS_PRUNED".into(),
                        message: "Unavailable case bodies cannot establish repair".into(),
                        span: None,
                    });
            }
            Ok(run)
        };
        let previous = load(before)?;
        let current = load(after)?;
        let mut report =
            dctx0_verification::comparison::compare_runs(&previous, &current, offset, limit)
                .map_err(|e| invalid(&e))?;
        // Missing before details also prevent an exact repair claim.
        if previous
            .provenance
            .incomplete
            .iter()
            .any(|i| i.code == "COMPARISON_DETAILS_PRUNED")
        {
            report.unproven += report.resolved;
            report.resolved = 0;
            for item in &mut report.issues {
                if item.change == dctx0_verification::comparison::IssueChange::Resolved {
                    item.change = dctx0_verification::comparison::IssueChange::Unproven;
                }
            }
        }
        tx.commit()?;
        Ok(report)
    }
}
